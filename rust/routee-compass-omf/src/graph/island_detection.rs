use std::collections::{HashSet, VecDeque};

use geo::{line_string, Haversine, Length, Point};
use indexmap::IndexMap;
use kdam::{tqdm, BarExt};
use routee_compass_core::model::{
    network::{Edge, EdgeId, EdgeList, EdgeListId, Vertex, VertexId},
    unit::DistanceUnit,
};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length as uom_length;

use crate::collection::OvertureMapsCollectionError;

/// Algorithm used to identify disconnected sub-sections of the road network graph
/// (referred to here as 'islands') and remove them.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentsAlgorithmType {
    /// Strongly Connected Components (SCC).
    /// Treats the road network as strictly directed. To avoid being flagged, every road
    /// segment must be part of a cyclic path (a loop). This tends to aggressively
    /// destroy acyclic map structures like long one-way streets, divided highways,
    /// and terminal branches, identifying them as isolated components.
    Scc,
    #[default]
    /// Weakly Connected Components (WCC).
    /// Treats the road network as undirected. Grouping relies purely on physical
    /// intersection, completely ignoring one-way flow. This safely preserves
    /// functional acyclic structures (like divided highways or avenues) while
    /// accurately isolating genuinely physically disconnected road clusters ("islands").
    Wcc,
    /// Iterative Leaf Pruning.
    /// Topologically grooms the network by iteratively snipping off map "traps".
    /// It explicitly targets and deletes dangling dead-end paths (where nodes have
    /// `in_degree == 0` or `out_degree == 0`), progressively pruning branches up to
    /// the point where they join a valid intersection or cycle.
    IterativeLeafPruning,
}

/// runs a connected components algorithm to identify edges
/// that can be removed from the graph.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IslandDetectionAlgorithm {
    /// the minimum distance across the diagonal of the bounding box covering
    /// the island. if the island is bounded by (xmin, ymin) and (xmax, ymax),
    /// the island is accepted if the LINESTRING (xmin, ymin, xmax ymax) is at
    /// least as long as min_distance in distance_unit.
    pub min_distance: f64,
    /// distance unit of the min_distance value.
    pub distance_unit: DistanceUnit,
    /// algorithm to run.
    #[serde(default)]
    pub algorithm_type: ComponentsAlgorithmType,
}

impl IslandDetectionAlgorithm {
    /// run the algorithm, producing the list of edges that are part of
    /// mobility islands which we want to remove from the algorithm.
    pub fn run(
        &self,
        edge_lists: &[&EdgeList],
        vertices: &[Vertex],
    ) -> Result<Vec<(EdgeListId, EdgeId)>, OvertureMapsCollectionError> {
        match self.algorithm_type {
            ComponentsAlgorithmType::Scc => {
                kosaraju_scc(edge_lists, vertices, self.min_distance, self.distance_unit)
            }
            ComponentsAlgorithmType::Wcc => {
                wcc_components(edge_lists, vertices, self.min_distance, self.distance_unit)
            }
            ComponentsAlgorithmType::IterativeLeafPruning => {
                iterative_leaf_pruning(edge_lists, vertices, self.min_distance, self.distance_unit)
            }
        }
    }
}

pub type DenseAdjacencyList = Box<[IndexMap<(EdgeListId, EdgeId), VertexId>]>;

