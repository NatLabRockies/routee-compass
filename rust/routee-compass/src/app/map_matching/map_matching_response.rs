use serde::Serialize;

/// JSON-serializable response from map matching.
#[derive(Debug, Clone, Serialize)]
pub struct MapMatchingResponse {
    /// Match results for each input point in the trace.
    pub point_matches: Vec<PointMatchResponse>,

    /// The inferred complete path through the network.
    pub matched_path: Vec<MatchedEdgeResponse>,
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
}

impl MatchedEdgeResponse {
    pub fn new(edge_list_id: usize, edge_id: u64, geometry: Option<geo::LineString<f32>>) -> Self {
        Self {
            edge_list_id,
            edge_id,
            geometry,
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
        matched_path: Vec<MatchedEdgeResponse>,
    ) -> Self {
        Self {
            point_matches,
            matched_path,
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

    #[test]
    fn test_serialize_response() {
        let response = MapMatchingResponse {
            point_matches: vec![
                PointMatchResponse::new(0, 1, 5.5),
                PointMatchResponse::new(0, 2, 3.2),
            ],
            matched_path: vec![
                MatchedEdgeResponse::new(0, 1, None),
                MatchedEdgeResponse::new(0, 2, None),
            ],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"point_matches\""));
        assert!(json.contains("\"matched_path\""));
        assert!(!json.contains("\"geometry\""));
    }
}
