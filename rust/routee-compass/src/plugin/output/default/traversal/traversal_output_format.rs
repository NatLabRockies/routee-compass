use std::sync::Arc;

use super::traversal_ops as ops;
use crate::plugin::output::OutputPluginError;
use geo::{CoordFloat, Geometry, TryConvert};
use routee_compass_core::{
    algorithm::search::{EdgeTraversal, SearchTree},
    model::{map::MapModel, state::StateModel},
};
use serde::{Deserialize, Serialize};
use wkb::writer::WriteOptions;
use wkt::ToWkt;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TraversalOutputFormat {
    // concatenates all LINESTRINGS and returns the geometry as a WKT
    Wkt,
    // concatenates all LINESTRINGS and returns the geometry as a WKB
    Wkb,
    // returns the properties of each link traversal as a JSON array of objects
    Json,
    // returns the geometries and properties as GeoJSON
    GeoJson,
    EdgeId,
}

impl TraversalOutputFormat {
    /// generates output for a route based on the configured TraversalOutputFormat
    pub fn generate_route_output(
        &self,
        route: &Vec<EdgeTraversal>,
        map_model: Arc<MapModel>,
        state_model: Arc<StateModel>,
    ) -> Result<serde_json::Value, OutputPluginError> {
        match self {
            TraversalOutputFormat::Wkt => {
                let route_geometry = ops::create_route_linestring(route, map_model.clone())?;
                let route_wkt = route_geometry.wkt_string();
                Ok(serde_json::Value::String(route_wkt))
            }
            TraversalOutputFormat::Wkb => {
                let linestring = ops::create_route_linestring(route, map_model.clone())?;
                let geometry = geo::Geometry::LineString(linestring);
                let wkb_str = geometry_to_wkb_string(&geometry)?;
                Ok(serde_json::Value::String(wkb_str))
            }
            TraversalOutputFormat::Json => {
                let result = serde_json::to_value(route)?;
                Ok(result)
            }
            TraversalOutputFormat::GeoJson => {
                let result = ops::create_route_geojson(route, map_model, state_model)?;
                Ok(result)
            }
            TraversalOutputFormat::EdgeId => {
                let route_ids = route.iter().map(|e| e.edge_id).collect::<Vec<_>>();
                let json = serde_json::json![route_ids];
                Ok(json)
            }
        }
    }

    /// generates output for a tree based on the configured TraversalOutputFormat
    pub fn generate_tree_output(
        &self,
        tree: &SearchTree,
        map_model: Arc<MapModel>,
        state_model: Arc<StateModel>,
    ) -> Result<serde_json::Value, OutputPluginError> {
        match self {
            TraversalOutputFormat::Wkt => {
                let route_geometry = ops::create_tree_multilinestring(tree, map_model)?;
                let route_wkt = route_geometry.wkt_string();
                Ok(serde_json::Value::String(route_wkt))
            }
            TraversalOutputFormat::Wkb => {
                let route_geometry = ops::create_tree_multilinestring(tree, map_model)?;
                let geometry = geo::Geometry::MultiLineString(route_geometry);
                let wkb_str = geometry_to_wkb_string(&geometry)?;
                Ok(serde_json::Value::String(wkb_str))
            }
            TraversalOutputFormat::Json => {
                let result = serde_json::to_value(tree.values().collect::<Vec<_>>())?;
                Ok(result)
            }
            TraversalOutputFormat::GeoJson => {
                let result = ops::create_tree_geojson(tree, map_model, state_model)?;
                Ok(result)
            }
            TraversalOutputFormat::EdgeId => {
                let tree_ids = tree
                    .values()
                    .filter_map(|b| b.incoming_edge().map(|e| (e.edge_list_id, e.edge_id)))
                    .collect::<Vec<_>>();
                let json = serde_json::json![tree_ids];
                Ok(json)
            }
        }
    }
}