/// compute potential islands in a set of edge lists based on a radius distance
/// extension of each component. Returns the list of edges that need to be removed because they
/// belong to an island
pub fn kosaraju_scc(
    edge_lists: &[&EdgeList],
    vertices: &[Vertex],
    distance_threshold: f64,
    distance_threshold_unit: DistanceUnit,
) -> Result<Vec<(EdgeListId, EdgeId)>, OvertureMapsCollectionError> {
    let forward_adjacency: DenseAdjacencyList = build_adjacency(edge_lists, vertices.len(), true)
        .map_err(|s| {
        OvertureMapsCollectionError::InternalError(format!(
            "failed to compute adjacency matrix for island detection algorithm: {s}"
        ))
    })?;
    let backward_adjacency: DenseAdjacencyList = build_adjacency(edge_lists, vertices.len(), false)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute adjacency matrix for island detection algorithm: {s}"
            ))
        })?;

    // Progress bar
    let total_edges = edge_lists.iter().map(|el| el.len()).sum::<usize>();
    let mut pb = tqdm!(
        total = total_edges,
        desc = "computing components - scanning edges"
    );

    // Main Loop: Kosaraju's Algorithm for SCCs

    // Pass 1: Forward DFS to get post-order finishing times
    let mut visited_forward = HashSet::<(EdgeListId, EdgeId)>::new();
    let mut post_order = Vec::<(EdgeListId, EdgeId)>::new();

    for start_edge in edge_lists.iter().flat_map(|el| el.edges()) {
        let edge_key = (start_edge.edge_list_id, start_edge.edge_id);
        if visited_forward.contains(&edge_key) {
            continue;
        }

        // Iterative DFS
        let mut stack = vec![(edge_key, false)];
        while let Some((curr, is_post)) = stack.pop() {
            if is_post {
                post_order.push(curr);
                continue;
            }

            if visited_forward.contains(&curr) {
                continue;
            }
            visited_forward.insert(curr);
            stack.push((curr, true));

            let curr_edge = edge_lists[curr.0 .0].0[curr.1 .0];
            let outward_edges: Vec<&(EdgeListId, EdgeId)> = forward_adjacency
                [curr_edge.dst_vertex_id.0]
                .keys()
                .collect();

            for &next_edge_key in outward_edges {
                if !visited_forward.contains(&next_edge_key) {
                    stack.push((next_edge_key, false));
                }
            }
        }
    }

    // Pass 2: Backward DFS to find SCCs
    let mut visited_backward = HashSet::<(EdgeListId, EdgeId)>::new();
    let mut flagged = Vec::<(EdgeListId, EdgeId)>::new();

    for &start_edge_key in post_order.iter().rev() {
        if visited_backward.contains(&start_edge_key) {
            continue;
        }

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut scc = Vec::<(EdgeListId, EdgeId)>::new();

        let mut queue = VecDeque::<(EdgeListId, EdgeId)>::new();
        queue.push_back(start_edge_key);
        visited_backward.insert(start_edge_key);

        while let Some(curr) = queue.pop_front() {
            scc.push(curr);
            let current_edge = edge_lists[curr.0 .0].0[curr.1 .0];

            let src_vertex = vertices[current_edge.src_vertex_id.0];
            let dst_vertex = vertices[current_edge.dst_vertex_id.0];
            min_x = min_x.min(src_vertex.x()).min(dst_vertex.x());
            max_x = max_x.max(src_vertex.x()).max(dst_vertex.x());
            min_y = min_y.min(src_vertex.y()).min(dst_vertex.y());
            max_y = max_y.max(src_vertex.y()).max(dst_vertex.y());

            let inward_edges: Vec<&(EdgeListId, EdgeId)> = backward_adjacency
                [current_edge.src_vertex_id.0]
                .keys()
                .collect();

            for &next_edge_key in inward_edges {
                if !visited_backward.contains(&next_edge_key) {
                    visited_backward.insert(next_edge_key);
                    queue.push_back(next_edge_key);
                }
            }

            if let Err(e) = pb.update(1) {
                log::warn!("error during update of progress bar: {e}")
            };
        }

        // At the end, check the bounding box diagonal distance to determine flag
        let component_diagonal_meters =
            Haversine.length(&line_string![(min_x, min_y).into(), (max_x, max_y).into()]);
        let diameter_uom =
            uom_length::new::<uom::si::length::meter>(component_diagonal_meters as f64);

        if diameter_uom < distance_threshold_unit.to_uom(distance_threshold) {
            flagged.append(&mut scc);
        }
    }

    eprintln!();
    Ok(flagged)
}

