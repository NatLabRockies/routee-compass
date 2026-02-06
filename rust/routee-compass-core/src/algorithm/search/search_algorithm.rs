use super::edge_traversal::EdgeTraversal;
use super::ksp::KspQuery;
use super::ksp::KspTerminationCriteria;
use super::ksp::{svp, yens};
use super::search_algorithm_result::SearchAlgorithmResult;
use super::search_error::SearchError;
use super::util::RouteSimilarityFunction;
use super::SearchInstance;
use super::{a_star, direction::Direction};
use crate::algorithm::search::search_algorithm_config::SearchAlgorithmConfig;
use crate::algorithm::search::TerminationFailurePolicy;
use crate::model::cost::TraversalCost;
use crate::model::network::EdgeListId;
use crate::model::network::{EdgeId, VertexId};

#[derive(Clone, Debug)]
pub enum SearchAlgorithm {
    /// algorithm to support classic SSSP algorithms such as Dijkstra's and A*.
    SingleSourceShortestPath {
        /// if the termination model returns early, treat it as a search error.
        /// if false, the result is still returned. this option is not valid
        /// for searches without destinations (path searches).
        termination_behavior: TerminationFailurePolicy,
        /// if true, use a cost estimate heuristic to guide the search towards destinations
        a_star: bool,
    },
    /// KSP using the single via paths algorithm.
    KspSingleVia {
        /// number of alternative paths to attempt
        k: usize,
        /// path search algorithm to use
        underlying: Box<SearchAlgorithm>,
        /// if provided, filters out potential solution paths based on their
        /// similarity to the paths in the stored result set
        similarity: Option<RouteSimilarityFunction>,
        /// termination criteria for the inner path search function
        termination: Option<KspTerminationCriteria>,
    },
    /// KSP using Yen's Algorithm
    Yens {
        /// number of alternative paths to attempt
        k: usize,
        /// path search algorithm to use
        underlying: Box<SearchAlgorithm>,
        /// if provided, filters out potential solution paths based on their
        /// similarity to the paths in the stored result set
        similarity: Option<RouteSimilarityFunction>,
        /// termination criteria for the inner path search function
        termination: Option<KspTerminationCriteria>,
    },
}

impl SearchAlgorithm {
    pub fn run_vertex_oriented(
        &self,
        src_id: VertexId,
        dst_id_opt: Option<VertexId>,
        query: &serde_json::Value,
        direction: &Direction,
        si: &SearchInstance,
    ) -> Result<SearchAlgorithmResult, SearchError> {
        match self {
            SearchAlgorithm::SingleSourceShortestPath {
                termination_behavior,
                a_star,
            } => {
                let search_result =
                    a_star::run_vertex_oriented(src_id, dst_id_opt, direction, *a_star, si)?;
                termination_behavior.handle_termination(&search_result, dst_id_opt.is_some())?;

                let routes = match dst_id_opt {
                    None => vec![],
                    Some(dst_id) => {
                        let route = search_result.tree.backtrack(dst_id)?;
                        vec![route]
                    }
                };
                Ok(SearchAlgorithmResult {
                    trees: vec![search_result.tree],
                    routes,
                    iterations: search_result.iterations,
                    terminated: search_result.terminated.clone(),
                })
            }
            SearchAlgorithm::Yens {
                k,
                underlying,
                similarity,
                termination,
            } => {
                let dst_id = dst_id_opt.ok_or_else(|| {
                    SearchError::BuildError(String::from(
                        "attempting to run KSP algorithm without destination",
                    ))
                })?;
                let sim_fn = similarity.clone().unwrap_or_default();
                let term_fn = termination.clone().unwrap_or_default();
                let ksp_query = KspQuery::new(src_id, dst_id, query, *k)?;
                yens::run(&ksp_query, &term_fn, &sim_fn, si, underlying)
            }
            SearchAlgorithm::KspSingleVia {
                k,
                underlying,
                similarity,
                termination,
            } => {
                let dst_id = dst_id_opt.ok_or_else(|| {
                    SearchError::BuildError(String::from(
                        "attempting to run KSP algorithm without destination",
                    ))
                })?;
                let sim_fn = similarity.clone().unwrap_or_default();
                let term_fn = termination.clone().unwrap_or_default();
                let ksp_query = KspQuery::new(src_id, dst_id, query, *k)?;
                svp::run(&ksp_query, &term_fn, &sim_fn, si, underlying)
            }
        }
    }
    pub fn run_edge_oriented(
        &self,
        src: (EdgeListId, EdgeId),
        dst_opt: Option<(EdgeListId, EdgeId)>,
        query: &serde_json::Value,
        direction: &Direction,
        si: &SearchInstance,
    ) -> Result<SearchAlgorithmResult, SearchError> {
        match self {
            SearchAlgorithm::SingleSourceShortestPath {
                termination_behavior,
                a_star,
            } => {
                let search_result =
                    a_star::run_edge_oriented(src, dst_opt, direction, *a_star, si)?;

                termination_behavior.handle_termination(&search_result, dst_opt.is_some())?;

                let routes = match dst_opt {
                    None => vec![],
                    Some(dst_id) => {
                        let route = search_result
                            .tree
                            .backtrack_edge_oriented_route(dst_id, si.graph.clone())?;
                        vec![route]
                    }
                };
                Ok(SearchAlgorithmResult {
                    trees: vec![search_result.tree],
                    routes,
                    iterations: search_result.iterations,
                    terminated: search_result.terminated.clone(),
                })
            }
            SearchAlgorithm::KspSingleVia {
                k: _,
                underlying: _,
                similarity: _,
                termination: _,
            } => run_edge_oriented(src, dst_opt, query, direction, self, si),
            SearchAlgorithm::Yens {
                k: _,
                underlying: _,
                similarity: _,
                termination: _,
            } => run_edge_oriented(src, dst_opt, query, direction, self, si),
        }
    }
}

