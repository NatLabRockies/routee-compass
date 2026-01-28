use crate::algorithm::map_matching::map_matching_algorithm::MapMatchingAlgorithm;
use crate::algorithm::map_matching::map_matching_error::MapMatchingError;
use crate::algorithm::map_matching::map_matching_result::{MapMatchingResult, PointMatch};
use crate::algorithm::map_matching::map_matching_trace::MapMatchingTrace;
use crate::algorithm::search::a_star::run_edge_oriented;
use crate::algorithm::search::{Direction, SearchInstance};
use crate::model::map::NearestSearchResult;
use crate::model::network::{EdgeId, EdgeListId};
use crate::util::geo::haversine;
use itertools::Itertools;

/// A map matching algorithm based on the Longest Common Subsequence (LCSS) similarity.
///
/// This is a port of the LCSS matcher from the mappymatch package.
///
/// # Parameters
///
/// - `distance_epsilon`: The distance epsilon to use for matching (default: 50.0 meters)
/// - `similarity_cutoff`: The similarity cutoff to use for stopping the algorithm (default: 0.9)
/// - `cutting_threshold`: The distance threshold to use for computing cutting points (default: 10.0 meters)
/// - `random_cuts`: The number of random cuts to add at each iteration (default: 0)
/// - `distance_threshold`: The distance threshold above which no match is made (default: 10000.0 meters)
#[derive(Debug, Clone)]
pub struct LcssMapMatching {
    pub distance_epsilon: f64,
    pub similarity_cutoff: f64,
    pub cutting_threshold: f64,
    pub random_cuts: usize,
    pub distance_threshold: f64,
    /// Search query requirements for this algorithm
    pub search_parameters: serde_json::Value,
}

impl Default for LcssMapMatching {
    fn default() -> Self {
        Self {
            distance_epsilon: 50.0,
            similarity_cutoff: 0.9,
            cutting_threshold: 10.0,
            random_cuts: 0,
            distance_threshold: 10000.0,
            search_parameters: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone)]
struct TrajectorySegment {
    trace: MapMatchingTrace,
    path: Vec<(EdgeListId, EdgeId)>,
    matches: Vec<PointMatch>,
    score: f64,
    cutting_points: Vec<usize>,
}

#[derive(Debug, Clone)]
struct StationaryIndex {
    i_index: Vec<usize>,
}

impl LcssMapMatching {
    pub fn new() -> Self {
        Self::default()
    }

    /// Finds the nearest edge for a point.
    fn find_nearest_edge(
        &self,
        point: &geo::Point<f32>,
        si: &SearchInstance,
    ) -> Result<(EdgeListId, EdgeId, f64), MapMatchingError> {
        let nearest = si
            .map_model
            .spatial_index
            .nearest_graph_id(point)
            .map_err(|e| {
                MapMatchingError::InternalError(format!("spatial index query failed: {}", e))
            })?;

        match nearest {
            NearestSearchResult::NearestEdge(list_id, eid) => {
                let distance = self.compute_distance_to_edge(point, &list_id, &eid, si);
                Ok((list_id, eid, distance))
            }
            NearestSearchResult::NearestVertex(_) => Err(MapMatchingError::InternalError(
                "vertex-oriented spatial index not supported for LCSS map matching".to_string(),
            )),
        }
    }

