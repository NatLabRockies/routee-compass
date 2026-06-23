use std::sync::Arc;

use super::{TemplateEngine, TemplateModel, TemplateParams};

use routee_compass_core::model::{
    constraint::{ConstraintModel, ConstraintModelError, ConstraintModelService},
    state::StateModel,
};

pub struct TemplateService {
    engine: Arc<TemplateEngine>,
}

impl TemplateService {
    pub fn new(engine: TemplateEngine) -> Self {
        Self {
            engine: Arc::new(engine),
        }
    }
}

impl ConstraintModelService for TemplateService {
    fn build(
        &self,
        query: &serde_json::Value,
        #[allow(unused)] state_model: Arc<StateModel>,
    ) -> Result<Arc<dyn ConstraintModel>, ConstraintModelError> {
        let params: TemplateParams = serde_json::from_value(query.clone()).map_err(|e| {
            let msg = format!("failure reading params for Template service: {e}");
            ConstraintModelError::BuildError(msg)
        })?;
        let model = TemplateModel::new(self.engine.clone(), params);
        Ok(Arc::new(model))
    }
}
