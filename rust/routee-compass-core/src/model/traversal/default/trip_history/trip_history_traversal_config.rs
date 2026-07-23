use crate::model::state::StateVariableConfig;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct HistoryFeature {
    pub name: String,
    pub state_variable_config: StateVariableConfig,
}
/// The configuration required to build the `TripHistoryTraversalService`
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TripHistoryTraversalConfig {
    /// the features on the present link that will be collected in the history.  the values
    /// collected by other traversal models, stored into the state vector, that will be `shifted` into
    /// slots representing past links as the links are traversed
    pub history_features: Vec<HistoryFeature>,
    /// how deep into the past link attributes will be collected.
    pub depth: std::num::NonZeroUsize,
}
