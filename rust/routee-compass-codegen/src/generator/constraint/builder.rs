use std::sync::Arc;

use super::{TemplateConfig, TemplateEngine, TemplateService};

use routee_compass_core::model::constraint::{
    ConstraintModelBuilder, ConstraintModelError, ConstraintModelService,
};

pub struct TemplateBuilder {}

impl ConstraintModelBuilder for TemplateBuilder {
    fn build(
        &self,
        value: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, ConstraintModelError> {
        let config: TemplateConfig = serde_json::from_value(value.clone()).map_err(|e| {
            let msg = format!("failure reading config for Template builder: {e}");
            ConstraintModelError::BuildError(msg)
        })?;
        let engine = TemplateEngine::try_from(config).map_err(|e| {
            let msg = format!("failure building engine from config for Template builder: {e}");
            ConstraintModelError::BuildError(msg)
        })?;
        let service = TemplateService::new(engine);
        Ok(Arc::new(service))
    }
}
