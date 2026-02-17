use crate::model::unit::TimeUnit;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct TimeTraversalConfig {
    /// time unit for state modeling
    pub time_unit: TimeUnit,
    #[serde(default)]
    pub include_trip_time: Option<bool>,
}
