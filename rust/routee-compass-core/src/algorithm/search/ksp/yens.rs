use super::{ksp_query::KspQuery, ksp_termination_criteria::KspTerminationCriteria};
use crate::{
    algorithm::search::{
        edge_traversal::EdgeTraversal,
        search_algorithm::SearchAlgorithm,
        search_algorithm_result::SearchAlgorithmResult,
        search_error::SearchError,
        util::{EdgeCutConstraintModel, RouteSimilarityFunction},
        SearchInstance,
    },
    model::{
        constraint::{ConstraintModel, ConstraintModelError},
        network::EdgeId,
        unit::Cost,
    },
};
use itertools::Itertools;
use std::{collections::HashSet, sync::Arc};

/// an implementation of Yen's k-Shortest Paths Algorithm as described in the paper
///
/// Yen, Jin Y. "Finding the k shortest loopless paths in a network."
/// management Science 17.11 (1971): 712-716.
///
/// # Returns
///
/// The search tree of the true shortest path, along with all paths found
pub fn run(
    query: &KspQuery,
    termination: &KspTerminationCriteria,
    similarity: &RouteSimilarityFunction,
    si: &SearchInstance,
    underlying: &SearchAlgorithm,
) -> Result<SearchAlgorithmResult, SearchError> {
    // base case: we always have the true-shortest path
    let shortest = underlying.run_vertex_oriented(
        query.source,
        Some(query.target),
        query.user_query,
        &crate::algorithm::search::Direction::Forward,
        si,
    )?;
    if shortest.routes.is_empty() {
        return Ok(SearchAlgorithmResult::default());
    }
    let shortest_path = get_first_route(&shortest)?;
    let mut accepted: Vec<Vec<EdgeTraversal>> = vec![shortest_path.to_owned()];
    let mut iterations: u64 = 1; // number of times we call underlying search

    while accepted.len() < query.k {
        if termination.terminate_search(query.k, accepted.len()) {
            break;
        }

        let mut best_candidate: Option<(Vec<EdgeTraversal>, Cost)> = None;

        // build alternates off of most recently-picked accepted result
        let prev_accepted_path =
            accepted
                .last()
                .cloned()
                .ok_or(SearchError::InternalError(String::from(
                    "at least one route should be in routes",
                )))?;

        // step through each index along the most recently-accepted path
        for spur_idx in 0..prev_accepted_path.len() - 2 {
            let spur_len: usize = spur_idx + 1;
            let mut cut_edges: Vec<HashSet<EdgeId>> = (0..si.graph.n_edge_lists())
                .map(|_| HashSet::new())
                .collect_vec();
            let root_path = prev_accepted_path.iter().take(spur_len).collect_vec();
            let spur_edge_traversal =
                root_path
                    .last()
                    .ok_or(SearchError::InternalError(String::from(
                        "root path is empty",
                    )))?;
            let spur_vertex_id = si
                .graph
                .get_edge(
                    &spur_edge_traversal.edge_list_id,
                    &spur_edge_traversal.edge_id,
                )?
                .dst_vertex_id;

            // cut frontier edges based on previous paths with matching root path
            for accepted_path in accepted.iter() {
                let accepted_path_root = accepted_path.iter().take(spur_len).collect_vec();
                if same_path(&root_path, &accepted_path_root) {
                    if let Some(cut_edge) = accepted_path.get(spur_idx + 1) {
                        if let Some(cut_set) = cut_edges.get_mut(cut_edge.edge_list_id.0) {
                            cut_set.insert(cut_edge.edge_id);
                        }
                    }
                }
            }

            // execute a new path search using a wrapped constraint model to exclude edges
            let yens_frontier: Vec<Arc<dyn ConstraintModel>> = cut_edges.iter().enumerate().map(|(edge_list_id, cut)| {
                let underlying = si.constraint_models.get(edge_list_id).ok_or_else(|| ConstraintModelError::ConstraintModelError(format!("when constructing edge cut constraint model, could not find edge list '{edge_list_id}'")))?;
                let model: Arc<dyn ConstraintModel> = Arc::new(EdgeCutConstraintModel::new(underlying.clone(), cut.clone()));
                Ok(model)
            }).collect::<Result<Vec<_>, SearchError>>()?;
            let yens_si = SearchInstance {
                graph: si.graph.clone(),
                map_model: si.map_model.clone(),
                state_model: si.state_model.clone(),
                traversal_models: si.traversal_models.iter().cloned().collect_vec(),
                cost_model: si.cost_model.clone(),
                constraint_models: yens_frontier,
                termination_model: si.termination_model.clone(),
                label_model: si.label_model.clone(),
                default_edge_list: si.default_edge_list,
            };
            let spur_result = underlying.run_vertex_oriented(
                spur_vertex_id,
                Some(query.target),
                query.user_query,
                &crate::algorithm::search::Direction::Forward,
                &yens_si,
            )?;
            iterations += 1;

            let spur_path = get_first_route(&spur_result)?;
            let candidate_path = root_path
                .into_iter()
                .chain(spur_path)
                .cloned()
                .collect_vec();
            let candidate_test_path: &Vec<&EdgeTraversal> = &candidate_path.iter().collect_vec();
            // replace best candidate if current candidate is sufficiently dissimilar and improves on cost
            for test_path in accepted.iter() {
                let similar = similarity.clone().test_similarity(
                    &test_path.iter().collect_vec(),
                    candidate_test_path,
                    &yens_si,
                )?;
                if !similar {
                    let candidate_cost: Cost =
                        candidate_test_path.iter().map(|e| e.cost.edge_cost).sum();
                    match best_candidate {
                        Some((_, best_cost)) if candidate_cost < best_cost => {
                            best_candidate = Some((candidate_path.clone(), candidate_cost));
                        }
                        None => {
                            best_candidate = Some((candidate_path.clone(), candidate_cost));
                        }
                        Some(_) => {}
                    }
                }
            }
            if let Some((ref best_path, _)) = best_candidate {
                accepted.push(best_path.clone());
            }
        }
    }

    let result = SearchAlgorithmResult {
        trees: shortest.trees,
        routes: accepted,
        iterations,
        terminated: None,
    };
    Ok(result)
}

/// helper function to grab the first route from a search algorithm result
fn get_first_route(res: &SearchAlgorithmResult) -> Result<&Vec<EdgeTraversal>, SearchError> {
    res.routes
        .first()
        .ok_or(SearchError::InternalError(String::from(
            "no empty results should be stored in routes",
        )))
}

/// compares two routes by their sequence of EdgeIds, returning true if they are the same
fn same_path(a: &[&EdgeTraversal], b: &[&EdgeTraversal]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (a_edge, b_edge) in a.iter().zip(b) {
        if a_edge.edge_id != b_edge.edge_id {
            return false;
        }
    }
    true
}
