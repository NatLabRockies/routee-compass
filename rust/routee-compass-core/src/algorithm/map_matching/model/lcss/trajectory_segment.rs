use crate::algorithm::map_matching::map_matching_error::MapMatchingError;
use crate::algorithm::map_matching::map_matching_result::PointMatch;
use crate::algorithm::map_matching::map_matching_trace::MapMatchingTrace;
use crate::algorithm::search::SearchInstance;
use crate::model::network::{EdgeId, EdgeListId};
use itertools::Itertools;
use uom::si::f64::Length;
use uom::si::length::meter;

use super::lcss_map_matching::LcssMapMatching;
use super::lcss_ops;

/// A segment of a trajectory that includes the trace points, the matched path,
/// and the score of the match.
#[derive(Debug, Clone)]
pub(crate) struct TrajectorySegment {
    pub(crate) trace: MapMatchingTrace,
    pub(crate) path: Vec<(EdgeListId, EdgeId)>,
    pub(crate) matches: Vec<PointMatch>,
    pub(crate) score: f64,
    pub(crate) cutting_points: Vec<usize>,
}

impl TrajectorySegment {
    /// Creates a new `TrajectorySegment` with the given trace and path.
    ///
    /// The segment is initialized with an empty set of matches, a score of 0.0,
    /// and no cutting points. To populate these fields, call [`score_and_match`].
    ///
    /// # Arguments
    /// * `trace` - The map matching trace containing the sequence of GPS points.
    /// * `path` - The sequence of edges and edge list IDs that are matched to the trace.
    pub(crate) fn new(trace: MapMatchingTrace, path: Vec<(EdgeListId, EdgeId)>) -> Self {
        Self {
            trace,
            path,
            matches: Vec::new(),
            score: 0.0,
            cutting_points: Vec::new(),
        }
    }

    /// Scores the segment using a Longest Common Subsequence (LCSS) approach and
    /// matches each individual trace point to the nearest edge in the path.
    ///
    /// This function performs several steps:
    /// 1. Precomputes distances from every trace point to every edge in the path.
    /// 2. Uses a dynamic programming approach (LCSS) to calculate a similarity score
    ///    between the trace and the path, based on the `distance_epsilon`.
    /// 3. Normalizes the score based on the lengths of the trace and path.
    /// 4. Assigns each trace point to its nearest path edge, as long as the distance
    ///    is within the `distance_threshold`.
    /// 5. Applies an optional penalty if the first or last points are poorly matched,
    ///    ensuring the path covers the extent of the trace.
    ///
    /// # Arguments
    /// * `lcss` - The LCSS map matching configuration and parameters.
    /// * `si` - The search instance used to lookup edge geometry and calculate distances.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(MapMatchingError::EmptyTrace)` if the trace contains no points.
    pub(crate) fn score_and_match(
        &mut self,
        lcss: &LcssMapMatching,
        si: &SearchInstance,
    ) -> Result<(), MapMatchingError> {
        let m = self.trace.len();
        let n = self.path.len();

        if m == 0 {
            return Err(MapMatchingError::EmptyTrace);
        }

        if n == 0 {
            self.score = 0.0;
            self.matches = self
                .trace
                .points
                .iter()
                .map(|_| {
                    PointMatch::new(
                        EdgeListId(0),
                        EdgeId(0),
                        Length::new::<meter>(f64::INFINITY),
                    )
                })
                .collect();
            return Ok(());
        }

        // Precompute distances
        let mut distances = vec![vec![Length::new::<meter>(0.0); m]; n];
        for (j, (next_el, next_e)) in self.path.iter().enumerate() {
            for (i, trace_point) in self.trace.points.iter().enumerate() {
                distances[j][i] =
                    lcss_ops::compute_distance_to_edge(&trace_point.coord, next_el, next_e, si);
            }
        }

        let mut c = vec![vec![0.0; n + 1]; m + 1];
        let mut point_matches = Vec::with_capacity(m);

        for i in 1..=m {
            let mut min_dist = Length::new::<meter>(f64::INFINITY);
            let mut nearest_edge = self.path[0];

            for j in 1..=n {
                let dt = distances[j - 1][i - 1];
                if dt < min_dist {
                    min_dist = dt;
                    nearest_edge = self.path[j - 1];
                }

                let point_similarity = if dt < lcss.distance_epsilon {
                    1.0 - (dt.get::<meter>() / lcss.distance_epsilon.get::<meter>())
                } else {
                    0.0
                };

                c[i][j] = f64::max(
                    c[i - 1][j - 1] + point_similarity,
                    f64::max(c[i][j - 1], c[i - 1][j]),
                );
            }

            if min_dist > lcss.distance_threshold {
                min_dist = Length::new::<meter>(f64::INFINITY);
            }

            point_matches.push(PointMatch::new(nearest_edge.0, nearest_edge.1, min_dist));
        }

        self.score = c[m][n] / (m.min(n) as f64);
        self.matches = point_matches;

        // Penalize paths that don't cover the endpoints well
        if !self.matches.is_empty() {
            let first_point_dist = self.matches[0].distance_to_edge;
            let last_point_dist = self.matches[m - 1].distance_to_edge;

            // Apply penalty if first or last point is not well-matched
            if first_point_dist > lcss.distance_epsilon || last_point_dist > lcss.distance_epsilon {
                let first_ratio =
                    first_point_dist.get::<meter>() / lcss.distance_epsilon.get::<meter>();
                let last_ratio =
                    last_point_dist.get::<meter>() / lcss.distance_epsilon.get::<meter>();
                let endpoint_penalty = (first_ratio.max(1.0) + last_ratio.max(1.0)) / 2.0;
                self.score /= endpoint_penalty;
            }
        }

        Ok(())
    }

