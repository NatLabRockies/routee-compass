use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::interpolation::feature_bounds::FeatureBounds;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Smartcore,
    Onnx,
    Interpolate {
        underlying_model_type: Box<ModelType>,
        feature_bounds: HashMap<String, FeatureBounds>,
    },
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{s}")
    }
}
