//! This module provides the helpers for map matching.
//! The map matching logic itself is now integrated into `CompassApp`.

mod map_matching_app_error;
mod map_matching_request;
mod map_matching_response;

#[cfg(test)]
mod map_matching_tests;

pub use map_matching_app_error::MapMatchingAppError;
pub use map_matching_request::{MapMatchingRequest, TracePoint};
pub use map_matching_response::{MapMatchingResponse, MatchedEdgeResponse, PointMatchResponse};
