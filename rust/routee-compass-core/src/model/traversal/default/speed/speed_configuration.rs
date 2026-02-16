use crate::model::unit::SpeedUnit;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SpeedConfiguration {
    #[serde(rename = "type")]
    pub r#type: String,
    /// file containing speed values for each edge id
    pub speed_table_input_file: String,
    /// unit the speeds were recorded in
    pub speed_unit: SpeedUnit,
}
