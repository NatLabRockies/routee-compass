use serde::Serialize;

/// JSON-serializable response from map matching.
#[derive(Debug, Clone, Serialize)]
pub struct MapMatchingResponse {
    /// Match results for each input point in the trace.
    pub point_matches: Vec<PointMatchResponse>,

    /// The inferred complete path through the network as edge IDs.
    /// Each element is a tuple of (edge_list_id, edge_id).
    pub matched_path: Vec<(usize, u64)>,
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
    pub fn new(point_matches: Vec<PointMatchResponse>, matched_path: Vec<(usize, u64)>) -> Self {
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
            matched_path: vec![(0, 1), (0, 2)],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("point_matches"));
        assert!(json.contains("matched_path"));
    }
}
