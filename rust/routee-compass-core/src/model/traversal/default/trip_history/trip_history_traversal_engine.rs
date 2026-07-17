use super::TripHistoryConfig;
use crate::model::state::{StateVariable, StateVariableConfig};
use crate::model::traversal::{EdgeFrontierContext, TraversalModelError};

pub struct TripHistoryEngine {
    pub input_features: Vec<StateVariableConfig>,
    pub depth: std::num::NonZeroUsize,
}

impl TryFrom<TripHistoryConfig> for TripHistoryEngine {
    type Error = TraversalModelError;

    fn try_from(config: TripHistoryConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            input_features: config.input_features,
            depth: config.depth,
        })
    }
}

impl TripHistoryEngine {
    pub fn update_history(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
    ) -> Result<(), TraversalModelError> {
        self.shift(state);
        self.insert_first(ctx, state)?;
        Ok(())
    }
    /// Takes all values from depth 1..(depth) and shifts the value from
    /// `format!({feature_name}_{depth_value}` to `format!("{feature_name}_{depth_value+1}")`
    /// in the state vector, shifting values one link into "the past".
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
