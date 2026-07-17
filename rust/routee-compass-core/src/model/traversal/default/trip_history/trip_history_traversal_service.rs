use std::sync::Arc;

use super::{TripHistoryEngine, TripHistoryModel};

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
        _query: &serde_json::Value, // no query needed for trip history.
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let model = TripHistoryModel::new(self.engine.clone());
        Ok(Arc::new(model))
    }
}