    /// Computes the approximate distance from a point to an edge.
    /// (Copied from HmmMapMatching)
    fn compute_distance_to_edge(
        &self,
        point: &geo::Point<f32>,
        edge_list_id: &EdgeListId,
        edge_id: &EdgeId,
        si: &SearchInstance,
    ) -> f64 {
        if let Ok(linestring) = si.map_model.get_linestring(edge_list_id, edge_id) {
            let mut min_distance = f64::INFINITY;
            let points: Vec<geo::Point<f32>> = linestring.points().collect();
            for window in points.windows(2) {
                let segment_dist = self.distance_to_segment(point, &window[0], &window[1]);
                if segment_dist < min_distance {
                    min_distance = segment_dist;
                }
            }
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

    fn distance_to_segment(
        &self,
        point: &geo::Point<f32>,
        seg_start: &geo::Point<f32>,
        seg_end: &geo::Point<f32>,
    ) -> f64 {
        let dx = seg_end.x() - seg_start.x();
        let dy = seg_end.y() - seg_start.y();

        if dx == 0.0 && dy == 0.0 {
            return haversine::haversine_distance(
                point.x(),
                point.y(),
                seg_start.x(),
                seg_start.y(),
            )
            .map(|d| d.get::<uom::si::length::meter>())
            .unwrap_or(f64::INFINITY);
        }

        let t = ((point.x() - seg_start.x()) * dx + (point.y() - seg_start.y()) * dy)
            / (dx * dx + dy * dy);
        let t = t.clamp(0.0, 1.0);
        let closest_x = seg_start.x() + t * dx;
        let closest_y = seg_start.y() + t * dy;

        haversine::haversine_distance(point.x(), point.y(), closest_x, closest_y)
            .map(|d| d.get::<uom::si::length::meter>())
            .unwrap_or(f64::INFINITY)
    }

    fn run_shortest_path(
        &self,
        start: (EdgeListId, EdgeId),
        end: (EdgeListId, EdgeId),
        si: &SearchInstance,
    ) -> Vec<(EdgeListId, EdgeId)> {
        if start == end {
            return vec![start];
        }

        match run_edge_oriented(start, Some(end), &Direction::Forward, true, si) {
            Ok(search_result) => {
                if let Ok(path) = search_result
                    .tree
                    .backtrack_edge_oriented_route(end, si.graph.clone())
                {
                    let mut result = Vec::with_capacity(path.len() + 2);
                    result.push(start);
                    for et in path {
                        result.push((et.edge_list_id, et.edge_id));
                    }
                    result.push(end);
                    result.dedup();
                    result
                } else {
                    vec![start, end]
                }
            }
            Err(_) => vec![start, end],
        }
    }

    fn score_and_match(
        &self,
        segment: &mut TrajectorySegment,
        si: &SearchInstance,
    ) -> Result<(), MapMatchingError> {
        let m = segment.trace.len();
        let n = segment.path.len();

        if m == 0 {
            return Err(MapMatchingError::EmptyTrace);
        }

        if n == 0 {
            segment.score = 0.0;
            segment.matches = segment
                .trace
                .points
                .iter()
                .map(|_| PointMatch::new(EdgeListId(0), EdgeId(0), f64::INFINITY))
                .collect();
            return Ok(());
        }

        // Precompute distances
        let mut distances = vec![vec![0.0; m]; n];
        for (j, path_edge) in segment.path.iter().enumerate() {
            for (i, trace_point) in segment.trace.points.iter().enumerate() {
                distances[j][i] = self.compute_distance_to_edge(
                    &trace_point.coord,
                    &path_edge.0,
                    &path_edge.1,
                    si,
                );
            }
        }

        let mut c = vec![vec![0.0; n + 1]; m + 1];
        let mut point_matches = Vec::with_capacity(m);

        for i in 1..=m {
            let mut min_dist = f64::INFINITY;
            let mut nearest_edge = segment.path[0];

            for j in 1..=n {
                let dt = distances[j - 1][i - 1];
                if dt < min_dist {
                    min_dist = dt;
                    nearest_edge = segment.path[j - 1];
                }

                let point_similarity = if dt < self.distance_epsilon {
                    1.0 - (dt / self.distance_epsilon)
                } else {
                    0.0
                };

                c[i][j] = f64::max(
                    c[i - 1][j - 1] + point_similarity,
                    f64::max(c[i][j - 1], c[i - 1][j]),
                );
            }

            if min_dist > self.distance_threshold {
                min_dist = f64::INFINITY;
            }

            point_matches.push(PointMatch::new(nearest_edge.0, nearest_edge.1, min_dist));
        }

        segment.score = c[m][n] / (m.min(n) as f64);
        segment.matches = point_matches;

        Ok(())
    }

    fn compute_cutting_points(&self, segment: &mut TrajectorySegment) {
        let mut cutting_points = Vec::new();

        let no_match = segment
            .matches
            .iter()
            .all(|m| m.distance_to_edge.is_infinite());

        if segment.path.is_empty() || no_match {
            // Pick the middle point
            cutting_points.push(segment.trace.len() / 2);
        } else {
            // Find furthest point
            if let Some((idx, _)) = segment
                .matches
                .iter()
                .enumerate()
                .filter(|(_, m)| !m.distance_to_edge.is_infinite())
                .max_by(|(_, a), (_, b)| {
                    a.distance_to_edge
                        .partial_cmp(&b.distance_to_edge)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                cutting_points.push(idx);
            }

            // Collect points close to epsilon
            for (i, m) in segment.matches.iter().enumerate() {
                if !m.distance_to_edge.is_infinite() {
                    if (m.distance_to_edge - self.distance_epsilon).abs() < self.cutting_threshold {
                        cutting_points.push(i);
                    }
                }
            }
        }

        // Add random cuts (omitted for now to keep it deterministic and simpler)

        // Filter out start/end
        let n = segment.trace.len();
        segment.cutting_points = cutting_points
            .into_iter()
            .unique()
            .filter(|&idx| idx > 1 && idx < n - 2)
            .collect();
        segment.cutting_points.sort();
    }

    fn split_segment(
        &self,
        segment: &TrajectorySegment,
        si: &SearchInstance,
    ) -> Vec<TrajectorySegment> {
        if segment.trace.len() < 2 || segment.cutting_points.is_empty() {
            return vec![segment.clone()];
        }

        let mut result = Vec::new();
        let mut last_idx = 0;

        for &cp in &segment.cutting_points {
            let sub_points = segment.trace.points[last_idx..cp].to_vec();
            if sub_points.len() >= 1 {
                let sub_trace = MapMatchingTrace::new(sub_points);
                let path = self.new_path_for_trace(&sub_trace, si);
                result.push(TrajectorySegment {
                    trace: sub_trace,
                    path,
                    matches: Vec::new(),
                    score: 0.0,
                    cutting_points: Vec::new(),
                });
            }
            last_idx = cp;
        }

        let sub_points = segment.trace.points[last_idx..].to_vec();
        if sub_points.len() >= 1 {
            let sub_trace = MapMatchingTrace::new(sub_points);
            let path = self.new_path_for_trace(&sub_trace, si);
            result.push(TrajectorySegment {
                trace: sub_trace,
                path,
                matches: Vec::new(),
                score: 0.0,
                cutting_points: Vec::new(),
            });
        }

        result
    }

    fn new_path_for_trace(
        &self,
        trace: &MapMatchingTrace,
        si: &SearchInstance,
    ) -> Vec<(EdgeListId, EdgeId)> {
        if trace.len() < 1 {
            return Vec::new();
        }

        let start_res = self.find_nearest_edge(&trace.points[0].coord, si);
        let end_res = self.find_nearest_edge(&trace.points[trace.len() - 1].coord, si);

        match (start_res, end_res) {
            (Ok(start), Ok(end)) => self.run_shortest_path((start.0, start.1), (end.0, end.1), si),
            _ => Vec::new(),
        }
    }

    fn join_segments(
        &self,
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
                    let prev_end = prev_path[prev_path.len() - 1];
                    let curr_start = curr_path[0];

                    if prev_end != curr_start {
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
                            let gap_path = self.run_shortest_path(prev_end, curr_start, si);
                            // prepend/append already includes start/end edges
                            if gap_path.len() > 2 {
                                total_path.extend(gap_path[1..gap_path.len() - 1].iter().cloned());
                            }
                        }
                    }
                }
            }
            total_path.extend(segments[i].path.clone());
        }

