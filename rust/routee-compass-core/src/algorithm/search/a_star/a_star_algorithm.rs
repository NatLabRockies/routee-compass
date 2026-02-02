use crate::algorithm::search::a_star::frontier_instance::FrontierInstance;
use crate::algorithm::search::Direction;
use crate::algorithm::search::EdgeTraversal;
use crate::algorithm::search::SearchError;
use crate::algorithm::search::SearchInstance;
use crate::algorithm::search::SearchResult;
use crate::algorithm::search::SearchTree;
use crate::algorithm::search::SearchTreeError;
use crate::model::cost::TraversalCost;
use crate::model::label::Label;
use crate::model::label::LabelModel;
use crate::model::label::LabelModelError;
use crate::model::network::EdgeListId;
use crate::model::network::{EdgeId, VertexId};
use crate::model::state::StateVariable;
use crate::model::unit::Cost;
use crate::model::unit::ReverseCost;
use crate::util::priority_queue::InternalPriorityQueue;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// run an A* Search over the given directed graph model. traverses links
/// from the source, via the provided direction, to the target. uses the
/// provided traversal model for state updates and link costs. estimates
/// the distance to the destination (the a* heuristic) using the provided
/// cost estimate function.
pub fn run_vertex_oriented(
    source: VertexId,
    target: Option<VertexId>,
    direction: &Direction,
    a_star: bool,
    si: &SearchInstance,
) -> Result<SearchResult, SearchError> {
    log::debug!(
        "sssp::run_vertex_oriented: source: {source}, target: {target:?}, direction: {direction:?}, astar: {a_star}"
    );
    if target == Some(source) {
        let initial_state = si.state_model.initial_state(None)?;
        let initial_label =
            si.label_model
                .label_from_state(source, &initial_state, &si.state_model)?;
        let tree = SearchTree::with_root(initial_label, *direction);
        return Ok(SearchResult::completed(tree, 0));
    }

    // context for the search (graph, search functions, frontier priority queue)
    let mut frontier: InternalPriorityQueue<Label, ReverseCost> = InternalPriorityQueue::default();
    let mut traversal_costs: HashMap<Label, Cost> = HashMap::new();
    let mut solution = SearchTree::new(*direction);

    // setup initial search state
    let initial_state = si.state_model.initial_state(None)?;
    let inital_label = si
        .label_model
        .label_from_state(source, &initial_state, &si.state_model)?;
    traversal_costs.insert(inital_label.clone(), Cost::ZERO);
    let origin_cost = match (target, a_star) {
        (Some(target), true) => {
            let cost_est = estimate_traversal_cost(source, target, &initial_state, &solution, si)?;
            cost_est.objective_cost
        }
        _ => Cost::ZERO,
    };
    frontier.push(inital_label, origin_cost.into());

    let start_time = Instant::now();
    let mut iterations = 0;

    loop {
        // terminate the search if a termination condition was met.
        if let Some(explanation) =
            si.termination_model
                .continue_or_explain(&start_time, &solution, iterations)
        {
            return Ok(SearchResult::terminated(solution, iterations, explanation));
        }

        // grab the frontier assets, or break if there is nothing to pop
        let f = match FrontierInstance::pop_new(
            &mut frontier,
            source,
            target,
            &solution,
            &initial_state,
        )? {
            None => break,
            Some(f) => f,
        };

        let prev_gscore = traversal_costs
            .get(&f.prev_label)
            .unwrap_or(&Cost::INFINITY)
            .to_owned();

        // visit all neighbors of this source vertex
        let incident_edge_iterator = direction.get_incident_edges(f.prev_label.vertex_id(), si);
        for (edge_list_id, edge_id) in incident_edge_iterator {
            let e = si.graph.get_edge(edge_list_id, edge_id)?;

            let terminal_vertex_id = direction.terminal_vertex_id(e);
            let terminal_label = si.label_model.label_from_state(
                terminal_vertex_id,
                &f.prev_state,
                &si.state_model,
            )?;
            let key_vertex_id = direction.tree_key_vertex_id(e);

            let previous_edge = match f.prev_edge {
                Some((edge_list_id, edge_id)) => Some(si.graph.get_edge(&edge_list_id, &edge_id)?),
                None => None,
            };
            let valid_frontier = {
                si.get_constraint_model(edge_list_id)?.valid_frontier(
                    e,
                    previous_edge,
                    &f.prev_state,
                    &si.state_model,
                )?
            };
            if !valid_frontier {
                continue;
            }

            let next_edge = (*edge_list_id, *edge_id);
            let et = EdgeTraversal::new(next_edge, &solution, &f.prev_state, si)?;
            let key_label = si.label_model.label_from_state(
                key_vertex_id,
                &et.result_state,
                &si.state_model,
            )?;

            let tentative_gscore = prev_gscore + et.cost.objective_cost;

            let existing_gscore = traversal_costs
                .get(&key_label)
                .unwrap_or(&Cost::INFINITY)
                .to_owned();

            if tentative_gscore < existing_gscore {
                // accept this traversal, updating search state
                prune_tree(&mut solution, &key_label, &et, si.label_model.clone())?;
                traversal_costs.insert(key_label.clone(), tentative_gscore);
                solution.insert(terminal_label, et.clone(), key_label.clone())?;

                let dst_h_cost = match (target, a_star) {
                    (Some(target), true) => {
                        let cost_est = estimate_traversal_cost(
                            key_vertex_id,
                            target,
                            &initial_state,
                            &solution,
                            si,
                        )?;
                        cost_est.objective_cost
                    }
                    _ => Cost::ZERO,
                };

                let f_score_value = tentative_gscore + dst_h_cost;
                frontier.push_increase(key_label, f_score_value.into());
            }
        }
        iterations += 1;
    }
    log::debug!(
        "search iterations: {}, size of search tree: {}",
        iterations,
        solution.len()
    );

    let result = SearchResult::completed(solution, iterations);
    Ok(result)
}

