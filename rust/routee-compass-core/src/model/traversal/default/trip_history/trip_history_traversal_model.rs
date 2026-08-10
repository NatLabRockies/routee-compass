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
            .map(|feature| {
                InputFeature::from_state_variable_config(
                    &feature.name,
                    &feature.state_variable_config,
                )
            })
            .collect()
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        // below, f is "feature", d is "depth", m is number of features, n is max depth
        // output is: ["f1_d0", "f1_d1", "f2_d0", ... "fm_d0", "f1_d1", "f2_d1", ..., "f1_dn",..."fm_dn"]
        (0..=self.engine.depth.get())
            .flat_map(|depth| {
                self.engine.history_features.iter().map(move |feature| {
                    (
                        format!("{}_{depth}", feature.name),
                        set_initial_as_sentinel(feature.state_variable_config.clone()),
                    )
                })
            })
            .collect()
    }

    fn traverse_edge(
        &self,
        _ctx: &EdgeFrontierContext,
        state: &mut Vec<StateVariable>,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        self.engine.update_history(state, state_model)?;
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

/// Sets the unset (aka initial) value of a state variable to a sentinel value.
///
/// Sentinels for each underlying type:
/// * `f64`: `NAN`
/// * `i64`: `-1`
/// * `u64`: `u64::MAX`
/// * `boolean`: `false`
fn set_initial_as_sentinel(mut conf: StateVariableConfig) -> StateVariableConfig {
    match &mut conf {
        StateVariableConfig::Distance {
            initial,
            accumulator,
            ..
        } => {
            *initial = Length::new::<uom::si::length::meter>(f64::NAN);
            *accumulator = true;
        }
        StateVariableConfig::Time {
            initial,
            accumulator,
            ..
        } => {
            *initial = Time::new::<uom::si::time::second>(f64::NAN);
            *accumulator = true;
        }
        StateVariableConfig::Speed {
            initial,
            accumulator,
            ..
        } => {
            *initial = Velocity::new::<uom::si::velocity::meter_per_second>(f64::NAN);
            *accumulator = true;
        }
        StateVariableConfig::Energy {
            initial,
            accumulator,
            ..
        } => {
            *initial = Energy::new::<uom::si::energy::joule>(f64::NAN);
            *accumulator = true;
        }
        StateVariableConfig::Ratio {
            initial,
            accumulator,
            ..
        } => {
            *initial = Ratio::new::<uom::si::ratio::ratio>(f64::NAN);
            *accumulator = true;
        }
        StateVariableConfig::Temperature {
            initial,
            accumulator,
            ..
        } => {
            *initial = ThermodynamicTemperature::new::<
                uom::si::thermodynamic_temperature::degree_celsius,
            >(f64::NAN);
            *accumulator = true;
        }
        StateVariableConfig::Custom {
            value, accumulator, ..
        } => {
            *accumulator = true;
            match value {
                CustomVariableConfig::FloatingPoint { initial } => {
                    *initial = ordered_float::OrderedFloat(f64::NAN)
                }
                CustomVariableConfig::SignedInteger { initial } => *initial = -1,
                CustomVariableConfig::UnsignedInteger { initial } => *initial = u64::MAX,
                CustomVariableConfig::Boolean { initial } => *initial = false,
            }
        }
    }
    conf
}
