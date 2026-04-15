use uom::{
    si::f64::{Length, Time, Velocity},
    ConstZero,
};

use crate::{
    algorithm::search::SearchTree,
    model::{
        network::Vertex,
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{
            default::{fieldname, time::TimeTraversalConfig},
            TraversalModel, TraversalModelError, TraversalModelService,
        },
    },
    util::geo::haversine,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TimeTraversalModel {
    config: TimeTraversalConfig,
}

impl TimeTraversalModel {
    pub fn new(config: TimeTraversalConfig) -> TimeTraversalModel {
        TimeTraversalModel { config }
    }
}

impl TraversalModelService for TimeTraversalModel {
    fn build(
        &self,
        _query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        Ok(Arc::new(self.clone()))
    }
}

impl TraversalModel for TimeTraversalModel {
    fn name(&self) -> String {
        String::from("Time Traversal Model")
    }
    fn input_features(&self) -> Vec<InputFeature> {
        vec![
            InputFeature::Distance {
                name: String::from(fieldname::EDGE_DISTANCE),
                unit: None,
            },
            InputFeature::Speed {
                name: String::from(fieldname::EDGE_SPEED),
                unit: None,
            },
        ]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        let mut features = vec![(
            String::from(fieldname::EDGE_TIME),
            StateVariableConfig::Time {
                initial: Time::ZERO,
                accumulator: false,
                output_unit: Some(self.config.time_unit),
            },
        )];
        if self.config.include_trip_time.unwrap_or(true) {
            features.push((
                String::from(fieldname::TRIP_TIME),
                StateVariableConfig::Time {
                    initial: Time::ZERO,
                    accumulator: true,
                    output_unit: Some(self.config.time_unit),
                },
            ));
        }
        features
    }

    fn traverse_edge(
        &self,
        _ctx: &crate::model::traversal::EdgeFrontierContext,
        state: &mut Vec<StateVariable>,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let distance: Length = state_model.get_distance(state, fieldname::EDGE_DISTANCE)?;
        let speed: Velocity = state_model.get_speed(state, fieldname::EDGE_SPEED)?;

        let edge_time = distance / speed;

        if self.config.include_trip_time.unwrap_or(true) {
            state_model.add_time(state, fieldname::TRIP_TIME, &edge_time)?;
        }
        state_model.add_time(state, fieldname::EDGE_TIME, &edge_time)?;

        Ok(())
    }

    fn estimate_traversal(
        &self,
        od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let (src, dst) = od;
        let distance: Length = haversine::coord_distance(&src.coordinate, &dst.coordinate)
            .map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "could not compute haversine distance between {src} and {dst}: {e}"
                ))
            })?;

        if distance == Length::ZERO {
            return Ok(());
        }
        let speed = state_model.get_speed(state, fieldname::EDGE_SPEED)?;
        let time = distance / speed;

        if self.config.include_trip_time.unwrap_or(true) {
            state_model.add_time(state, fieldname::TRIP_TIME, &time)?;
        }
        state_model.add_time(state, fieldname::EDGE_TIME, &time)?;

        Ok(())
    }
}
