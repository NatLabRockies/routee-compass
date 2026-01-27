use routee_compass_core::algorithm::map_matching::MapMatchingError;
use thiserror::Error;

/// Error types for the map matching application layer.
#[derive(Error, Debug)]
pub enum MapMatchingAppError {
    #[error("failed to build map matching app: {0}")]
    BuildFailure(String),

    #[error("map matching algorithm error: {source}")]
    AlgorithmError {
        #[from]
        source: MapMatchingError,
    },

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("JSON error: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
}
