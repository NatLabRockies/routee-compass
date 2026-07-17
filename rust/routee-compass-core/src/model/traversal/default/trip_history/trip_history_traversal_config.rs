use crate::model::state::StateVariableConfig;
use serde::{Deserialize, Serialize};

/// The configuration required to build the `TripHistoryTraversalService`
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TripHistoryConfig {
    /// the features on the present link that will be collected in the history.  the values
    /// collected by other traversal models, stored into the state vector, that will be `shifted` into
    /// slots representing past links as the links are traversed
    pub input_features: Vec<StateVariableConfig>,
    /// how deep into the past link attributes will be collected.
    pub depth: std::num::NonZeroUsize,
}
