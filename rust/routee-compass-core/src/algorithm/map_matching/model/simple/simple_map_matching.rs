use crate::algorithm::map_matching::map_matching_algorithm::MapMatchingAlgorithm;
use crate::algorithm::map_matching::map_matching_error::MapMatchingError;
use crate::algorithm::map_matching::map_matching_result::{MapMatchingResult, PointMatch};
use crate::algorithm::map_matching::map_matching_trace::MapMatchingTrace;
use crate::algorithm::search::SearchInstance;
use crate::model::map::NearestSearchResult;

/// A simple map matching algorithm that matches each GPS point to the nearest edge
/// and connects matched edges via the existing path in the trace order.
///
/// This is a baseline implementation suitable for high-quality GPS data with
/// frequent observations. For noisy GPS data or sparse traces, more sophisticated
/// algorithms (e.g., HMM-based) should be used.
///
/// # Algorithm
///
/// 1. For each point in the trace, find the nearest edge using the spatial index
/// 2. Record the matched edge and distance for each point
/// 3. Build the matched path as the sequence of unique consecutive edges
///
/// # Limitations
///
/// - Does not compute shortest paths between non-adjacent matched edges
/// - Assumes GPS points are already on or very close to the correct road
/// - Does not use timestamps for speed-based matching
#[derive(Debug, Clone, Default)]
pub struct SimpleMapMatching;

impl SimpleMapMatching {
    /// Creates a new instance of the simple map matching algorithm.
    pub fn new() -> Self {
        Self
    }
}

impl MapMatchingAlgorithm for SimpleMapMatching {
    fn match_trace(
        &self,
        trace: &MapMatchingTrace,
        si: &SearchInstance,
    ) -> Result<MapMatchingResult, MapMatchingError> {
        if trace.is_empty() {
            return Err(MapMatchingError::EmptyTrace);
        }

        let mut point_matches = Vec::with_capacity(trace.len());
        let mut matched_path = Vec::new();
        let mut last_edge: Option<(
            crate::model::network::EdgeListId,
            crate::model::network::EdgeId,
        )> = None;

        for (index, point) in trace.points.iter().enumerate() {
            // Find nearest edge using the spatial index
            let nearest = si
                .map_model
                .spatial_index
                .nearest_graph_id(&point.coord)
                .map_err(|e| MapMatchingError::PointMatchFailed {
                    index,
                    message: e.to_string(),
                })?;

            let (edge_list_id, edge_id, distance) = match nearest {
                NearestSearchResult::NearestEdge(list_id, eid) => {
                    // Calculate distance to edge (approximation using point-to-point distance)
                    // A more accurate implementation would compute perpendicular distance to linestring
                    let distance = 0.0; // Placeholder - would need geometry computation
                    (list_id, eid, distance)
                }
                NearestSearchResult::NearestVertex(vid) => {
                    // For vertex-oriented matching, we need to find an incident edge
                    // For now, return an error indicating edge-oriented matching is expected
                    return Err(MapMatchingError::PointMatchFailed {
                        index,
                        message: format!(
                            "vertex-oriented spatial index not supported for map matching, found vertex {}",
                            vid.0
                        ),
                    });
                }
            };

            point_matches.push(PointMatch::new(edge_list_id, edge_id, distance));

            // Add to matched path if different from last edge
            let current_edge = (edge_list_id, edge_id);
            if last_edge.map(|e| e != current_edge).unwrap_or(true) {
                matched_path.push(current_edge);
                last_edge = Some(current_edge);
            }
        }

        Ok(MapMatchingResult::new(point_matches, matched_path))
    }

    fn name(&self) -> &str {
        "simple_map_matching"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_algorithm_name() {
        let alg = SimpleMapMatching::new();
        assert_eq!(alg.name(), "simple_map_matching");
    }
}
