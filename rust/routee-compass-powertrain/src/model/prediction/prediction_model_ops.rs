use super::routee_powertrain_v2_metadata::Feature;
use super::PredictionModel;
use itertools::Itertools;
use routee_compass_core::model::{
    state::InputFeature,
    traversal::TraversalModelError,
    unit::{DistanceUnit, EnergyRateUnit, RatioUnit, SpeedUnit, TemperatureUnit},
};
use std::sync::Arc;
use uom::si::f64::{Length, Ratio, ThermodynamicTemperature, Velocity};

const MIN_ENERGY_ERROR_MESSAGE: &str =
    "Failure while executing grid search for minimum energy rate in prediction model:";

/// sweep speed and grade values to find the minimum energy per mile rate from the incoming rf model
pub fn find_min_energy_rate(
    model: &Arc<dyn PredictionModel>,
    input_features: &[InputFeature],
    feature_set: &[Feature],
    energy_model_energy_rate_unit: &EnergyRateUnit,
) -> Result<f64, TraversalModelError> {
    // sweep a fixed set of speed and grade values to find the minimum energy per mile rate from the incoming rf model
    let mut minimum_energy_rate = f64::MAX;
    let start_time = std::time::Instant::now();

    // Create vectors of sample values for each feature type
    let mut sample_values: Vec<Vec<f64>> = Vec::new();

    for (input_feature, feature) in input_features.iter().zip(feature_set) {
        let values = match input_feature {
            InputFeature::Distance {..} => get_fixed_sample_value(feature),

            InputFeature::Speed { name: _, unit } => match unit {
                Some(speed_unit) => get_speed_sample_values(speed_unit),
                None => {
                    return Err(TraversalModelError::TraversalModelFailure(format!(
                        "{MIN_ENERGY_ERROR_MESSAGE} Unit must be set for speed input feature {input_feature} but got None"
                    )))
                }
            },
            // time is not treated as an energy-rate driver; hold at a single in-domain value
            InputFeature::Time { .. } => get_fixed_sample_value(feature),
            InputFeature::Ratio { name: _, unit } => match unit {
                Some(grade_unit) => get_grade_sample_values(grade_unit),
                None => {
                    return Err(TraversalModelError::TraversalModelFailure(format!(
                        "{MIN_ENERGY_ERROR_MESSAGE} Unit must be set for grade input feature {input_feature} but got None"
                    )))
                }
            },
            InputFeature::Temperature { name: _, unit } => match unit {
                Some(temp_unit) => get_temperature_sample_values(temp_unit),
                None => {
                    return Err(TraversalModelError::TraversalModelFailure(format!(
                        "{MIN_ENERGY_ERROR_MESSAGE} Unit must be set for temperature input feature {input_feature} but got None"
                    )))
                }
            },
            // custom features have no verifiable range; hold at a single in-domain value
            InputFeature::Custom { .. } => get_fixed_sample_value(feature),
            _ => {
                return Err(TraversalModelError::TraversalModelFailure(format!(
                    "{MIN_ENERGY_ERROR_MESSAGE} got an unexpected input feature in the smartcore model prediction {input_feature}"
                )))
            }
        };
        sample_values.push(values);
    }

    for feature_vec in sample_values.into_iter().multi_cartesian_product() {
        // Predict energy rate
        let (energy_rate, _) = model.predict(&feature_vec).map_err(|e| {
            TraversalModelError::BuildError(format!("{MIN_ENERGY_ERROR_MESSAGE} {e}"))
        })?;

        if energy_rate < minimum_energy_rate {
            minimum_energy_rate = energy_rate;
        }
    }

    // Cap the lower bound of the minimum energy rate to a realsitic value
    if minimum_energy_rate < 0.0 {
        minimum_energy_rate = 0.0;
    }

    let end_time = std::time::Instant::now();
    let search_time = end_time - start_time;

    log::debug!(
        "found minimum energy: {} {} in {} milliseconds",
        minimum_energy_rate,
        energy_model_energy_rate_unit,
        search_time.as_millis()
    );

    Ok(minimum_energy_rate)
}

/// generate Percent Grade values in the range [-20, 0] converted to the target grade unit
fn get_grade_sample_values(grade_unit: &RatioUnit) -> Vec<f64> {
    (1..101)
        .map(|i| {
            let grade = Ratio::new::<uom::si::ratio::ratio>(i as f64 * 0.2 - 20.0); // values in range [-20.0, 0.0]
            grade_unit.from_uom(grade)
        })
        .collect()
}

/// generate MPH Speed values in the range [1, 100]
fn get_speed_sample_values(speed_unit: &SpeedUnit) -> Vec<f64> {
    (1..101)
        .map(|i| {
            let speed = Velocity::new::<uom::si::velocity::mile_per_hour>(i as f64); // values in range [1, 100.0]
            speed_unit.from_uom(speed)
        })
        .collect()
}

fn get_temperature_sample_values(temperature_unit: &TemperatureUnit) -> Vec<f64> {
    (0..=110)
        .map(|i| {
            let temp = ThermodynamicTemperature::new::<
                uom::si::thermodynamic_temperature::degree_celsius,
            >(i as f64); // values in range [0, 110.0]
            temperature_unit.from_uom(temp)
        })
        .collect()
}

/// non-contributing features (time, custom) are held at one in-domain value — the
/// lower constraint when known — to preserve model input arity without expanding the
/// search grid or assuming a value range we cannot verify
fn get_fixed_sample_value(feature: &Feature) -> Vec<f64> {
    let value = feature
        .constraints
        .lower
        .or(feature.constraints.upper)
        .unwrap_or(0.0);
    vec![value]
}
