use crate::algorithm::search::SearchTreeError;
use crate::model::network::NetworkError;
use crate::model::state::StateModelError;
use crate::model::unit::UnitError;

#[derive(thiserror::Error, Debug)]
pub enum TraversalModelError {
    #[error("failure building traversal model: {0}")]
    BuildError(String),
    #[error("{0}")]
    TraversalModelFailure(String),
    #[error("internal error: {0}")]
    InternalError(String),
    #[error("failure executing traversal model due to numeric units: {source}")]
    UnitsFailure {
        #[from]
        source: UnitError,
    },
    #[error("failure executing traversal model due to network: {source}")]
    NetworkFailure {
        #[from]
        source: NetworkError,
    },
    #[error("failure executing traversal model due to search state: {source}")]
    StateError {
        #[from]
        source: StateModelError,
    },
    #[error("failure executing traversal model due to search tree: {source}")]
    TreeError {
        #[from]
        source: SearchTreeError,
    },
}
