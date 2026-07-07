use crate::model::{
    constraint::{ConstraintModel, ConstraintModelError, ConstraintModelService},
    state::StateModel,
};
use serde_json;
use std::{collections::HashMap, sync::Arc};
pub struct CategoricalModelService {
    pub key: Arc<String>,                           // the category name
    pub category_by_edge: Arc<Box<[u8]>>,           // categorizes each edge with the encoding
    pub category_mapping: Arc<HashMap<String, u8>>, // maps the categories (String) to the encoding (u8)
}

impl ConstraintModelService for CategoricalModelService {
    fn build(
        &self,
        query: &serde_json::Value,
        state_model: Arc<StateModel>,
    ) -> Result<Arc<dyn ConstraintModel>, ConstraintModelError> {
        todo!();
    }
}
