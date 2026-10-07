use super::{model_type::ModelType, PredictionModel, PredictionModelConfig};
use crate::model::{
    fieldname,
    prediction::{
        onnx::onnx_model::OnnxModel,
        prediction_model_ops,
        routee_powertrain_v2_metadata::{EstimatorType, InputSpec, PadStrategy},
    },
};
use routee_compass_core::model::{
    state::{InputFeature, StateModel, StateVariable},
    traversal::TraversalModelError,
    unit::{EnergyRateUnit, EnergyUnit},
};
use std::{str::FromStr, sync::Arc};
use uom::si::f64::{Energy, Mass};

/// A struct to hold the prediction model and associated metadata
///
/// A lookback of zero denotes tabular input: one current feature row, no history
/// dependencies, and an ONNX shape of `[1, features]` rather than a zero-sized axis.
///
/// Sequence models require a `trip_history` traversal providing the model's input
/// features at depths 1 through `lookback - 1`. Complete leading rows of NaN
/// sentinels are padded according to `input_spec`; missing state fields are errors.
/// Sequence models use a zero energy-rate heuristic instead of a pointwise grid
/// estimate, which does not bound predictions across different histories.
pub struct PredictionModelRecord {
    pub name: String,
    pub prediction_model: Arc<dyn PredictionModel>,
    pub model_type: ModelType,
    /// Complete timestep-major inputs, oldest history first and current link last.
    pub input_features: Vec<InputFeature>,
    pub input_spec: InputSpec,
    pub energy_rate_unit: EnergyRateUnit,
    pub mass_estimate: Mass,
    pub a_star_heuristic_energy_rate: f64,
    pub real_world_energy_adjustment: f64,
}

impl TryFrom<&PredictionModelConfig> for PredictionModelRecord {
    type Error = TraversalModelError;

    fn try_from(config: &PredictionModelConfig) -> Result<Self, Self::Error> {
        if config.contract.feature_set.is_empty() {
            return Err(TraversalModelError::BuildError(format!(
                "you must supply at least one input feature for vehicle model {}",
                config.model_key
            )));
        }

        if config.contract.target.is_empty() {
            return Err(TraversalModelError::BuildError(format!(
                "you must supply at least one target feature for vehicle model {}",
                config.model_key
            )));
        }

        let lookback = usize::try_from(config.estimator.input_spec.lookback).map_err(|_| {
            TraversalModelError::BuildError(format!(
                "lookback must be nonnegative for vehicle model {}",
                config.model_key
            ))
        })?;

        // Map PowertrainV2 Feature vector to Compass InputFeature vector
        let input_features: Vec<InputFeature> = config
            .contract
            .feature_set
            .iter()
            .map(InputFeature::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                TraversalModelError::BuildError(format!(
                    "{}: couldn't map powertrain features to compass features in vehicle model {}",
                    err, config.model_key
                ))
            })?;

        let prediction_model: Arc<dyn PredictionModel>;
        let energy_rate_unit: EnergyRateUnit;
        let distance_units = &config.contract.distance.units;
        // Create the prediction model
        // NOTE: Only supporting one target feature for now (the first one specified)
        if let Some(feature) = config.contract.target.first() {
            // append the distance unit to the feature unit. for example:
            // target feature unit:    "kilowatt-hour"
            // contract distance unit:  "miles"
            // becomes                  "killowatt-hour/miles"
            let mut energy_unit: String = feature.units.clone();
            energy_unit.push('/');
            energy_unit.push_str(distance_units);

            energy_rate_unit = EnergyRateUnit::from_str(&energy_unit).map_err(|err| {
                TraversalModelError::BuildError(format!(
                    "{}: could not determine the energy unit for {} in vehicle model {}.",
                    err, feature.name, config.model_key
                ))
            })?;
            prediction_model = match config.estimator.estimator_type {
                EstimatorType::ONNXEstimator => {
                    let model = OnnxModel::new(&config.estimator.model_file, energy_rate_unit)?;
                    let expected_shape = if lookback == 0 {
                        vec![1, input_features.len()]
                    } else {
                        vec![1, lookback, input_features.len()]
                    };
                    if model.input_shape() != expected_shape {
                        return Err(TraversalModelError::BuildError(format!(
                            "vehicle model {} metadata expects input shape {:?}, but ONNX model has {:?}",
                            config.model_key,
                            expected_shape,
                            model.input_shape()
                        )));
                    }
                    Arc::new(model)
                }
                // NGBoost unsupported for now.
                EstimatorType::NGBoostEstimator => {
                    return Err(TraversalModelError::BuildError(format!(
                        "unsupported estimator type NGBoostEstimator for vehicle model {}; only ONNXEstimator is currently supported",
                        config.model_key
                    )));
                }
            };
        } else {
            return Err(TraversalModelError::BuildError(format!(
                "the first target was invalid for vehicle model {}",
                config.model_key
            )));
        };

