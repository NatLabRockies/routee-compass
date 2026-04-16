use uom::{si::f64::Time, ConstZero};

use super::TurnDelayTraversalModelEngine;
use crate::{
    algorithm::search::SearchTree,
    model::{
        network::Vertex,
        state::{StateModel, StateVariable, StateVariableConfig},
        traversal::{default::fieldname, TraversalModel, TraversalModelError},
        unit::TimeUnit,
    },
};
use std::sync::Arc;

pub struct TurnDelayTraversalModel {
    pub engine: Arc<TurnDelayTraversalModelEngine>,
    pub include_trip_time: bool,
}

impl TurnDelayTraversalModel {
    pub fn new(engine: Arc<TurnDelayTraversalModelEngine>, include_trip_time: bool) -> Self {
        TurnDelayTraversalModel {
            engine,
            include_trip_time,
        }
    }
}

impl TraversalModel for TurnDelayTraversalModel {
    fn name(&self) -> String {
        "Turn Delay Traversal Model".to_string()
    }

    fn input_features(&self) -> Vec<crate::model::state::InputFeature> {
        vec![]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        let mut features = vec![
            (
                String::from(fieldname::EDGE_TURN_DELAY),
                StateVariableConfig::Time {
                    initial: Time::ZERO,
                    accumulator: false,
                    output_unit: Some(TimeUnit::Seconds),
                },
            ),
            (
                String::from(fieldname::EDGE_TIME),
                StateVariableConfig::Time {
                    initial: Time::ZERO,
                    accumulator: false,
                    output_unit: None,
                },
            ),
        ];
        if self.include_trip_time {
            features.push((
                String::from(fieldname::TRIP_TIME),
                StateVariableConfig::Time {
                    initial: Time::ZERO,
                    accumulator: true,
                    output_unit: None,
                },
            ));
        }
        features
    }

    fn traverse_edge(
        &self,
        ctx: &crate::model::traversal::EdgeFrontierContext,
        state: &mut Vec<StateVariable>,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let prev_edge_traversal = ctx.previous_edge_traversal().map_err(|e| {
            let msg = format!("while accessing previous edge during turn delay calculation, {e}");
            TraversalModelError::TraversalModelFailure(msg)
        })?;
        let prev_edge_id = match prev_edge_traversal {
            Some(prev_traversal) => prev_traversal.edge_id,
            None => return Ok(()), // no previous edge, no turn delay to apply
        };

        let delay = self.engine.get_delay(prev_edge_id, ctx.edge.edge_id)?;
        state_model.set_time(state, fieldname::EDGE_TURN_DELAY, &delay)?;
        state_model.add_time(state, fieldname::EDGE_TIME, &delay)?;
        if self.include_trip_time {
            state_model.add_time(state, fieldname::TRIP_TIME, &delay)?;
        }

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
