

#[derive(thiserror::Error, Debug)]
pub enum MapMatchingError {
    #[error("failure building map matching algorithm: {0}")]
    BuildError(String),
    #[error("Could not match trace due to: {0}")]
    MatchError(String),
    #[error("internal map matching error error due to: {0}")]
    InternalError(String),
}