    /// Identifies "cutting points" within the trace where the current path match
    /// might be suboptimal, allowing for potential segment splitting.
    ///
    /// Cutting points are selected based on several heuristics:
    /// - If the path is empty or no points were matched within the threshold, the
    ///   midpoint of the trace is selected.
    /// - The point with the maximum distance to its matched edge is selected.
    /// - Points with distances that are very close to the `distance_epsilon` (within `cutting_threshold`)
    ///   are selected, as these represent boundary cases in the LCSS similarity logic.
    ///
    /// The resulting points are compressed to avoid redundant splits (see [`compress`])
    /// and filtered to exclude points too close to the start or end of the trace.
    ///
    /// # Arguments
    /// * `lcss` - The LCSS configuration containing epsilon and threshold parameters.
    pub(crate) fn compute_cutting_points(&mut self, lcss: &LcssMapMatching) {
        let mut cutting_points = Vec::new();

        let no_match = self
            .matches
            .iter()
            .all(|m| m.distance_to_edge.get::<meter>().is_infinite());

        if self.path.is_empty() || no_match {
            // Pick the middle point
            cutting_points.push(self.trace.len() / 2);
        } else {
            // Find furthest point
            if let Some((idx, _)) = self
                .matches
                .iter()
                .enumerate()
                .filter(|(_, m)| !m.distance_to_edge.get::<meter>().is_infinite())
                .max_by(|(_, a), (_, b)| {
                    a.distance_to_edge
                        .partial_cmp(&b.distance_to_edge)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                cutting_points.push(idx);
            }

            // Collect points close to epsilon
            for (i, m) in self.matches.iter().enumerate() {
                if !m.distance_to_edge.get::<meter>().is_infinite()
                    && (m.distance_to_edge - lcss.distance_epsilon).abs() < lcss.cutting_threshold
                {
                    cutting_points.push(i);
                }
            }
        }

        let compressed_points = compress(cutting_points);

        // Filter out start/end and points directly adjacent to them since it doesn't make sense to split there
        let n = self.trace.len();
        self.cutting_points = compressed_points
            .into_iter()
            .unique()
            .filter(|&idx| idx > 1 && idx < n - 2)
            .collect();
        self.cutting_points.sort();
    }

    /// Splits this segment into multiple sub-segments based on the previously
    /// computed `cutting_points`.
    ///
    /// For each sub-sequence of trace points defined by the cutting points, a new
    /// optimal path is searched for using [`lcss_ops::new_path_for_trace`].
    ///
    /// # Arguments
    /// * `si` - The search instance used to find new paths for the resulting sub-segments.
    ///
    /// # Returns
    /// - `Ok(Vec<TrajectorySegment>)`: A list of new segments. If there are no cutting
    ///   points, it returns a vector containing a clone of the current segment.
    /// - `Err(MapMatchingError)` if a path cannot be found for one of the sub-segments.
    pub(crate) fn split_segment(
        &self,
        si: &SearchInstance,
    ) -> Result<Vec<TrajectorySegment>, MapMatchingError> {
        if self.trace.len() < 2 || self.cutting_points.is_empty() {
            return Ok(vec![self.clone()]);
        }

        let mut result = Vec::new();
        let mut last_idx = 0;

        for &cp in &self.cutting_points {
            let sub_points = self.trace.points[last_idx..cp].to_vec();
            if !sub_points.is_empty() {
                let sub_trace = MapMatchingTrace::new(sub_points);
                let path = lcss_ops::new_path_for_trace(&sub_trace, si)?;
                result.push(TrajectorySegment::new(sub_trace, path));
            }
            last_idx = cp;
        }

        let sub_points = self.trace.points[last_idx..].to_vec();
        if !sub_points.is_empty() {
            let sub_trace = MapMatchingTrace::new(sub_points);
            let path = lcss_ops::new_path_for_trace(&sub_trace, si)?;
            result.push(TrajectorySegment::new(sub_trace, path));
        }

        Ok(result)
    }
}
/// Combines multiple `TrajectorySegment`s into a single cohesive segment.
///
/// This function iterates through the provided segments, concatenating their trace
/// points. It also stitches their paths together. If a gap exists between the end
/// of one segment's path and the start of the next (i.e., the vertices don't match),
/// a shortest-path search is performed to bridge the gap.
///
/// Finally, the combined segment is re-scored and re-matched against the full trace.
///
/// # Arguments
/// * `lcss` - The LCSS configuration for re-scoring the joined segment.
/// * `segments` - The ordered list of segments to be joined.
/// * `si` - The search instance used for gap-filling shortest path searches and re-scoring.
///
/// # Returns
/// - `Ok(TrajectorySegment)` representing the fully joined trajectory.
/// - `Err(MapMatchingError)` if the segments list is empty or if an error occurs during
///   gap-filling or re-scoring.
pub(crate) fn join_segments(
    lcss: &LcssMapMatching,
    segments: Vec<TrajectorySegment>,
    si: &SearchInstance,
) -> Result<TrajectorySegment, MapMatchingError> {
    if segments.is_empty() {
        return Err(MapMatchingError::InternalError(
            "empty segments to join".to_string(),
        ));
    }

    let mut total_points = Vec::new();
    let mut total_path = Vec::new();

    for i in 0..segments.len() {
        total_points.extend(segments[i].trace.points.clone());

        if i > 0 {
            let prev_path = &segments[i - 1].path;
            let curr_path = &segments[i].path;
            if !prev_path.is_empty() && !curr_path.is_empty() {
                let prev_end = &prev_path[prev_path.len() - 1];
                let curr_start = &curr_path[0];

                if prev_end.0 != curr_start.0 || prev_end.1 != curr_start.1 {
                    // Check if they are connected
                    let prev_dst_v = si
                        .graph
                        .dst_vertex_id(&prev_end.0, &prev_end.1)
                        .map_err(|e| MapMatchingError::InternalError(e.to_string()))?;
                    let curr_src_v = si
                        .graph
                        .src_vertex_id(&curr_start.0, &curr_start.1)
                        .map_err(|e| MapMatchingError::InternalError(e.to_string()))?;

                    if prev_dst_v != curr_src_v {
                        let gap_path = lcss_ops::run_shortest_path(prev_dst_v, curr_src_v, si)?;
                        total_path.extend(gap_path);
                    }
                }
            }
        }
        total_path.extend(segments[i].path.clone());
    }

    // De-duplicate consecutive edges in path
    total_path.dedup();

    let mut joined = TrajectorySegment::new(MapMatchingTrace::new(total_points), total_path);

    joined.score_and_match(lcss, si)?;
    Ok(joined)
}

/// Reduces a list of cutting point indices by grouping consecutive integers.
///
/// For each group of consecutive indices (e.g., `[1, 2, 3]`), only the middle index
/// is retained (e.g., `2`). This prevents the algorithm from splitting the trace
/// at every single point in a "bad" region, instead picking a single representative
/// split point for each region.
///
/// # Arguments
/// * `cutting_points` - A list of (potentially unsorted) indices into the trace.
///
/// # Returns
/// A sorted `Vec<usize>` containing the representative split points.
pub(crate) fn compress(mut cutting_points: Vec<usize>) -> Vec<usize> {
    if cutting_points.is_empty() {
        return Vec::new();
    }
    cutting_points.sort();

    let mut result = Vec::new();
    let mut current_group = vec![cutting_points[0]];

    for &point in &cutting_points[1..] {
        if point == current_group.last().unwrap() + 1 {
            current_group.push(point);
        } else {
            let mid = current_group.len() / 2;
            result.push(current_group[mid]);
            current_group = vec![point];
        }
    }

    if !current_group.is_empty() {
        let mid = current_group.len() / 2;
        result.push(current_group[mid]);
    }

    result
}

#[cfg(test)]
mod compress_tests {
    use super::*;

    #[test]
    fn test_compress_no_consecutive() {
        let points = vec![1, 3, 5, 7];
        let compressed = compress(points.clone());
        assert_eq!(compressed, points);
    }

    #[test]
    fn test_compress_all_consecutive() {
        let points = vec![1, 2, 3, 4, 5];
        let compressed = compress(points);
        assert_eq!(compressed, vec![3]);
    }

    #[test]
    fn test_compress_mixed() {
        let points = vec![1, 2, 3, 6, 7, 8, 10];
        let compressed = compress(points);
        assert_eq!(compressed, vec![2, 7, 10]);
    }

    #[test]
    fn test_compress_empty() {
        let points: Vec<usize> = vec![];
        let compressed = compress(points);
        assert!(compressed.is_empty());
    }

    #[test]
    fn test_compress_single() {
        let points = vec![5];
        let compressed = compress(points);
        assert_eq!(compressed, vec![5]);
    }

    #[test]
    fn test_compress_groups() {
        let points = vec![1, 2, 4, 5];
        let compressed = compress(points);
        assert_eq!(compressed, vec![2, 5]);
    }
}
