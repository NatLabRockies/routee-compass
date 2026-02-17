use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct VehicleRestrictionConfig {
    /// CSV file containing rows of [super::RestrictionRow] values
    pub vehicle_restriction_input_file: String,
}
