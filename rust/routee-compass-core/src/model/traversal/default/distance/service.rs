use super::DistanceTraversalModel;
use crate::model::traversal::default::distance::DistanceTraversalConfig;
use crate::model::traversal::model::TraversalModel;
use crate::model::traversal::TraversalModelError;
use crate::model::traversal::TraversalModelService;
use std::sync::Arc;

pub struct DistanceTraversalService {
    pub config: Arc<DistanceTraversalConfig>,
}

impl DistanceTraversalService {
    pub fn new(config: DistanceTraversalConfig) -> DistanceTraversalService {
        Self {
            config: Arc::new(config),
        }
    }
}

impl TraversalModelService for DistanceTraversalService {
    fn build(
        &self,
        _parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let m: Arc<dyn TraversalModel> = Arc::new(DistanceTraversalModel::new(self.config.clone()));
        Ok(m)
    }
}
