use crate::model::{
    constraint::{error::ConstraintModelError, ConstraintModel, ConstraintModelService},
    state::StateModel,
    traversal::EdgeFrontierContext,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct NoRestriction {}

impl ConstraintModel for NoRestriction {
    fn valid_frontier(
        &self,
        _ctx: &EdgeFrontierContext,
        _state: &[crate::model::state::StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        Ok(true)
    }

    fn valid_edge(
        &self,
        _edge: &crate::model::network::Edge,
    ) -> Result<bool, ConstraintModelError> {
        Ok(true)
    }
}

impl ConstraintModelService for NoRestriction {
    fn build(
        &self,
        _query: &serde_json::Value,
        _state_model: Arc<StateModel>,
    ) -> Result<Arc<dyn ConstraintModel>, ConstraintModelError> {
        Ok(Arc::new(self.clone()))
    }
}