fn geometry_to_wkb_string<T: CoordFloat + Into<f64>>(
    geometry: &Geometry<T>,
) -> Result<String, OutputPluginError> {
    let mut out_bytes = vec![];
    let geom: Geometry<f64> = geometry.try_convert().map_err(|e| {
        OutputPluginError::OutputPluginFailed(format!("unable to convert geometry to f64: {e}"))
    })?;
    let write_options = WriteOptions {
        endianness: wkb::Endianness::BigEndian,
    };
    wkb::writer::write_geometry(&mut out_bytes, &geom, &write_options).map_err(|e| {
        OutputPluginError::OutputPluginFailed(format!("failed to write geometry as WKB: {e}"))
    })?;
    let out_string = String::from_utf8(out_bytes).map_err(|e| {
        OutputPluginError::OutputPluginFailed(format!("failed to read WKB as utf8: {e}"))
    })?;
    Ok(out_string)
}

// #[cfg(test)]
// mod test {

//     use crate::app::search::SearchAppResult;
//     use chrono::Local;
//     use geo::{coord, LineString};
//     use routee_compass_core::{
//         algorithm::search::EdgeTraversal,
//         model::{network::EdgeId, state::StateVariable, unit::Cost},
//     };
//     use std::time::Duration;

//     // #[ignore = "needs mocked graph for map model integration in test"]
//     // fn test_e2e() {
//     //     let route = vec![
//     //         EdgeTraversal {
//     //             edge_id: EdgeId(0),
//     //             access_cost: Cost::from(0.0),
//     //             traversal_cost: Cost::from(10.0),
//     //             result_state: vec![StateVariable(10.0)],
//     //         },
//     //         EdgeTraversal {
//     //             edge_id: EdgeId(1),
//     //             access_cost: Cost::from(5.0),
//     //             traversal_cost: Cost::from(9.0),
//     //             result_state: vec![StateVariable(24.0)],
//     //         },
//     //         EdgeTraversal {
//     //             edge_id: EdgeId(2),
//     //             access_cost: Cost::from(0.0),
//     //             traversal_cost: Cost::from(11.0),
//     //             result_state: vec![StateVariable(35.0)],
//     //         },
//     //     ];
//     //     let _ = SearchAppResult {
//     //         routes: vec![route],
//     //         trees: vec![],
//     //         search_executed_time: Local::now().to_rfc3339(),
//     //         search_runtime: Duration::ZERO,
//     //         iterations: 0,
//     //     };

//     //     let geoms = vec![
//     //         LineString(vec![
//     //             coord! { x: 1.0, y: 0.0 },
//     //             coord! { x: 1.0, y: 0.0 },
//     //             coord! { x: 1.0, y: 1.0 },
//     //         ]),
//     //         LineString(vec![coord! { x: 2.0, y: 2.0 }, coord! { x: 2.0, y: 3.0 }]),
//     //         LineString(vec![coord! { x: 3.0, y: 3.0 }, coord! { x: 3.0, y: 4.0 }]),
//     //     ]
//     //     .into_boxed_slice();

//     //     // let map_model = MapModel::new(graph, config)

//     //     // println!(
//     //     //     "{:?}",
//     //     //     TraversalOutputFormat::Wkt
//     //     //         .generate_route_output(&result.routes[0], &geoms)
//     //     //         .map(|r| serde_json::to_string_pretty(&r))
//     //     // );
//     //     // println!(
//     //     //     "{:?}",
//     //     //     TraversalOutputFormat::Json
//     //     //         .generate_route_output(&result.routes[0], &geoms)
//     //     //         .map(|r| serde_json::to_string_pretty(&r))
//     //     // );
//     //     // println!(
//     //     //     "{:?}",
//     //     //     TraversalOutputFormat::GeoJson
//     //     //         .generate_route_output(&result.routes[0], &geoms)
//     //     //         .map(|r| serde_json::to_string_pretty(&r))
//     //     // );
//     //     // println!(
//     //     //     "{:?}",
//     //     //     TraversalOutputFormat::EdgeId
//     //     //         .generate_route_output(&result.routes[0], &geoms)
//     //     //         .map(|r| serde_json::to_string_pretty(&r))
//     //     // );
//     // }
// }
