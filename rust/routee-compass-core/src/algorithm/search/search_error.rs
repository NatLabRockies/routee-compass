use crate::{
    algorithm::search::SearchGraphError,
    model::{
        constraint::ConstraintModelError,
        cost::CostModelError,
        label::label_model_error::LabelModelError,
        network::{EdgeId, NetworkError, VertexId},
        state::StateModelError,
        termination::TerminationModelError,
        traversal::TraversalModelError,
    },
};

#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("failure building search algorithm: {0}")]
    BuildError(String),
    #[error("The search failed due to a label setting error: {source}")]
    LabelFailure {
        #[from]
        source: LabelModelError,
    },
    #[error("The search failed due to state model error. The state model is responsible for updates to the state of the search at each increment of traversal. Please review the [state] section of your Compass configuration. Source: {source}")]
    StateFailure {
        #[from]
        source: StateModelError,
    },
    #[error("The search failed due with an internal error due to a malformed search tree. Source: {source}")]
    SearchGraphError {
        #[from]
        source: SearchGraphError,
    },
    #[error("The search failed due to a road network error. Please review the [graph] section of your Compass configuration. Source: {source}")]
    NetworkFailure {
        #[from]
        source: NetworkError,
    },
    #[error("The search failed due to termination model error. Please review the [termination] section of your Compass Configuration to make changes. Source: {source}")]
    TerminationModelFailure {
        #[from]
        source: TerminationModelError,
    },
    #[error("The search failed due to traversal model error. The traversal model performs edge traversals by updating the various search dimensions, based on the state and cost models. Please review the [traversal] section of your Compass Configuration. Source: {source}")]
    TraversalModelFailure {
        #[from]
        source: TraversalModelError,
    },
    #[error("The search failed due to constraint model error. The constraint model restricts access to edges. Please review the [constraint] section of your Compass Configuration. Source: {source}")]
    ConstraintModelFailure {
        #[from]
        source: ConstraintModelError,
    },
    #[error("The search failed due to cost model error. The cost model interprets a delta of search state dimensions as having a cost value, which is minimized by the search. Please see the [cost] section of your Compass Configuration and additionally any query-time overrides. Source: {source}")]
    CostFailure {
        #[from]
        source: CostModelError,
    },
    #[error("query terminated due to {0}")]
    QueryTerminated(String),
    #[error("no path exists between vertices {0} and {1} after searching {2} edges")]
    NoPathExistsBetweenVertices(VertexId, VertexId, usize),
    #[error("no path exists between edges {0} and {1} after searching {2} edges")]
    NoPathExistsBetweenEdges(EdgeId, EdgeId, usize),
    #[error("error accessing shared read-only dataset: {0}")]
    ReadOnlyPoisonError(String),
    #[error("internal error due to search logic: {0}")]
    InternalError(String),
}
