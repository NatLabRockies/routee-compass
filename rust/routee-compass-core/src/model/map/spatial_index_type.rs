//! configuration value to declare the type of [`super::SpatialIndex`] to build.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub enum SpatialIndexType {
    #[default]
    #[serde(rename = "vertex")]
    VertexOriented,
    #[serde(rename = "edge")]
    EdgeOriented,
}
