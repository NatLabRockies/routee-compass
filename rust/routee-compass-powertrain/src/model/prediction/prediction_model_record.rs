use crate::model::fieldname;

use super::routee_powertrain_v2_metadata::routee_powertrain_v2_metadata::Feature;
use super::{
    interpolation::InterpolationModel, model_type::ModelType, onnx::onnx_model::OnnxModel,
    prediction_model_ops, smartcore::SmartcoreModel, PredictionModel, PredictionModelConfig,
};
use routee_compass_core::model::{
    state::{InputFeature, StateModel, StateVariable},
    traversal::TraversalModelError,
    unit::{
        DistanceUnit, EnergyRateUnit, EnergyUnit, RatioUnit, SpeedUnit, TemperatureUnit, TimeUnit,
    },
};
use std::str::FromStr;
use std::sync::Arc;
use uom::si::f64::{Energy, Mass};

/// converts a routee-powertrain v2 metadata `Feature` into an [`InputFeature`],
/// mapping the feature's `dtype`/`units` onto the appropriate variant and unit.
impl TryFrom<Feature> for InputFeature {
    type Error = TraversalModelError;

    fn try_from(feature: Feature) -> Result<Self, Self::Error> {
        let name = feature.name.clone();
        let units = feature.units.trim();
        let input_feature = match feature.dtype.trim().to_lowercase().as_str() {
            "distance" => InputFeature::Distance {
                name,
                unit: Some(DistanceUnit::from_str(units).map_err(|e| unit_err(&feature, e))?),
            },
            "speed" => InputFeature::Speed {
                name,
                unit: Some(SpeedUnit::from_str(units).map_err(|e| unit_err(&feature, e))?),
            },
            "time" => InputFeature::Time {
                name,
                unit: Some(TimeUnit::from_str(units).map_err(|e| unit_err(&feature, e))?),
            },
            "energy" => InputFeature::Energy {
                name,
                unit: Some(EnergyUnit::from_str(units).map_err(|e| unit_err(&feature, e))?),
            },
            "ratio" | "grade" => InputFeature::Ratio {
                name,
                unit: Some(RatioUnit::from_str(units).map_err(|e| unit_err(&feature, e))?),
            },
            "temperature" => InputFeature::Temperature {
                name,
                unit: Some(TemperatureUnit::from_str(units).map_err(|e| unit_err(&feature, e))?),
            },
            _ => InputFeature::Custom {
                name,
                unit: feature.units.clone(),
            },
        };
        Ok(input_feature)
    }
}

fn unit_err<E: std::fmt::Display>(feature: &Feature, e: E) -> TraversalModelError {
    TraversalModelError::BuildError(format!(
        "unable to parse units '{}' for feature '{}' with dtype '{}': {}",
        feature.units, feature.name, feature.dtype, e
    ))
}

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
        match config {
            PredictionModelConfig::PowertrainV1Schema {
                name,
                model_input_file,
                model_type,
                input_features,
                energy_rate_unit,
                mass_estimate_lbs,
                a_star_heuristic_energy_rate,
                real_world_energy_adjustment,
            } => {
                if input_features.is_empty() {
                    return Err(TraversalModelError::BuildError(format!(
                        "You must supply at least one input feature for vehicle model {}",
                        name
                    )));
                }

                // build the prediction model from the config
                let prediction_model: Arc<dyn PredictionModel> = match model_type {
                    ModelType::Smartcore => {
                        let model =
                            SmartcoreModel::new(model_input_file, energy_rate_unit.clone())?;
                        Arc::new(model)
                    }
                    ModelType::Onnx => {
                        let model = OnnxModel::new(model_input_file, energy_rate_unit.clone())?;
                        Arc::new(model)
                    }
                    ModelType::Interpolate {
                        underlying_model_type: underlying_model,
                        feature_bounds,
                    } => {
                        let model = InterpolationModel::new(
                            model_input_file,
                            *underlying_model.clone(),
                            input_features.clone(),
                            feature_bounds.clone(),
                            energy_rate_unit.clone(),
                        )?;
                        Arc::new(model)
                    }
                };

                let a_star_heuristic_energy_rate = match a_star_heuristic_energy_rate {
                    None => prediction_model_ops::find_min_energy_rate(
                        &prediction_model,
                        input_features,
                        energy_rate_unit,
                    )?,
                    Some(rate) => *rate,
                };

                let real_world_energy_adjustment = real_world_energy_adjustment.unwrap_or(1.0);

                let mass_estimate = Mass::new::<uom::si::mass::pound>(mass_estimate_lbs.clone());

                Ok(PredictionModelRecord {
                    name: name.clone(),
                    prediction_model,
                    model_type: model_type.clone(),
                    input_features: input_features.clone(),
                    energy_rate_unit: energy_rate_unit.clone(),
                    mass_estimate,
                    a_star_heuristic_energy_rate,
                    real_world_energy_adjustment,
                })
            }
            PredictionModelConfig::PowertrainV2Schema {
                model_key,
                vehicle,
                contract,
                estimator,
            } => {
                // TODO: only ONNX for now (don't change PredictionModelRecord) and inject
                // a_star_heuristic_energy_rate. also write Unit test deserealizing a metadata.json from
                // powertrainV2
                if contract.feature_set.is_empty() {
                    return Err(TraversalModelError::BuildError(format!(
                        "You must supply at least one input feature for vehicle model {}",
                        model_key
                    )));
                }

                let prediction_model: Arc<dyn PredictionModel> = Arc::new(OnnxModel::new(
                    &estimator.model_file,
                    EnergyRateUnit::KWHPM,
                )?);

                let mass_estimate = Mass::new::<uom::si::mass::pound>(vehicle.mass_lbs.clone());

                let input_features: Vec<InputFeature> = contract
                    .feature_set
                    .iter()
                    .map(|feature| InputFeature::try_from(feature.clone()))
                    .collect::<Result<Vec<InputFeature>, TraversalModelError>>()?;

                let a_star_heuristic_energy_rate = prediction_model_ops::find_min_energy_rate(
                    &prediction_model,
                    &input_features,
                    &EnergyRateUnit::KWHPM,
                )?;

                Ok(PredictionModelRecord {
                    name: model_key.clone(),
                    prediction_model: prediction_model,
                    model_type: ModelType::Onnx,
                    input_features,
                    energy_rate_unit: EnergyRateUnit::KWHPM,
                    mass_estimate,
                    a_star_heuristic_energy_rate,
                    real_world_energy_adjustment: contract.real_world_adjustment_factor,
                })
            }
        }
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
mod test {
    use crate::model::prediction::PredictionModelRecord;

    use super::PredictionModelConfig;
    use serde_json::Value;
    use std::fs::File;
    use std::io::BufReader;
    #[test]
    fn test_powertrain_v2_prediction_model_config() {
        // load the example file
        let file = File::open("src/model/prediction/test/v2_metadata_example.json").unwrap();
        let buf = BufReader::new(file);
        // load the data into a serde_json::Value
        let data: Value = serde_json::from_reader(buf).unwrap();
        // try deserializing the JSON data into a PredictionModelConfig
        let prediction_model_config: PredictionModelConfig = serde_json::from_value(data).unwrap();

        // try creating a PredictionModelRecord from the config.
        let prediction_model_record = PredictionModelRecord::try_from(&prediction_model_config);
        assert!(prediction_model_record.is_ok());
    }
}