/// compute potential islands in a set of edge lists based on a Weakly Connected Components (WCC) algorithm.
///
/// Unlike Strongly Connected Components (SCC) which requires a full cycle for nodes to be grouped,
/// WCC treats the graph as undirected (traversing both `forward` and `backward` edges unconditionally).
/// This prevents acyclic components—like major one-way arteries, highway ramps, and long stretches
/// of divided roads—from being classified as isolated "islands" and aggressively deleted.
/// If the resultant weakly connected map segment's bounding-box diagonal falls below the
/// `distance_threshold`, all its edges are returned to be stripped out of the network.
pub fn wcc_components(
    edge_lists: &[&EdgeList],
    vertices: &[Vertex],
    distance_threshold: f64,
    distance_threshold_unit: DistanceUnit,
) -> Result<Vec<(EdgeListId, EdgeId)>, OvertureMapsCollectionError> {
    let forward_adjacency: DenseAdjacencyList = build_adjacency(edge_lists, vertices.len(), true)
        .map_err(|s| {
        OvertureMapsCollectionError::InternalError(format!(
            "failed to compute forward adjacency matrix for wcc: {s}"
        ))
    })?;
    let backward_adjacency: DenseAdjacencyList = build_adjacency(edge_lists, vertices.len(), false)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute backward adjacency matrix for wcc: {s}"
            ))
        })?;

    let total_edges = edge_lists.iter().map(|el| el.len()).sum::<usize>();
    let mut pb = tqdm!(total = total_edges, desc = "computing wcc components");

    let mut visited = HashSet::<(EdgeListId, EdgeId)>::new();
    let mut flagged = Vec::<(EdgeListId, EdgeId)>::new();

    for start_edge in edge_lists.iter().flat_map(|el| el.edges()) {
        let start_edge_key = (start_edge.edge_list_id, start_edge.edge_id);
        if visited.contains(&start_edge_key) {
            continue;
        }

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut component = Vec::<(EdgeListId, EdgeId)>::new();

        let mut queue = VecDeque::<(EdgeListId, EdgeId)>::new();
        queue.push_back(start_edge_key);
        visited.insert(start_edge_key);

        while let Some(curr) = queue.pop_front() {
            component.push(curr);
            let current_edge = edge_lists[curr.0 .0].0[curr.1 .0];

            let src_vertex = vertices[current_edge.src_vertex_id.0];
            let dst_vertex = vertices[current_edge.dst_vertex_id.0];
            min_x = min_x.min(src_vertex.x()).min(dst_vertex.x());
            max_x = max_x.max(src_vertex.x()).max(dst_vertex.x());
            min_y = min_y.min(src_vertex.y()).min(dst_vertex.y());
            max_y = max_y.max(src_vertex.y()).max(dst_vertex.y());

            // To explore the undirected graph, we look at all edges connected to `src` and `dst` vertices.
            // 1) Outbound from `dst`
            for (&next_edge_key, _) in forward_adjacency[current_edge.dst_vertex_id.0].iter() {
                if !visited.contains(&next_edge_key) {
                    visited.insert(next_edge_key);
                    queue.push_back(next_edge_key);
                }
            }
            // 2) Inbound to `dst`
            for (&next_edge_key, _) in backward_adjacency[current_edge.dst_vertex_id.0].iter() {
                if !visited.contains(&next_edge_key) {
                    visited.insert(next_edge_key);
                    queue.push_back(next_edge_key);
                }
            }
            // 3) Outbound from `src`
            for (&next_edge_key, _) in forward_adjacency[current_edge.src_vertex_id.0].iter() {
                if !visited.contains(&next_edge_key) {
                    visited.insert(next_edge_key);
                    queue.push_back(next_edge_key);
                }
            }
            // 4) Inbound to `src`
            for (&next_edge_key, _) in backward_adjacency[current_edge.src_vertex_id.0].iter() {
                if !visited.contains(&next_edge_key) {
                    visited.insert(next_edge_key);
                    queue.push_back(next_edge_key);
                }
            }

            let _ = pb.update(1);
        }

        let component_diagonal_meters =
            Haversine.length(&line_string![(min_x, min_y).into(), (max_x, max_y).into()]);
        let diameter_uom =
            uom_length::new::<uom::si::length::meter>(component_diagonal_meters as f64);

        if diameter_uom < distance_threshold_unit.to_uom(distance_threshold) {
            flagged.append(&mut component);
        }
    }

    eprintln!();
    Ok(flagged)
}

