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
            default::time::TimeTraversalConfig, TraversalModel, TraversalModelError,
            TraversalModelService,
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
                name: self.config.distance_name.clone(),
                unit: None,
            },
            InputFeature::Speed {
                name: self.config.speed_name.clone(),
                unit: None,
            },
        ]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        let mut features = vec![(
            self.config.time_name.clone(),
            StateVariableConfig::Time {
                initial: Time::ZERO,
                accumulator: false,
                output_unit: Some(self.config.time_unit),
            },
        )];
        if self.config.include_trip_time.unwrap_or(true) {
            features.push((
                self.config.trip_time_name.clone(),
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
        let distance: Length = state_model.get_distance(state, &self.config.distance_name)?;
        let speed: Velocity = state_model.get_speed(state, &self.config.speed_name)?;

        let edge_time = distance / speed;

        if self.config.include_trip_time.unwrap_or(true) {
            state_model.add_time(state, &self.config.trip_time_name, &edge_time)?;
        }
        state_model.add_time(state, &self.config.time_name, &edge_time)?;

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
        let speed = state_model.get_speed(state, &self.config.speed_name)?;
        let time = distance / speed;

        if self.config.include_trip_time.unwrap_or(true) {
            state_model.add_time(state, &self.config.trip_time_name, &time)?;
        }
        state_model.add_time(state, &self.config.time_name, &time)?;

        Ok(())
    }
}
