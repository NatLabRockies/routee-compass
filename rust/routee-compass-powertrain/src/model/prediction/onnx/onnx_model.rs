use std::path::Path;
use std::sync::Mutex;

use crate::model::prediction::prediction_model::PredictionModel;

use ndarray::Array2;
use ort::session::Session;
use routee_compass_core::model::{traversal::TraversalModelError, unit::EnergyRateUnit};

/// A prediction model backed by a single ONNX Runtime session behind a [`Mutex`].
///
/// Because [`Session::run`] requires `&mut self` while the [`PredictionModel`] trait requires
/// `Send + Sync` with an immutable `&self` receiver, the session is wrapped in a [`Mutex`].
///
/// This model is currently only used as a feeder to build the interpolation grid in
/// [`InterpolationModel`](crate::model::prediction::interpolation::InterpolationModel).
/// It is not intended for direct use in parallel search. If used with a large number of
/// threads, the single mutex-guarded session will serialize all inference calls and may
/// cause performance issues.
pub struct OnnxModel {
    session: Mutex<Session>,
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

        let mut session = self.session.lock().map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!(
                "Failed to lock ONNX session: {}",
                e
            ))
        })?;

        let outputs = session
            .run(ort::inputs!["input" => input_tensor])
            .map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "Failed to run ONNX model: {}",
                    e
                ))
            })?;

        let (_output_name, output_tensor) = outputs.iter().next().ok_or_else(|| {
            TraversalModelError::TraversalModelFailure("ONNX model returned no outputs".to_string())
        })?;
        let tensor_data = output_tensor.try_extract_tensor::<f32>().map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!(
                "Failed to extract output tensor: {}",
                e
            ))
        })?;

        let energy_rate = *tensor_data.1.first().ok_or_else(|| {
            TraversalModelError::TraversalModelFailure("ONNX output tensor is empty".to_string())
        })? as f64;

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

        let session = Session::builder()
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
            session: Mutex::new(session),
            energy_rate_unit,
        })
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;
    use crate::model::prediction::prediction_model::PredictionModel;

    fn model_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("test")
            .join("Toyota_Camry.onnx")
    }

    #[test]
    fn test_onnx_model_predicts_energy_rate() {
        let model = OnnxModel::new(&model_path(), EnergyRateUnit::GGPM).unwrap();

        // Predict energy rate at 50 mph, 0% grade
        let (energy_rate, unit) = model.predict(&[50.0, 0.0]).unwrap();

        assert_eq!(unit, EnergyRateUnit::GGPM);

        // Energy rate should be between 28-32 mpg (i.e. 1/32 to 1/28 gallons per mile)
        let expected_lower = 1.0 / 32.0;
        let expected_upper = 1.0 / 28.0;
        assert!(
            energy_rate >= expected_lower && energy_rate <= expected_upper,
            "energy_rate {} not in expected range [{}, {}]",
            energy_rate,
            expected_lower,
            expected_upper,
        );
    }

    #[test]
    fn test_onnx_model_uphill_uses_more_energy() {
        let model = OnnxModel::new(&model_path(), EnergyRateUnit::GGPM).unwrap();

        let (flat_rate, _) = model.predict(&[50.0, 0.0]).unwrap();
        let (uphill_rate, _) = model.predict(&[50.0, 0.05]).unwrap();

        assert!(
            uphill_rate > flat_rate,
            "expected uphill rate {} > flat rate {}",
            uphill_rate,
            flat_rate,
        );
    }

    #[test]
    fn test_onnx_model_rejects_non_onnx_file() {
        let bad_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("test")
            .join("Toyota_Camry.bin");

        let result = OnnxModel::new(&bad_path, EnergyRateUnit::GGPM);
        assert!(result.is_err());
    }
}
