use crate::plugin::output::OutputPluginError;
use geo::{LineString, MultiLineString, Point};
use geo_types::MultiPoint;
use geojson::{Feature, FeatureCollection};
use routee_compass_core::algorithm::search::EdgeTraversal;
use routee_compass_core::algorithm::search::SearchTree;
use routee_compass_core::model::map::MapModel;
use routee_compass_core::model::state::StateModel;
use routee_compass_core::util::geo::geo_io_utils;
use serde_json::{json, Map};
use std::sync::Arc;

pub fn create_tree_geojson(
    tree: &SearchTree,
    map_model: Arc<MapModel>,
    state_model: Arc<StateModel>,
) -> Result<serde_json::Value, OutputPluginError> {
    let features = tree
        .values()
        .filter_map(|t| {
            let et = match t.incoming_edge() {
                None => return None,
                Some(e) => e,
            };
            let row_result = map_model
                .get_linestring(&et.edge_list_id, &et.edge_id)
                .cloned()
                .map_err(|e| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "failure creating tree GeoJSON: {e}"
                    ))
                })
                .and_then(|g| create_geojson_feature(et, g, state_model.clone()));

            Some(row_result)
        })
        .collect::<Result<Vec<_>, OutputPluginError>>()?;
    // let result_json = serde_json::to_value(features)?;/
    let feature_collection = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };
    let result = serde_json::to_value(feature_collection)?;
    Ok(result)
}

pub fn create_route_geojson(
    route: &[EdgeTraversal],
    map_model: Arc<MapModel>,
    state_model: Arc<StateModel>,
) -> Result<serde_json::Value, OutputPluginError> {
    let features = route
        .iter()
        .map(|t| {
            let g = map_model
                .get_linestring(&t.edge_list_id, &t.edge_id)
                .cloned()
                .map_err(|e| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "failure building route geojson: {e}"
                    ))
                })?;
            let geojson_feature = create_geojson_feature(t, g, state_model.clone())?;
            Ok(geojson_feature)
        })
        .collect::<Result<Vec<_>, OutputPluginError>>()?;
    // let result_json = serde_json::to_value(features)?;/
    let feature_collection = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };
    let result = serde_json::to_value(feature_collection)?;
    Ok(result)
}

pub fn create_geojson_feature(
    t: &EdgeTraversal,
    g: LineString<f32>,
    state_model: Arc<StateModel>,
) -> Result<Feature, OutputPluginError> {
    let serialized_state = state_model
        .serialize_state(&t.result_state, false)
        .map_err(|e| {
            OutputPluginError::OutputPluginFailed(format!(
                "failure serializing final trip state while constructing geojson output: {e}"
            ))
        })?;
    let serialized_cost = json![t.cost];

    let mut properties = Map::new();
    properties.insert(String::from("edge_id"), json![t.edge_id]);
    properties.insert(String::from("edge_list_id"), json![t.edge_list_id]);
    properties.insert(String::from("state"), serialized_state);
    properties.insert(String::from("cost"), serialized_cost);

    // let id = Id::Number(serde_json::Number::from(t.edge_id.0));
    let geometry = geojson::Geometry::from(&g);
    let feature = Feature {
        bbox: None,
        geometry: Some(geometry),
        id: None,
        properties: Some(properties),
        foreign_members: None,
    };
    Ok(feature)
}

pub fn create_edge_geometry(
    edge: &EdgeTraversal,
    geoms: &[LineString<f32>],
) -> Result<LineString<f32>, OutputPluginError> {
    geoms.get(edge.edge_id.0).cloned().ok_or_else(|| {
        OutputPluginError::OutputPluginFailed(format!(
            "geometry table missing edge id {}",
            edge.edge_id
        ))
    })
}

pub fn create_route_linestring(
    route: &[EdgeTraversal],
    map_model: Arc<MapModel>,
) -> Result<LineString<f32>, OutputPluginError> {
    let edges = route
        .iter()
        .map(|et| (et.edge_list_id, et.edge_id))
        .collect::<Vec<_>>();

    let edge_linestrings = edges
        .iter()
        .map(|(elid, eid)| {
            let geom = map_model.get_linestring(elid, eid).map_err(|e| {
                OutputPluginError::OutputPluginFailed(format!(
                    "failure building route linestring: {e}"
                ))
            });
            geom
        })
        .collect::<Result<Vec<&LineString<f32>>, OutputPluginError>>()?;
    let geometry = geo_io_utils::concat_linestrings(edge_linestrings);
    Ok(geometry)
}

pub fn create_tree_multilinestring(
    tree: &SearchTree,
    // geoms: &[LineString<f32>],
    map_model: Arc<MapModel>,
) -> Result<MultiLineString<f32>, OutputPluginError> {
    let edges = tree
        .values()
        .flat_map(|node| node.incoming_edge().map(|et| (et.edge_list_id, et.edge_id)))
        .collect::<Vec<_>>();

    let tree_linestrings = edges
        .iter()
        .map(|(elid, eid)| {
            let geom = map_model.get_linestring(elid, eid).map_err(|e| {
                OutputPluginError::OutputPluginFailed(format!("failure building tree WKT: {e}"))
            });
            geom.cloned()
        })
        .collect::<Result<Vec<LineString<f32>>, OutputPluginError>>()?;
    let geometry = MultiLineString::new(tree_linestrings);
    Ok(geometry)
}

pub fn create_tree_multipoint(
    tree: &SearchTree,
    map_model: Arc<MapModel>,
) -> Result<MultiPoint<f32>, OutputPluginError> {
    let edges = tree
        .values()
        .filter_map(|node| node.incoming_edge().map(|et| (et.edge_list_id, et.edge_id)))
        .collect::<Vec<_>>();

    let tree_destinations = edges
        .iter()
        .map(|(elid, eid)| {
            let linestring = map_model.get_linestring(elid, eid).map_err(|e| {
                OutputPluginError::OutputPluginFailed(format!(
                    "failed to get linestring for edge list, edge: {elid}, {eid}: {e}"
                ))
            })?;
            let points = linestring.points().next_back().ok_or_else(|| {
                OutputPluginError::OutputPluginFailed(format!(
                    "linestring is invalid for edge_id {eid}"
                ))
            })?;
            Ok(points)
        })
        .collect::<Result<Vec<Point<f32>>, OutputPluginError>>()?;
    let geometry = MultiPoint::new(tree_destinations);
    Ok(geometry)
}
