use std::str::FromStr;

use crate::algorithm::map_matching::map_matching_algorithm::MapMatchingAlgorithm;
use crate::algorithm::map_matching::map_matching_error::MapMatchingError;
use crate::algorithm::map_matching::map_matching_result::MapMatchingResult;
use crate::algorithm::map_matching::map_matching_trace::MapMatchingTrace;
use crate::algorithm::map_matching::model::lcss::trajectory_segment;
use crate::algorithm::search::SearchInstance;
use crate::model::unit::DistanceUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use super::lcss_ops;
use super::trajectory_segment::TrajectorySegment;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcssConfig {
    #[serde(default = "LcssConfig::default_distance_unit")]
    pub distance_unit: String,
    #[serde(default = "LcssConfig::default_distance_epsilon")]
    pub distance_epsilon: f64,
    #[serde(default = "LcssConfig::default_similarity_cutoff")]
    pub similarity_cutoff: f64,
    #[serde(default = "LcssConfig::default_cutting_threshold")]
    pub cutting_threshold: f64,
    #[serde(default = "LcssConfig::default_random_cuts")]
    pub random_cuts: usize,
    #[serde(default = "LcssConfig::default_distance_threshold")]
    pub distance_threshold: f64,
    #[serde(default = "LcssConfig::default_search_parameters")]
    pub search_parameters: serde_json::Value,
}

impl LcssConfig {
    pub fn default_distance_unit() -> String {
        "meters".to_string()
    }
    pub fn default_distance_epsilon() -> f64 {
        50.0
    }
    pub fn default_similarity_cutoff() -> f64 {
        0.9
    }
    pub fn default_cutting_threshold() -> f64 {
        10.0
    }
    pub fn default_random_cuts() -> usize {
        0
    }
    pub fn default_distance_threshold() -> f64 {
        10000.0
    }
    pub fn default_search_parameters() -> serde_json::Value {
        serde_json::json!({})
    }
}

/// A map matching algorithm based on the Longest Common Subsequence (LCSS) similarity.
///
/// This is a port of the LCSS matcher from the mappymatch package.
///
/// # Parameters
///
/// - `distance_epsilon`: The distance epsilon to use for matching (default: 50.0 meters)
/// - `similarity_cutoff`: The similarity cutoff to use for stopping the algorithm (default: 0.9)
/// - `cutting_threshold`: The distance threshold to use for computing cutting points (default: 10.0 meters)
/// - `random_cuts`: The number of random cuts to add at each iteration (default: 0)
/// - `distance_threshold`: The distance threshold above which no match is made (default: 10000.0)
#[derive(Debug, Clone)]
pub struct LcssMapMatching {
    pub distance_epsilon: Length,
    pub similarity_cutoff: f64,
    pub cutting_threshold: Length,
    pub random_cuts: usize,
    pub distance_threshold: Length,
    /// Search query requirements for this algorithm
    pub search_parameters: serde_json::Value,
}

impl LcssMapMatching {
    pub fn from_config(config: LcssConfig) -> Result<Self, MapMatchingError> {
        let unit = DistanceUnit::from_str(&config.distance_unit).map_err(|_| {
            MapMatchingError::InternalError(format!(
                "Invalid distance unit: {}",
                config.distance_unit
            ))
        })?;
        Ok(Self {
            distance_epsilon: unit.to_uom(config.distance_epsilon),
            similarity_cutoff: config.similarity_cutoff,
            cutting_threshold: unit.to_uom(config.cutting_threshold),
            random_cuts: config.random_cuts,
            distance_threshold: unit.to_uom(config.distance_threshold),
            search_parameters: config.search_parameters,
        })
    }
}

impl MapMatchingAlgorithm for LcssMapMatching {
    fn match_trace(
        &self,
        trace: &MapMatchingTrace,
        si: &SearchInstance,
    ) -> Result<MapMatchingResult, MapMatchingError> {
        if trace.is_empty() {
            return Err(MapMatchingError::EmptyTrace);
        }

        // LCSS map matching requires an edge-oriented spatial index
        if !si.map_model.spatial_index.is_edge_oriented() {
            return Err(MapMatchingError::InternalError(
                "LCSS map matching requires an edge-oriented spatial index.".to_string(),
            ));
        }

        let stationary_indices = lcss_ops::find_stationary_points(trace);
        let skip_indices: std::collections::HashSet<_> = stationary_indices
            .iter()
            .flat_map(|si| si.i_index[1..].iter().cloned())
            .collect();

        let sub_trace_points: Vec<_> = trace
            .points
            .iter()
            .enumerate()
            .filter(|(i, _)| !skip_indices.contains(i))
            .map(|(_, p)| p.clone())
            .collect();
        let sub_trace = MapMatchingTrace::new(sub_trace_points);

        let initial_path = lcss_ops::new_path_for_trace(&sub_trace, si)?;
        let mut initial_segment = TrajectorySegment::new(sub_trace.clone(), initial_path);

        initial_segment.score_and_match(self, si)?;
        initial_segment.compute_cutting_points(self);

        let mut scheme = initial_segment.split_segment(si)?;

        for _ in 0..10 {
            let mut next_scheme = Vec::new();
            let mut changed = false;

            for mut segment in scheme.clone() {
                segment.score_and_match(self, si)?;
                segment.compute_cutting_points(self);

                if segment.score >= self.similarity_cutoff {
                    next_scheme.push(segment);
                } else {
                    let new_split = segment.split_segment(si)?;
                    if new_split.len() > 1 {
                        let joined =
                            trajectory_segment::join_segments(self, new_split.clone(), si)?;
                        if joined.score > segment.score {
                            next_scheme.extend(new_split);
                            changed = true;
                        } else {
                            next_scheme.push(segment);
                        }
                    } else {
                        next_scheme.push(segment);
                    }
                }
            }

            if !changed {
                break;
            }
            scheme = next_scheme;
        }

        let final_segment = trajectory_segment::join_segments(self, scheme, si)?;

        let final_matches =
            lcss_ops::add_matches_for_stationary_points(final_segment.matches, stationary_indices);

        Ok(MapMatchingResult::new(final_matches, final_segment.path))
    }

    fn name(&self) -> &str {
        "lcss_map_matching"
    }

    fn search_parameters(&self) -> serde_json::Value {
        self.search_parameters.clone()
    }
}
