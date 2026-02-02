use crate::algorithm::map_matching::map_matching_error::MapMatchingError;
use crate::algorithm::map_matching::map_matching_result::PointMatch;
use crate::algorithm::map_matching::map_matching_trace::MapMatchingTrace;
use crate::algorithm::search::a_star::run_vertex_oriented;
use crate::algorithm::search::{Direction, SearchError, SearchInstance};
use crate::model::map::NearestSearchResult;
use crate::model::network::{EdgeId, EdgeListId, VertexId};
use crate::util::geo::haversine;
use geo::ClosestPoint;

#[derive(Debug, Clone)]
pub(crate) struct StationaryIndex {
    pub(crate) i_index: Vec<usize>,
}

pub(crate) fn compute_distance_to_edge(
    point: &geo::Point<f32>,
    edge_list_id: &EdgeListId,
    edge_id: &EdgeId,
    si: &SearchInstance,
) -> f64 {
    if let Ok(linestring) = si.map_model.get_linestring(edge_list_id, edge_id) {
        match linestring.closest_point(point) {
            geo::Closest::SinglePoint(p) | geo::Closest::Intersection(p) => {
                haversine::haversine_distance(point.x(), point.y(), p.x(), p.y())
                    .map(|d| d.get::<uom::si::length::meter>())
                    .unwrap_or(f64::INFINITY)
            }
            geo::Closest::Indeterminate => f64::INFINITY,
        }
    } else {
        f64::INFINITY
    }
}

pub(crate) fn get_closest_vertex(
    point: &geo::Point<f32>,
    edge_list_id: &EdgeListId,
    edge_id: &EdgeId,
    si: &SearchInstance,
) -> Result<(VertexId, f64), MapMatchingError> {
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
            .map(|d| d.get::<uom::si::length::meter>())
            .unwrap_or(f64::INFINITY);

    let dst_dist =
        haversine::haversine_distance(point.x(), point.y(), dst_vertex.x(), dst_vertex.y())
            .map(|d| d.get::<uom::si::length::meter>())
            .unwrap_or(f64::INFINITY);

    if src_dist <= dst_dist {
        Ok((src_id, src_dist))
    } else {
        Ok((dst_id, dst_dist))
    }
}

pub(crate) fn run_shortest_path(
    start: VertexId,
    end: VertexId,
    si: &SearchInstance,
) -> Result<Vec<(EdgeListId, EdgeId)>, MapMatchingError> {
    match run_vertex_oriented(start, Some(end), &Direction::Forward, true, si) {
        Ok(search_result) => match search_result.tree.backtrack(end) {
            Ok(path) => Ok(path
                .iter()
                .map(|et| (et.edge_list_id, et.edge_id))
                .collect()),
            Err(e) => Err(MapMatchingError::SearchTreeError(e)),
        },
        Err(SearchError::NoPathExistsBetweenVertices(_, _, _)) => Ok(Vec::new()),
        Err(e) => Err(MapMatchingError::SearchError(e)),
    }
}

pub(crate) fn find_candidates(
    point: &geo::Point<f32>,
    si: &SearchInstance,
    k: usize,
) -> Result<Vec<(EdgeListId, EdgeId, f64)>, MapMatchingError> {
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

pub(crate) fn find_stationary_points(trace: &MapMatchingTrace) -> Vec<StationaryIndex> {
    let mut collections = Vec::new();
    let mut current_index = Vec::new();

    for i in 1..trace.len() {
        let p1 = &trace.points[i - 1];
        let p2 = &trace.points[i];
        if let Ok(dist) =
            haversine::haversine_distance(p1.coord.x(), p1.coord.y(), p2.coord.x(), p2.coord.y())
        {
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
