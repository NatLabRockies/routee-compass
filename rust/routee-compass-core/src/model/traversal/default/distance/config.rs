use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::unit::DistanceUnit;

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct DistanceTraversalConfig {
    pub distance_unit: Option<DistanceUnit>,
    #[serde(default)]
    pub include_trip_distance: Option<bool>,
}