        // Determine the minimum a star heuristic from the prediction model, input features, and unit
        // TODO: This will be replaced by
        let a_star_heuristic_energy_rate = if lookback > 1 {
            log::debug!(
                "Using zero energy-rate heuristic for sequence model {}; the pointwise grid search does not bound history-dependent predictions",
                config.model_key
            );
            0.0
        } else {
            prediction_model_ops::find_min_energy_rate(
                &prediction_model,
                input_features.as_slice(),
                &config.contract.feature_set,
                &energy_rate_unit,
            )?
        };

        let input_features = (0..lookback.max(1))
            .rev()
            .flat_map(|depth| {
                input_features.iter().cloned().map(move |mut feature| {
                    if depth > 0 {
                        let name = match &mut feature {
                            InputFeature::Distance { name, .. }
                            | InputFeature::Speed { name, .. }
                            | InputFeature::Time { name, .. }
                            | InputFeature::Energy { name, .. }
                            | InputFeature::Ratio { name, .. }
                            | InputFeature::Temperature { name, .. }
                            | InputFeature::Custom { name, .. } => name,
                        };
                        *name = format!("{name}_{depth}");
                    }
                    feature
                })
            })
            .collect();

        Ok(PredictionModelRecord {
            name: config.model_key.to_string(),
            prediction_model,
            model_type: ModelType::Onnx,
            input_features,
            input_spec: config.estimator.input_spec.clone(),
            energy_rate_unit,
            mass_estimate: Mass::new::<uom::si::mass::pound>(config.vehicle.mass_lbs),
            a_star_heuristic_energy_rate,
            real_world_energy_adjustment: config.contract.real_world_adjustment_factor,
        })
    }
}

impl PredictionModelRecord {
    fn feature_vector(
        &self,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<Vec<f64>, TraversalModelError> {
        let mut feature_vector = Vec::with_capacity(self.input_features.len());
        for input_feature in &self.input_features {
            let state_variable_f64: f64 = match input_feature {
                InputFeature::Distance { name, unit } => {
                    let distance = state_model.get_distance(state, name)?;
                    match unit {
                        None => {
                            return Err(TraversalModelError::TraversalModelFailure(format!(
                                "Unit must be set for distance input feature {input_feature} but got None"
                            )));
                        }
                        Some(unit) => unit.from_uom(distance),
                    }
                }
                InputFeature::Speed { name, unit } => {
                    let speed = state_model.get_speed(state, name)?;
                    match unit {
                        None => {
                            return Err(TraversalModelError::TraversalModelFailure(format!(
                                "Unit must be set for speed input feature {input_feature} but got None"
                            )));
                        }
                        Some(u) => u.from_uom(speed),
                    }
                }
                InputFeature::Time { name, unit } => {
                    let time = state_model.get_time(state, name)?;
                    match unit {
                        None => {
                            return Err(TraversalModelError::TraversalModelFailure(format!(
                                "Unit must be set for time input feature {input_feature} but got None"
                            )));
                        }
                        Some(u) => u.from_uom(time),
                    }
                }
                InputFeature::Ratio { name, unit } => {
                    let grade = state_model.get_ratio(state, name)?;
                    match unit {
                        None => {
                            return Err(TraversalModelError::TraversalModelFailure(format!(
                                "Unit must be set for grade input feature {input_feature} but got None"
                            )));
                        }
                        Some(u) => u.from_uom(grade),
                    }
                }
                InputFeature::Temperature { name, unit } => {
                    let temperature = state_model.get_temperature(state, name)?;
                    match unit {
                        None => {
                            return Err(TraversalModelError::TraversalModelFailure(format!(
                                "Unit must be set for temperature input feature {input_feature} but got None"
                            )));
                        }
                        Some(u) => u.from_uom(temperature),
                    }
                }
                InputFeature::Custom { name, .. } => state_model.get_custom_f64(state, name)?,
                _ => {
                    return Err(TraversalModelError::TraversalModelFailure(format!(
                        "got an unexpected input feature in the model prediction {input_feature}"
                    )))
                }
            };
            feature_vector.push(state_variable_f64);
        }

        let window_size = self.input_spec.lookback.max(1) as usize;
        let num_features = feature_vector.len() / window_size;
        if num_features == 0 || !feature_vector.len().is_multiple_of(window_size) {
            return Err(TraversalModelError::TraversalModelFailure(
                "prediction inputs do not form a complete lookback window".to_string(),
            ));
        }
        let first_present = feature_vector
            .chunks_exact(num_features)
            .position(|row| row.iter().any(|value| !value.is_nan()))
            .ok_or_else(|| {
                TraversalModelError::TraversalModelFailure(
                    "current-link prediction features are missing".to_string(),
                )
            })?;
        let start = first_present * num_features;
        if feature_vector[start..]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(TraversalModelError::TraversalModelFailure(
                "prediction features must be finite; only complete leading history rows may be missing"
                    .to_string(),
            ));
        }
        match self.input_spec.pad_strategy {
            PadStrategy::Zero => feature_vector[..start].fill(0.0),
            PadStrategy::RepeatFirst => {
                for offset in (0..start).step_by(num_features) {
                    feature_vector.copy_within(start..start + num_features, offset);
                }
            }
        }
        Ok(feature_vector)
    }

