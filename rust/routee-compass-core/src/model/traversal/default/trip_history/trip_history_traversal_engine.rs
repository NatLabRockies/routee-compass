use std::num::{NonZero, NonZeroUsize};

use super::TripHistoryConfig;
use crate::model::state::{InputFeature, StateModel, StateVariable, StateVariableConfig};
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
        state_model: &StateModel,
    ) {
        self.shift(ctx, state, state_model);
        self.insert_first(ctx);
    }
    /// Takes all values from depth 1..(depth) and shifts the value from
    /// `format!({feature_name}_{depth_value}` to `format!("{feature_name}_{depth_value+1}")`
    /// in the state vector, shifting values one link into "the past".
    pub fn shift(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
        state_model: &StateModel,
    ) {
        todo!();
    }
    /// Traverse one step into the history via `ctx.tree.backtrack_with_depth(state_variable, depth)`
    /// and record the value at `format!({feature_name}_1")`. This must be run after `shift` to avoid
    /// overwriting the first history value before shifting. if the backtrack result is empty,
    /// do nothing.
    fn insert_first(&self, ctx: &EdgeFrontierContext) {
        todo!();
    }
}
