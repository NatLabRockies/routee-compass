use routee_compass_core::algorithm::map_matching::MapMatchingError;
use thiserror::Error;

use crate::app::compass::CompassAppError;

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

    #[error("compass app error: {source}")]
    CompassError {
        #[from]
        source: CompassAppError,
    },

    #[error("JSON error: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
}
