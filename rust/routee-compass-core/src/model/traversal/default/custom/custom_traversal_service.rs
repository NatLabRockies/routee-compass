use super::{CustomTraversalEngine, CustomTraversalModel};
use crate::model::traversal::{
    error::TraversalModelError, model::TraversalModel, service::TraversalModelService,
};
use std::sync::Arc;

pub struct CustomTraversalService {
    pub engine: Arc<CustomTraversalEngine>,
}

impl TraversalModelService for CustomTraversalService {
    fn build(
        &self,
        _parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let model = CustomTraversalModel::new(self.engine.clone());
        Ok(Arc::new(model))
    }
}
