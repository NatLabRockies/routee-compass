use serde::{Deserialize, Serialize};

use crate::model::unit::DistanceUnit;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DistanceTraversalConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub distance_unit: Option<DistanceUnit>,
    #[serde(default)]
    pub include_trip_distance: Option<bool>,
}