/// convenience method when origin and destination are specified using
/// edge ids instead of vertex ids. invokes a vertex-oriented search
/// from the out-vertex of the source edge to the in-vertex of the
/// target edge. composes the result with the source and target.
pub fn run_edge_oriented(
    source: (EdgeListId, EdgeId),
    target: Option<(EdgeListId, EdgeId)>,
    direction: &Direction,
    a_star: bool,
    si: &SearchInstance,
) -> Result<SearchResult, SearchError> {
    // For now, convert to vertex-oriented search and use compatibility layer
    let _e1_src = si.graph.src_vertex_id(&source.0, &source.1)?;
    let e1_dst = si.graph.dst_vertex_id(&source.0, &source.1)?;

    match target {
        None => run_vertex_oriented(e1_dst, None, direction, a_star, si),
        Some(target_edge) => {
            let e2_src = si.graph.src_vertex_id(&target_edge.0, &target_edge.1)?;
            let _e2_dst = si.graph.dst_vertex_id(&target_edge.0, &target_edge.1)?;

            if source == target_edge {
                let initial_state = si.state_model.initial_state(None)?;
                let initial_label =
                    si.label_model
                        .label_from_state(e1_dst, &initial_state, &si.state_model)?;
                let tree = SearchTree::with_root(initial_label, *direction);
                Ok(SearchResult::completed(tree, 0))
            } else {
                run_vertex_oriented(e1_dst, Some(e2_src), direction, a_star, si)
            }
        }
    }
}

/// approximates the traversal state delta between two vertices and uses
/// the result to compute a cost estimate.
pub fn estimate_traversal_cost(
    src: VertexId,
    dst: VertexId,
    state: &[StateVariable],
    tree: &SearchTree,
    si: &SearchInstance,
) -> Result<TraversalCost, SearchError> {
    let src = si.graph.get_vertex(&src)?;
    let dst = si.graph.get_vertex(&dst)?;
    let mut dst_state = state.to_vec();

    si.get_traversal_estimation_model().estimate_traversal(
        (src, dst),
        &mut dst_state,
        tree,
        &si.state_model,
    )?;
    let cost_estimate = si.cost_model.estimate_cost(&dst_state, &si.state_model)?;
    Ok(cost_estimate)
}

