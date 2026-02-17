use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct TurnRestrictionConstraintConfig {
    /// CSV file containing turn restrictions. matches [super::Turn]
    pub turn_restriction_input_file: String
}