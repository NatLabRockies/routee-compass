use std::sync::Arc;

use uom::{si::f64::Length, ConstZero};

use crate::{
    algorithm::search::SearchGraph,
    model::{
        network::{Edge, Vertex},
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{
            default::fieldname, TraversalModel, TraversalModelError, TraversalModelService,
        },
        unit::DistanceUnit,
    },
};

use super::elevation_change::ElevationChange;

#[derive(Clone, Debug)]
pub struct ElevationTraversalModel {}

impl TraversalModelService for ElevationTraversalModel {
    fn build(
        &self,
        _query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        Ok(Arc::new(self.clone()))
    }
}

impl TraversalModel for ElevationTraversalModel {
    fn name(&self) -> String {
        String::from("Elevation Traversal Model")
    }
    fn input_features(&self) -> Vec<InputFeature> {
        vec![
            InputFeature::Distance {
                name: String::from(fieldname::EDGE_DISTANCE),
                unit: None,
            },
            InputFeature::Ratio {
                name: String::from(fieldname::EDGE_GRADE),
                unit: None,
            },
        ]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        vec![
            (
                String::from(fieldname::TRIP_ELEVATION_GAIN),
                StateVariableConfig::Distance {
                    initial: Length::ZERO,
                    accumulator: true,
                    output_unit: Some(DistanceUnit::default()),
                },
            ),
            (
                String::from(fieldname::TRIP_ELEVATION_LOSS),
                StateVariableConfig::Distance {
                    initial: Length::ZERO,
                    accumulator: true,
                    output_unit: Some(DistanceUnit::default()),
                },
            ),
        ]
    }

    /// compute the change in elevation along this edge and store it to the state vector
    fn traverse_edge(
        &self,
        _trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let distance = state_model.get_distance(state, fieldname::EDGE_DISTANCE)?;
        let grade = state_model.get_ratio(state, fieldname::EDGE_GRADE)?;
        let elevation_change = ElevationChange::new(distance, grade).map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!("Elevation change error: {e}"))
        })?;
        elevation_change.add_elevation_to_state(state, state_model)?;
        Ok(())
    }

    /// we do not currently have the data available to estimate elevation changes using only pairs of coordinates
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
