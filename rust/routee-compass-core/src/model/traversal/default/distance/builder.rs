use super::DistanceTraversalService;
use crate::model::traversal::default::distance::DistanceTraversalConfig;
use crate::model::traversal::TraversalModelBuilder;
use crate::model::traversal::TraversalModelError;
use crate::model::traversal::TraversalModelService;
use std::sync::Arc;

pub struct DistanceTraversalBuilder {}

impl TraversalModelBuilder for DistanceTraversalBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {
        let config: DistanceTraversalConfig =
            serde_json::from_value(parameters.clone()).map_err(|e| {
                TraversalModelError::BuildError(format!("while reading distance config, {e}"))
            })?;

        let m: Arc<dyn TraversalModelService> = Arc::new(DistanceTraversalService::new(config));
        Ok(m)
    }
}
