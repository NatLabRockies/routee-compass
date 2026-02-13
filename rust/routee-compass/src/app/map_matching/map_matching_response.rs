use routee_compass_core::model::cost::TraversalCost;
use routee_compass_core::model::state::StateVariable;
use serde::Serialize;

/// JSON-serializable response from map matching.
#[derive(Debug, Clone, Serialize)]
pub struct MapMatchingResponse {
    /// Match results for each input point in the trace.
    pub point_matches: Vec<PointMatchResponse>,

    /// The inferred complete path through the network.
    /// This can be an array of edges, WKT string, GeoJSON, etc. depending on format.
    pub matched_path: serde_json::Value,

    /// Summary of the traversal (e.g. total energy, distance, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traversal_summary: Option<serde_json::Value>,
}

/// A single edge in the matched path.
#[derive(Debug, Clone, Serialize)]
pub struct MatchedEdgeResponse {
    /// Index of the edge list containing the matched edge
    pub edge_list_id: usize,
    /// ID of the matched edge
    pub edge_id: u64,
    /// Optional geometry of the edge
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geometry: Option<geo::LineString<f32>>,
    /// The cost of traversing this edge
    pub cost: TraversalCost,
    /// The state after traversing this edge
    pub result_state: Vec<StateVariable>,
}

impl MatchedEdgeResponse {
    pub fn new(
        edge_list_id: usize,
        edge_id: u64,
        geometry: Option<geo::LineString<f32>>,
        cost: TraversalCost,
        result_state: Vec<StateVariable>,
    ) -> Self {
        Self {
            edge_list_id,
            edge_id,
            geometry,
            cost,
            result_state,
        }
    }
}

/// Match result for a single GPS point in the response.
#[derive(Debug, Clone, Serialize)]
pub struct PointMatchResponse {
    /// Index of the edge list containing the matched edge
    pub edge_list_id: usize,

    /// ID of the matched edge
    pub edge_id: u64,

    /// Distance from the GPS point to the matched edge (in meters)
    pub distance: f64,
}

impl MapMatchingResponse {
    /// Creates a new response from point matches and path.
    pub fn new(
        point_matches: Vec<PointMatchResponse>,
        matched_path: serde_json::Value,
        traversal_summary: Option<serde_json::Value>,
    ) -> Self {
        Self {
            point_matches,
            matched_path,
            traversal_summary,
        }
    }
}

impl PointMatchResponse {
    /// Creates a new point match response.
    pub fn new(edge_list_id: usize, edge_id: u64, distance: f64) -> Self {
        Self {
            edge_list_id,
            edge_id,
            distance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_serialize_response() {
        let response = MapMatchingResponse {
            point_matches: vec![
                PointMatchResponse::new(0, 1, 5.5),
                PointMatchResponse::new(0, 2, 3.2),
            ],
            matched_path: json!([
                MatchedEdgeResponse::new(0, 1, None, TraversalCost::default(), vec![]),
                MatchedEdgeResponse::new(0, 2, None, TraversalCost::default(), vec![]),
            ]),
            traversal_summary: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"point_matches\""));
        assert!(json.contains("\"matched_path\""));
        assert!(!json.contains("\"geometry\""));
        assert!(json.contains("\"cost\""));
        assert!(json.contains("\"result_state\""));
    }
}
