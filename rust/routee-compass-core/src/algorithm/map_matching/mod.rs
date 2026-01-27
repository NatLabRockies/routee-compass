//! Map matching algorithms for matching GPS traces to road networks.
//!
//! This module provides abstractions for map matching algorithms that take
//! GPS traces and match them to edges in the road network graph.
//!
//! # Core Types
//!
//! - [`MapMatchingAlgorithm`] - Trait defining the algorithm interface
//! - [`MapMatchingTrace`] - Input GPS trace
//! - [`MapMatchingResult`] - Output with matched edges and path
//! - [`MapMatchingError`] - Error types for matching operations
//!
//! # Implementations
//!
//! - [`SimpleMapMatching`] - Baseline nearest-edge matching algorithm
//! - [`HmmMapMatching`] - Hidden Markov Model-based map matching

mod hmm_map_matching;
mod map_matching_algorithm;
mod map_matching_error;
mod map_matching_result;
mod map_matching_trace;
mod simple_map_matching;

pub use hmm_map_matching::HmmMapMatching;
pub use map_matching_algorithm::MapMatchingAlgorithm;
pub use map_matching_error::MapMatchingError;
pub use map_matching_result::{MapMatchingResult, PointMatch};
pub use map_matching_trace::{MapMatchingPoint, MapMatchingTrace};
pub use simple_map_matching::SimpleMapMatching;
