use super::map_error::MapError;
use super::map_model_config::MapModelConfig;
use super::matching_type::MatchingType;
use super::spatial_index::SpatialIndex;
use super::{geometry_model::GeometryModel, matching_type::MapInputResult};
use crate::algorithm::search::SearchInstance;
use crate::model::map::map_model_config::MapModelGeometryConfig;
use crate::model::network::{EdgeId, EdgeListId, Graph};
use geo::LineString;
use std::sync::Arc;

pub struct MapModel {
    /// way in which map matching is attempted
    pub matching_type: MatchingType,
    /// index used during map matching
    pub spatial_index: SpatialIndex,
    /// collection of geometries associated with the graph edge lists
    pub geometry: Vec<GeometryModel>,
    /// allow for queries without a destination location, such as when generating
    /// shortest path trees or isochrones.
    pub queries_without_destinations: bool,
}

impl MapModel {
    pub fn new(graph: Arc<Graph>, config: &MapModelConfig) -> Result<MapModel, MapError> {
        let geometry = config
            .geometry
            .iter()
            .enumerate()
            .map(|(edge_list, g)| {
                let edge_list_id = EdgeListId(edge_list);
                match g {
                    MapModelGeometryConfig::FromVertices => {
                        GeometryModel::new_from_vertices(graph.clone(), edge_list_id)
                    }
                    MapModelGeometryConfig::FromLinestrings {
                        geometry_input_file,
                    } => GeometryModel::new_from_edges(
                        geometry_input_file,
                        edge_list_id,
                        graph.clone(),
                    ),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let queries_without_destinations = config.queries_without_destinations;
        let tolerance = config.tolerance.as_ref().map(|t| t.to_uom());
        let matching_type =
            MatchingType::deserialize_matching_types(config.matching_type.as_ref())?;
        let spatial_index_type = config.spatial_index_type.clone().unwrap_or_default();
        let spatial_index =
            SpatialIndex::build(&spatial_index_type, graph.clone(), &geometry, tolerance);

        Ok(MapModel {
            matching_type,
            spatial_index,
            geometry,
            queries_without_destinations,
        })
    }

    pub fn get_linestring<'a>(
        &'a self,
        edge_list_id: &EdgeListId,
        edge_id: &EdgeId,
    ) -> Result<&'a LineString<f32>, MapError> {
        let linestrings = self
            .geometry
            .get(edge_list_id.0)
            .ok_or(MapError::MissingEdgeListId(*edge_list_id))?;
        linestrings
            .get(edge_id)
            .ok_or(MapError::MissingEdgeId(*edge_list_id, *edge_id))
    }

    pub fn map_match(
        &self,
        query: &mut serde_json::Value,
        si: &SearchInstance,
    ) -> Result<(), MapError> {
        self.matching_type.process_origin(query, si)?;
        let result = self.matching_type.process_destination(query, si)?;
        match result {
            MapInputResult::NotFound if !self.queries_without_destinations => {
                Err(MapError::DestinationsRequired(self.matching_type.clone()))
            }
            _ => Ok(()),
        }
    }
}
