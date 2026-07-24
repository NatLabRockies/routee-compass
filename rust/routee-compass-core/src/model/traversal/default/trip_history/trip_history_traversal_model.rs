use std::sync::Arc;

use uom::si::f64::{Energy, Length, Ratio, ThermodynamicTemperature, Time, Velocity};

use super::trip_history_traversal_engine::*;

use crate::{
    algorithm::search::SearchTree,
    model::{
        network::Vertex,
        state::{
            CustomVariableConfig, InputFeature, StateModel, StateVariable, StateVariableConfig,
        },
        traversal::{EdgeFrontierContext, TraversalModel, TraversalModelError},
    },
};

pub struct TripHistoryTraversalModel {
    pub engine: Arc<TripHistoryTraversalEngine>,
}

impl TripHistoryTraversalModel {
    pub fn new(engine: Arc<TripHistoryTraversalEngine>) -> Self {
        // modify this and the struct definition if additional pre-processing
        // is required during model instantiation from query parameters.
        Self { engine }
    }
}

impl TraversalModel for TripHistoryTraversalModel {
    fn name(&self) -> String {
        "TripHistoryModel".to_string()
    }

    fn input_features(&self) -> Vec<InputFeature> {
        // the set of input features from the trip history configuration
        self.engine
            .history_features
            .iter()
            .map(|feature| match &feature.state_variable_config {
                StateVariableConfig::Distance { output_unit, .. } => InputFeature::Distance {
                    name: feature.name.clone(),
                    unit: *output_unit,
                },
                StateVariableConfig::Time { output_unit, .. } => InputFeature::Time {
                    name: feature.name.clone(),
                    unit: *output_unit,
                },
                StateVariableConfig::Speed { output_unit, .. } => InputFeature::Speed {
                    name: feature.name.clone(),
                    unit: *output_unit,
                },
                StateVariableConfig::Energy { output_unit, .. } => InputFeature::Energy {
                    name: feature.name.clone(),
                    unit: *output_unit,
                },
                StateVariableConfig::Ratio { output_unit, .. } => InputFeature::Ratio {
                    name: feature.name.clone(),
                    unit: *output_unit,
                },
                StateVariableConfig::Temperature { output_unit, .. } => InputFeature::Temperature {
                    name: feature.name.clone(),
                    unit: *output_unit,
                },
                StateVariableConfig::Custom { custom_type, .. } => InputFeature::Custom {
                    name: feature.name.clone(),
                    unit: custom_type.clone(),
                },
            })
            .collect()
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        // below, f is "feature", d is "depth", m is number of features, n is max depth
        // output is: ["f1_d1", "f2_d1", ... "fm_d1", "f1_d2", "f2_d2", ..., "f1_dn",..."fm_dn"]
        (1..=self.engine.depth.get()) // depth_n
            .flat_map(|depth| {
                self.engine
                    .history_features
                    .iter() // feature_m
                    .map(move |feature| {
                        (
                            format!("{}_{depth}", feature.name),
                            // OVERWRITE the initial property with the required sentinel
                            set_initial_as_sentinel(feature.state_variable_config.clone()),
                        )
                    })
            })
            .collect()
    }

    fn traverse_edge(
        &self,
        _ctx: &EdgeFrontierContext,
        _state: &mut Vec<StateVariable>,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        self.engine.update_history(_ctx, _state, _state_model)?;
        Ok(())
    }

    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        _state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        Ok(())
    }
}

fn set_initial_as_sentinel(mut conf: StateVariableConfig) -> StateVariableConfig {
    match &mut conf {
        StateVariableConfig::Distance { initial, .. } => {
            *initial = Length::new::<uom::si::length::meter>(f64::NAN);
        }
        StateVariableConfig::Time { initial, .. } => {
            *initial = Time::new::<uom::si::time::second>(f64::NAN);
        }
        StateVariableConfig::Speed { initial, .. } => {
            *initial = Velocity::new::<uom::si::velocity::meter_per_second>(f64::NAN);
        }
        StateVariableConfig::Energy { initial, .. } => {
            *initial = Energy::new::<uom::si::energy::joule>(f64::NAN);
        }
        StateVariableConfig::Ratio { initial, .. } => {
            *initial = Ratio::new::<uom::si::ratio::ratio>(f64::NAN);
        }
        StateVariableConfig::Temperature { initial, .. } => {
            *initial = ThermodynamicTemperature::new::<
                uom::si::thermodynamic_temperature::degree_celsius,
            >(f64::NAN);
        }
        StateVariableConfig::Custom { value, .. } => match value {
            CustomVariableConfig::FloatingPoint { initial } => {
                *initial = ordered_float::OrderedFloat(f64::NAN)
            }
            CustomVariableConfig::SignedInteger { initial } => *initial = -1,
            CustomVariableConfig::UnsignedInteger { initial } => *initial = u64::MAX,
            CustomVariableConfig::Boolean { initial } => *initial = false,
        },
    }
    conf
}
