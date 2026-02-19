use super::TurnDelayModelConfig;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TurnDelayTraversalConfig {
    pub edge_heading_input_file: String,
    pub turn_delay_model: TurnDelayModelConfig,
    pub include_trip_time: Option<bool>,
}
