use uom::{si::f64::ThermodynamicTemperature, ConstZero};

use crate::{
    algorithm::search::SearchGraph,
    model::{
        network::{Edge, Vertex},
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{default::fieldname, TraversalModel, TraversalModelError},
        unit::TemperatureUnit,
    },
};

#[derive(Clone, Debug)]
pub struct TemperatureTraversalModel {
    pub ambient_temperature: ThermodynamicTemperature,
}

impl TraversalModel for TemperatureTraversalModel {
    fn name(&self) -> String {
        String::from("Temperature Traversal Model")
    }
    fn input_features(&self) -> Vec<InputFeature> {
        vec![]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        vec![(
            String::from(fieldname::AMBIENT_TEMPERATURE),
            StateVariableConfig::Temperature {
                initial: ThermodynamicTemperature::ZERO,
                accumulator: false,
                output_unit: Some(TemperatureUnit::Fahrenheit),
            },
        )]
    }

    fn traverse_edge(
        &self,
        _trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        state_model.set_temperature(
            state,
            fieldname::AMBIENT_TEMPERATURE,
            &self.ambient_temperature,
        )?;
        Ok(())
    }

    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        _state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        Ok(())
    }
}
