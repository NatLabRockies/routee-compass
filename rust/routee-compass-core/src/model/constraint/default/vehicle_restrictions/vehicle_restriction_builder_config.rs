use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct VehicleRestrictionBuilderConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub vehicle_restriction_input_file: String,
}
