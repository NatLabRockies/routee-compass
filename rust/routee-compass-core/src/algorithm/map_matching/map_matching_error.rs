use thiserror::Error;

use crate::algorithm::search::SearchError;
use crate::model::map::MapError;

/// Error types for map matching operations.
#[derive(Error, Debug)]
pub enum MapMatchingError {
    #[error("no points provided in trace")]
    EmptyTrace,

    #[error("failed to match point at index {index}: {message}")]
    PointMatchFailed { index: usize, message: String },

    #[error("failed to compute path between matched points: {0}")]
    PathComputationFailed(String),

    #[error("map error: {source}")]
    MapError {
        #[from]
        source: MapError,
    },

    #[error("search error: {source}")]
    SearchError {
        #[from]
        source: SearchError,
    },

    #[error("internal error: {0}")]
    InternalError(String),
}