/// allow the new label to remove any old labels which are dominated both
/// by cost and by their LabelModel's definition of "dominated".
fn prune_tree(
    tree: &mut SearchTree,
    next_label: &Label,
    traversal: &EdgeTraversal,
    label_model: Arc<dyn LabelModel>,
) -> Result<(), SearchError> {
    if next_label.does_not_require_pruning() {
        return Ok(());
    }
    let next_cost = traversal.cost.objective_cost;
    let prev_entries = tree
        .get_labels_iter(*next_label.vertex_id())
        .map(|label| {
            let node = tree
                .get(&label)
                .ok_or_else(|| SearchTreeError::MissingNodeForLabel(label.clone()))?;
            let cost = node.traversal_cost().map(|tc| tc.objective_cost);
            Ok((label, cost.unwrap_or_default()))
        })
        .collect::<Result<Vec<_>, SearchError>>()?;

    for (prev_label, prev_cost) in prev_entries.into_iter() {
        let remove = next_label_dominates_prev(
            &prev_label,
            prev_cost,
            next_label,
            next_cost,
            label_model.clone(),
        )?;
        if remove {
            // new label is pareto-dominant over this previous label.
            let _ = tree.remove(&prev_label);
        }
    }

    Ok(())
}

/// remove previous label only if it is dominated by the new label
/// in a Pareto sense: new is at least as good on all objectives
/// (label state and cost) and strictly better on at least one.
fn next_label_dominates_prev(
    prev_label: &Label,
    prev_cost: Cost,
    next_label: &Label,
    next_cost: Cost,
    label_model: Arc<dyn LabelModel>,
) -> Result<bool, LabelModelError> {
    let label_comparison = label_model.compare(prev_label, next_label)?;
    let dominates = match label_comparison {
        // prev_label is worse in label state than the new label; new must
        // also be no worse in cost to dominate.
        std::cmp::Ordering::Less => next_cost <= prev_cost,
        // label states are equivalent; new must be strictly cheaper to dominate.
        std::cmp::Ordering::Equal => next_cost < prev_cost,
        // prev_label is better in label state; new cannot dominate regardless of cost.
        std::cmp::Ordering::Greater => false,
    };
    Ok(dominates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::constraint::default::no_restriction::NoRestriction;
    use crate::model::cost::CostAggregation;
    use crate::model::cost::CostModel;
    use crate::model::cost::VehicleCostRate;
    use crate::model::label::default::vertex_label_model::VertexLabelModel;
    use crate::model::map::MapModel;
    use crate::model::map::MapModelConfig;
    use crate::model::network::Edge;
    use crate::model::network::EdgeId;
    use crate::model::network::EdgeList;
    use crate::model::network::Graph;
    use crate::model::network::Vertex;
    use crate::model::state::StateModel;
    use crate::model::termination::TerminationModel;
    use crate::model::traversal::default::distance::DistanceTraversalModel;
    use crate::model::traversal::TraversalModel;
    use crate::model::unit::DistanceUnit;
    use indexmap::IndexMap;
    use rayon::prelude::*;
    use std::sync::Arc;
    use uom::si::f64::Length;

    fn build_square_graph() -> Graph {
        use uom::si::length::kilometer;

        let vertices = vec![
            Vertex::new(0, 0.0, 0.0),
            Vertex::new(1, 0.0, 0.0),
            Vertex::new(2, 0.0, 0.0),
            Vertex::new(3, 0.0, 0.0),
        ];

        let edges = vec![
            Edge::new(0, 0, 0, 1, Length::new::<kilometer>(10.0)),
            Edge::new(0, 1, 1, 0, Length::new::<kilometer>(10.0)),
            Edge::new(0, 2, 1, 2, Length::new::<kilometer>(2.0)),
            Edge::new(0, 3, 2, 1, Length::new::<kilometer>(2.0)),
            Edge::new(0, 4, 2, 3, Length::new::<kilometer>(1.0)),
            Edge::new(0, 5, 3, 2, Length::new::<kilometer>(1.0)),
            Edge::new(0, 6, 3, 0, Length::new::<kilometer>(2.0)),
            Edge::new(0, 7, 0, 3, Length::new::<kilometer>(2.0)),
        ];

        let mut adj = vec![IndexMap::new(); vertices.len()];
        let mut rev = vec![IndexMap::new(); vertices.len()];
        let edge_list_id = EdgeListId(0);

        for edge in &edges {
            adj[edge.src_vertex_id.0].insert((edge_list_id, edge.edge_id), edge.dst_vertex_id);
            rev[edge.dst_vertex_id.0].insert((edge_list_id, edge.edge_id), edge.src_vertex_id);
        }

        // Construct the Graph instance.

        Graph {
            vertices: vertices.into_boxed_slice(),
            edge_lists: vec![EdgeList(edges.into_boxed_slice())],
            adj: adj.into_boxed_slice(),
            rev: rev.into_boxed_slice(),
        }
    }

    /// builds a cross-shaped test graph where A* should outperform Dijkstra's.
    fn build_astar_graph() -> Graph {
        use uom::si::length::kilometer;

        //       (3)
        //        |
        // (2) - (0) - (4) - (5)
        //        |
        //       (1)

        let vertices = vec![
            Vertex::new(0, 0.0, 0.0),
            Vertex::new(1, 0.0, -0.01),
            Vertex::new(2, -0.01, 0.0),
            Vertex::new(3, 0.0, 0.01),
            Vertex::new(4, 0.01, 0.0),
            Vertex::new(5, 0.02, 0.0),
        ];

        let onekm = Length::new::<kilometer>(1.0);
        let edges = vec![
            Edge::new(0, 0, 0, 1, onekm),
            Edge::new(0, 1, 1, 0, onekm),
            Edge::new(0, 2, 0, 2, onekm),
            Edge::new(0, 3, 2, 0, onekm),
            Edge::new(0, 4, 0, 3, onekm),
            Edge::new(0, 5, 3, 0, onekm),
            Edge::new(0, 6, 0, 4, onekm),
            Edge::new(0, 7, 4, 0, onekm),
            Edge::new(0, 8, 4, 5, onekm),
        ];

        let mut adj = vec![IndexMap::new(); vertices.len()];
        let mut rev = vec![IndexMap::new(); vertices.len()];
        let edge_list_id = EdgeListId(0);

        for edge in &edges {
            adj[edge.src_vertex_id.0].insert((edge_list_id, edge.edge_id), edge.dst_vertex_id);
            rev[edge.dst_vertex_id.0].insert((edge_list_id, edge.edge_id), edge.src_vertex_id);
        }

        // Construct the Graph instance.

        Graph {
            vertices: vertices.into_boxed_slice(),
            edge_lists: vec![EdgeList(edges.into_boxed_slice())],
            adj: adj.into_boxed_slice(),
            rev: rev.into_boxed_slice(),
        }
    }

    fn build_search_instance(graph: Arc<Graph>) -> SearchInstance {
        let map_model = Arc::new(MapModel::new(graph.clone(), &MapModelConfig::default()).unwrap());
        let traversal_model = Arc::new(DistanceTraversalModel::new(DistanceUnit::default(), true));

        // setup the graph, traversal model, and a* heuristic to be shared across the queries in parallel
        // these live in the "driver" process and are passed as read-only memory to each executor process
        let state_model = Arc::new(
            StateModel::empty()
                .register(
                    traversal_model.clone().input_features(),
                    traversal_model.clone().output_features(),
                )
                .unwrap(),
        );
        let cost_model = CostModel::new(
            // vec![(String::from("distance"), 0usize)],
            Arc::new(HashMap::from([(String::from("trip_distance"), 1.0)])),
            Arc::new(HashMap::from([(
                String::from("trip_distance"),
                VehicleCostRate::Raw,
            )])),
            Arc::new(HashMap::new()),
            CostAggregation::Sum,
            state_model.clone(),
        )
        .unwrap();
        SearchInstance {
            graph,
            map_model,
            state_model: state_model.clone(),
            traversal_models: vec![traversal_model.clone()],
            constraint_models: vec![Arc::new(NoRestriction {})],
            cost_model: Arc::new(cost_model),
            termination_model: Arc::new(TerminationModel::IterationsLimit { limit: 20 }),
            label_model: Arc::new(VertexLabelModel {}),
            default_edge_list: None,
        }
    }

    fn build_soc_label_model() -> Arc<dyn LabelModel> {
        struct SocLabelModel {}
        impl LabelModel for SocLabelModel {
            fn label_from_state(
                &self,
                _vertex_id: VertexId,
                _state: &[StateVariable],
                _state_model: &StateModel,
            ) -> Result<Label, LabelModelError> {
                unreachable!()
            }

            fn compare(
                &self,
                prev: &Label,
                next: &Label,
            ) -> Result<std::cmp::Ordering, LabelModelError> {
                match (prev, next) {
                    (
                        Label::VertexWithIntState { state: s1, .. },
                        Label::VertexWithIntState { state: s2, .. },
                    ) => Ok(s1.cmp(s2)),
                    _ => unreachable!(),
                }
            }
        }
        Arc::new(SocLabelModel {})
    }

    #[test]
    fn test_not_pareto_dominated() {
        let label_model = build_soc_label_model();
        let prev_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 30,
        };
        let prev_cost = Cost::new(50.0);
        let next_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 80,
        };
        let next_cost = Cost::new(70.0);
        let is_dominated = next_label_dominates_prev(
            &prev_label,
            prev_cost,
            &next_label,
            next_cost,
            label_model.clone(),
        )
        .expect("test invariant failed");
        assert!(!is_dominated);
    }

    #[test]
    fn test_is_pareto_dominated() {
        let label_model = build_soc_label_model();
        let prev_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 30,
        };
        let prev_cost = Cost::new(50.0);
        let next_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 80,
        };
        let next_cost = Cost::new(40.0);
        let is_dominated = next_label_dominates_prev(
            &prev_label,
            prev_cost,
            &next_label,
            next_cost,
            label_model.clone(),
        )
        .expect("test invariant failed");
        assert!(is_dominated);
    }

    #[test]
    fn test_astar_graph() {
        let query_origin = VertexId(0);
        let query_destination = VertexId(5);
        let optimal_route = vec![EdgeId(6), EdgeId(8)];
        let graph = Arc::new(build_astar_graph());
        let si = build_search_instance(graph.clone());
        let result = run_vertex_oriented(
            query_origin,
            Some(query_destination),
            &Direction::Forward,
            true,
            &si,
        )
        .expect("failure running search for A* test");
        assert_eq!(result.iterations, 2);
        let route = result
            .tree
            .backtrack(query_destination)
            .expect("failure creating search result");
        for (route_edge, expected_edge) in route.into_iter().zip(optimal_route.into_iter()) {
            assert_eq!(route_edge.edge_id, expected_edge);
        }
    }

    #[test]
    fn test_square_graph() {
        // simple box world that exists in a non-euclidean plane that stretches
        // the distances between vertex 0 and 1. tested using a distance cost
        // function. to zero out the cost estimate function, all vertices have a
        // position of (0,0).
        // (0) <---> (1)
        //  ^         ^
        //  |         |
        //  v         v
        // (3) <---> (2)
        // (0) -[0]-> (1) 10 units distance
        // (1) -[1]-> (0) 10 units distance
        // (1) -[2]-> (2) 2 units distance
        // (2) -[3]-> (1) 2 units distance
        // (2) -[4]-> (3) 1 units distance
        // (3) -[5]-> (2) 1 units distance
        // (3) -[6]-> (0) 2 units distance
        // (0) -[7]-> (3) 2 units distance

        // these are the queries to test the grid world. for each query,
        // we have the vertex pair (source, target) to submit to the
        // search algorithm, and then the expected route traversal vector for each pair.
        // a comment is provided that illustrates each query/expected traversal combination.
        let queries: Vec<(VertexId, VertexId, Vec<EdgeId>)> = vec![
            (
                // 0 -[7]-> 3 -[5]-> 2 -[3]-> 1
                VertexId(0),
                VertexId(1),
                vec![EdgeId(7), EdgeId(5), EdgeId(3)],
            ),
            (
                // 0 -[7]-> 3
                VertexId(0),
                VertexId(3),
                vec![EdgeId(7)],
            ),
            (
                // 1 -[2]-> 2 -[4]-> 3 -[6]-> 0
                VertexId(1),
                VertexId(0),
                vec![EdgeId(2), EdgeId(4), EdgeId(6)],
            ),
            (VertexId(1), VertexId(2), vec![EdgeId(2)]), // 1 -[2]-> 2
            (VertexId(2), VertexId(3), vec![EdgeId(4)]), // 2 -[4]-> 3
        ];

        let graph = Arc::new(build_square_graph());
        let si = build_search_instance(graph.clone());

        // execute the route search
        let result: Vec<Result<SearchTree, SearchError>> = queries
            .clone()
            .into_par_iter()
            .map(|(o, d, _expected)| {
                run_vertex_oriented(o, Some(d), &Direction::Forward, false, &si)
                    .map(|search_result| search_result.tree)
            })
            .collect();

        // review the search results, confirming that the route result matches the expected route
        for (r, (_, d, expected_route)) in result.into_iter().zip(queries) {
            let solution = r.unwrap();
            let route = solution.backtrack(d).unwrap();
            let route_edges: Vec<EdgeId> = route.iter().map(|r| r.edge_id).collect();
            assert_eq!(
                route_edges, expected_route,
                "route did not match expected: {route_edges:?} {expected_route:?}"
            );
        }
    }
}
