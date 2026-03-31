use crate::algorithm::search::SearchGraph;

/// the result of running a [`super::SearchAlgorithm`].
#[derive(Default)]
pub struct SearchResult {
    /// the tree created by the search algorithm
    pub tree: SearchGraph,
    /// number of iterations run to create this tree
    pub iterations: u64,
    /// if present, a message explaining a forced termination of the search.
    /// if not present, the search terminated naturally by reaching an
    /// empty frontier state.
    pub terminated: Option<String>,
}

impl SearchResult {
    /// create a [`SearchResult`] for a search that completed, aka, which reached
    /// an empty frontier state.
    pub fn completed(tree: SearchGraph, iterations: u64) -> SearchResult {
        SearchResult {
            tree,
            iterations,
            terminated: None,
        }
    }

    /// create a [`SearchResult`] for a search that was forced to terminate by the
    /// [`crate::model::termination::TerminationModel`]. include a message explaining the
    /// reason it was terminated.
    pub fn terminated(tree: SearchGraph, iterations: u64, explanation: String) -> SearchResult {
        SearchResult {
            tree,
            iterations,
            terminated: Some(explanation),
        }
    }
}