/// iteratively prune terminal edges (leaf nodes where in_degree == 0 or out_degree == 0).
/// Continues removing dead-end edges until the network contains only paths that can be fully traversed.
///
/// This allows cleanly severing "traps" (dangling one-way paths exiting map boundaries or random data artifacts)
/// without blanket-deleting non-cyclic but connected structures. It evaluates the network and repeatedly snipps
/// edges attached to sink vertices (`in_degree > 0 && out_degree == 0`) or source vertices (`in_degree == 0 && out_degree > 0`),
/// progressively pushing the pruning process inwards up the branch until a cycle or intersection is hit.
pub fn iterative_leaf_pruning(
    edge_lists: &[&EdgeList],
    vertices: &[Vertex],
    _distance_threshold: f64,
    _distance_threshold_unit: DistanceUnit,
) -> Result<Vec<(EdgeListId, EdgeId)>, OvertureMapsCollectionError> {
    let forward_adjacency: DenseAdjacencyList = build_adjacency(edge_lists, vertices.len(), true)
        .map_err(|s| {
        OvertureMapsCollectionError::InternalError(format!(
            "failed to compute forward adjacency matrix for leaf pruning: {s}"
        ))
    })?;
    let backward_adjacency: DenseAdjacencyList = build_adjacency(edge_lists, vertices.len(), false)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute backward adjacency matrix for leaf pruning: {s}"
            ))
        })?;

    let total_edges = edge_lists.iter().map(|el| el.len()).sum::<usize>();
    let mut pb = tqdm!(total = total_edges, desc = "computing leaf pruning");

    let mut out_degree = vec![0; vertices.len()];
    let mut in_degree = vec![0; vertices.len()];
    let mut active_edges = HashSet::<(EdgeListId, EdgeId)>::new();

    for edge in edge_lists.iter().flat_map(|el| el.edges()) {
        out_degree[edge.src_vertex_id.0] += 1;
        in_degree[edge.dst_vertex_id.0] += 1;
        active_edges.insert((edge.edge_list_id, edge.edge_id));
    }

    let mut queue = VecDeque::<VertexId>::new();
    for v in 0..vertices.len() {
        if (in_degree[v] > 0 && out_degree[v] == 0) || (in_degree[v] == 0 && out_degree[v] > 0) {
            queue.push_back(VertexId(v));
        }
    }

    let mut flagged = Vec::<(EdgeListId, EdgeId)>::new();

    while let Some(v) = queue.pop_front() {
        if in_degree[v.0] > 0 && out_degree[v.0] == 0 {
            for (&edge_key, _) in backward_adjacency[v.0].iter() {
                if active_edges.remove(&edge_key) {
                    flagged.push(edge_key);
                    let _ = pb.update(1);
                    let e_info = edge_lists[edge_key.0 .0].0[edge_key.1 .0];
                    let src = e_info.src_vertex_id;
                    out_degree[src.0] -= 1;
                    if (in_degree[src.0] > 0 && out_degree[src.0] == 0)
                        || (in_degree[src.0] == 0 && out_degree[src.0] > 0)
                    {
                        queue.push_back(src);
                    }
                    in_degree[v.0] -= 1;
                }
            }
        } else if in_degree[v.0] == 0 && out_degree[v.0] > 0 {
            for (&edge_key, _) in forward_adjacency[v.0].iter() {
                if active_edges.remove(&edge_key) {
                    flagged.push(edge_key);
                    let _ = pb.update(1);
                    let e_info = edge_lists[edge_key.0 .0].0[edge_key.1 .0];
                    let dst = e_info.dst_vertex_id;
                    in_degree[dst.0] -= 1;
                    if (in_degree[dst.0] > 0 && out_degree[dst.0] == 0)
                        || (in_degree[dst.0] == 0 && out_degree[dst.0] > 0)
                    {
                        queue.push_back(dst);
                    }
                    out_degree[v.0] -= 1;
                }
            }
        }
    }

    // Since pruning may end early, update the progress bar to 100% just in case
    // to keep kdam happy.
    let _ = pb.update_to(total_edges);
    eprintln!();

    Ok(flagged)
}

/// visit operation for weakly-connected BFS traversal
fn visit_edge(
    edge: &Edge,
    visited: &mut HashSet<(EdgeListId, EdgeId)>,
    queue: &mut VecDeque<(EdgeListId, EdgeId)>,
    forward_adjacency: &DenseAdjacencyList,
    backward_adjacency: &DenseAdjacencyList,
) {
    let (edge_list_id, edge_id) = (edge.edge_list_id, edge.edge_id);

    // get all neighbors, add them to queue
    // forward_adjacency[dst]: edges leaving dst (v → *)
    let outward_edges: Vec<&(EdgeListId, EdgeId)> =
        forward_adjacency[edge.dst_vertex_id.0].keys().collect();
    for (edge_list_id, edge_id) in outward_edges {
        queue.push_back((*edge_list_id, *edge_id));
    }
    // backward_adjacency[src]: edges entering src (* → u)
    let inward_edges: Vec<&(EdgeListId, EdgeId)> =
        backward_adjacency[edge.src_vertex_id.0].keys().collect();
    for (edge_list_id, edge_id) in inward_edges {
        queue.push_back((*edge_list_id, *edge_id));
    }
    // forward_adjacency[src]: other edges leaving src (u → *) — catches pure source vertices
    let sibling_outward_edges: Vec<&(EdgeListId, EdgeId)> =
        forward_adjacency[edge.src_vertex_id.0].keys().collect();
    for (edge_list_id, edge_id) in sibling_outward_edges {
        queue.push_back((*edge_list_id, *edge_id));
    }
    // backward_adjacency[dst]: other edges entering dst (* → v) — catches pure sink vertices
    let sibling_inward_edges: Vec<&(EdgeListId, EdgeId)> =
        backward_adjacency[edge.dst_vertex_id.0].keys().collect();
    for (edge_list_id, edge_id) in sibling_inward_edges {
        queue.push_back((*edge_list_id, *edge_id));
    }

    // mark as visited
    visited.insert((edge_list_id, edge_id));
}

