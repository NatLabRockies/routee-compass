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

pub mod map_matching_algorithm;
pub mod map_matching_builder;
pub mod map_matching_error;
pub mod map_matching_result;
pub mod map_matching_trace;
pub mod model;

pub use map_matching_algorithm::MapMatchingAlgorithm;
pub use map_matching_builder::MapMatchingBuilder;
pub use map_matching_error::MapMatchingError;
pub use map_matching_result::{MapMatchingResult, PointMatch};
pub use map_matching_trace::{MapMatchingPoint, MapMatchingTrace};
pub use model::hmm::{HmmMapMatching, HmmMapMatchingBuilder};
pub use model::lcss::{LcssMapMatching, LcssMapMatchingBuilder};
pub use model::simple::{SimpleMapMatching, SimpleMapMatchingBuilder};
