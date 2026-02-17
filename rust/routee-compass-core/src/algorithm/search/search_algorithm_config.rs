use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::algorithm::search::{
    ksp::KspTerminationCriteria, util::RouteSimilarityFunction, TerminationFailurePolicy,
};

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SearchAlgorithmConfig {
    /// Edgard Dijkstra's breadth first search algorithm, implemented as
    /// A* with a zero-valued cost estimate function.
    Dijkstras {
        /// optional argument to define handling of terminated searches
        termination_behavior: Option<TerminationFailurePolicy>,
    },
    /// Classic best-first search algorithm.
    #[serde(rename = "a*")]
    AStar {
        /// optional argument to define handling of terminated searches
        termination_behavior: Option<TerminationFailurePolicy>,
    },
    /// K-shortest paths algorithm that relies on a novel bidirectional search algorithm
    /// combined with a map-algebraic heuristic to identify midpoints on approximate ksp
    /// paths.
    /// Taken from the paper Häcker, Christian, et al. "Most diverse near-shortest paths."
    /// Proceedings of the 29th International Conference on Advances in
    /// Geographic Information Systems. 2021.
    #[serde(rename = "svp")]
    KspSingleVia {
        /// number of alternative paths to attempt
        k: usize,
        /// path search algorithm to use
        underlying: Box<SearchAlgorithmConfig>,
        /// if provided, filters out potential solution paths based on their
        /// similarity to the paths in the stored result set
        similarity: Option<RouteSimilarityFunction>,
        /// termination criteria for the inner path search function
        termination: Option<KspTerminationCriteria>,
    },
    /// K-shortest paths algorithm that relies on successive edge cuts to find alternatives
    /// to the true shortest path. Taken from the paper Yen, Jin Y. "An algorithm for finding
    /// shortest routes from all source nodes to a given destination in general networks."
    /// Quarterly of applied mathematics 27.4 (1970): 526-530.
    Yens {
        /// number of alternative paths to attempt
        k: usize,
        /// path search algorithm to use
        underlying: Box<SearchAlgorithmConfig>,
        /// if provided, filters out potential solution paths based on their
        /// similarity to the paths in the stored result set
        similarity: Option<RouteSimilarityFunction>,
        /// termination criteria for the inner path search function
        termination: Option<KspTerminationCriteria>,
    },
}
