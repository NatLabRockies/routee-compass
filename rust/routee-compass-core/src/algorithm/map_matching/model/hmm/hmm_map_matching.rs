use crate::algorithm::map_matching::map_matching_algorithm::MapMatchingAlgorithm;
use crate::algorithm::map_matching::map_matching_error::MapMatchingError;
use crate::algorithm::map_matching::map_matching_result::{MapMatchingResult, PointMatch};
use crate::algorithm::map_matching::map_matching_trace::MapMatchingTrace;
use crate::algorithm::search::a_star::run_edge_oriented;
use crate::algorithm::search::{Direction, SearchInstance};
use crate::model::map::NearestSearchResult;
use crate::model::network::{EdgeId, EdgeListId};
use crate::util::geo::haversine;

/// Hidden Markov Model-based map matching algorithm.
///
/// This algorithm uses the Viterbi algorithm to find the most likely sequence
/// of road segments that a GPS trace represents. It considers both:
///
/// - **Emission probability**: How likely a GPS point was observed from a road edge
///   (based on perpendicular distance, modeled as Gaussian)
/// - **Transition probability**: How likely a vehicle traveled between two edges
///   (based on route distance vs. great-circle distance, modeled as exponential decay)
///
/// This approach is based on Newson & Krumm's "Hidden Markov Map Matching Through
/// Noise and Sparseness" (2009).
///
/// # Parameters
///
/// - `sigma`: GPS measurement noise standard deviation in meters. Controls the
///   spread of the emission probability distribution. Default: 50.0
/// - `beta`: Transition probability decay factor. Higher values penalize
///   differences between route distance and great-circle distance more heavily.
///   Default: 2.0
/// - `max_candidates`: Maximum number of candidate edges to consider per GPS point.
///   Default: 5
#[derive(Debug, Clone)]
pub struct HmmMapMatching {
    /// GPS measurement noise (meters)
    pub sigma: f64,
    /// Transition probability decay factor
    pub beta: f64,
    /// Maximum candidate edges per GPS point
    pub max_candidates: usize,
    /// Search query requirements for this algorithm
    pub search_parameters: serde_json::Value,
}

/// A candidate edge for a GPS observation
#[derive(Debug, Clone)]
struct Candidate {
    edge_list_id: EdgeListId,
    edge_id: EdgeId,
    distance_to_edge: f64,
}

/// State for Viterbi algorithm tracking
#[derive(Debug, Clone)]
struct ViterbiState {
    /// Log probability of reaching this state
    log_prob: f64,
    /// Index of the previous state that led here (for backtracking)
    prev_state_idx: Option<usize>,
}

impl Default for HmmMapMatching {
    fn default() -> Self {
        Self {
            sigma: 50.0,
            beta: 2.0,
            max_candidates: 5,
            search_parameters: serde_json::json!({}),
        }
    }
}

impl HmmMapMatching {
    /// Creates a new HMM map matching algorithm with default parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new HMM map matching algorithm with custom parameters.
    pub fn with_params(
        sigma: f64,
        beta: f64,
        max_candidates: usize,
        search_parameters: serde_json::Value,
    ) -> Self {
        Self {
            sigma,
            beta,
            max_candidates,
            search_parameters,
        }
    }

    /// Computes the emission log-probability for a GPS point given a candidate edge.
    /// Uses a Gaussian distribution based on the distance from the point to the edge.
    fn emission_log_prob(&self, distance: f64) -> f64 {
        // Gaussian: P(z|r) = (1 / (sqrt(2*pi) * sigma)) * exp(-d^2 / (2*sigma^2))
        // Log form: log(P) = -log(sqrt(2*pi)*sigma) - d^2 / (2*sigma^2)
        // We can drop the normalization constant since it's the same for all candidates
        let sigma_sq = self.sigma * self.sigma;
        -(distance * distance) / (2.0 * sigma_sq)
    }

    /// Computes the transition log-probability between two edges.
    /// Uses exponential decay based on the difference between route distance
    /// and great-circle distance.
    fn transition_log_prob(&self, route_distance: f64, great_circle_distance: f64) -> f64 {
        // Transition probability: P(r_j | r_i) = (1/beta) * exp(-|d_route - d_gc| / beta)
        // Log form: log(P) = -log(beta) - |d_route - d_gc| / beta
        // Drop normalization constant
        let diff = (route_distance - great_circle_distance).abs();
        -diff / self.beta
    }