impl From<&SearchAlgorithmConfig> for SearchAlgorithm {
    fn from(value: &SearchAlgorithmConfig) -> Self {
        match value {
            SearchAlgorithmConfig::Dijkstras {
                termination_behavior,
            } => Self::SingleSourceShortestPath {
                termination_behavior: termination_behavior.clone().unwrap_or_default(),
                a_star: false,
            },
            SearchAlgorithmConfig::AStar {
                termination_behavior,
            } => Self::SingleSourceShortestPath {
                termination_behavior: termination_behavior.clone().unwrap_or_default(),
                a_star: true,
            },
            SearchAlgorithmConfig::KspSingleVia {
                k,
                underlying,
                similarity,
                termination,
            } => {
                let underlying: Box<SearchAlgorithm> = Box::new(underlying.as_ref().into());
                Self::KspSingleVia {
                    k: *k,
                    underlying,
                    similarity: similarity.clone(),
                    termination: termination.clone(),
                }
            }
            SearchAlgorithmConfig::Yens {
                k,
                underlying,
                similarity,
                termination,
            } => {
                let underlying: Box<SearchAlgorithm> = Box::new(underlying.as_ref().into());
                Self::Yens {
                    k: *k,
                    underlying,
                    similarity: similarity.clone(),
                    termination: termination.clone(),
                }
            }
        }
    }
}

// convenience method when origin and destination are specified using
/// edge ids instead of vertex ids. invokes a vertex-oriented search
/// from the out-vertex of the source edge to the in-vertex of the
/// target edge. composes the result with the source and target.
///
/// not tested.
pub fn run_edge_oriented(
    source: (EdgeListId, EdgeId),
    target: Option<(EdgeListId, EdgeId)>,
    query: &serde_json::Value,
    direction: &Direction,
    alg: &SearchAlgorithm,
    si: &SearchInstance,
) -> Result<SearchAlgorithmResult, SearchError> {
    // 1. guard against edge conditions (src==dst, src.dst_v == dst.src_v)
    let initial_state = si.state_model.initial_state(None)?;
    let e1_src = si.graph.src_vertex_id(&source.0, &source.1)?;
    let e1_label = si
        .label_model
        .label_from_state(e1_src, &initial_state, &si.state_model)?;
    let e1_dst = si.graph.dst_vertex_id(&source.0, &source.1)?;
    let src_et = EdgeTraversal {
        edge_list_id: source.0,
        edge_id: source.1,
        cost: TraversalCost::default(),
        result_state: si.state_model.initial_state(None)?,
    };

    match target {
        None => {
            let SearchAlgorithmResult {
                mut trees,
                mut routes,
                iterations,
                terminated,
            } = alg.run_vertex_oriented(e1_dst, None, query, direction, si)?;

            let dst_label =
                si.label_model
                    .label_from_state(e1_dst, &initial_state, &si.state_model)?;

            for tree in trees.iter_mut() {
                if !tree.contains(&dst_label) {
                    tree.insert(
                        e1_label.clone(),
                        src_et.clone(),
                        dst_label.clone(),
                        si.label_model.clone(),
                    )?;
                }
            }
            for route in routes.iter_mut() {
                route.insert(0, src_et.clone());
            }
            let updated = SearchAlgorithmResult {
                trees,
                routes,
                iterations: iterations + 1,
                terminated,
            };
            Ok(updated)
        }
        Some(target_edge) => {
            let e2_src = si.graph.src_vertex_id(&target_edge.0, &target_edge.1)?;

            if source == target_edge {
                return Ok(SearchAlgorithmResult::default());
            }

            // removed specialized case that depended on creating EdgeTraversals mechanically. broken
            //   (without substantial refactor) with the inclusion of the SearchTree abstraction.

            // run a search and append source/target edges to result
            let SearchAlgorithmResult {
                trees,
                mut routes,
                iterations,
                terminated,
            } = alg.run_vertex_oriented(e1_dst, Some(e2_src), query, direction, si)?;

            if trees.is_empty() {
                return Err(SearchError::NoPathExistsBetweenVertices(e1_dst, e2_src, 0));
            }

            // it is possible that the search already found these vertices. one major edge
            // case is when the trip starts with a u-turn.
            for route in routes.iter_mut() {
                let final_state = route.last().ok_or_else(|| {
                    SearchError::InternalError(String::from("found empty result route"))
                })?;

                let dst_et = EdgeTraversal {
                    edge_list_id: target_edge.0,
                    edge_id: target_edge.1,
                    cost: TraversalCost::default(),
                    result_state: final_state.result_state.to_vec(),
                };
                route.insert(0, src_et.clone());
                route.push(dst_et.clone());
            }

            let result = SearchAlgorithmResult {
                trees,
                routes,
                iterations: iterations + 2,
                terminated,
            };
            Ok(result)
        }
    }
}
