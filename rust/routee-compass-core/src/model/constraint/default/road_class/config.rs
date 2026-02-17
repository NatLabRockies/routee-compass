use schemars::JsonSchema;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct RoadClassConstraintConfig {
    /// file containing class labels by edge id. each row index 
    /// corresponds to the EdgeId index.
    pub road_class_input_file: String
}