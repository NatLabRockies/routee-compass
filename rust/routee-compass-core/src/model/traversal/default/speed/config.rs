use crate::model::{traversal::default::fieldname, unit::SpeedUnit};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SpeedModelConfig {
    /// file containing speed values for each edge id
    pub speed_table_input_file: String,

    /// unit the speeds were recorded in
    pub speed_unit: SpeedUnit,

    /// optional override of the state variable providing edge distance. by
    /// default, points to [fieldname::EDGE_DISTANCE].
    #[serde(default = "default_distance_name")]
    pub distance_name: String,

    /// optional override of the state variable where speed is written. by
    /// default, points to [fieldname::EDGE_SPEED].
    #[serde(default = "default_speed_name")]
    pub speed_name: String,
}

fn default_distance_name() -> String {
    fieldname::EDGE_DISTANCE.to_string()
}

fn default_speed_name() -> String {
    fieldname::EDGE_SPEED.to_string()
}
