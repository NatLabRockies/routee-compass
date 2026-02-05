use super::{
    map_error::MapError, map_json_extensions::MapJsonExtensions,
    nearest_search_result::NearestSearchResult,
};
use crate::{
    algorithm::search::SearchInstance,
    model::{constraint::ConstraintModel, network::Edge},
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, sync::Arc};
use wkt::ToWkt;

/// a [`MatchingType`] is the type of data expected on a query
/// that can be mapped to the graph.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum MatchingType {
    /// expect origin [, destination] VertexIds on the query.
    VertexId,
    /// expect origin [, destination] EdgeIds on the query.
    EdgeId,
    /// expect origin [, destination] Points on the query.
    Point,
    /// expect any combination of the map input types provided
    Combined(Vec<MatchingType>),
}

impl MatchingType {
    pub const ALL: [MatchingType; 3] = [Self::Point, Self::VertexId, Self::EdgeId];

    pub fn names() -> Vec<String> {
        MatchingType::ALL
            .iter()
            .map(|t| t.to_string())
            .collect_vec()
    }

    pub fn names_str() -> String {
        Self::names().iter().join(", ")
    }
}

impl Default for MatchingType {
    /// the default MatchingType is to first attempt to process a Point into VertexIds,
    /// then attempt to find VertexIds on the query,
    /// then finally attempt to find EdgeIds on the query.
    fn default() -> Self {
        Self::Combined(Self::ALL.to_vec())
    }
}

impl Display for MatchingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let self_str = match self {
            MatchingType::Combined(vec) => vec.iter().map(|mit| mit.to_string()).join(","),
            MatchingType::VertexId => String::from("vertex_id"),
            MatchingType::EdgeId => String::from("edge_id"),
            MatchingType::Point => String::from("point"),
        };
        write!(f, "{self_str}")
    }
}

impl FromStr for MatchingType {
    type Err = MapError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "vertex_id" => Ok(Self::VertexId),
            "edge_id" => Ok(Self::EdgeId),
            "point" => Ok(Self::Point),
            _ => Err(MapError::BuildError(format!(
                "unrecognized matching type '{}', must be one of [{}]",
                s,
                MatchingType::names_str()
            ))),
        }
    }
}

#[derive(Debug)]
pub enum MapInputResult {
    Found,
    NotFound,
}

impl MatchingType {
    /// deserialize optional lists of strings from some configuration into matching types.
    pub fn deserialize_matching_types(
        types: Option<&Vec<String>>,
    ) -> Result<MatchingType, MapError> {
        match types {
            None => Ok(MatchingType::default()),
            Some(string_list) => {
                let deserialized = string_list
                    .iter()
                    .map(|s| MatchingType::from_str(s.as_str()))
                    .collect::<Result<Vec<_>, _>>()?;
                match deserialized[..] {
                    [MatchingType::Point] => Ok(MatchingType::Point),
                    [MatchingType::VertexId] => Ok(MatchingType::VertexId),
                    [MatchingType::EdgeId] => Ok(MatchingType::EdgeId),
                    _ => Ok(MatchingType::Combined(deserialized)),
                }
            }
        }
    }

