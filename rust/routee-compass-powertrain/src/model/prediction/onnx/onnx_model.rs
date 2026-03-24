use std::path::Path;
use std::sync::Mutex;

use crate::model::prediction::prediction_model::PredictionModel;

use ndarray::Array2;
use ort::session::Session;
use routee_compass_core::model::{traversal::TraversalModelError, unit::EnergyRateUnit};

pub struct OnnxModel {
    model: Mutex<Session>,
    energy_rate_unit: EnergyRateUnit,
}

impl PredictionModel for OnnxModel {
    fn predict(
        &self,
        feature_vector: &[f64],
    ) -> Result<(f64, EnergyRateUnit), TraversalModelError> {
        let input_shape = (1, feature_vector.len());
        let input_data: Vec<f32> = feature_vector.iter().map(|&v| v as f32).collect();
        let array = Array2::from_shape_vec(input_shape, input_data).map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!(
                "Failed to create ndarray from feature vector: {}",
                e
            ))
        })?;

        let input_tensor = ort::value::Value::from_array(array).map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!(
                "Failed to create ONNX tensor: {}",
                e
            ))
        })?;

        let mut model = self.model.lock().map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!("Failed to lock ONNX model: {}", e))
        })?;

        let outputs = model
            .run(ort::inputs!["input" => input_tensor])
            .map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "Failed to run ONNX model: {}",
                    e
                ))
            })?;

        let output_tensor = &outputs[0];
        let tensor_data = output_tensor.try_extract_tensor::<f32>().map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!(
                "Failed to extract output tensor: {}",
                e
            ))
        })?;

        let energy_rate = tensor_data.1[0] as f64;
        Ok((energy_rate, self.energy_rate_unit))
    }
}

impl OnnxModel {
    pub fn new<P: AsRef<Path>>(
        routee_model_path: &P,
        energy_rate_unit: EnergyRateUnit,
    ) -> Result<Self, TraversalModelError> {
        // make sure we have an .onnx file
        let is_onnx = routee_model_path
            .as_ref()
            .extension()
            .map(|ext| ext == "onnx")
            .unwrap_or(false);

        if !is_onnx {
            return Err(TraversalModelError::BuildError(format!(
                "OnnxModel expected an .onnx file, got {}",
                routee_model_path.as_ref().to_string_lossy()
            )));
        }

        let model = Session::builder()
            .map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Failed to build ONNXRuntime session from {} due to: {}",
                    routee_model_path.as_ref().to_string_lossy(),
                    e,
                ))
            })?
            .commit_from_file(routee_model_path)
            .map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Failed to build ONNXRuntime session from {} due to: {}",
                    routee_model_path.as_ref().to_string_lossy(),
                    e,
                ))
            })?;

        Ok(OnnxModel {
            model: Mutex::new(model),
            energy_rate_unit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_onnx_predict() {
        let model_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("python/nrel/routee/compass/resources/models/camry_4cyl_2wd/2016/default/grade_percent_speed_mph/v1/model.onnx");

        if !model_path.exists() {
            println!("Test skipped: model file not found at {:?}", model_path);
            return;
        }

        let model = OnnxModel::new(&model_path, EnergyRateUnit::GGPM).unwrap();
        let feature_vector = vec![60.0, 0.0]; // 60 mph, 0% grade
        let (energy_rate, unit) = model.predict(&feature_vector).unwrap();

        assert!(energy_rate > 0.0);
        assert_eq!(unit, EnergyRateUnit::GGPM);
        println!("Energy rate at 60 mph, 0% grade: {} {}", energy_rate, unit);
    }
}
