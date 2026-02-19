use crate::model::unit::RatioUnit;
use serde::{Deserialize, Serialize};

/// provides configuration for instantiating the grade engine used in grade modeling.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GradeConfiguration {
    /// file with dense mapping from edge id to grade value
    pub grade_input_file: String,
    /// type of grade values in file
    pub grade_unit: RatioUnit,
}
