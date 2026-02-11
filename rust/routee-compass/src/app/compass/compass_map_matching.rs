use crate::app::map_matching::{
    MapMatchingRequest, MapMatchingResponse, PointMatchResponse, TracePoint,
};
use crate::app::search::generate_route_output;
use crate::plugin::output::default::traversal::TraversalOutputFormat;
use geo::Point;
use routee_compass_core::algorithm::map_matching::{
    MapMatchingPoint, MapMatchingResult, MapMatchingTrace,
};
use routee_compass_core::algorithm::search::{EdgeTraversal, SearchInstance};

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
