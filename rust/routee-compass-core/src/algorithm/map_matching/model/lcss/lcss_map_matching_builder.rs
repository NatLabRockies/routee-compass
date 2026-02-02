use crate::algorithm::map_matching::{
    map_matching_algorithm::MapMatchingAlgorithm, map_matching_builder::MapMatchingBuilder,
    map_matching_error::MapMatchingError,
};
use std::sync::Arc;

use super::{lcss_map_matching::LcssConfig, LcssMapMatching};

pub struct LcssMapMatchingBuilder;

impl MapMatchingBuilder for LcssMapMatchingBuilder {
    fn build(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn MapMatchingAlgorithm>, MapMatchingError> {
        let lcss_config: LcssConfig = serde_json::from_value(config.clone()).map_err(|e| {
            MapMatchingError::InternalError(format!(
                "failed to deserialize LCSS map matching config: {}",
                e
            ))
        })?;

        log::debug!("LCSS map matching configured: {:?}", lcss_config);

        let alg = LcssMapMatching::from_config(lcss_config)?;
        Ok(Arc::new(alg))
    }
}
