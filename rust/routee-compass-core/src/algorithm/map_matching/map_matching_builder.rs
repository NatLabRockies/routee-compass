use super::{map_matching_algorithm::MapMatchingAlgorithm, map_matching_error::MapMatchingError};
use std::sync::Arc;

pub trait MapMatchingBuilder: Send + Sync {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn MapMatchingAlgorithm>, MapMatchingError>;
}
