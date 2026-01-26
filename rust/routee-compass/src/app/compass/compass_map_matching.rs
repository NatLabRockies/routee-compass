use crate::app::map_matching::{
    MapMatchingRequest, MapMatchingResponse, PointMatchResponse, TracePoint,
};
use geo::Point;
use routee_compass_core::algorithm::map_matching::{
    MapMatchingPoint, MapMatchingResult, MapMatchingTrace,
};

/// Converts a JSON request to the internal trace format.
pub fn convert_request_to_trace(request: &MapMatchingRequest) -> MapMatchingTrace {
    let points: Vec<MapMatchingPoint> = request.trace.iter().map(convert_trace_point).collect();
    MapMatchingTrace::new(points)
}

/// Converts a single trace point from the request format.
pub fn convert_trace_point(point: &TracePoint) -> MapMatchingPoint {
    let coord = Point::new(point.x as f32, point.y as f32);
    match &point.timestamp {
        Some(ts) => MapMatchingPoint::with_timestamp(coord, ts.clone()),
        None => MapMatchingPoint::new(coord),
    }
}

/// Converts the internal result to the response format.
pub fn convert_result_to_response(result: MapMatchingResult) -> MapMatchingResponse {
    let point_matches: Vec<PointMatchResponse> = result
        .point_matches
        .into_iter()
        .map(|pm| {
            PointMatchResponse::new(pm.edge_list_id.0, pm.edge_id.0 as u64, pm.distance_to_edge)
        })
        .collect();

    let matched_path: Vec<(usize, u64)> = result
        .matched_path
        .into_iter()
        .map(|(list_id, edge_id)| (list_id.0, edge_id.0 as u64))
        .collect();

    MapMatchingResponse::new(point_matches, matched_path)
}
