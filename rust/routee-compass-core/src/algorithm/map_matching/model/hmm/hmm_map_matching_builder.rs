use crate::algorithm::map_matching::{
    map_matching_algorithm::MapMatchingAlgorithm, map_matching_builder::MapMatchingBuilder,
    map_matching_error::MapMatchingError,
};
use std::sync::Arc;

use super::HmmMapMatching;

pub struct HmmMapMatchingBuilder;

impl MapMatchingBuilder for HmmMapMatchingBuilder {
    fn build(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn MapMatchingAlgorithm>, MapMatchingError> {
        let defaults = HmmMapMatching::default();

        let sigma = config
            .get("sigma")
            .and_then(|v| v.as_f64())
            .unwrap_or(defaults.sigma);

        let beta = config
            .get("beta")
            .and_then(|v| v.as_f64())
            .unwrap_or(defaults.beta);

        let max_candidates = config
            .get("max_candidates")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(defaults.max_candidates);

        let search_parameters = config
            .get("search_parameters")
            .cloned()
            .unwrap_or(defaults.search_parameters);

        log::debug!(
            "HMM map matching configured: sigma={}, beta={}, max_candidates={}, search_parameters={:?}",
            sigma,
            beta,
            max_candidates,
            search_parameters
        );

        let alg = HmmMapMatching {
            sigma,
            beta,
            max_candidates,
            search_parameters,
        };
        Ok(Arc::new(alg))
    }
}
