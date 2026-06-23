use std::sync::Arc;

use super::{TemplateEngine, TemplateParams};

use routee_compass_core::model::{
    constraint::{ConstraintModel, ConstraintModelError},
    network::Edge,
    state::{StateModel, StateVariable},
    traversal::EdgeFrontierContext,
};

pub struct TemplateModel {
    pub engine: Arc<TemplateEngine>,
    pub params: TemplateParams,
}

impl TemplateModel {
    pub fn new(engine: Arc<TemplateEngine>, params: TemplateParams) -> Self {
        // modify this and the struct definition if additional pre-processing
        // is required during model instantiation from query parameters.
        Self { engine, params }
    }
}

impl ConstraintModel for TemplateModel {
    fn valid_frontier(
        &self,
        _ctx: &EdgeFrontierContext,
        _state: &[StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        todo!()
    }

    fn valid_edge(&self, _edge: &Edge) -> Result<bool, ConstraintModelError> {
        todo!()
    }
}
