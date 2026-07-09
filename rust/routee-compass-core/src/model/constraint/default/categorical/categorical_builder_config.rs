use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
/// The configuration arguments to be passed into the CategoricalModelBuilder.
pub struct CategoricalModelBuilderConfig {
    pub key: String, // The type of categorical constraint service (e.g., "road_class") to build
    pub input_file: String, // The path to the input file for the constraint service
}
