use std::sync::Arc;

use super::{TripHistoryTraversalEngine, TripHistoryTraversalModel};

use crate::model::traversal::{TraversalModel, TraversalModelError, TraversalModelService};

pub struct TripHistoryTraversalService {
    engine: Arc<TripHistoryTraversalEngine>,
}

impl TripHistoryTraversalService {
    pub fn new(engine: TripHistoryTraversalEngine) -> Self {
        Self {
            engine: Arc::new(engine),
        }
    }
}

impl TraversalModelService for TripHistoryTraversalService {
    fn build(
        &self,
        _query: &serde_json::Value, // no query needed for trip history.
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let model = TripHistoryTraversalModel::new(self.engine.clone());
        Ok(Arc::new(model))
    }
}
