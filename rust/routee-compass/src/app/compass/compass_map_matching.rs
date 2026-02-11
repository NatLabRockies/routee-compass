use crate::app::compass::CompassAppError;
use crate::app::map_matching::{
    MapMatchingAppError, MapMatchingRequest, MapMatchingResponse, PointMatchResponse, TracePoint,
};
use crate::app::search::generate_route_output;
use crate::app::search::SearchApp;
use crate::plugin::output::default::traversal::TraversalOutputFormat;
use geo::Point;
use routee_compass_core::algorithm::map_matching::MapMatchingAlgorithm;
use routee_compass_core::algorithm::map_matching::{
    MapMatchingPoint, MapMatchingResult, MapMatchingTrace,
};
use routee_compass_core::algorithm::search::{EdgeTraversal, SearchInstance};
use serde_json::Value;
use std::sync::Arc;

/// Converts a JSON request to the internal trace format.
pub fn convert_request_to_trace(request: &MapMatchingRequest) -> MapMatchingTrace {
    let points: Vec<MapMatchingPoint> = request.trace.iter().map(convert_trace_point).collect();
    MapMatchingTrace::new(points)
}

/// Converts a single trace point from the request format.
pub fn convert_trace_point(point: &TracePoint) -> MapMatchingPoint {
    let coord = Point::new(point.x as f32, point.y as f32);
    MapMatchingPoint::new(coord)
}

/// Converts the internal result to the response format.
pub fn convert_result_to_response(
    result: MapMatchingResult,
    matched_path: Vec<EdgeTraversal>,
    si: &SearchInstance,
    request: &MapMatchingRequest,
) -> MapMatchingResponse {
    let point_matches: Vec<PointMatchResponse> = result
        .point_matches
        .into_iter()
        .map(|pm| {
            PointMatchResponse::new(
                pm.edge_list_id.0,
                pm.edge_id.0 as u64,
                pm.distance_to_edge.get::<uom::si::length::meter>(),
            )
        })
        .collect();

    let output_format = request.output_format;
    let summary_ops = &request.summary_ops;

    let (mut path_json, traversal_summary) =
        match generate_route_output(&matched_path, si, &output_format, summary_ops) {
            Ok(output) => {
                let path = output
                    .get("path")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let summary = output.get("traversal_summary").cloned();
                (path, summary)
            }
            Err(e) => {
                log::error!("failed to generate route output for map matching: {}", e);
                (
                    serde_json::to_value(&matched_path).unwrap_or(serde_json::Value::Null),
                    None,
                )
            }
        };

    // If format is JSON, we need to add geometry manually since TraversalOutputFormat::Json doesn't include it by default
    // and map matching expects it.
    if matches!(output_format, TraversalOutputFormat::Json) {
        if let Some(arr) = path_json.as_array_mut() {
            for (i, edge_val) in arr.iter_mut().enumerate() {
                if let Some(et) = matched_path.get(i) {
                    if let Ok(geom) = si.map_model.get_linestring(&et.edge_list_id, &et.edge_id) {
                        if let Some(obj) = edge_val.as_object_mut() {
                            obj.insert("geometry".to_string(), serde_json::to_value(geom).unwrap());
                        }
                    }
                }
            }
        }
    }

    MapMatchingResponse::new(point_matches, path_json, traversal_summary)
}

/// Inner implementation of single map match that returns Result for easier error handling
pub fn run_single_map_match(
    query: &Value,
    search_app: &SearchApp,
    map_matching_algorithm: &Arc<dyn MapMatchingAlgorithm>,
) -> Result<Value, CompassAppError> {
    let request: MapMatchingRequest = serde_json::from_value(query.clone())?;

    // Validate the request
    request
        .validate()
        .map_err(MapMatchingAppError::InvalidRequest)?;

    // Convert request to internal trace format
    let trace = convert_request_to_trace(&request);

    // Build a search instance for this query
    let mut query_config = map_matching_algorithm.search_parameters();
    if let Some(search_overrides) = &request.search_parameters {
        if let Some(obj) = search_overrides.as_object() {
            for (k, v) in obj {
                query_config[k] = v.clone();
            }
        }
    }
    let search_instance = search_app
        .build_search_instance(&query_config)
        .map_err(|e| MapMatchingAppError::BuildFailure(e.to_string()))?;

    // Run the algorithm
    let result = map_matching_algorithm
        .match_trace(&trace, &search_instance)
        .map_err(|e| MapMatchingAppError::AlgorithmError { source: e })?;

    // Recalculate the path to get correct accumulated state
    let matched_path = search_instance
        .compute_path(&result.matched_path)
        .map_err(|e| MapMatchingAppError::AlgorithmError {
            source: routee_compass_core::algorithm::map_matching::map_matching_error::MapMatchingError::SearchError(e),
        })?;

    // Convert result to response format
    let response = convert_result_to_response(result, matched_path, &search_instance, &request);
    let response_json = serde_json::to_value(response)?;
    Ok(response_json)
}
