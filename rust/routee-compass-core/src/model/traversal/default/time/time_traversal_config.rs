use crate::model::unit::TimeUnit;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TimeTraversalConfig {
    /// time unit for state modeling
    pub time_unit: TimeUnit,
    #[serde(default)]
    pub include_trip_time: Option<bool>,
}
