use super::SpeedLookupService;
use super::SpeedModelConfig;
use super::SpeedTraversalEngine;
use crate::model::traversal::TraversalModelBuilder;
use crate::model::traversal::TraversalModelError;
use crate::model::traversal::TraversalModelService;
use std::sync::Arc;

pub struct SpeedTraversalBuilder {}

impl TraversalModelBuilder for SpeedTraversalBuilder {
    fn build(
        &self,
        params: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {
        let config: SpeedModelConfig = serde_json::from_value(params.clone()).map_err(|e| {
            TraversalModelError::BuildError(format!("failed to read speed configuration: {e}"))
        })?;

        let e = SpeedTraversalEngine::new(config)?;
        let service = Arc::new(SpeedLookupService { e: Arc::new(e) });
        Ok(service)
    }
}
