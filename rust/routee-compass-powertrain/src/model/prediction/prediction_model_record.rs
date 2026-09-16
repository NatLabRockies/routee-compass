use super::{model_type::ModelType, PredictionModel, PredictionModelConfig};
use crate::model::{
    fieldname,
    prediction::{onnx::onnx_model::OnnxModel, prediction_model_ops},
};
use routee_compass_core::model::{
    state::{InputFeature, StateModel, StateVariable},
    traversal::TraversalModelError,
    unit::{EnergyRateUnit, EnergyUnit},
};
use std::{str::FromStr, sync::Arc};
use uom::si::f64::{Energy, Mass};

/// A struct to hold the prediction model and associated metadata
pub struct PredictionModelRecord {
    pub name: String,
    pub prediction_model: Arc<dyn PredictionModel>,
    pub model_type: ModelType,
    pub input_features: Vec<InputFeature>,
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

        // Create the prediction model
        // NOTE: Only supporting one target feature for now (the first one specified)
        if let Some(feature) = config.contract.target.first() {
            energy_rate_unit = EnergyRateUnit::from_str(&feature.units).map_err(|err| {
                TraversalModelError::BuildError(format!(
                    "{}: could not determine the energy unit for {} in vehicle model {}.",
                    err, feature.name, config.model_key
                ))
            })?;
            prediction_model = Arc::new(OnnxModel::new(
                &config.estimator.model_file,
                energy_rate_unit,
            )?);
        } else {
            return Err(TraversalModelError::BuildError(format!(
                "the first target was invalid for vehicle model {}",
                config.model_key
            )));
        };

        // Determine the minimum a star heuristic from the prediction model, input features, and unit
        let a_star_heuristic_energy_rate = prediction_model_ops::find_min_energy_rate(
            &prediction_model,
            input_features.as_slice(),
            &energy_rate_unit,
        )?;

        Ok(PredictionModelRecord {
            name: config.model_key.to_string(),
            prediction_model,
            model_type: ModelType::Onnx,
            input_features,
            energy_rate_unit,
            mass_estimate: Mass::new::<uom::si::mass::pound>(config.vehicle.mass_lbs),
            a_star_heuristic_energy_rate,
            real_world_energy_adjustment: config.contract.real_world_adjustment_factor,
        })
    }
}

impl PredictionModelRecord {
    pub fn predict(
        &self,
        state: &mut [StateVariable],
        state_model: &StateModel,
    ) -> Result<Energy, TraversalModelError> {
        let distance = state_model.get_distance(state, fieldname::EDGE_DISTANCE)?;
        let mut feature_vector: Vec<f64> = Vec::new();
        for input_feature in &self.input_features {
            let state_variable_f64: f64 = match input_feature {
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
                InputFeature::Custom { name, unit: _ } => {
                    state_model.get_custom_f64(state, name)?
                }
                _ => {
                    return Err(TraversalModelError::TraversalModelFailure(format!(
                        "got an unexpected input feature in the smartcore model prediction {input_feature}"
                    )))
                }
            };
            feature_vector.push(state_variable_f64);
        }

        // TODO: Integrate TripHistoryTraversalModel here.
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
    use crate::model::prediction::prediction_model_config::PredictionModelConfig;
    use crate::model::prediction::PredictionModelRecord;
    use serde_json::Value;
    use std::fs::File;
    use std::io::BufReader;
    #[test]
    fn test_success_prediction_model_record() {
        use std::path::PathBuf;

        let test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/model/prediction/test");

        let file = File::open(test_dir.join("v2_metadata_example.json")).unwrap();
        let buf = BufReader::new(file);
        let data: Value = serde_json::from_reader(buf).unwrap();

        let mut prediction_model_config: PredictionModelConfig =
            serde_json::from_value(data).unwrap();

        // resolve the bare model filename against the config's directory
        prediction_model_config.estimator.model_file = test_dir
            .join(&prediction_model_config.estimator.model_file)
            .to_string_lossy()
            .into_owned();

        let prediction_model_record =
            PredictionModelRecord::try_from(&prediction_model_config).unwrap();

        assert!(matches!(
            prediction_model_record,
            PredictionModelRecord { .. }
        ));
    }
}