    /// Finds candidate edges for a GPS point using the spatial index.
    fn find_candidates(
        &self,
        point_idx: usize,
        point: &geo::Point<f32>,
        si: &SearchInstance,
    ) -> Result<Vec<Candidate>, MapMatchingError> {
        // Get k-nearest edges using the iterator
        let nearest_iter = si
            .map_model
            .spatial_index
            .nearest_graph_id_iter(point)
            .take(self.max_candidates);

        let mut candidates = Vec::new();
        for result in nearest_iter {
            match result {
                NearestSearchResult::NearestEdge(edge_list_id, edge_id) => {
                    // Compute distance from point to edge
                    let distance =
                        self.compute_distance_to_edge(point, &edge_list_id, &edge_id, si);
                    candidates.push(Candidate {
                        edge_list_id,
                        edge_id,
                        distance_to_edge: distance,
                    });
                }
                NearestSearchResult::NearestVertex(_) => {
                    // Skip vertex results - we need edge-oriented matching
                    continue;
                }
            }
        }

        // Sort candidates by actual distance (closest first)
        candidates.sort_by(|a, b| {
            a.distance_to_edge
                .partial_cmp(&b.distance_to_edge)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if candidates.is_empty() {
            // Fall back to single nearest if iterator returns nothing
            match si.map_model.spatial_index.nearest_graph_id(point) {
                Ok(NearestSearchResult::NearestEdge(edge_list_id, edge_id)) => {
                    let distance =
                        self.compute_distance_to_edge(point, &edge_list_id, &edge_id, si);
                    candidates.push(Candidate {
                        edge_list_id,
                        edge_id,
                        distance_to_edge: distance,
                    });
                }
                Ok(NearestSearchResult::NearestVertex(_)) => {
                    return Err(MapMatchingError::PointMatchFailed {
                        index: point_idx,
                        message: "vertex-oriented spatial index not supported for HMM map matching"
                            .to_string(),
                    });
                }
                Err(e) => {
                    return Err(MapMatchingError::PointMatchFailed {
                        index: point_idx,
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(candidates)
    }

    /// Computes the approximate distance from a point to an edge.
    fn compute_distance_to_edge(
        &self,
        point: &geo::Point<f32>,
        edge_list_id: &EdgeListId,
        edge_id: &EdgeId,
        si: &SearchInstance,
    ) -> f64 {
        // Get edge endpoints and compute perpendicular distance
        if let Ok(linestring) = si.map_model.get_linestring(edge_list_id, edge_id) {
            // Find the closest point on the linestring by checking each segment
            let mut min_distance = f64::INFINITY;

            // Check each segment of the linestring
            let points: Vec<geo::Point<f32>> = linestring.points().collect();
            for window in points.windows(2) {
                // Compute distance to the segment
                let segment_dist = self.distance_to_segment(point, &window[0], &window[1]);
                if segment_dist < min_distance {
                    min_distance = segment_dist;
                }
            }

            // Also check distance to each vertex
            for p in linestring.points() {
                if let Ok(dist) = haversine::haversine_distance(point.x(), point.y(), p.x(), p.y())
                {
                    let dist_m = dist.get::<uom::si::length::meter>();
                    if dist_m < min_distance {
                        min_distance = dist_m;
                    }
                }
            }

            min_distance
        } else {
            f64::INFINITY
        }
    }

    /// Computes the approximate distance from a point to a line segment in meters.
    fn distance_to_segment(
        &self,
        point: &geo::Point<f32>,
        seg_start: &geo::Point<f32>,
        seg_end: &geo::Point<f32>,
    ) -> f64 {
        // Use simple projection onto the segment
        // This is an approximation that works well for short segments
        let dx = seg_end.x() - seg_start.x();
        let dy = seg_end.y() - seg_start.y();

        if dx == 0.0 && dy == 0.0 {
            // Segment is a point
            return haversine::haversine_distance(
                point.x(),
                point.y(),
                seg_start.x(),
                seg_start.y(),
            )
            .map(|d| d.get::<uom::si::length::meter>())
            .unwrap_or(f64::INFINITY);
        }

        // Project point onto line defined by segment
        let t = ((point.x() - seg_start.x()) * dx + (point.y() - seg_start.y()) * dy)
            / (dx * dx + dy * dy);

        // Clamp t to [0, 1] to stay on segment
        let t = t.clamp(0.0, 1.0);

        // Compute closest point on segment
        let closest_x = seg_start.x() + t * dx;
        let closest_y = seg_start.y() + t * dy;

        haversine::haversine_distance(point.x(), point.y(), closest_x, closest_y)
            .map(|d| d.get::<uom::si::length::meter>())
            .unwrap_or(f64::INFINITY)
    }

    /// Computes the great-circle distance between two GPS points in meters.
    fn great_circle_distance(&self, p1: &geo::Point<f32>, p2: &geo::Point<f32>) -> f64 {
        haversine::haversine_distance(p1.x(), p1.y(), p2.x(), p2.y())
            .map(|d| d.get::<uom::si::length::meter>())
            .unwrap_or(f64::INFINITY)
    }

    /// Computes the route distance between two candidate edges using A* search.
    /// Returns the approximate distance from the midpoint of the source edge
    /// to the midpoint of the destination edge.
    fn route_distance(&self, from: &Candidate, to: &Candidate, si: &SearchInstance) -> Option<f64> {
        // Get the distances of source and destination edges
        let from_edge = si.graph.get_edge(&from.edge_list_id, &from.edge_id).ok()?;
        let to_edge = si.graph.get_edge(&to.edge_list_id, &to.edge_id).ok()?;
        let from_edge_dist = from_edge.distance.get::<uom::si::length::meter>();
        let to_edge_dist = to_edge.distance.get::<uom::si::length::meter>();

        // Same edge: distance is 0
        if from.edge_list_id == to.edge_list_id && from.edge_id == to.edge_id {
            return Some(0.0);
        }

        // Check if edges are directly connected (dst of source = src of destination)
        // In this case, A* returns early with empty tree, so handle specially
        let from_dst = si
            .graph
            .dst_vertex_id(&from.edge_list_id, &from.edge_id)
            .ok()?;
        let to_src = si.graph.src_vertex_id(&to.edge_list_id, &to.edge_id).ok()?;
        if from_dst == to_src {
            // Edges are directly connected: midpoint to midpoint distance is
            // half of source edge + half of destination edge
            return Some((from_edge_dist / 2.0) + (to_edge_dist / 2.0));
        }

        // Run A* search from source edge to target edge
        let result = run_edge_oriented(
            (from.edge_list_id, from.edge_id),
            Some((to.edge_list_id, to.edge_id)),
            &Direction::Forward,
            true,
            si,
        );

        match result {
            Ok(search_result) => {
                // Get the destination vertex to backtrack from
                let dst_vertex = si.graph.src_vertex_id(&to.edge_list_id, &to.edge_id).ok()?;

                // Backtrack to get the path
                if let Ok(path) = search_result.tree.backtrack(dst_vertex) {
                    // Sum up the edge distances for intermediate edges
                    let intermediate_distance: f64 = path
                        .iter()
                        .filter_map(|et| {
                            si.graph
                                .get_edge(&et.edge_list_id, &et.edge_id)
                                .ok()
                                .map(|e| e.distance.get::<uom::si::length::meter>())
                        })
                        .sum();

                    // Add half of source edge (from midpoint to end) and half of
                    // destination edge (from start to midpoint) to approximate
                    // the midpoint-to-midpoint travel distance
                    Some(intermediate_distance + (from_edge_dist / 2.0) + (to_edge_dist / 2.0))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}

impl MapMatchingAlgorithm for HmmMapMatching {
    fn match_trace(
        &self,
        trace: &MapMatchingTrace,
        si: &SearchInstance,
    ) -> Result<MapMatchingResult, MapMatchingError> {
        if trace.is_empty() {
            return Err(MapMatchingError::EmptyTrace);
        }

        // HMM map matching requires an edge-oriented spatial index
        if !si.map_model.spatial_index.is_edge_oriented() {
            return Err(MapMatchingError::InternalError(
                "HMM map matching requires an edge-oriented spatial index. \
                 Set 'spatial_index_type = \"edge\"' in the [mapping] configuration."
                    .to_string(),
            ));
        }

        let n_points = trace.len();

        // Step 1: Find candidates for each point
        let mut all_candidates: Vec<Vec<Candidate>> = Vec::with_capacity(n_points);
        for (idx, point) in trace.points.iter().enumerate() {
            let candidates = self.find_candidates(idx, &point.coord, si)?;
            all_candidates.push(candidates);
        }

        // Step 2: Initialize Viterbi for first observation
        let first_candidates = &all_candidates[0];
        let mut prev_states: Vec<ViterbiState> = first_candidates
            .iter()
            .map(|c| ViterbiState {
                log_prob: self.emission_log_prob(c.distance_to_edge),
                prev_state_idx: None,
            })
            .collect();

        // Step 3: Forward pass - compute Viterbi probabilities
        for t in 1..n_points {
            let curr_candidates = &all_candidates[t];
            let prev_candidates = &all_candidates[t - 1];
            let curr_point = &trace.points[t].coord;
            let prev_point = &trace.points[t - 1].coord;
            let gc_distance = self.great_circle_distance(prev_point, curr_point);

            let mut curr_states: Vec<ViterbiState> = Vec::with_capacity(curr_candidates.len());

            for curr_cand in curr_candidates.iter() {
                let emission_lp = self.emission_log_prob(curr_cand.distance_to_edge);
                let mut best_log_prob = f64::NEG_INFINITY;
                let mut best_prev_idx: Option<usize> = None;

                for (prev_idx, prev_cand) in prev_candidates.iter().enumerate() {
                    let prev_log_prob = prev_states[prev_idx].log_prob;
                    if prev_log_prob == f64::NEG_INFINITY {
                        continue;
                    }

                    // Compute transition probability
                    let route_dist = self.route_distance(prev_cand, curr_cand, si);
                    let transition_lp = match route_dist {
                        Some(rd) => self.transition_log_prob(rd, gc_distance),
                        None => f64::NEG_INFINITY, // No valid route
                    };

                    let total_lp = prev_log_prob + transition_lp + emission_lp;
                    if total_lp > best_log_prob {
                        best_log_prob = total_lp;
                        best_prev_idx = Some(prev_idx);
                    }
                }

                curr_states.push(ViterbiState {
                    log_prob: best_log_prob,
                    prev_state_idx: best_prev_idx,
                });
            }

            prev_states = curr_states;
        }

        // Step 4: Backtrack to find the best path
        // Find the best ending state
        let (_best_end_idx, _) = prev_states
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.log_prob
                    .partial_cmp(&b.log_prob)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| {
                MapMatchingError::InternalError("no valid ending state found".to_string())
            })?;

        // The first-pass forward algorithm above discards intermediate states.
        // We need to re-run the forward pass storing all states for proper backtracking.
        // Due to the backtracking issue above, let's use a different approach:
        // Store all Viterbi states during forward pass
        let mut all_states: Vec<Vec<ViterbiState>> = Vec::with_capacity(n_points);

        // Re-run forward pass storing all states
        let first_candidates = &all_candidates[0];
        let first_states: Vec<ViterbiState> = first_candidates
            .iter()
            .map(|c| ViterbiState {
                log_prob: self.emission_log_prob(c.distance_to_edge),
                prev_state_idx: None,
            })
            .collect();
        all_states.push(first_states);

        for t in 1..n_points {
            let curr_candidates = &all_candidates[t];
            let prev_candidates = &all_candidates[t - 1];
            let curr_point = &trace.points[t].coord;
            let prev_point = &trace.points[t - 1].coord;
            let gc_distance = self.great_circle_distance(prev_point, curr_point);

            let mut curr_states: Vec<ViterbiState> = Vec::with_capacity(curr_candidates.len());

            for curr_cand in curr_candidates.iter() {
                let emission_lp = self.emission_log_prob(curr_cand.distance_to_edge);
                let mut best_log_prob = f64::NEG_INFINITY;
                let mut best_prev_idx: Option<usize> = None;

                for (prev_idx, prev_cand) in prev_candidates.iter().enumerate() {
                    let prev_log_prob = all_states[t - 1][prev_idx].log_prob;
                    if prev_log_prob == f64::NEG_INFINITY {
                        continue;
                    }

                    let route_dist = self.route_distance(prev_cand, curr_cand, si);
                    let transition_lp = match route_dist {
                        Some(rd) => self.transition_log_prob(rd, gc_distance),
                        None => f64::NEG_INFINITY,
                    };

                    let total_lp = prev_log_prob + transition_lp + emission_lp;
                    if total_lp > best_log_prob {
                        best_log_prob = total_lp;
                        best_prev_idx = Some(prev_idx);
                    }
                }

                curr_states.push(ViterbiState {
                    log_prob: best_log_prob,
                    prev_state_idx: best_prev_idx,
                });
            }

            all_states.push(curr_states);
        }

        // Backtrack with full state history
        let (best_end_idx, _) = all_states[n_points - 1]
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.log_prob
                    .partial_cmp(&b.log_prob)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| {
                MapMatchingError::InternalError("no valid ending state found".to_string())
            })?;

        let mut best_indices: Vec<usize> = vec![0; n_points];
        best_indices[n_points - 1] = best_end_idx;

        for t in (1..n_points).rev() {
            let current_state = &all_states[t][best_indices[t]];
            best_indices[t - 1] = match current_state.prev_state_idx {
                Some(idx) => idx,
                None => {
                    // No valid transition found - fall back to the state with best log probability
                    // at time t-1 (which corresponds to best emission probability)
                    all_states[t - 1]
                        .iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| {
                            a.log_prob
                                .partial_cmp(&b.log_prob)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(idx, _)| idx)
                        .unwrap_or(0)
                }
            };
        }

        // Build result from best path
        let mut point_matches = Vec::with_capacity(n_points);
        let mut matched_path: Vec<(EdgeListId, EdgeId)> = Vec::new();
        let mut last_edge: Option<(EdgeListId, EdgeId)> = None;

        for (t, &state_idx) in best_indices.iter().enumerate() {
            let candidate = &all_candidates[t][state_idx];
            point_matches.push(PointMatch::new(
                candidate.edge_list_id,
                candidate.edge_id,
                candidate.distance_to_edge,
            ));

            let current_edge = (candidate.edge_list_id, candidate.edge_id);
            if last_edge.map(|e| e != current_edge).unwrap_or(true) {
                matched_path.push(current_edge);
                last_edge = Some(current_edge);
            }
        }

        Ok(MapMatchingResult::new(point_matches, matched_path))
    }

    fn name(&self) -> &str {
        "hmm_map_matching"
    }

    fn search_parameters(&self) -> serde_json::Value {
        self.search_parameters.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_algorithm_name() {
        let alg = HmmMapMatching::new();
        assert_eq!(alg.name(), "hmm_map_matching");
    }

    #[test]
    fn test_default_params() {
        let alg = HmmMapMatching::default();
        assert_eq!(alg.sigma, 50.0);
        assert_eq!(alg.beta, 2.0);
        assert_eq!(alg.max_candidates, 5);
    }

    #[test]
    fn test_custom_params() {
        let alg = HmmMapMatching::with_params(100.0, 5.0, 10, serde_json::json!({}));
        assert_eq!(alg.sigma, 100.0);
        assert_eq!(alg.beta, 5.0);
        assert_eq!(alg.max_candidates, 10);
    }

    #[test]
    fn test_emission_log_prob() {
        let alg = HmmMapMatching::with_params(50.0, 2.0, 5, serde_json::json!({}));
        // At distance 0, log prob should be 0
        assert_eq!(alg.emission_log_prob(0.0), 0.0);
        // Higher distance should give lower (more negative) log prob
        let p1 = alg.emission_log_prob(10.0);
        let p2 = alg.emission_log_prob(50.0);
        assert!(p1 > p2);
    }

    #[test]
    fn test_transition_log_prob() {
        let alg = HmmMapMatching::with_params(50.0, 2.0, 5, serde_json::json!({}));
        // When route distance equals gc distance, log prob should be 0
        assert_eq!(alg.transition_log_prob(100.0, 100.0), 0.0);
        // Larger difference should give lower log prob
        let p1 = alg.transition_log_prob(110.0, 100.0);
        let p2 = alg.transition_log_prob(150.0, 100.0);
        assert!(p1 > p2);
    }
}
