use super::TripHistoryTraversalConfig;
use crate::model::state::{StateVariable, StateVariableConfig};
use crate::model::traversal::{EdgeFrontierContext, TraversalModelError};

pub struct TripHistoryTraversalEngine {
    pub input_features: Vec<StateVariableConfig>,
    pub depth: std::num::NonZeroUsize,
}

impl TryFrom<TripHistoryTraversalConfig> for TripHistoryTraversalEngine {
    type Error = TraversalModelError;

    fn try_from(config: TripHistoryTraversalConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            input_features: config.input_features,
            depth: config.depth,
        })
    }
}

impl TripHistoryTraversalEngine {
    pub fn update_history(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
    ) -> Result<(), TraversalModelError> {
        self.shift(state);
        self.insert_first(ctx, state)?;
        Ok(())
    }
    /// Takes all values in the state vector from depth 1..(depth) and shifts them from
    /// `format!({feature_name}_{depth_value}` to `format!("{feature_name}_{depth_value+1}")`
    /// shifting values one link into "the past".
    pub fn shift(&self, state: &mut [StateVariable]) {
        state.rotate_right(self.input_features.len()); // deepest features are placed at the front to be overwritten in insert_first()
    }
    /// Traverse one step into the history via `ctx.tree.backtrack_with_depth(state_variable, depth)`
    /// and record the value at `format!({feature_name}_1")`. This must be run after `shift` to avoid
    /// overwriting the first history value before shifting. if the backtrack result is empty,
    /// do nothing.
    fn insert_first(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
    ) -> Result<(), TraversalModelError> {
        let prev_edge = ctx.tree.backtrack_with_depth(ctx.src.vertex_id, 1)?;
        let prev_state = prev_edge
            .first()
            .ok_or_else(|| {
                TraversalModelError::TraversalModelFailure(
                    "when traversing the trip history, could not find the previous edge traversal"
                        .to_string(),
                )
            })?
            .result_state
            .clone();
        let num_features = self.input_features.len();
        state[0..num_features].copy_from_slice(&prev_state[0..num_features]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /*
    use crate::model::state::{StateModel, StateVariableConfig};
    use crate::model::traversal::default::{
        distance::DistanceTraversalModel,
        time::{TimeTraversalConfig, TimeTraversalModel},
    };
    */
    use crate::model::unit::{DistanceUnit, TimeUnit};
    use std::num::NonZeroUsize;
    use uom::si::{f64::Length, f64::Time, length::meter, time::second};
    #[test]
    fn test_shift() {
        // Mock StateVariableConfig for distance and time.
        let dist_cfg = StateVariableConfig::Distance {
            initial: Length::new::<meter>(0.0),
            accumulator: false,
            output_unit: Some(DistanceUnit::Meters),
        };
        let time_cfg = StateVariableConfig::Time {
            initial: Time::new::<second>(0.0),
            accumulator: false,
            output_unit: Some(TimeUnit::Seconds),
        };
        // Mock trip history engine for shift()
        let trip_history_engine =
            TripHistoryTraversalEngine::try_from(TripHistoryTraversalConfig {
                input_features: vec![time_cfg, dist_cfg],
                depth: NonZeroUsize::new(5).unwrap(),
            })
            .unwrap();

        // The input state vector for shift
        let state = &mut [
            StateVariable(0.0),      // t, depth = 1
            StateVariable(0.0),      // d, depth = 1
            StateVariable(f64::NAN), // t, depth = 2
            StateVariable(f64::NAN), // d, depth = 2
            StateVariable(f64::NAN), // t, depth = 3
            StateVariable(f64::NAN), // d, depth = 3
            StateVariable(f64::NAN), // t, depth = 4
            StateVariable(f64::NAN), // d, depth = 4
            StateVariable(f64::NAN), // t, depth = 5
            StateVariable(f64::NAN), // d, depth = 5
        ];
        trip_history_engine.shift(state);

        // expected post-shift
        assert!(state[0].0.is_nan());
        assert!(state[1].0.is_nan());
        assert_eq!(state[2].0, 0.0);
        assert_eq!(state[3].0, 0.0);
        assert!(state[4].0.is_nan());
        assert!(state[5].0.is_nan());
        assert!(state[6].0.is_nan());
        assert!(state[7].0.is_nan());
        assert!(state[8].0.is_nan());
        assert!(state[9].0.is_nan());
    }

    #[test]
    fn test_insert_first() {}
}