/// parallelizable implementation. Explores the entire component this
/// edge belongs to up to a given distance threshold and returns whether or
/// not the component is an island
fn is_component_island_parallel(
    edge: &Edge,
    distance_threshold: f64,
    distance_threshold_unit: DistanceUnit,
    edge_lists: &[&EdgeList],
    vertices: &[Vertex],
    forward_adjacency: &DenseAdjacencyList,
    backward_adjacency: &DenseAdjacencyList,
) -> Result<bool, OvertureMapsCollectionError> {
    let mut visited = HashSet::<(EdgeListId, EdgeId)>::new();
    let mut visit_queue: VecDeque<(EdgeListId, EdgeId)> = VecDeque::new();
    visit_queue.push_back((edge.edge_list_id, edge.edge_id));

    let edge_midpoint = compute_midpoint(edge, vertices);
    let mut max_distance_reached = uom_length::new::<uom::si::length::meter>(0 as f64);
    let threshold_uom = distance_threshold_unit.to_uom(distance_threshold);

    while max_distance_reached < threshold_uom {
        if let Some((current_edge_list_id, current_edge_id)) = visit_queue.pop_front() {
            // Skip if we already visited
            if visited
                .get(&(current_edge_list_id, current_edge_id))
                .is_some()
            {
                continue;
            }

            // Retrieve current edge information
            let current_edge = edge_lists.get(current_edge_list_id.0)
                .and_then(|el| el.get(&current_edge_id))
                .ok_or(OvertureMapsCollectionError::InternalError(format!("edge list {current_edge_list_id:?} or edge {current_edge_id:?} not found during island detection starting at edge {edge:?}")))?;

            // Expand queue
            visit_edge(
                current_edge,
                &mut visited,
                &mut visit_queue,
                forward_adjacency,
                backward_adjacency,
            );

            // Update counter
            let current_midpoint = compute_midpoint(current_edge, vertices);
            let current_distance_to_start_meters =
                Haversine.length(&line_string![edge_midpoint.0, current_midpoint.0]);
            let current_distance_uom =
                uom_length::new::<uom::si::length::meter>(current_distance_to_start_meters as f64);
            max_distance_reached = max_distance_reached.max(current_distance_uom);
        } else {
            // Ran out of edges
            return Ok(true);
        }
    }

    // Got enough edges
    Ok(false)
}

// Given an edge, compute the midpoint of the straight line
// between beginning and end vertices
fn compute_midpoint(edge: &Edge, vertices: &[Vertex]) -> Point<f32> {
    let src_vertex = vertices[edge.src_vertex_id.0];
    let dst_vertex = vertices[edge.dst_vertex_id.0];
    Point::new(
        (src_vertex.x() + dst_vertex.x()) / 2.,
        (src_vertex.y() + dst_vertex.y()) / 2.,
    )
}

/// build the outgoing adjacency matrix
fn build_adjacency(
    edge_lists: &[&EdgeList],
    n_vertices: usize,
    forward: bool,
) -> Result<DenseAdjacencyList, String> {
    let total_edges = edge_lists.iter().map(|el| el.len()).sum();

    let build_adjacencies_iter = tqdm!(
        edge_lists.iter().flat_map(|el| el.edges()),
        desc = "building adjacencies",
        total = total_edges
    );

    let mut out_adjacency = vec![IndexMap::<(EdgeListId, EdgeId), VertexId>::new(); n_vertices];
    for edge in build_adjacencies_iter {
        append_to_adjacency(&mut out_adjacency, edge, forward)?;
    }

    Ok(out_adjacency.into_boxed_slice())
}