    pub fn predict(
        &self,
        state: &mut [StateVariable],
        state_model: &StateModel,
    ) -> Result<Energy, TraversalModelError> {
        let distance = state_model.get_distance(state, fieldname::EDGE_DISTANCE)?;
        let feature_vector = self.feature_vector(state, state_model)?;
        let (energy_rate, energy_rate_unit) = self.prediction_model.predict(&feature_vector)?;

        let energy_rate_real_world = energy_rate * self.real_world_energy_adjustment;

        // TODO: This should be updated once we have EnergyRate as a UOM quantity
        let energy = match energy_rate_unit {
            EnergyRateUnit::GGPM => {
                let distance_miles = distance.get::<uom::si::length::mile>();
                let energy_f64 = energy_rate_real_world * distance_miles;
                EnergyUnit::GallonsGasolineEquivalent.to_uom(energy_f64)
            }
            EnergyRateUnit::GDPM => {
                let distance_miles = distance.get::<uom::si::length::mile>();
                let energy_f64 = energy_rate_real_world * distance_miles;
                EnergyUnit::GallonsDieselEquivalent.to_uom(energy_f64)
            }
            EnergyRateUnit::KWHPKM => {
                let distance_kilometers = distance.get::<uom::si::length::kilometer>();
                let energy_f64 = energy_rate_real_world * distance_kilometers;
                EnergyUnit::KilowattHours.to_uom(energy_f64)
            }
            EnergyRateUnit::KWHPM => {
                let distance_miles = distance.get::<uom::si::length::mile>();
                let energy_f64 = energy_rate_real_world * distance_miles;
                EnergyUnit::KilowattHours.to_uom(energy_f64)
            }
        };

        Ok(energy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::prediction::prediction_model_config::PredictionModelConfig;
    use routee_compass_core::model::state::StateVariableConfig;
    use serde_json::Value;
    use std::fs::File;
    use std::io::BufReader;
    use uom::si::f64::{Length, Ratio, Velocity};
    fn model_config() -> PredictionModelConfig {
        model_config_from_file("v2_metadata_example.json")
    }

    fn model_config_from_file(metadata_file: &str) -> PredictionModelConfig {
        use std::path::PathBuf;

        let metadata_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/model/prediction/test")
            .join(metadata_file);
        let test_dir = metadata_path.parent().unwrap();

        let file = File::open(&metadata_path).unwrap();
        let buf = BufReader::new(file);
        let data: Value = serde_json::from_reader(buf).unwrap();

        let mut prediction_model_config: PredictionModelConfig =
            serde_json::from_value(data).unwrap();

        // resolve the bare model filename against the config's directory
        prediction_model_config.estimator.model_file = test_dir
            .join(&prediction_model_config.estimator.model_file)
            .to_string_lossy()
            .into_owned();

        prediction_model_config
    }

    fn state_model(record: &PredictionModelRecord) -> StateModel {
        let features = record
            .input_features
            .iter()
            .map(|feature| {
                let config = match feature {
                    InputFeature::Speed { .. } => StateVariableConfig::Speed {
                        initial: Velocity::new::<uom::si::velocity::meter_per_second>(f64::NAN),
                        accumulator: false,
                        output_unit: None,
                    },
                    InputFeature::Distance { .. } => StateVariableConfig::Distance {
                        initial: Length::new::<uom::si::length::meter>(f64::NAN),
                        accumulator: false,
                        output_unit: None,
                    },
                    InputFeature::Ratio { .. } => StateVariableConfig::Ratio {
                        initial: Ratio::new::<uom::si::ratio::ratio>(f64::NAN),
                        accumulator: false,
                        output_unit: None,
                    },
                    _ => panic!("unexpected test feature {feature}"),
                };
                (feature.name(), config)
            })
            .collect();
        StateModel::new(features)
    }

    fn window_state(state_model: &StateModel, rows: &[[f64; 2]]) -> Vec<StateVariable> {
        let mut state = state_model.initial_state(None).unwrap();
        for (depth, [speed, distance]) in rows.iter().rev().enumerate() {
            let suffix = if depth == 0 {
                String::new()
            } else {
                format!("_{depth}")
            };
            state_model
                .set_speed(
                    &mut state,
                    &format!("edge_speed{suffix}"),
                    &Velocity::new::<uom::si::velocity::meter_per_second>(speed * 0.44704),
                )
                .unwrap();
            state_model
                .set_distance(
                    &mut state,
                    &format!("edge_distance{suffix}"),
                    &Length::new::<uom::si::length::meter>(distance * 1609.344),
                )
                .unwrap();
        }
        state
    }

    fn assert_features(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
        }
    }

    #[test]
    fn test_success_prediction_model_record() {
        let prediction_model_record = PredictionModelRecord::try_from(&model_config()).unwrap();

        let names: Vec<_> = prediction_model_record
            .input_features
            .iter()
            .map(|feature| feature.name())
            .collect();
        assert_eq!(
            names,
            [
                "edge_speed_4",
                "edge_distance_4",
                "edge_speed_3",
                "edge_distance_3",
                "edge_speed_2",
                "edge_distance_2",
                "edge_speed_1",
                "edge_distance_1",
                "edge_speed",
                "edge_distance",
            ]
        );
        assert_eq!(prediction_model_record.a_star_heuristic_energy_rate, 0.0);
    }

    #[test]
    fn test_rejects_mismatched_lookback() {
        let mut config = model_config();
        for lookback in [-1, 0, 4] {
            config.estimator.input_spec.lookback = lookback;
            assert!(PredictionModelRecord::try_from(&config).is_err());
        }
    }

    #[test]
    fn test_predicts_complete_lookback() {
        let record = PredictionModelRecord::try_from(&model_config()).unwrap();
        let state_model = state_model(&record);
        let rows = [
            [10.0, 0.1],
            [20.0, 0.2],
            [30.0, 0.3],
            [40.0, 0.4],
            [50.0, 0.5],
        ];
        let mut state = window_state(&state_model, &rows);
        let expected: Vec<f64> = rows.into_iter().flatten().collect();
        assert_features(
            &record.feature_vector(&state, &state_model).unwrap(),
            &expected,
        );

        let (rate, _) = record.prediction_model.predict(&expected).unwrap();
        let energy = record.predict(&mut state, &state_model).unwrap();
        assert!(
            (energy.get::<uom::si::energy::kilowatt_hour>()
                - rate * 0.5 * record.real_world_energy_adjustment)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn test_pads_only_missing_leading_history() {
        for pad_strategy in [PadStrategy::Zero, PadStrategy::RepeatFirst] {
            let mut config = model_config();
            config.estimator.input_spec.pad_strategy = pad_strategy.clone();
            let record = PredictionModelRecord::try_from(&config).unwrap();
            let state_model = state_model(&record);
            for rows in [vec![[50.0, 0.5]], vec![[40.0, 0.4], [50.0, 0.5]]] {
                let state = window_state(&state_model, &rows);
                let padding = match pad_strategy {
                    PadStrategy::Zero => [0.0, 0.0],
                    PadStrategy::RepeatFirst => rows[0],
                };
                let expected: Vec<f64> = std::iter::repeat_n(padding, 5 - rows.len())
                    .chain(rows.iter().copied())
                    .flatten()
                    .collect();
                assert_features(
                    &record.feature_vector(&state, &state_model).unwrap(),
                    &expected,
                );
            }
        }
    }

    #[test]
    fn test_rejects_invalid_or_unconfigured_history() {
        let record = PredictionModelRecord::try_from(&model_config()).unwrap();
        let state_model = state_model(&record);
        for rows in [
            vec![[f64::NAN, 0.5]],
            vec![[f64::INFINITY, 0.5]],
            vec![[f64::NAN, 0.4], [50.0, 0.5]],
            vec![[30.0, 0.3], [f64::NAN, f64::NAN], [50.0, 0.5]],
            vec![[40.0, 0.4], [f64::NAN, f64::NAN]],
            vec![[f64::NAN, f64::NAN]],
        ] {
            let state = window_state(&state_model, &rows);
            assert!(record.feature_vector(&state, &state_model).is_err());
        }
        let error = record
            .feature_vector(&[], &StateModel::new(vec![]))
            .unwrap_err();
        assert!(error.to_string().contains("edge_speed_4"));
    }

    #[test]
    fn test_2016_camry_ice_cnn_record() {
        let config = model_config_from_file("2016_camry_ice_cnn/metadata.json");
        assert_eq!(config.estimator.input_spec.lookback, 5);
        assert!(matches!(
            config.estimator.input_spec.pad_strategy,
            PadStrategy::Zero
        ));
        let mut record = PredictionModelRecord::try_from(&config).unwrap();
        record.a_star_heuristic_energy_rate = 0.028;
        assert_eq!(record.input_spec.lookback, 5);
        let names: Vec<_> = record
            .input_features
            .iter()
            .map(InputFeature::name)
            .collect();
        assert_eq!(
            names,
            [
                "edge_speed_4",
                "edge_distance_4",
                "edge_speed_3",
                "edge_distance_3",
                "edge_speed_2",
                "edge_distance_2",
                "edge_speed_1",
                "edge_distance_1",
                "edge_speed",
                "edge_distance",
            ]
        );
        let state_model = state_model(&record);
        // inject a link state
        let rows = [
            [10.0, 0.1],
            [20.0, 0.2],
            [30.0, 0.3],
            [40.0, 0.4],
            [50.0, 0.5],
        ];
        let mut state = window_state(&state_model, &rows);
        let features: Vec<f64> = rows.into_iter().flatten().collect();
        assert_features(
            &record.feature_vector(&state, &state_model).unwrap(),
            &features,
        );
        let (rate, unit) = record.prediction_model.predict(&features).unwrap();
        assert_eq!(unit, EnergyRateUnit::GGPM);
        let energy = record.predict(&mut state, &state_model).unwrap();
        let expected = EnergyUnit::GallonsGasolineEquivalent
            .to_uom(rate * 0.5 * record.real_world_energy_adjustment);
        assert!(energy.value.is_finite());
        assert!((energy.value - expected.value).abs() < 1e-9);
        assert!(record.a_star_heuristic_energy_rate.is_finite());
    }

    #[test]
    fn test_2016_camry_ice_rf_record() {
        let config = model_config_from_file("2016_camry_ice_rf/metadata.json");
        assert_eq!(config.estimator.input_spec.lookback, 0);
        assert!(matches!(
            config.estimator.input_spec.pad_strategy,
            PadStrategy::RepeatFirst
        ));
        let mut record = PredictionModelRecord::try_from(&config).unwrap();
        record.a_star_heuristic_energy_rate = 0.028;
        assert_eq!(record.input_spec.lookback, 0);
        let names: Vec<_> = record
            .input_features
            .iter()
            .map(InputFeature::name)
            .collect();
        assert_eq!(names, ["edge_speed", "edge_distance"]);
        let state_model = state_model(&record);
        let mut state = window_state(&state_model, &[[50.0, 0.5]]);
        assert_features(
            &record.feature_vector(&state, &state_model).unwrap(),
            &[50.0, 0.5],
        );
        let (rate, unit) = record.prediction_model.predict(&[50.0, 0.5]).unwrap();
        assert_eq!(unit, EnergyRateUnit::GGPM);
        let energy = record.predict(&mut state, &state_model).unwrap();
        let expected = EnergyUnit::GallonsGasolineEquivalent
            .to_uom(rate * 0.5 * record.real_world_energy_adjustment);
        assert!(energy.value.is_finite());
        assert!((energy.value - expected.value).abs() < 1e-9);
        assert!(record.a_star_heuristic_energy_rate.is_finite());
    }
}