        // De-duplicate consecutive edges in path
        total_path.dedup();

        let mut joined = TrajectorySegment {
            trace: MapMatchingTrace::new(total_points),
            path: total_path,
            matches: Vec::new(),
            score: 0.0,
            cutting_points: Vec::new(),
        };

        self.score_and_match(&mut joined, si)?;
        Ok(joined)
    }

    fn find_stationary_points(&self, trace: &MapMatchingTrace) -> Vec<StationaryIndex> {
        let mut collections = Vec::new();
        let mut current_index = Vec::new();

        for i in 1..trace.len() {
            let p1 = &trace.points[i - 1];
            let p2 = &trace.points[i];
            if let Ok(dist) = haversine::haversine_distance(
                p1.coord.x(),
                p1.coord.y(),
                p2.coord.x(),
                p2.coord.y(),
            ) {
                if dist.get::<uom::si::length::meter>() < 0.001 {
                    if current_index.is_empty() {
                        current_index.push(i - 1);
                    }
                    current_index.push(i);
                } else if !current_index.is_empty() {
                    collections.push(StationaryIndex {
                        i_index: current_index.clone(),
                    });
                    current_index.clear();
                }
            }
        }

        if !current_index.is_empty() {
            collections.push(StationaryIndex {
                i_index: current_index,
            });
        }

        collections
    }

