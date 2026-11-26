use crate::model::{traversal::default::fieldname, unit::TimeUnit};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TimeTraversalConfig {
    /// optional override of the state variable providing edge distance. by
    /// default, points to [fieldname::EDGE_DISTANCE].
    #[serde(default = "default_distance_name")]
    pub distance_name: String,

    /// optional override of the state variable providing edge speed. by
    /// default, points to [fieldname::EDGE_SPEED].
    #[serde(default = "default_speed_name")]
    pub speed_name: String,

    /// optional override of the state variable where time is written. by
    /// default, points to [fieldname::EDGE_TIME].
    #[serde(default = "default_time_name")]
    pub time_name: String,

    /// optional override of the state variable where accumulated time is written. by
    /// default, points to [fieldname::TRIP_TIME].
    #[serde(default = "default_trip_time_name")]
    pub trip_time_name: String,

    /// time unit for values written to the state vector.
    pub time_unit: TimeUnit,

    /// if true, write [fieldname::TRIP_TIME] values to the state vector (default is TRUE).
    #[serde(default = "default_include_trip_time")]
    pub include_trip_time: bool,
}

fn default_distance_name() -> String {
    fieldname::EDGE_DISTANCE.to_string()
}

fn default_speed_name() -> String {
    fieldname::EDGE_SPEED.to_string()
}

fn default_time_name() -> String {
    fieldname::EDGE_TIME.to_string()
}

fn default_trip_time_name() -> String {
    fieldname::TRIP_TIME.to_string()
}

fn default_include_trip_time() -> bool {
    true
}
