use super::map_matching_error::MapMatchingError;
use super::map_matching_result::MapMatchingResult;
use super::map_matching_trace::MapMatchingTrace;
use crate::algorithm::search::SearchInstance;
use std::sync::Arc;

/// Trait defining the interface for map matching algorithms.
///
/// Map matching algorithms take a GPS trace and match it to the road network,
/// producing both per-point matches and an inferred path through the network.
///
/// Implementations receive a [`SearchInstance`] which provides access to:
/// - The road network graph
/// - Spatial indexing for nearest-neighbor queries
/// - Shortest path computation capabilities
///
/// # Example Implementation
///
/// ```ignore
/// struct SimpleMapMatching;
///
/// impl MapMatchingAlgorithm for SimpleMapMatching {
///     fn match_trace(
///         &self,
///         trace: &MapMatchingTrace,
///         si: &SearchInstance,
///     ) -> Result<MapMatchingResult, MapMatchingError> {
///         // Match each point to nearest edge
///         // Connect matched edges via shortest path
///         todo!()
///     }
/// }
/// ```
pub trait MapMatchingAlgorithm: Send + Sync {
    /// Matches a GPS trace to the road network.
    ///
    /// # Arguments
    ///
    /// * `trace` - The GPS trace to match
    /// * `si` - Search instance providing access to graph and spatial index
    ///
    /// # Returns
    ///
    /// A [`MapMatchingResult`] containing:
    /// - Per-point match information (matched edge and distance)
    /// - The complete inferred path through the network
    fn match_trace(
        &self,
        trace: &MapMatchingTrace,
        si: &SearchInstance,
    ) -> Result<MapMatchingResult, MapMatchingError>;

    /// Returns the name of this algorithm for logging and debugging.
    fn name(&self) -> &str {
        "map_matching_algorithm"
    }

    /// Creates a configured instance from JSON config.
    ///
    /// # Arguments
    ///
    /// * `config` - JSON configuration for the algorithm
    ///
    /// # Returns
    ///
    /// A configured instance of this algorithm wrapped in Arc.
    /// Default implementation returns self as-is (no configuration).
    fn configure(
        &self,
        _config: &serde_json::Value,
    ) -> Result<Arc<dyn MapMatchingAlgorithm>, MapMatchingError>;
}
