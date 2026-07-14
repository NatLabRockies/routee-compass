use std::sync::Arc;

use super::{TripHistoryEngine, TripHistoryModel, TripHistoryParams};

use crate::model::traversal::{TraversalModel, TraversalModelError, TraversalModelService};

pub struct TripHistoryService {
    engine: Arc<TripHistoryEngine>,
}

impl TripHistoryService {
    pub fn new(engine: TripHistoryEngine) -> Self {
        Self {
            engine: Arc::new(engine),
        }
    }
}

impl TraversalModelService for TripHistoryService {
    fn build(
        &self,
        query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let params: TripHistoryParams = serde_json::from_value(query.clone()).map_err(|e| {
            let msg = format!("failure reading params for TripHistory service: {e}");
            TraversalModelError::BuildError(msg)
        })?;
        let model = TripHistoryModel::new(self.engine.clone(), params);
        Ok(Arc::new(model))
    }
}
