use std::sync::Arc;

use super::{TripHistoryTraversalConfig, TripHistoryTraversalEngine, TripHistoryTraversalService};

use crate::model::traversal::{TraversalModelBuilder, TraversalModelError, TraversalModelService};

pub struct TripHistoryTraversalBuilder {}

impl TraversalModelBuilder for TripHistoryTraversalBuilder {
    fn build(
        &self,
        value: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {
        let config: TripHistoryTraversalConfig =
            serde_json::from_value(value.clone()).map_err(|e| {
                let msg = format!("failure reading config for TripHistory builder: {e}");
                TraversalModelError::BuildError(msg)
            })?;
        let engine = TripHistoryTraversalEngine::try_from(config).map_err(|e| {
            let msg = format!("failure building engine from config for TripHistory builder: {e}");
            TraversalModelError::BuildError(msg)
        })?;
        let service = TripHistoryTraversalService::new(engine);
        Ok(Arc::new(service))
    }
}
