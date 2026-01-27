use crate::algorithm::map_matching::{
    map_matching_algorithm::MapMatchingAlgorithm, map_matching_builder::MapMatchingBuilder,
    map_matching_error::MapMatchingError,
};
use std::sync::Arc;

use super::LcssMapMatching;

pub struct LcssMapMatchingBuilder;

impl MapMatchingBuilder for LcssMapMatchingBuilder {
    fn build(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn MapMatchingAlgorithm>, MapMatchingError> {
        let defaults = LcssMapMatching::default();

        let distance_epsilon = config
            .get("distance_epsilon")
            .and_then(|v| v.as_f64())
            .unwrap_or(defaults.distance_epsilon);

        let similarity_cutoff = config
            .get("similarity_cutoff")
            .and_then(|v| v.as_f64())
            .unwrap_or(defaults.similarity_cutoff);

        let cutting_threshold = config
            .get("cutting_threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(defaults.cutting_threshold);

        let random_cuts = config
            .get("random_cuts")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(defaults.random_cuts);

        let distance_threshold = config
            .get("distance_threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(defaults.distance_threshold);

        log::debug!(
            "LCSS map matching configured: distance_epsilon={}, similarity_cutoff={}, cutting_threshold={}, random_cuts={}, distance_threshold={}",
            distance_epsilon,
            similarity_cutoff,
            cutting_threshold,
            random_cuts,
            distance_threshold
        );

        let alg = LcssMapMatching {
            distance_epsilon,
            similarity_cutoff,
            cutting_threshold,
            random_cuts,
            distance_threshold,
        };
        Ok(Arc::new(alg))
    }
}
