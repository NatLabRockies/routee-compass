use std::sync::Arc;

use super::{TemplateEngine, TemplateModel, TemplateParams};

use crate::model::traversal::{TraversalModel, TraversalModelError, TraversalModelService};

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

impl TraversalModelService for TemplateService {
    fn build(
        &self,
        query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let params: TemplateParams = serde_json::from_value(query.clone()).map_err(|e| {
            let msg = format!("failure reading params for Template service: {e}");
            TraversalModelError::BuildError(msg)
        })?;
        let model = TemplateModel::new(self.engine.clone(), params);
        Ok(Arc::new(model))
    }
}
