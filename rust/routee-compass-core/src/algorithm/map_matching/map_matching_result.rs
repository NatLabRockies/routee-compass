use crate::model::network::{EdgeId, EdgeListId};
use serde::{Deserialize, Serialize};

/// Result of matching a GPS trace to the road network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMatchingResult {
    /// Match results for each input point in the trace
    pub point_matches: Vec<PointMatch>,

    /// The inferred complete path through the network as edge IDs.
    /// This represents the assumed path the vehicle took, including
    /// edges between matched points that were computed via shortest path.
    pub matched_path: Vec<(EdgeListId, EdgeId)>,
}

impl MapMatchingResult {
    /// Creates a new result with the given point matches and path.
    pub fn new(point_matches: Vec<PointMatch>, matched_path: Vec<(EdgeListId, EdgeId)>) -> Self {
        Self {
            point_matches,
            matched_path,
        }
    }
}

/// Match result for a single GPS point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMatch {
    /// Index of the edge list containing the matched edge
    pub edge_list_id: EdgeListId,

    /// ID of the matched edge
    pub edge_id: EdgeId,

    /// Distance from the GPS point to the matched edge (in meters)
    pub distance_to_edge: f64,
}

impl PointMatch {
    /// Creates a new point match.
    pub fn new(edge_list_id: EdgeListId, edge_id: EdgeId, distance_to_edge: f64) -> Self {
        Self {
            edge_list_id,
            edge_id,
            distance_to_edge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_creation() {
        let point_matches = vec![
            PointMatch::new(EdgeListId(0), EdgeId(1), 5.5),
            PointMatch::new(EdgeListId(0), EdgeId(2), 3.2),
        ];
        let matched_path = vec![(EdgeListId(0), EdgeId(1)), (EdgeListId(0), EdgeId(2))];
        let result = MapMatchingResult::new(point_matches, matched_path);

        assert_eq!(result.point_matches.len(), 2);
        assert_eq!(result.matched_path.len(), 2);
    }
}