    fn add_matches_for_stationary_points(
        &self,
        matches: Vec<PointMatch>,
        stationary_indices: Vec<StationaryIndex>,
    ) -> Vec<PointMatch> {
        // Handle in reverse order to keep indices stable
        let mut stationary_indices = stationary_indices;
        stationary_indices.sort_by_key(|si| si.i_index[0]);

        let mut final_matches: Vec<PointMatch> = Vec::new();
        let mut sub_trace_idx = 0;
        let mut skip_indices = std::collections::HashSet::new();
        for si in &stationary_indices {
            for &idx in &si.i_index[1..] {
                skip_indices.insert(idx);
            }
        }

        for i in 0.. {
            if sub_trace_idx >= matches.len() {
                break;
            }

            if skip_indices.contains(&i) {
                // This was a dropped stationary point, use the match of the previous point
                if let Some(last_match) = final_matches.last() {
                    final_matches.push(last_match.clone());
                }
            } else {
                final_matches.push(matches[sub_trace_idx].clone());
                sub_trace_idx += 1;
            }
        }

        final_matches
    }
}

impl MapMatchingAlgorithm for LcssMapMatching {
    fn match_trace(
        &self,
        trace: &MapMatchingTrace,
        si: &SearchInstance,
    ) -> Result<MapMatchingResult, MapMatchingError> {
        if trace.is_empty() {
            return Err(MapMatchingError::EmptyTrace);
        }

        // LCSS map matching requires an edge-oriented spatial index
        if !si.map_model.spatial_index.is_edge_oriented() {
            return Err(MapMatchingError::InternalError(
                "LCSS map matching requires an edge-oriented spatial index.".to_string(),
            ));
        }

        // 1. Identify and handle stationary points
        let stationary_indices = self.find_stationary_points(trace);
        let skip_indices: std::collections::HashSet<_> = stationary_indices
            .iter()
            .flat_map(|si| si.i_index[1..].iter().cloned())
            .collect();

        let sub_trace_points: Vec<_> = trace
            .points
            .iter()
            .enumerate()
            .filter(|(i, _)| !skip_indices.contains(i))
            .map(|(_, p)| p.clone())
            .collect();
        let sub_trace = MapMatchingTrace::new(sub_trace_points);

        let initial_path = self.new_path_for_trace(&sub_trace, si);
        let mut initial_segment = TrajectorySegment {
            trace: sub_trace.clone(),
            path: initial_path,
            matches: Vec::new(),
            score: 0.0,
            cutting_points: Vec::new(),
        };

        self.score_and_match(&mut initial_segment, si)?;
        self.compute_cutting_points(&mut initial_segment);

        let mut scheme = self.split_segment(&initial_segment, si);

        for _ in 0..10 {
            let mut next_scheme = Vec::new();
            let mut changed = false;

            for mut segment in scheme.clone() {
                self.score_and_match(&mut segment, si)?;
                self.compute_cutting_points(&mut segment);

                if segment.score >= self.similarity_cutoff {
                    next_scheme.push(segment);
                } else {
                    let new_split = self.split_segment(&segment, si);
                    if new_split.len() > 1 {
                        let joined = self.join_segments(new_split.clone(), si)?;
                        if joined.score > segment.score {
                            next_scheme.extend(new_split);
                            changed = true;
                        } else {
                            next_scheme.push(segment);
                        }
                    } else {
                        next_scheme.push(segment);
                    }
                }
            }

            if !changed {
                break;
            }
            scheme = next_scheme;
        }

        let final_segment = self.join_segments(scheme, si)?;

        let final_matches =
            self.add_matches_for_stationary_points(final_segment.matches, stationary_indices);

        Ok(MapMatchingResult::new(final_matches, final_segment.path))
    }

    fn name(&self) -> &str {
        "lcss_map_matching"
    }

    fn search_parameters(&self) -> serde_json::Value {
        self.search_parameters.clone()
    }
}
