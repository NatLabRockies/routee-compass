use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::interpolation::feature_bounds::FeatureBounds;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Smartcore,
    Onnx {
        pool_size: Option<usize>,
    },
    Interpolate {
        underlying_model_type: Box<ModelType>,
        feature_bounds: HashMap<String, FeatureBounds>,
    },
}

/// Custom deserializer that accepts both `"onnx"` (as a unit variant with default pool_size)
/// and `{"onnx": {"pool_size": N}}` (as a struct variant).
impl<'de> Deserialize<'de> for ModelType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Use an untagged helper enum to try both forms
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Helper {
            Smartcore,
            Onnx(OnnxHelper),
            Interpolate {
                underlying_model_type: Box<ModelType>,
                feature_bounds: HashMap<String, FeatureBounds>,
            },
        }

        // Accept either a struct with pool_size or nothing (unit variant)
        #[derive(Deserialize)]
        struct OnnxHelper {
            pool_size: Option<usize>,
        }

        // Uses an untagged wrapper enum (Outer) that tries two representations in order:
        // 1. Tagged(Helper) — handles externally-tagged forms like "smartcore" and {"onnx": {...}}
        // 2. BareOnnx — handles "onnx" as a bare string, which the tagged form rejects
        //    since Onnx expects a struct body.
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        #[serde(untagged)]
        enum Outer {
            Tagged(Helper),
            BareOnnx(BareOnnxTag),
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum BareOnnxTag {
            Smartcore,
            Onnx,
        }

        let outer = Outer::deserialize(deserializer)?;
        match outer {
            Outer::Tagged(h) => match h {
                Helper::Smartcore => Ok(ModelType::Smartcore),
                Helper::Onnx(o) => Ok(ModelType::Onnx {
                    pool_size: o.pool_size,
                }),
                Helper::Interpolate {
                    underlying_model_type,
                    feature_bounds,
                } => Ok(ModelType::Interpolate {
                    underlying_model_type,
                    feature_bounds,
                }),
            },
            Outer::BareOnnx(tag) => match tag {
                BareOnnxTag::Smartcore => Ok(ModelType::Smartcore),
                BareOnnxTag::Onnx => Ok(ModelType::Onnx { pool_size: None }),
            },
        }
    }
}

impl std::fmt::Display for ModelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_onnx_bare_string() {
        let json = r#""onnx""#;
        let model_type: ModelType = serde_json::from_str(json).unwrap();
        assert!(matches!(model_type, ModelType::Onnx { pool_size: None }));
    }

    #[test]
    fn test_deserialize_onnx_with_pool_size() {
        let json = r#"{"onnx": {"pool_size": 4}}"#;
        let model_type: ModelType = serde_json::from_str(json).unwrap();
        assert!(matches!(model_type, ModelType::Onnx { pool_size: Some(4) }));
    }

    #[test]
    fn test_deserialize_onnx_with_null_pool_size() {
        let json = r#"{"onnx": {"pool_size": null}}"#;
        let model_type: ModelType = serde_json::from_str(json).unwrap();
        assert!(matches!(model_type, ModelType::Onnx { pool_size: None }));
    }

    #[test]
    fn test_deserialize_smartcore() {
        let json = r#""smartcore""#;
        let model_type: ModelType = serde_json::from_str(json).unwrap();
        assert!(matches!(model_type, ModelType::Smartcore));
    }

    #[test]
    fn test_deserialize_interpolate() {
        let json = r#"{"interpolate": {"underlying_model_type": "onnx", "feature_bounds": {}}}"#;
        let model_type: ModelType = serde_json::from_str(json).unwrap();
        assert!(matches!(model_type, ModelType::Interpolate { .. }));
    }
}
