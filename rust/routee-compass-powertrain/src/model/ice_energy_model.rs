use super::prediction::PredictionModelConfig;
use crate::model::{fieldname, prediction::PredictionModelRecord};
use routee_compass_core::{
    algorithm::search::SearchTree,
    model::{
        network::{Edge, Vertex},
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{TraversalModel, TraversalModelError, TraversalModelService},
        unit::{EnergyRateUnit, EnergyUnit},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uom::{si::f64::Energy, ConstZero};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct IceEnergyModelConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(flatten)]
    pub prediction_model: PredictionModelConfig,
    pub include_trip_energy: Option<bool>,
}

#[derive(Clone)]
pub struct IceEnergyModel {
    pub prediction_model_record: Arc<PredictionModelRecord>,
    pub include_trip_energy: bool,
}

impl IceEnergyModel {
    pub fn new(
        prediction_model_record: PredictionModelRecord,
        include_trip_energy: bool,
    ) -> Result<Self, TraversalModelError> {
        Ok(Self {
            prediction_model_record: Arc::new(prediction_model_record),
            include_trip_energy,
        })
    }
}

impl TraversalModelService for IceEnergyModel {
    fn build(
        &self,
        _query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        Ok(Arc::new(self.clone()))
    }
}

impl TryFrom<&Value> for IceEnergyModel {
    type Error = TraversalModelError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let config: IceEnergyModelConfig = serde_json::from_value(value.clone()).map_err(|e| {
            TraversalModelError::BuildError(format!(
                "failure reading ICE energy configuration: {e}"
            ))
        })?;
        let prediction_model = PredictionModelRecord::try_from(&config.prediction_model)?;
        let ice_model =
            IceEnergyModel::new(prediction_model, config.include_trip_energy.unwrap_or(true))?;
        Ok(ice_model)
    }
}

impl TraversalModel for IceEnergyModel {
    fn name(&self) -> String {
        format!("ICE Energy Model: {}", self.prediction_model_record.name)
    }
    fn input_features(&self) -> Vec<InputFeature> {
        let mut input_features = vec![InputFeature::Distance {
            name: String::from(fieldname::EDGE_DISTANCE),
            unit: None,
        }];
        input_features.extend(self.prediction_model_record.input_features.clone());
        input_features
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        let mut features = vec![(
            String::from(fieldname::EDGE_ENERGY_LIQUID),
            StateVariableConfig::Energy {
                initial: Energy::ZERO,
                accumulator: false,
                output_unit: Some(
                    self.prediction_model_record
                        .energy_rate_unit
                        .associated_energy_unit(),
                ),
            },
        )];
        if self.include_trip_energy {
            features.push((
                String::from(fieldname::TRIP_ENERGY_LIQUID),
                StateVariableConfig::Energy {
                    initial: Energy::ZERO,
                    accumulator: true,
                    output_unit: Some(
                        self.prediction_model_record
                            .energy_rate_unit
                            .associated_energy_unit(),
                    ),
                },
            ));
        }
        features
    }

    fn traverse_edge(
        &self,
        _trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        ice_traversal(
            state,
            state_model,
            self.prediction_model_record.clone(),
            self.include_trip_energy,
            false,
        )
    }

    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        ice_traversal(
            state,
            state_model,
            self.prediction_model_record.clone(),
            self.include_trip_energy,
            true,
        )
    }
}

fn ice_traversal(
    state: &mut [StateVariable],
    state_model: &StateModel,
    record: Arc<PredictionModelRecord>,
    include_trip_energy: bool,
    estimate: bool,
) -> Result<(), TraversalModelError> {
    let distance = state_model.get_distance(state, fieldname::EDGE_DISTANCE)?;

    // generate energy for link traversal
    let energy = if estimate {
        match record.energy_rate_unit {
            EnergyRateUnit::GGPM => {
                let distance_miles = distance.get::<uom::si::length::mile>();
                let energy_gallons_gas = record.a_star_heuristic_energy_rate * distance_miles;
                EnergyUnit::GallonsGasolineEquivalent.to_uom(energy_gallons_gas)
            }
            EnergyRateUnit::GDPM => {
                let distance_miles = distance.get::<uom::si::length::mile>();
                let energy_gallons_diesel = record.a_star_heuristic_energy_rate * distance_miles;
                EnergyUnit::GallonsDieselEquivalent.to_uom(energy_gallons_diesel)
            }
            _ => {
                return Err(TraversalModelError::InternalError(format!(
                    "Unsupported energy rate unit: {}",
                    record.energy_rate_unit
                )));
            }
        }
    } else {
        record.predict(state, state_model)?
    };

    if include_trip_energy {
        state_model.add_energy(state, fieldname::TRIP_ENERGY_LIQUID, &energy)?;
    }
    state_model.set_energy(state, fieldname::EDGE_ENERGY_LIQUID, &energy)?;
    Ok(())
}
