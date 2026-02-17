use super::ModelType;
use routee_compass_core::model::{state::InputFeature, unit::EnergyRateUnit};
use serde::{Deserialize, Serialize};

/// Configuration for a prediction model that estimates energy consumption.
///
/// This struct encapsulates all the necessary information to load and use a machine learning
/// model for predicting vehicle energy consumption during route planning. The configuration
/// is typically parsed from a Compass configuration file and includes details about the model
/// type, input features, and energy rate calculations.
///
/// # Fields
///
/// * `name` - A human-readable identifier for the model
/// * `model_input_file` - Path to the serialized model file to be loaded
/// * `model_type` - The type of machine learning model (e.g., ONNX, SmartCore)
/// * `input_features` - Ordered list of features that the model expects as input
/// * `energy_rate_unit` - The unit of measurement for the model's energy rate output
/// * `a_star_heuristic_energy_rate` - Optional energy rate used for A* heuristic calculations.
///   Should be in the same unit as the `energy_rate_unit` and should be "best case" energy rate over a relaistic trip.
///   If not provided, defaults to the minimum energy rate determined from the model.
/// * `real_world_energy_adjustment` - Optional multiplier to adjust model predictions to match real-world conditions
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictionModelConfig {
    pub name: String,
    pub model_input_file: String,
    pub model_type: ModelType,
    pub input_features: Vec<InputFeature>,
    pub energy_rate_unit: EnergyRateUnit,
    pub mass_estimate_lbs: f64,
    pub a_star_heuristic_energy_rate: Option<f64>,
    pub real_world_energy_adjustment: Option<f64>,
}

impl PredictionModelConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        model_input_file: String,
        model_type: ModelType,
        input_features: Vec<InputFeature>,
        energy_rate_unit: EnergyRateUnit,
        mass_estimate_lbs: f64,
        a_star_heuristic_energy_rate: Option<f64>,
        real_world_energy_adjustment: Option<f64>,
    ) -> Self {
        Self {
            name,
            model_input_file,
            model_type,
            input_features,
            energy_rate_unit,
            mass_estimate_lbs,
            a_star_heuristic_energy_rate,
            real_world_energy_adjustment,
        }
    }
}