    /// attempts to find any valid input origin fields, or performs map matching, in order to
    /// append those fields to the query, based on the type of [`MatchingType`] supported.
    pub fn process_origin(
        &self,
        query: &mut serde_json::Value,
        si: &SearchInstance,
    ) -> Result<(), MapError> {
        use MatchingType as MT;
        match self {
            MT::Combined(vec) => {
                let mut errors = vec![];
                for matching_type in vec.iter() {
                    match matching_type.process_origin(query, si) {
                        Ok(_) => return Ok(()),
                        Err(e) => {
                            let mit = serde_json::to_string(matching_type).unwrap_or_default();
                            let msg = format!("no origin {mit} on input query: {e}");
                            errors.push(msg);
                        }
                    }
                }
                if !errors.is_empty() {
                    let msg = errors.iter().join("; ");
                    Err(MapError::MapMatchError(format!(
                        "unable to match query to map: {msg}"
                    )))
                } else {
                    Ok(())
                }
            }
            MT::VertexId => {
                // validate all out-edges for this vertex
                let vertex_id = query.get_origin_vertex()?;
                let edges = si.graph.out_edges(&vertex_id).iter().map(|(edge_list_id, edge_id)| si.graph.get_edge(edge_list_id, edge_id)).collect::<Result<Vec<_>, _>>().map_err(|e| MapError::MapMatchError(format!("while attempting to validate vertex id {vertex_id} for map matching, the underlying Graph model caused an error: {e}")))?;
                for edge in edges.into_iter() {
                    let fm =  si.get_constraint_model(&edge.edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching vertex_id {vertex_id}, failed to retrieve constraint model for out edge list '{}', edge '{}': {e}", edge.edge_list_id, edge.edge_id)))?;
                    if let Ok(true) = test_edge(edge, fm) {
                        return Ok(());
                    }
                }
                Err(MapError::MapMatchError(format!("attempted to map match origin vertex_id {vertex_id} provided in query, but no out-edges are valid for traversal according to this ConstraintModel instance")))
            }
            MT::EdgeId => {
                // validate this edge
                let (edge_list_id, edge_id) = query.get_origin_edge()?;
                let fm =  si.get_constraint_model(&edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching edge_list_id '{edge_list_id}', edge_id '{edge_id}', failed to retrieve constraint model for out edge list '{edge_list_id}', edge '{edge_id}': {e}")))?;
                let edge = si.graph.get_edge(&edge_list_id, &edge_id).map_err(|e| MapError::MapMatchError(format!("while attempting to validate edge id {edge_id} for map matching, the underlying Graph model caused an error: {e}")))?;
                validate_edge(edge, fm)
            }
            MT::Point => {
                // iterate through nearest values in the spatial index to this point that
                // are within our matching tolerance and validate them with the constraint model
                let src_point = geo::Point(query.get_origin_coordinate()?);
                for nearest in si.map_model.spatial_index.nearest_graph_id_iter(&src_point) {
                    match nearest {
                        NearestSearchResult::NearestVertex(vertex_id) => {
                            // if any of the out-edges of this vertex are valid, we can finish
                            let edges = si.graph.out_edges(&vertex_id).iter().map(|(edge_list_id, edge_id)| si.graph.get_edge(edge_list_id, edge_id)).collect::<Result<Vec<_>, _>>().map_err(|e| MapError::MapMatchError(format!("while attempting to validate vertex id {vertex_id} for map matching, the underlying Graph model caused an error: {e}")))?;
                            for edge in edges.into_iter() {
                                let fm =  si.get_constraint_model(&edge.edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching point {}, failed to retrieve constraint model for out edge list '{}', edge '{}': {e}", src_point.to_wkt(), edge.edge_list_id, edge.edge_id)))?;
                                let is_valid = test_edge(edge, fm)?;
                                if is_valid {
                                    query.add_origin_vertex(vertex_id)?;
                                    return Ok(());
                                }
                            }
                            continue;
                        }
                        NearestSearchResult::NearestEdge(edge_list_id, edge_id) => {
                            let edge = si.graph.get_edge(&edge_list_id, &edge_id).map_err(|e| MapError::MapMatchError(format!("while attempting to validate edge_list_id '{edge_list_id}', edge_id {edge_id} from nearest neighbor search for map matching, the underlying Graph model caused an error: {e}")))?;
                            let fm =  si.get_constraint_model(&edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching edge_list_id '{edge_list_id}', edge_id '{edge_id}', failed to retrieve constraint model for out edge list '{edge_list_id}', edge '{edge_id}': {e}")))?;
                            let is_valid = test_edge(edge, fm)?;
                            if is_valid {
                                query.add_origin_edge(edge_list_id, edge_id)?;
                                return Ok(());
                            }
                        }
                    }
                }
                Err(MapError::MapMatchError(format!(
                    "attempted to match query origin coordinate ({}, {}) to map but exausted all possibilities",
                    src_point.x(),
                    src_point.y(),
                )))
            }
        }
    }

    /// attempts to find any valid input destination fields, or performs map matching, in order to
    /// append those fields to the query, based on the type of [`MatchingType`] supported.
    /// since destinations are optional, the method returns a [`MapInputResult`] that indicates if
    /// a destination was found or not found.
    pub fn process_destination(
        &self,
        query: &mut serde_json::Value,
        si: &SearchInstance,
    ) -> Result<MapInputResult, MapError> {
        use MatchingType as MT;
        match self {
            MT::Combined(vec) => {
                let mut errors = vec![];
                for matching_type in vec.iter() {
                    let result = matching_type.process_destination(query, si);
                    match result {
                        Ok(MapInputResult::Found) => return Ok(MapInputResult::Found),
                        Ok(MapInputResult::NotFound) => continue,
                        Err(e) => {
                            let mit = serde_json::to_string(matching_type).unwrap_or_default();
                            let msg = format!("no destination {mit} on input query: {e}");
                            errors.push(msg);
                        }
                    }
                }
                if !errors.is_empty() {
                    let msg = errors.iter().join("; ");
                    Err(MapError::MapMatchError(format!(
                        "unable to match query to map: {msg}"
                    )))
                } else {
                    Ok(MapInputResult::NotFound)
                }
            }

            MT::VertexId => {
                // validate all out-edges for this vertex, if one is accepted, we are done.
                let vertex_id_option = query.get_destination_vertex()?;
                match vertex_id_option {
                    Some(vertex_id) => {
                        let edges = si.graph.in_edges(&vertex_id).iter().map(|(edge_list_id, edge_id)| si.graph.get_edge(edge_list_id, edge_id)).collect::<Result<Vec<_>, _>>().map_err(|e| MapError::MapMatchError(format!("while attempting to validate vertex id {vertex_id} for map matching, the underlying Graph model caused an error: {e}")))?;
                        for edge in edges.into_iter() {
                            let fm =  si.get_constraint_model(&edge.edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching destination vertex_id {vertex_id}, failed to retrieve constraint model for out edge list '{}', edge '{}': {e}", edge.edge_list_id, edge.edge_id)))?;
                            if let Ok(true) = test_edge(edge, fm) {
                                return Ok(MapInputResult::Found);
                            }
                        }
                        Err(MapError::MapMatchError(format!("attempted to map match destination vertex_id {vertex_id} provided in query, but no in-edges are valid for traversal according to this ConstraintModel instance")))
                    }
                    None => Ok(MapInputResult::NotFound),
                }
            }

            MT::EdgeId => {
                // validate this edge
                let dest_edge_option = query.get_destination_edge()?;
                match dest_edge_option {
                    Some((edge_list_id, edge_id)) => {
                        let edge = si.graph.get_edge(&edge_list_id, &edge_id).map_err(|e| MapError::MapMatchError(format!("while attempting to validate edge_list_id '{edge_list_id}', edge_id {edge_id} for map matching, the underlying Graph model caused an error: {e}")))?;
                        let fm =  si.get_constraint_model(&edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching edge_list_id '{edge_list_id}', edge_id '{edge_id}', failed to retrieve constraint model for out edge list '{edge_list_id}', edge '{edge_id}': {e}")))?;
                        validate_edge(edge, fm)?;
                        Ok(MapInputResult::Found)
                    }
                    None => Ok(MapInputResult::NotFound),
                }
            }

            MT::Point => {
                // iterate through nearest values in the spatial index to this point that
                // are within our matching tolerance and validate them with the constraint model
                let dst_point = match query.get_destination_coordinate()? {
                    Some(coord) => geo::Point(coord),
                    None => return Ok(MapInputResult::NotFound),
                };

                for nearest in si.map_model.spatial_index.nearest_graph_id_iter(&dst_point) {
                    match nearest {
                        NearestSearchResult::NearestVertex(vertex_id) => {
                            // if any of the out-edges of this vertex are valid, we can finish
                            let edges = si.graph.out_edges(&vertex_id).iter().map(|(edge_list_id, edge_id)| si.graph.get_edge(edge_list_id, edge_id)).collect::<Result<Vec<_>, _>>().map_err(|e| MapError::MapMatchError(format!("while attempting to validate vertex id {vertex_id} for map matching, the underlying Graph model caused an error: {e}")))?;
                            for edge in edges.into_iter() {
                                let fm =  si.get_constraint_model(&edge.edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching point '{}', failed to retrieve constraint model for out edge list '{}', edge '{}': {e}", dst_point.to_wkt(), edge.edge_list_id, edge.edge_id)))?;
                                let is_valid = test_edge(edge, fm)?;
                                if is_valid {
                                    query.add_destination_vertex(vertex_id)?;
                                    return Ok(MapInputResult::Found);
                                }
                            }
                            continue;
                        }
                        NearestSearchResult::NearestEdge(edge_list_id, edge_id) => {
                            let edge = si.graph.get_edge(&edge_list_id, &edge_id).map_err(|e| MapError::MapMatchError(format!("while attempting to validate edge id {edge_id} from nearest neighbor search for map matching, the underlying Graph model caused an error: {e}")))?;
                            let fm =  si.get_constraint_model(&edge_list_id).map_err(|e| MapError::InternalError(format!("while map matching edge_list_id '{edge_list_id}', edge_id '{edge_id}', failed to retrieve constraint model for out edge list '{edge_list_id}', edge '{edge_id}': {e}")))?;
                            let is_valid = test_edge(edge, fm)?;
                            if is_valid {
                                query.add_destination_edge(edge_list_id, edge_id)?;
                                return Ok(MapInputResult::Found);
                            }
                        }
                    }
                }
                Err(MapError::MapMatchError(format!(
                    "attempted to match query destination coordinate ({}, {}) to map but exausted all possibilities",
                    dst_point.x(),
                    dst_point.y(),
                )))
            }
        }
    }
}

fn test_edge(edge: &Edge, fm: Arc<dyn ConstraintModel>) -> Result<bool, MapError> {
    let is_valid = fm.valid_edge(edge).map_err(|e| MapError::MapMatchError(format!("while attempting to validate edge id {} for map matching, the underlying ConstraintModel caused an error: {}", edge.edge_id, e)))?;
    Ok(is_valid)
}

fn validate_edge(edge: &Edge, fm: Arc<dyn ConstraintModel>) -> Result<(), MapError> {
    let is_valid = test_edge(edge, fm)?;
    if !is_valid {
        Err(MapError::MapMatchError(format!(
            "query assigned origin of edge {} is not valid according to the ConstraintModel",
            edge.edge_id
        )))
    } else {
        Ok(())
    }
}
