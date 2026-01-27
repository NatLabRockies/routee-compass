use crate::algorithm::map_matching::{
    map_matching_algorithm::MapMatchingAlgorithm, map_matching_builder::MapMatchingBuilder,
    map_matching_error::MapMatchingError,
};
use std::sync::Arc;

use super::SimpleMapMatching;

pub struct SimpleMapMatchingBuilder;

impl MapMatchingBuilder for SimpleMapMatchingBuilder {
    fn build(
        &self,
        _parameters: &serde_json::Value,
    ) -> Result<Arc<dyn MapMatchingAlgorithm>, MapMatchingError> {
        let alg = SimpleMapMatching::new();
        Ok(Arc::new(alg))
    }
}
