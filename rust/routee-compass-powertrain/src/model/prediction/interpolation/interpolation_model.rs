use super::{feature_bounds::FeatureBounds, utils::linspace};
use crate::model::prediction::{
    model_type::ModelType, onnx::onnx_model::OnnxModel, prediction_model::PredictionModel,
    smartcore::SmartcoreModel,
};
use itertools::Itertools;
use ndarray::{Array1, Array2, Array3, ArrayD, IxDyn};
use ninterp::prelude::*;
use routee_compass_core::model::{
    state::InputFeature, traversal::TraversalModelError, unit::EnergyRateUnit,
};
use std::{collections::HashMap, path::Path};

/// Enum to hold different interpolator types based on dimensionality
enum InterpolatorVariant {
    One(Interp1DOwned<f64, strategy::Linear>),
    Two(Interp2DOwned<f64, strategy::Linear>),
    Three(Interp3DOwned<f64, strategy::Linear>),
    N(InterpNDOwned<f64, strategy::Linear>),
}

impl InterpolatorVariant {
    fn interpolate(&self, point: &[f64]) -> Result<f64, ninterp::error::InterpolateError> {
        match self {
            InterpolatorVariant::One(interp) => interp.interpolate(point),
            InterpolatorVariant::Two(interp) => interp.interpolate(point),
            InterpolatorVariant::Three(interp) => interp.interpolate(point),
            InterpolatorVariant::N(interp) => interp.interpolate(point),
        }
    }
}

pub struct InterpolationModel {
    interpolator: InterpolatorVariant,
    energy_rate_unit: EnergyRateUnit,
}

impl PredictionModel for InterpolationModel {
    fn predict(
        &self,
        feature_vector: &[f64],
    ) -> Result<(f64, EnergyRateUnit), TraversalModelError> {
        let y = self.interpolator.interpolate(feature_vector).map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!(
                "Failed to interpolate model output during prediction: {e}"
            ))
        })?;

        let energy_rate = y;
        Ok((energy_rate, self.energy_rate_unit))
    }
}

