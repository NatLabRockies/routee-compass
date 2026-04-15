use crate::algorithm::search::SearchTreeError;

#[derive(thiserror::Error, Debug)]
pub enum ConstraintModelError {
    #[error("failure building constraint model: {0}")]
    BuildError(String),
    #[error("{0}")]
    ConstraintModelError(String),
    #[error("failure running constraint model due to search tree: {source}")]
    SearchTreeError {
        #[from]
        source: SearchTreeError,
    },
}