fn append_to_adjacency(
    adjacency: &mut [IndexMap<(EdgeListId, EdgeId), VertexId>],
    edge: &Edge,
    forward: bool,
) -> Result<(), String> {
    let src_vertex = if forward {
        edge.src_vertex_id
    } else {
        edge.dst_vertex_id
    };

    match adjacency.get_mut(src_vertex.0) {
        None => {
            let direction = if forward { "forward" } else { "reverse" };
            Err(format!(
                "vertex {} not found in {} adjacencies for edge list, edge: {}, {}",
                src_vertex.0, direction, edge.edge_list_id.0, edge.edge_id.0
            ))
        }
        Some(out_links) => {
            let target_vertex = if forward {
                edge.dst_vertex_id
            } else {
                edge.src_vertex_id
            };
            out_links.insert((edge.edge_list_id, edge.edge_id), target_vertex);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::SQRT_2;

    use super::*;
    use routee_compass_core::model::network::{Edge, EdgeId, EdgeList, Vertex};
    use uom::si::f64::Length;

    /// Creates dummy vertices and edges for testing compute_midpoint
    fn create_test_data() -> (Vec<Vertex>, Vec<EdgeList>) {
        // Create vertices at specific coordinates for testing
        let vertices = vec![
            Vertex::new(0, 0.0, 0.0),
            Vertex::new(1, 1.0, 1.0),
            Vertex::new(2, 2.0, 0.0),
            Vertex::new(3, 0.0, 2.0),
        ];

        // Create edges connecting these vertices
        let edges = vec![
            Edge::new(
                0,
                0,
                0,
                1,
                Length::new::<uom::si::length::meter>(SQRT_2 as f64),
            ),
            Edge::new(0, 1, 0, 2, Length::new::<uom::si::length::meter>(2.)),
            Edge::new(0, 2, 0, 3, Length::new::<uom::si::length::meter>(2.)),
        ];
        let edge_list = EdgeList(edges.into_boxed_slice());
        let edge_lists = vec![edge_list];

        (vertices, edge_lists)
    }

    /// Creates test data for island detection testing
    /// Returns vertices, edge_lists, and adjacency matrix
    fn create_island_test_data() -> (
        Vec<Vertex>,
        Vec<EdgeList>,
        DenseAdjacencyList,
        DenseAdjacencyList,
    ) {
        // Create vertices forming two separate components with realistic lat/lon coordinates
        // Using Denver, CO area as reference (39.7392° N, 104.9903° W)
        // At this latitude, 1 degree longitude ≈ 87.7 km, 1 degree latitude ≈ 111 km

        let base_lat = 39.7392;
        let base_lon = -104.9903;

        // Small offsets for island component (within ~100 meters total extent)
        // 0.001 degrees ≈ 111 meters latitude, 87.7 meters longitude at Denver
        let small_offset_lat = 0.0005; // ~55 meters
        let small_offset_lon = 0.0006; // ~53 meters

        // Large offsets for non-island component (several kilometers)
        let large_offset_lon = 0.06; // ~5.3 km

        let vertices = vec![
            // Island component - small square (all within ~100m of each other)
            Vertex::new(0, base_lon, base_lat), // Base point
            Vertex::new(1, base_lon + small_offset_lon, base_lat), // East ~53m
            Vertex::new(2, base_lon + small_offset_lon, base_lat + small_offset_lat), // NE ~75m
            Vertex::new(3, base_lon, base_lat + small_offset_lat), // North ~55m
            // Non-island component - extends over large distances (kilometers apart)
            Vertex::new(4, base_lon + 0.1, base_lat + 0.1), // ~12km away
            Vertex::new(5, base_lon + 0.1 + large_offset_lon, base_lat + 0.1), // Another ~5km east
            Vertex::new(6, base_lon + 0.1 + 2.0 * large_offset_lon, base_lat + 0.1), // Another ~5km east
            Vertex::new(7, base_lon + 0.1 + 3.0 * large_offset_lon, base_lat + 0.1), // Another ~5km east
        ];

        // Create edges for both components
        let edges = vec![
            // Island component: square loop (0->1->2->3->0) - all edges ~50-75m long
            Edge::new(0, 0, 0, 1, Length::new::<uom::si::length::meter>(53.0)), // East edge ~53m
            Edge::new(0, 1, 1, 2, Length::new::<uom::si::length::meter>(55.0)), // North edge ~55m
            Edge::new(0, 2, 2, 3, Length::new::<uom::si::length::meter>(53.0)), // West edge ~53m
            Edge::new(0, 3, 3, 0, Length::new::<uom::si::length::meter>(55.0)), // South edge ~55m
            // Non-island component: linear chain - each edge ~5km+ long
            Edge::new(0, 4, 4, 5, Length::new::<uom::si::length::meter>(5300.0)), // ~5.3km
            Edge::new(0, 5, 5, 6, Length::new::<uom::si::length::meter>(5300.0)), // Another ~5.3km
            Edge::new(0, 6, 6, 7, Length::new::<uom::si::length::meter>(5300.0)), // Another ~5.3km
        ];

        let edge_list = EdgeList(edges.into_boxed_slice());
        let edge_lists = vec![edge_list];

        // Build adjacency matrix for traversal
        let forward_adjacency = build_adjacency(
            &edge_lists.iter().collect::<Vec<&EdgeList>>(),
            vertices.len(),
            true,
        )
        .unwrap();
        let backward_adjacency = build_adjacency(
            &edge_lists.iter().collect::<Vec<&EdgeList>>(),
            vertices.len(),
            false,
        )
        .unwrap();

        (vertices, edge_lists, forward_adjacency, backward_adjacency)
    }

    #[test]
    fn test_compute_midpoint_simple() {
        let (vertices, edge_lists) = create_test_data();

        // Test the edge from (0,0) to (1,1) - should have midpoint (0.5, 0.5)
        let edge = edge_lists[0].get(&EdgeId(0)).unwrap();
        let midpoint = compute_midpoint(edge, &vertices);

        assert!((midpoint.x() - 0.5).abs() < f32::EPSILON);
        assert!((midpoint.y() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_visit_edge_parallel_island_component() {
        let (vertices, edge_lists, forward_adjacency, backward_adjacency) =
            create_island_test_data();

        // Test an edge from the island component (small square)
        // Starting edge: 0->1 (from base point to ~53m east)
        let island_edge = edge_lists[0].get(&EdgeId(0)).unwrap();

        // This should return true (is an island) because all connected edges
        // are within the small square, well under 100 meters from the starting edge midpoint
        // Note: The threshold in visit_edge_parallel is 10 meters, and our small square
        // has edges that are all very close to each other (within ~75m total)
        let result = is_component_island_parallel(
            island_edge,
            100.,
            DistanceUnit::Meters,
            &edge_lists.iter().collect::<Vec<&EdgeList>>(),
            &vertices,
            &forward_adjacency,
            &backward_adjacency,
        )
        .unwrap();
        assert!(
            result,
            "Small square component should be detected as an island"
        );
    }

    #[test]
    fn test_visit_edge_parallel_non_island_component() {
        let (vertices, edge_lists, forward_adjacency, backward_adjacency) =
            create_island_test_data();

        // Test an edge from the large component
        // Starting edge: 4->5 (first edge of the long linear chain)
        let non_island_edge = edge_lists[0].get(&EdgeId(4)).unwrap();

        // This should return false (not an island) because the traversal will reach
        // edges that are more than 10 meters away from the starting edge midpoint
        // (the linear chain extends over many kilometers)
        let result = is_component_island_parallel(
            non_island_edge,
            100.,
            DistanceUnit::Meters,
            &edge_lists.iter().collect::<Vec<&EdgeList>>(),
            &vertices,
            &forward_adjacency,
            &backward_adjacency,
        )
        .unwrap();
        assert!(
            !result,
            "Large linear component should not be detected as an island"
        );
    }

    #[test]
    fn test_compute_midpoint_various_edges() {
        let (vertices, edge_lists, _, _) = create_island_test_data();

        // Test midpoint of edge 0->1: base_lon to base_lon + small_offset_lon
        let edge = edge_lists[0].get(&EdgeId(0)).unwrap();
        let midpoint = compute_midpoint(edge, &vertices);
        let expected_x = -104.9903 + 0.0006 / 2.0; // base_lon + half the longitude offset
        let expected_y = 39.7392; // same latitude
        assert!((midpoint.x() - expected_x).abs() < f32::EPSILON);
        assert!((midpoint.y() - expected_y).abs() < f32::EPSILON);

        // Test midpoint of another edge from the large component
        let edge = edge_lists[0].get(&EdgeId(4)).unwrap();
        let midpoint = compute_midpoint(edge, &vertices);
        // This edge goes from (base_lon + 0.1, base_lat + 0.1) to (base_lon + 0.1 + 0.06, base_lat + 0.1)
        let expected_x = -104.9903 + 0.1 + 0.06 / 2.0;
        let expected_y = 39.7392 + 0.1;
        assert!((midpoint.x() - expected_x).abs() < f32::EPSILON);
        assert!((midpoint.y() - expected_y).abs() < f32::EPSILON);
    }

    #[test]
    fn test_wcc_vs_kosaraju_long_one_way_arterial() {
        // A single long one-way street A -> B -> C -> D
        let base_lat = 39.7392;
        let base_lon = -104.9903;
        let offset = 0.001; // ~87m longitude

        let vertices = vec![
            Vertex::new(0, base_lon, base_lat),
            Vertex::new(1, base_lon + offset, base_lat), // 87m east
            Vertex::new(2, base_lon + 2.0 * offset, base_lat), // 174m east
            Vertex::new(3, base_lon + 3.0 * offset, base_lat), // 261m east
        ];

        let edges = vec![
            Edge::new(0, 0, 0, 1, Length::new::<uom::si::length::meter>(87.0)),
            Edge::new(0, 1, 1, 2, Length::new::<uom::si::length::meter>(87.0)),
            Edge::new(0, 2, 2, 3, Length::new::<uom::si::length::meter>(87.0)),
        ];
        let edge_list = EdgeList(edges.into_boxed_slice());
        let edge_lists = vec![&edge_list];

        // threshold 100 meters
        let distance_threshold = 100.0;

        let flagged_wcc = wcc_components(
            &edge_lists,
            &vertices,
            distance_threshold,
            DistanceUnit::Meters,
        )
        .unwrap();
        // WCC sees (261m > 100m) and keeps it (returns empty flagged)
        assert!(flagged_wcc.is_empty());

        let flagged_scc = kosaraju_scc(
            &edge_lists,
            &vertices,
            distance_threshold,
            DistanceUnit::Meters,
        )
        .unwrap();
        // SCC sees 3 independent 87m edges (< 100m) and flags all of them!
        assert_eq!(flagged_scc.len(), 3);
    }

    #[test]
    fn test_wcc_tiny_acyclic_island() {
        // A single short one-way street A -> B
        let base_lat = 39.7392;
        let base_lon = -104.9903;
        let offset = 0.0005; // ~43m longitude

        let vertices = vec![
            Vertex::new(0, base_lon, base_lat),
            Vertex::new(1, base_lon + offset, base_lat), // 43m east
        ];

        let edges = vec![Edge::new(
            0,
            0,
            0,
            1,
            Length::new::<uom::si::length::meter>(43.0),
        )];
        let edge_list = EdgeList(edges.into_boxed_slice());
        let edge_lists = vec![&edge_list];

        // threshold 100 meters
        let distance_threshold = 100.0;

        let flagged_wcc = wcc_components(
            &edge_lists,
            &vertices,
            distance_threshold,
            DistanceUnit::Meters,
        )
        .unwrap();
        // WCC sees (43m < 100m) and flags it
        assert_eq!(flagged_wcc.len(), 1);
    }

    #[test]
    fn test_iterative_leaf_pruning_lollipop() {
        // Roundabout A -> B -> C -> A, plus dangle C -> D -> E
        let vertices = vec![
            Vertex::new(0, 0.0, 0.0), // A
            Vertex::new(1, 0.0, 0.0), // B
            Vertex::new(2, 0.0, 0.0), // C
            Vertex::new(3, 0.0, 0.0), // D
            Vertex::new(4, 0.0, 0.0), // E
        ];

        let edges = vec![
            // Roundabout A -> B, B -> C, C -> A
            Edge::new(0, 0, 0, 1, Length::new::<uom::si::length::meter>(10.0)),
            Edge::new(0, 1, 1, 2, Length::new::<uom::si::length::meter>(10.0)),
            Edge::new(0, 2, 2, 0, Length::new::<uom::si::length::meter>(10.0)),
            // Dangle C -> D, D -> E
            Edge::new(0, 3, 2, 3, Length::new::<uom::si::length::meter>(10.0)),
            Edge::new(0, 4, 3, 4, Length::new::<uom::si::length::meter>(10.0)),
        ];
        let edge_list = EdgeList(edges.into_boxed_slice());
        let edge_lists = vec![&edge_list];

        // Iterative Leaf Pruning
        let flagged =
            iterative_leaf_pruning(&edge_lists, &vertices, 100.0, DistanceUnit::Meters).unwrap();

        // Should only prune dangle (edges 3 and 4)
        assert_eq!(flagged.len(), 2);

        let flagged_edges: std::collections::HashSet<_> =
            flagged.into_iter().map(|e| e.1 .0).collect();
        assert!(flagged_edges.contains(&3));
        assert!(flagged_edges.contains(&4));
    }
}
