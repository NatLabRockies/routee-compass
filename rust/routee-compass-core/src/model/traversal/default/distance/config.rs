use serde::{Deserialize, Serialize};

use crate::model::{traversal::default::fieldname, unit::DistanceUnit};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DistanceTraversalConfig {
    #[serde(default)]
    pub distance_unit: DistanceUnit,

    /// optionally configure the distance model so that the accumulated distance is
    /// not written to the state vector.
    #[serde(default = "default_include_trip_distance")]
    pub include_trip_distance: bool,

    /// optional override of the state variable where edge distance is written. by
    /// default, points to [fieldname::EDGE_DISTANCE].
    #[serde(default = "default_distance_name")]
    pub distance_name: String,

    /// optional override of the state variable where accumulated distance is written. by
    /// default, points to [fieldname::TRIP_DISTANCE].
    #[serde(default = "default_trip_distance_name")]
    pub trip_distance_name: String,
}

impl Default for DistanceTraversalConfig {
    fn default() -> Self {
        Self {
            distance_unit: Default::default(),
            include_trip_distance: default_include_trip_distance(),
            distance_name: default_distance_name(),
            trip_distance_name: default_trip_distance_name(),
        }
    }
}

fn default_distance_name() -> String {
    fieldname::EDGE_DISTANCE.to_string()
}

fn default_include_trip_distance() -> bool {
    true
}

fn default_trip_distance_name() -> String {
    fieldname::TRIP_DISTANCE.to_string()
}