impl InterpolationModel {
    pub fn new<P: AsRef<Path>>(
        underlying_model_path: &P,
        underlying_model_type: ModelType,
        input_features: Vec<InputFeature>,
        feature_bounds: HashMap<String, FeatureBounds>,
        energy_rate_unit: EnergyRateUnit,
    ) -> Result<Self, TraversalModelError> {
        let num_features = input_features.len();

        if num_features == 0 {
            return Err(TraversalModelError::BuildError(
                "InterpolationModel requires at least one input feature".to_string(),
            ));
        }

        // Load underlying model to build the interpolation grid
        let model: Box<dyn PredictionModel> = match underlying_model_type {
            ModelType::Smartcore => Box::new(SmartcoreModel::new(
                underlying_model_path,
                energy_rate_unit,
            )?),
            ModelType::Onnx => Box::new(OnnxModel::new(underlying_model_path, energy_rate_unit)?),
            _ => {
                return Err(TraversalModelError::TraversalModelFailure(
                    "Got unexpected model type when building the interpolation model".to_string(),
                ))
            }
        };

        // Build the grid for all features
        let mut grid: Vec<ndarray::Array1<f64>> = Vec::new();

        for input_feature in input_features.iter() {
            let feature_name = input_feature.name();
            let feature_bounds = feature_bounds.get(&feature_name).ok_or_else(|| {
                TraversalModelError::BuildError(format!(
                    "Missing feature bounds for {feature_name}, got: {feature_bounds:?}"
                ))
            })?;

            let feature_grid = ndarray::Array1::from_vec(linspace(
                feature_bounds.lower_bound,
                feature_bounds.upper_bound,
                feature_bounds.num_bins,
            ));
            grid.push(feature_grid);
        }

        // Build the appropriate interpolator based on dimensionality
        let interpolator = match num_features {
            1 => {
                let grid_0 = grid[0].clone();
                let values = Self::build_values_1d(model.as_ref(), &grid_0)?;
                let interp = Interp1D::new(grid_0, values, strategy::Linear, Extrapolate::Clamp)
                    .map_err(|e| {
                        TraversalModelError::TraversalModelFailure(format!(
                            "Failed to validate 1D interpolation model: {e}"
                        ))
                    })?;
                InterpolatorVariant::One(interp)
            }
            2 => {
                let grid_0 = grid[0].clone();
                let grid_1 = grid[1].clone();
                let values = Self::build_values_2d(model.as_ref(), &grid_0, &grid_1)?;
                let interp =
                    Interp2D::new(grid_0, grid_1, values, strategy::Linear, Extrapolate::Clamp)
                        .map_err(|e| {
                            TraversalModelError::TraversalModelFailure(format!(
                                "Failed to validate 2D interpolation model: {e}"
                            ))
                        })?;
                InterpolatorVariant::Two(interp)
            }
            3 => {
                let grid_0 = grid[0].clone();
                let grid_1 = grid[1].clone();
                let grid_2 = grid[2].clone();
                let values = Self::build_values_3d(model.as_ref(), &grid_0, &grid_1, &grid_2)?;
                let interp = Interp3D::new(
                    grid_0,
                    grid_1,
                    grid_2,
                    values,
                    strategy::Linear,
                    Extrapolate::Clamp,
                )
                .map_err(|e| {
                    TraversalModelError::TraversalModelFailure(format!(
                        "Failed to validate 3D interpolation model: {e}"
                    ))
                })?;
                InterpolatorVariant::Three(interp)
            }
            _ => {
                let values = Self::build_values_nd(model.as_ref(), &grid)?;
                let interp = InterpND::new(grid, values, strategy::Linear, Extrapolate::Clamp)
                    .map_err(|e| {
                        TraversalModelError::TraversalModelFailure(format!(
                            "Failed to validate N-D interpolation model: {e}"
                        ))
                    })?;
                InterpolatorVariant::N(interp)
            }
        };

        Ok(InterpolationModel {
            interpolator,
            energy_rate_unit,
        })
    }

