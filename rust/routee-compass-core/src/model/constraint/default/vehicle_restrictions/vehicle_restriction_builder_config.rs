use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct VehicleRestrictionBuilderConfig {
    pub vehicle_restriction_input_file: String,
}
