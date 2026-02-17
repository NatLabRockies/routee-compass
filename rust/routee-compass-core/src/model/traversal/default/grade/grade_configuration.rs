use crate::model::unit::RatioUnit;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// provides configuration for instantiating the grade engine used in grade modeling.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct GradeConfiguration {
    /// file with dense mapping from edge id to grade value
    pub grade_input_file: String,
    /// type of grade values in file
    pub grade_unit: RatioUnit,
}
