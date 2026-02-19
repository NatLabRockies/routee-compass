use std::collections::{HashSet, VecDeque};

use geo::{line_string, Haversine, Length, Point};
use indexmap::IndexMap;
use kdam::{tqdm, BarExt};
use rayon::prelude::*;
use routee_compass_core::model::{
    network::{Edge, EdgeId, EdgeList, EdgeListId, Vertex, VertexId},
    unit::DistanceUnit,
};
use uom::si::f64::Length as uom_length;

use crate::collection::OvertureMapsCollectionError;

pub type DenseAdjacencyList = Box<[IndexMap<(EdgeListId, EdgeId), VertexId>]>;

/// compute potential islands in a set of edge lists based on a radius distance
/// extension of each component. Returns the list of edges that need to be removed because they
/// belong to an island
pub fn island_detection_algorithm(
    edge_lists: &[&EdgeList],
    vertices: &[Vertex],
    distance_threshold: f64,
    distance_threshold_unit: DistanceUnit,
    parallel_execution: bool,
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

    let island_edges: Result<Vec<_>, _> = if parallel_execution {
        edge_lists
            .par_iter()
            .flat_map(|&el| el.0.par_iter())
            .filter_map(|edge| {
                match is_component_island_parallel(
                    edge,
                    distance_threshold,
                    distance_threshold_unit,
                    edge_lists,
                    vertices,
                    &forward_adjacency,
                    &backward_adjacency,
                ) {
                    Ok(true) => Some(Ok((edge.edge_list_id, edge.edge_id))),
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                }
            })
            .collect()
    } else {
        // Progress bar
        let total_edges = edge_lists.iter().map(|el| el.len()).sum::<usize>();
        let mut pb = tqdm!(
            total = total_edges,
            desc = "computing components - scanning edges"
        );

        // Initialization
        let mut visited = HashSet::<(EdgeListId, EdgeId)>::new();
        let mut queue = VecDeque::<(EdgeListId, EdgeId)>::new();
        let mut flagged = Vec::<(EdgeListId, EdgeId)>::new();

        // Main Loop
        // Each non-skipped iteration of the large loop is a separate component
        for start_edge in edge_lists.iter().flat_map(|el| el.edges()) {
            // NOTE: Due to directionality and the fact that I am using only forward connectivity
            // it is necessary to have two `if visited contains` checks.
            if visited.contains(&(start_edge.edge_list_id, start_edge.edge_id)) {
                continue;
            }

            // Initialize the component
            queue.push_back((start_edge.edge_list_id, start_edge.edge_id));
            let mut max_distance_reached = uom_length::new::<uom::si::length::meter>(0.0);
            let mut component = Vec::<(EdgeListId, EdgeId)>::new();
            let start_midpoint = compute_midpoint(start_edge, vertices);

            // Loop through the queue (explore the component)
            loop {
                if let Some((current_el_id, current_e_id)) = queue.pop_front() {
                    if visited.contains(&(current_el_id, current_e_id)) {
                        continue;
                    }
                    component.push((current_el_id, current_e_id));
                    let current_edge = edge_lists[current_el_id.0].0[current_e_id.0];

                    let current_distance = is_component_island_sequential(
                        &current_edge,
                        start_midpoint,
                        &mut visited,
                        &mut queue,
                        vertices,
                        &forward_adjacency,
                        &backward_adjacency,
                    );

                    // Update the max_distance
                    let current_distance_uom =
                        uom_length::new::<uom::si::length::meter>(current_distance as f64);
                    max_distance_reached = max_distance_reached.max(current_distance_uom);

                    // Update bar
                    if let Err(e) = pb.update(1) {
                        log::warn!("error during update of progress bar: {e}")
                    };
                } else {
                    break;
                };
            }

            // At the end, flag all the edges in the component for deletion
            if max_distance_reached < distance_threshold_unit.to_uom(distance_threshold) {
                flagged.append(&mut component);
            }
        }

        eprintln!();
        Ok(flagged)
    };

    island_edges
}

/// returns the f32 distance in meters from the current edge midpoint to the initial edge midpoint.
fn is_component_island_sequential(
    edge: &Edge,
    initial_midpoint: Point<f32>,
    visited: &mut HashSet<(EdgeListId, EdgeId)>,
    queue: &mut VecDeque<(EdgeListId, EdgeId)>,
    vertices: &[Vertex],
    forward_adjacency: &DenseAdjacencyList,
    backward_adjacency: &DenseAdjacencyList,
) -> f32 {
    let (edge_list_id, edge_id) = (edge.edge_list_id, edge.edge_id);

    // Update counter
    let current_midpoint = compute_midpoint(edge, vertices);
    let current_distance_to_start_meters =
        Haversine.length(&line_string![initial_midpoint.0, current_midpoint.0]);

    // get all neighbors, add them to queue
    let outward_edges: Vec<&(EdgeListId, EdgeId)> =
        forward_adjacency[edge.dst_vertex_id.0].keys().collect();
    for (edge_list_id, edge_id) in outward_edges {
        queue.push_back((*edge_list_id, *edge_id));
    }
    let inward_edges: Vec<&(EdgeListId, EdgeId)> =
        backward_adjacency[edge.src_vertex_id.0].keys().collect();
    for (edge_list_id, edge_id) in inward_edges {
        queue.push_back((*edge_list_id, *edge_id));
    }

    // mark as visited
    visited.insert((edge_list_id, edge_id));

    current_distance_to_start_meters
}

/// parallelizable implementation
fn is_component_island_parallel(
    edge: &Edge,
    distance_threshold: f64,
    distance_threshold_unit: DistanceUnit,
    edge_lists: &[&EdgeList],
    vertices: &[Vertex],
    forward_adjacency: &DenseAdjacencyList,
    backward_adjacency: &DenseAdjacencyList,
) -> Result<bool, OvertureMapsCollectionError> {
    let mut visited = HashSet::<(&EdgeListId, &EdgeId)>::new();
    let mut visit_queue: VecDeque<(&EdgeListId, &EdgeId)> = VecDeque::new();
    visit_queue.push_back((&edge.edge_list_id, &edge.edge_id));

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
            visited.insert((current_edge_list_id, current_edge_id));

            // Retrieve current edge information
            let current_edge = edge_lists.get(current_edge_list_id.0)
                                                            .and_then(|el| el.get(current_edge_id))
                                                            .ok_or(OvertureMapsCollectionError::InternalError(format!("edge list {current_edge_list_id:?} or edge {current_edge_id:?} not found during island detection starting at edge {edge:?}")))?;

            // Expand queue
            let outward_edges: Vec<&(EdgeListId, EdgeId)> = forward_adjacency
                [current_edge.dst_vertex_id.0]
                .keys()
                .collect();
            for (edge_list_id, edge_id) in outward_edges {
                visit_queue.push_back((edge_list_id, edge_id));
            }
            let inward_edges: Vec<&(EdgeListId, EdgeId)> = backward_adjacency
                [current_edge.src_vertex_id.0]
                .keys()
                .collect();
            for (edge_list_id, edge_id) in inward_edges {
                visit_queue.push_back((edge_list_id, edge_id));
            }

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
}
