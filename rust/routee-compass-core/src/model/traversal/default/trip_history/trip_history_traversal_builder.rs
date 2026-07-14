use std::sync::Arc;

use super::{TripHistoryConfig, TripHistoryEngine, TripHistoryService};

use crate::model::traversal::{TraversalModelBuilder, TraversalModelError, TraversalModelService};

pub struct TripHistoryBuilder {}

impl TraversalModelBuilder for TripHistoryBuilder {
    fn build(
        &self,
        value: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {
        let config: TripHistoryConfig = serde_json::from_value(value.clone()).map_err(|e| {
            let msg = format!("failure reading config for TripHistory builder: {e}");
            TraversalModelError::BuildError(msg)
        })?;
        let engine = TripHistoryEngine::try_from(config).map_err(|e| {
            let msg = format!("failure building engine from config for TripHistory builder: {e}");
            TraversalModelError::BuildError(msg)
        })?;
        let service = TripHistoryService::new(engine);
        Ok(Arc::new(service))
    }
}
