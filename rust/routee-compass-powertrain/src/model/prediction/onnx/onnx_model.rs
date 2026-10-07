use std::path::Path;
use std::sync::Mutex;

use crate::model::prediction::prediction_model::PredictionModel;

use ndarray::{ArrayD, IxDyn};
use ort::session::Session;
use routee_compass_core::model::{traversal::TraversalModelError, unit::EnergyRateUnit};

/// A prediction model backed by a single ONNX Runtime session behind a [`Mutex`].
///
/// Because [`Session::run`] requires `&mut self` while the [`PredictionModel`] trait requires
/// `Send + Sync` with an immutable `&self` receiver, the session is wrapped in a [`Mutex`].
///
/// Inputs are flattened tensors in timestep-major order, with the current
/// link last if lookback is used.
///
/// The caller is responsible for assembling and padding lookback windows.s
pub struct OnnxModel {
    session: Mutex<Session>,
    energy_rate_unit: EnergyRateUnit,
    /// expected input shape with dynamic (batch) dims resolved to 1, e.g. [1, lookback, features]
    input_shape: Vec<usize>,
}

impl PredictionModel for OnnxModel {
    fn predict(
        &self,
        feature_vector: &[f64],
    ) -> Result<(f64, EnergyRateUnit), TraversalModelError> {
        let total: usize = self.input_shape.iter().product();
        if total != feature_vector.len() {
            return Err(TraversalModelError::TraversalModelFailure(format!(
                "ONNX model expects {} values for input shape {:?} but got {}",
                total,
                self.input_shape,
                feature_vector.len()
            )));
        }

        let input_data = feature_vector.iter().map(|&value| value as f32).collect();
        let array = ArrayD::from_shape_vec(IxDyn(&self.input_shape), input_data).map_err(|e| {
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
    pub(crate) fn input_shape(&self) -> &[usize] {
        &self.input_shape
    }

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

        // resolve the expected input shape once; dynamic (batch) dims become 1
        let input_shape: Vec<usize> = {
            let input = session.inputs().iter().next().ok_or_else(|| {
                TraversalModelError::BuildError("ONNX model has no inputs".to_string())
            })?;
            let dtype = input.dtype();
            let dims = dtype.tensor_shape().ok_or_else(|| {
                TraversalModelError::BuildError(
                    "ONNX model input is not a tensor; cannot determine input shape".to_string(),
                )
            })?;

            // Ensure the dimensions of the tensor align with supported shape
            if !(2..=3).contains(&dims.len())
                || dims[0] == 0
                || dims[0] > 1
                || dims[1..].iter().any(|&dim| dim <= 0)
            {
                return Err(TraversalModelError::BuildError(format!(
                    "ONNX model requires [batch, features] or [batch, lookback, features] with batch 1 or dynamic and positive fixed remaining dimensions, got {dims:?}"
                )));
            }

            dims.iter()
                .map(|&d| if d < 0 { 1usize } else { d as usize })
                .collect()
        };
        if input_shape
            .iter()
            .try_fold(1usize, |total, &dim| total.checked_mul(dim))
            .is_none()
        {
            return Err(TraversalModelError::BuildError(
                "ONNX model input shape is too large".to_string(),
            ));
        }

        Ok(OnnxModel {
            session: Mutex::new(session),
            energy_rate_unit,
            input_shape,
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
    fn test_onnx_model_accepts_v2_rf_without_lookback() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/model/prediction/test/2016_camry_ice_rf/model.onnx");
        let model = OnnxModel::new(&path, EnergyRateUnit::GGPM).unwrap();
        assert_eq!(model.input_shape(), &[1, 2]);

        let (energy_rate, unit) = model.predict(&[50.0, 0.5]).unwrap();
        assert!(energy_rate.is_finite());
        assert_eq!(unit, EnergyRateUnit::GGPM);
        assert!(model.predict(&[]).is_err());
        assert!(model.predict(&[40.0, 0.4, 50.0, 0.5]).is_err());
    }

    #[test]
    fn test_onnx_model_accepts_complete_lookback() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/model/prediction/test/model.onnx");
        let model = OnnxModel::new(&path, EnergyRateUnit::KWHPM).unwrap();
        assert_eq!(model.input_shape(), &[1, 5, 2]);

        let features = [10.0, 0.1, 20.0, 0.2, 30.0, 0.3, 40.0, 0.4, 50.0, 0.5];
        let (energy_rate, unit) = model.predict(&features).unwrap();
        assert!(energy_rate.is_finite());
        assert_eq!(unit, EnergyRateUnit::KWHPM);

        // the tensor as an array with lookback
        let array = ndarray::arr3(&[[
            [10.0f32, 0.1],
            [20.0, 0.2],
            [30.0, 0.3],
            [40.0, 0.4],
            [50.0, 0.5],
        ]]);
        let tensor = ort::value::Value::from_array(array).unwrap();
        let mut session = model.session.lock().unwrap();
        let outputs = session.run(ort::inputs!["input" => tensor]).unwrap();
        let expected = outputs[0].try_extract_tensor::<f32>().unwrap().1[0] as f64;
        assert_eq!(energy_rate, expected);

        for invalid in [&features[..2], &features[..9], &features[..0]] {
            let error = model.predict(invalid).unwrap_err();
            assert!(error.to_string().contains("expects 10 values"));
        }
        assert!(model.predict(&[0.0; 12]).is_err());
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