    fn build_values_1d(
        model: &dyn PredictionModel,
        grid_0: &Array1<f64>,
    ) -> Result<Array1<f64>, TraversalModelError> {
        let mut values = Array1::<f64>::zeros(grid_0.len());

        for (i, &x) in grid_0.iter().enumerate() {
            let input = vec![x];
            let (energy_rate, _) = model.predict(&input).map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "Failed to predict energy rate: {e}"
                ))
            })?;
            values[i] = energy_rate;
        }

        Ok(values)
    }

    fn build_values_2d(
        model: &dyn PredictionModel,
        grid_0: &Array1<f64>,
        grid_1: &Array1<f64>,
    ) -> Result<Array2<f64>, TraversalModelError> {
        let mut values = Array2::<f64>::zeros((grid_0.len(), grid_1.len()));

        for (i, &x) in grid_0.iter().enumerate() {
            for (j, &y) in grid_1.iter().enumerate() {
                let input = vec![x, y];
                let (energy_rate, _) = model.predict(&input).map_err(|e| {
                    TraversalModelError::TraversalModelFailure(format!(
                        "Failed to predict energy rate: {e}"
                    ))
                })?;
                values[[i, j]] = energy_rate;
            }
        }

        Ok(values)
    }

    fn build_values_3d(
        model: &dyn PredictionModel,
        grid_0: &Array1<f64>,
        grid_1: &Array1<f64>,
        grid_2: &Array1<f64>,
    ) -> Result<Array3<f64>, TraversalModelError> {
        let mut values = Array3::<f64>::zeros((grid_0.len(), grid_1.len(), grid_2.len()));

        for (i, &x) in grid_0.iter().enumerate() {
            for (j, &y) in grid_1.iter().enumerate() {
                for (k, &z) in grid_2.iter().enumerate() {
                    let input = vec![x, y, z];
                    let (energy_rate, _) = model.predict(&input).map_err(|e| {
                        TraversalModelError::TraversalModelFailure(format!(
                            "Failed to predict energy rate: {e}"
                        ))
                    })?;
                    values[[i, j, k]] = energy_rate;
                }
            }
        }

        Ok(values)
    }

    fn build_values_nd(
        model: &dyn PredictionModel,
        grid: &[Array1<f64>],
    ) -> Result<ArrayD<f64>, TraversalModelError> {
        let shape: Vec<usize> = grid.iter().map(|feature| feature.len()).collect();
        let mut values = ArrayD::<f64>::zeros(IxDyn(&shape));

        // Predict energy rate values across the whole grid
        let index_ranges: Vec<_> = shape.iter().map(|&len| 0..len).collect();

        for indices in index_ranges.into_iter().multi_cartesian_product() {
            // Get the actual feature values corresponding to the current indices
            let input: Vec<f64> = indices
                .iter()
                .enumerate()
                .map(|(dim, &i)| grid[dim][i])
                .collect();

            // Predict the energy rate
            let (energy_rate, _) = model.predict(&input).map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "Failed to predict energy rate: {e}"
                ))
            })?;
            values[IxDyn(&indices)] = energy_rate;
        }

        Ok(values)
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;
    use crate::model::prediction::prediction_model::PredictionModel;
    use routee_compass_core::model::unit::{EnergyRateUnit, RatioUnit, SpeedUnit};

    #[test]
    fn test_interpolation_2d_speed_grade_model() {
        let model_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("test")
            .join("Toyota_Camry.bin");

        let input_features = vec![
            InputFeature::Speed {
                name: "speed".to_string(),
                unit: Some(SpeedUnit::MPH),
            },
            InputFeature::Ratio {
                name: "grade".to_string(),
                unit: Some(RatioUnit::Decimal),
            },
        ];
        let feature_bounds = HashMap::from([
            (
                "speed".to_string(),
                FeatureBounds {
                    lower_bound: 1.0,
                    upper_bound: 100.0,
                    num_bins: 100,
                },
            ),
            (
                "grade".to_string(),
                FeatureBounds {
                    lower_bound: -0.2,
                    upper_bound: 0.2,
                    num_bins: 41,
                },
            ),
        ]);
        let interp_model = InterpolationModel::new(
            &model_path,
            ModelType::Smartcore,
            input_features.clone(),
            feature_bounds,
            EnergyRateUnit::GGPM,
        )
        .unwrap();

        let underlying_model = SmartcoreModel::new(&model_path, EnergyRateUnit::GGPM).unwrap();

        // Check that the 2D interpolation model produces similar results to the underlying model
        for speed in 0..100 {
            for grade in -20..20 {
                let speed_f64 = speed as f64;
                let grade_f64 = grade as f64 / 100.0;
                let input = vec![speed_f64, grade_f64];
                let (interp_energy_rate, _energy_rate_unit) = interp_model.predict(&input).unwrap();
                let (underlying_energy_rate, _energy_rate_unit) =
                    underlying_model.predict(&input).unwrap();

                // Check if they're within 1% of each other
                let diff = (interp_energy_rate - underlying_energy_rate) / underlying_energy_rate;
                assert!(diff.abs() < 0.01);
            }
        }

        let (energy_rate, energy_rate_unit) = interp_model.predict(&[50.0, 0.0]).unwrap();

        assert_eq!(energy_rate_unit, EnergyRateUnit::GGPM);

        // Energy rate should be between 28-32 mpg
        let expected_lower = 1.0 / 32.0;
        let expected_upper = 1.0 / 28.0;
        assert!(energy_rate >= expected_lower);
        assert!(energy_rate <= expected_upper);
    }

    #[test]
    fn test_interpolation_rejects_zero_features() {
        let model_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("test")
            .join("Toyota_Camry.bin");

        let result = InterpolationModel::new(
            &model_path,
            ModelType::Smartcore,
            vec![],
            HashMap::new(),
            EnergyRateUnit::GGPM,
        );
        assert!(result.is_err());
    }
}
