use std::collections::{HashSet, VecDeque};

use geo::{line_string, Haversine, Length};
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
        let mut result = vec![];

        for edge_list in edge_lists.iter() {
            match self.algorithm_type {
                ComponentsAlgorithmType::Scc => {
                    let remove_list =
                        kosaraju_scc(edge_list, vertices, self.min_distance, self.distance_unit)?;
                    result.extend(remove_list);
                }
                ComponentsAlgorithmType::Wcc => {
                    let remove_list =
                        wcc_components(edge_list, vertices, self.min_distance, self.distance_unit)?;
                    result.extend(remove_list);
                }
                ComponentsAlgorithmType::IterativeLeafPruning => {
                    let remove_list = iterative_leaf_pruning(
                        edge_list,
                        vertices,
                        self.min_distance,
                        self.distance_unit,
                    )?;
                    result.extend(remove_list);
                }
            }
        }
        Ok(result)
    }
}

pub type DenseAdjacencyList = Box<[IndexMap<(EdgeListId, EdgeId), VertexId>]>;

/// compute potential islands in a set of edge lists based on a radius distance
/// extension of each component. Returns the list of edges that need to be removed because they
/// belong to an island
pub fn kosaraju_scc(
    edge_list: &EdgeList,
    vertices: &[Vertex],
    distance_threshold: f64,
    distance_threshold_unit: DistanceUnit,
) -> Result<Vec<(EdgeListId, EdgeId)>, OvertureMapsCollectionError> {
    let forward_adjacency: DenseAdjacencyList = build_adjacency(edge_list, vertices.len(), true)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute adjacency matrix for island detection algorithm: {s}"
            ))
        })?;
    let backward_adjacency: DenseAdjacencyList = build_adjacency(edge_list, vertices.len(), false)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute adjacency matrix for island detection algorithm: {s}"
            ))
        })?;

    // Progress bar
    // let total_edges = edge_list.len();
    let mut pb = tqdm!(
        total = edge_list.len(),
        desc = "computing components - scanning edges"
    );

    // Main Loop: Kosaraju's Algorithm for SCCs

    // Pass 1: Forward DFS to get post-order finishing times
    let mut visited_forward = HashSet::<(EdgeListId, EdgeId)>::new();
    let mut post_order = Vec::<(EdgeListId, EdgeId)>::new();

    for start_edge in edge_list.edges() {
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

            let curr_edge = edge_list.0[curr.1 .0];
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
            let current_edge = edge_list.0[curr.1 .0];

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
    edge_list: &EdgeList,
    vertices: &[Vertex],
    distance_threshold: f64,
    distance_threshold_unit: DistanceUnit,
) -> Result<Vec<(EdgeListId, EdgeId)>, OvertureMapsCollectionError> {
    let forward_adjacency: DenseAdjacencyList = build_adjacency(edge_list, vertices.len(), true)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute forward adjacency matrix for wcc: {s}"
            ))
        })?;
    let backward_adjacency: DenseAdjacencyList = build_adjacency(edge_list, vertices.len(), false)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute backward adjacency matrix for wcc: {s}"
            ))
        })?;
    let mut pb = tqdm!(total = edge_list.len(), desc = "computing wcc components");

    let mut visited = HashSet::<(EdgeListId, EdgeId)>::new();
    let mut flagged = Vec::<(EdgeListId, EdgeId)>::new();

    for start_edge in edge_list.edges() {
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
            let current_edge = edge_list.0[curr.1 .0];

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
    edge_list: &EdgeList,
    vertices: &[Vertex],
    _distance_threshold: f64,
    _distance_threshold_unit: DistanceUnit,
) -> Result<Vec<(EdgeListId, EdgeId)>, OvertureMapsCollectionError> {
    let forward_adjacency: DenseAdjacencyList = build_adjacency(edge_list, vertices.len(), true)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute forward adjacency matrix for leaf pruning: {s}"
            ))
        })?;
    let backward_adjacency: DenseAdjacencyList = build_adjacency(edge_list, vertices.len(), false)
        .map_err(|s| {
            OvertureMapsCollectionError::InternalError(format!(
                "failed to compute backward adjacency matrix for leaf pruning: {s}"
            ))
        })?;

    let mut pb = tqdm!(total = edge_list.len(), desc = "computing leaf pruning");

    let mut out_degree = vec![0; vertices.len()];
    let mut in_degree = vec![0; vertices.len()];
    let mut active_edges = HashSet::<(EdgeListId, EdgeId)>::new();

    for edge in edge_list.edges() {
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
                    let e_info = edge_list.0[edge_key.1 .0];
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
                    let e_info = edge_list.0[edge_key.1 .0];
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
    let _ = pb.update_to(edge_list.len());
    eprintln!();

    Ok(flagged)
}

/// build the outgoing adjacency matrix
fn build_adjacency(
    edge_list: &EdgeList,
    n_vertices: usize,
    forward: bool,
) -> Result<DenseAdjacencyList, String> {
    let build_adjacencies_iter = tqdm!(
        edge_list.edges(),
        desc = "building adjacencies",
        total = edge_list.len()
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

    use super::*;
    use routee_compass_core::model::network::{Edge, EdgeList, Vertex};
    use uom::si::f64::Length;

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

        // threshold 100 meters
        let distance_threshold = 100.0;

        let flagged_wcc = wcc_components(
            &edge_list,
            &vertices,
            distance_threshold,
            DistanceUnit::Meters,
        )
        .unwrap();
        // WCC sees (261m > 100m) and keeps it (returns empty flagged)
        assert!(flagged_wcc.is_empty());

        let flagged_scc = kosaraju_scc(
            &edge_list,
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

        // threshold 100 meters
        let distance_threshold = 100.0;

        let flagged_wcc = wcc_components(
            &edge_list,
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

        // Iterative Leaf Pruning
        let flagged =
            iterative_leaf_pruning(&edge_list, &vertices, 100.0, DistanceUnit::Meters).unwrap();

        // Should only prune dangle (edges 3 and 4)
        assert_eq!(flagged.len(), 2);

        let flagged_edges: std::collections::HashSet<_> =
            flagged.into_iter().map(|e| e.1 .0).collect();
        assert!(flagged_edges.contains(&3));
        assert!(flagged_edges.contains(&4));
    }
}
