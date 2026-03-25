use std::path::Path;
use std::sync::Mutex;

use crate::model::prediction::prediction_model::PredictionModel;

use ndarray::Array2;
use ort::session::Session;
use routee_compass_core::model::{traversal::TraversalModelError, unit::EnergyRateUnit};

/// A prediction model backed by one or more ONNX Runtime sessions.
///
/// Because [`Session::run`] requires `&mut self` while the [`PredictionModel`] trait requires
/// `Send + Sync` with an immutable `&self` receiver, each session is wrapped in a [`Mutex`].
/// When constructed with `pool_size > 1`, multiple sessions are loaded from the same model file
/// and [`predict`](PredictionModel::predict) acquires whichever session is available first,
/// enabling parallel inference across threads.
///
/// When used as the underlying model for
/// [`InterpolationModel`](crate::model::prediction::interpolation::InterpolationModel),
/// a pool size of 1 is sufficient since grid building is sequential.
pub struct OnnxModel {
    sessions: Vec<Mutex<Session>>,
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

        // Try to find an unlocked session; if all are busy, block on the first.
        let mut model = None;
        for session in &self.sessions {
            if let Ok(guard) = session.try_lock() {
                model = Some(guard);
                break;
            }
        }
        let mut model = match model {
            Some(guard) => guard,
            None => self.sessions[0].lock().map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "Failed to lock ONNX model: {}",
                    e
                ))
            })?,
        };

        let outputs = model
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
        pool_size: usize,
    ) -> Result<Self, TraversalModelError> {
        let pool_size = pool_size.max(1);

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

        let mut sessions = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
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
            sessions.push(Mutex::new(session));
        }

        Ok(OnnxModel {
            sessions,
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
        let model = OnnxModel::new(&model_path(), EnergyRateUnit::GGPM, 1).unwrap();

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
        let model = OnnxModel::new(&model_path(), EnergyRateUnit::GGPM, 1).unwrap();

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
    fn test_onnx_model_pool_size() {
        let model = OnnxModel::new(&model_path(), EnergyRateUnit::GGPM, 4).unwrap();
        assert_eq!(model.sessions.len(), 4);

        // Should still produce the same result
        let (energy_rate, _) = model.predict(&[50.0, 0.0]).unwrap();
        assert!(energy_rate > 0.0);
    }

    #[test]
    fn test_onnx_model_rejects_non_onnx_file() {
        let bad_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("test")
            .join("Toyota_Camry.bin");

        let result = OnnxModel::new(&bad_path, EnergyRateUnit::GGPM, 1);
        assert!(result.is_err());
    }
}
