use crate::algorithm::map_matching::map_matching_error::MapMatchingError;
use crate::algorithm::map_matching::map_matching_result::PointMatch;
use crate::algorithm::map_matching::map_matching_trace::MapMatchingTrace;
use crate::algorithm::search::a_star::run_vertex_oriented;
use crate::algorithm::search::{Direction, SearchError, SearchInstance};
use crate::model::map::NearestSearchResult;
use crate::model::network::{EdgeId, EdgeListId, VertexId};
use crate::util::geo::haversine;
use geo::ClosestPoint;
use uom::si::f64::Length;
use uom::si::length::meter;

/// A struct representing a collection of indices where the trace points are stationary.
#[derive(Debug, Clone)]
pub(crate) struct StationaryIndex {
    pub(crate) i_index: Vec<usize>,
}

/// Computes the distance from a point to an edge.
///
/// # Arguments
/// * `point` - The point to compute the distance from.
/// * `edge_list_id` - The edge list ID of the edge.
/// * `edge_id` - The edge ID of the edge.
/// * `si` - The search instance containing the map model.
///
/// # Returns
/// The distance from the point to the edge, or infinity if not found.
pub(crate) fn compute_distance_to_edge(
    point: &geo::Point<f32>,
    edge_list_id: &EdgeListId,
    edge_id: &EdgeId,
    si: &SearchInstance,
) -> Length {
    if let Ok(linestring) = si.map_model.get_linestring(edge_list_id, edge_id) {
        match linestring.closest_point(point) {
            geo::Closest::SinglePoint(p) | geo::Closest::Intersection(p) => {
                haversine::haversine_distance(point.x(), point.y(), p.x(), p.y())
                    .unwrap_or_else(|_| Length::new::<meter>(f64::INFINITY))
            }
            geo::Closest::Indeterminate => Length::new::<meter>(f64::INFINITY),
        }
    } else {
        Length::new::<meter>(f64::INFINITY)
    }
}

/// Finds the closest vertex (source or destination) of an edge to a given point.
///
/// # Arguments
/// * `point` - The point to find the closest vertex to.
/// * `edge_list_id` - The edge list ID of the edge.
/// * `edge_id` - The edge ID of the edge.
/// * `si` - The search instance containing the graph.
///
/// # Returns
/// A result containing the closest vertex ID and its distance, or a map matching error.
pub(crate) fn get_closest_vertex(
    point: &geo::Point<f32>,
    edge_list_id: &EdgeListId,
    edge_id: &EdgeId,
    si: &SearchInstance,
) -> Result<(VertexId, Length), MapMatchingError> {
    let src_id = si.graph.src_vertex_id(edge_list_id, edge_id).map_err(|_| {
        MapMatchingError::InternalError(format!(
            "Failed to get source vertex id for edge {} from edge list {}",
            edge_id, edge_list_id
        ))
    })?;
    let dst_id = si.graph.dst_vertex_id(edge_list_id, edge_id).map_err(|_| {
        MapMatchingError::InternalError(format!(
            "Failed to get destination vertex id for edge {} from edge list {}",
            edge_id, edge_list_id
        ))
    })?;

    let src_vertex = si.graph.get_vertex(&src_id).map_err(|_| {
        MapMatchingError::InternalError(format!(
            "Failed to get source vertex {} for edge {} from edge list {}",
            src_id, edge_id, edge_list_id
        ))
    })?;
    let dst_vertex = si.graph.get_vertex(&dst_id).map_err(|_| {
        MapMatchingError::InternalError(format!(
            "Failed to get destination vertex {} for edge {} from edge list {}",
            dst_id, edge_id, edge_list_id
        ))
    })?;

    let src_dist =
        haversine::haversine_distance(point.x(), point.y(), src_vertex.x(), src_vertex.y())
            .unwrap_or_else(|_| Length::new::<meter>(f64::INFINITY));

    let dst_dist =
        haversine::haversine_distance(point.x(), point.y(), dst_vertex.x(), dst_vertex.y())
            .unwrap_or_else(|_| Length::new::<meter>(f64::INFINITY));

    if src_dist <= dst_dist {
        Ok((src_id, src_dist))
    } else {
        Ok((dst_id, dst_dist))
    }
}

/// Runs a vertex-oriented shortest path search between two vertices.
///
/// # Arguments
/// * `start` - The starting vertex ID.
/// * `end` - The ending vertex ID.
/// * `si` - The search instance to run the search on.
///
/// # Returns
/// A result containing the path as a vector of (EdgeListId, EdgeId) pairs, or a map matching error.
pub(crate) fn run_shortest_path(
    start: VertexId,
    end: VertexId,
    si: &SearchInstance,
) -> Result<Vec<(EdgeListId, EdgeId)>, MapMatchingError> {
    match run_vertex_oriented(start, Some(end), &Direction::Forward, true, si) {
        Ok(search_result) => match search_result.tree.backtrack(end) {
            Ok(path) => {
                let edge_ids = path
                    .into_iter()
                    .map(|et| (et.edge_list_id, et.edge_id))
                    .collect();
                Ok(edge_ids)
            }
            Err(e) => Err(MapMatchingError::SearchTreeError(e)),
        },
        Err(SearchError::NoPathExistsBetweenVertices(_, _, _)) => Ok(Vec::new()),
        Err(e) => Err(MapMatchingError::SearchError(e)),
    }
}

