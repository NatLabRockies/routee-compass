use crate::model::{label::Label, state::StateModelError};

#[derive(thiserror::Error, Debug, Clone)]
pub enum LabelModelError {
    #[error("{0}")]
    LabelModelError(String),
    #[error("attempting to build label state array of size {0} which is greater than max of {1} elements")]
    BadLabelVecSize(usize, usize),
    #[error("found two labels with unmatched types: '{0}' '{1}'")]
    MismatchLabelTypes(Label, Label),
    #[error("failure building label due to search state: {source}")]
    StateError {
        #[from]
        source: StateModelError,
    },
}
