use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RoadClassBuilderConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub road_class_input_file: String,
}