/// # Returns
/// A result containing a vector of candidates (EdgeListId, EdgeId, distance), or a map matching error.
pub(crate) fn find_candidates(
    point: &geo::Point<f32>,
    si: &SearchInstance,
    k: usize,
) -> Result<Vec<(EdgeListId, EdgeId, Length)>, MapMatchingError> {
    let nearest_iter = si
        .map_model
        .spatial_index
        .nearest_graph_id_iter(point)
        .take(k);

    let mut candidates = Vec::new();
    for result in nearest_iter {
        match result {
            NearestSearchResult::NearestEdge(list_id, eid) => {
                let distance = compute_distance_to_edge(point, &list_id, &eid, si);
                candidates.push((list_id, eid, distance));
            }
            NearestSearchResult::NearestVertex(_) => continue,
        }
    }

    if candidates.is_empty() {
        let nearest = si
            .map_model
            .spatial_index
            .nearest_graph_id(point)
            .map_err(|e| {
                MapMatchingError::InternalError(format!("spatial index query failed: {}", e))
            })?;

        match nearest {
            NearestSearchResult::NearestEdge(list_id, eid) => {
                let distance = compute_distance_to_edge(point, &list_id, &eid, si);
                candidates.push((list_id, eid, distance));
            }
            NearestSearchResult::NearestVertex(_) => {
                return Err(MapMatchingError::InternalError(
                    "vertex-oriented spatial index not supported for LCSS map matching".to_string(),
                ));
            }
        }
    }

    // Sort by distance
    candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    Ok(candidates)
}

/// Creates a new path for a trace by connecting the closest vertices of the start and end candidates.
///
/// # Arguments
/// * `trace` - The trace to create a path for.
/// * `si` - The search instance to use for finding candidates and running shortest path.
///
/// # Returns
/// A result containing the path as a vector of (EdgeListId, EdgeId) pairs, or a map matching error.
pub(crate) fn new_path_for_trace(
    trace: &MapMatchingTrace,
    si: &SearchInstance,
) -> Result<Vec<(EdgeListId, EdgeId)>, MapMatchingError> {
    let start_candidate = find_candidates(&trace.points[0].coord, si, 10)
        .ok()
        .and_then(|c| c.first().cloned());
    let end_candidate = find_candidates(&trace.points[trace.len() - 1].coord, si, 10)
        .ok()
        .and_then(|c| c.first().cloned());

    if let (Some(start), Some(end)) = (start_candidate, end_candidate) {
        let start_v = get_closest_vertex(&trace.points[0].coord, &start.0, &start.1, si)
            .map(|(v, _)| v)
            .unwrap_or_else(|_| VertexId(0));
        let end_v = get_closest_vertex(&trace.points[trace.len() - 1].coord, &end.0, &end.1, si)
            .map(|(v, _)| v)
            .unwrap_or_else(|_| VertexId(0));

        run_shortest_path(start_v, end_v, si)
    } else {
        Ok(Vec::new())
    }
}

/// Identifies stationary points in a trace (points that are very close to each other).
///
/// # Arguments
/// * `trace` - The trace to find stationary points in.
///
/// # Returns
/// A vector of `StationaryIndex` objects representing stationary collections.
pub(crate) fn find_stationary_points(trace: &MapMatchingTrace) -> Vec<StationaryIndex> {
    let mut collections = Vec::new();
    let mut current_index = Vec::new();

    for i in 1..trace.len() {
        let p1 = &trace.points[i - 1];
        let p2 = &trace.points[i];
        if let Ok(dist) =
            haversine::haversine_distance(p1.coord.x(), p1.coord.y(), p2.coord.x(), p2.coord.y())
        {
            if dist < Length::new::<meter>(0.001) {
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

/// Adds matches back for stationary points that were removed during processing.
///
/// # Arguments
/// * `matches` - The matches computed for the reduced trace.
/// * `stationary_indices` - The stationary indices used to reduce the trace.
///
/// # Returns
/// A vector of `PointMatch` objects matching the original trace length.
pub(crate) fn add_matches_for_stationary_points(
    matches: Vec<PointMatch>,
    stationary_indices: Vec<StationaryIndex>,
) -> Vec<PointMatch> {
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

    let original_trace_len = matches.len() + skip_indices.len();

    for i in 0..original_trace_len {
        if skip_indices.contains(&i) {
            if let Some(last_match) = final_matches.last() {
                final_matches.push(last_match.clone());
            }
        } else if sub_trace_idx < matches.len() {
            final_matches.push(matches[sub_trace_idx].clone());
            sub_trace_idx += 1;
        }
    }

    final_matches
}
