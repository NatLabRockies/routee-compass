use serde::{Deserialize, Serialize};
use uom::si::f64::{Length, Ratio};

use crate::algorithm::map_matching::{map_matching_error::MapMatchingError, map_matching_result::MapMatchingResult, trace::Trace};



#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MapMatcher {
    LCSS {
        distance_epsilon: Length,
        cutting_threshold: Length,
        distance_threshold: Length,
        similarity_cutoff: Ratio,
    },
}

impl MapMatcher {
    pub fn match_trace(&self, trace: &Trace) -> Result<MapMatchingResult, MapMatchingError>  
    {
        Ok(MapMatchingResult { matches: Vec::new() })
    }
}