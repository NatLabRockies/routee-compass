use super::output_plugin_error::OutputPluginError;
use crate::app::compass::CompassAppError;
use crate::app::search::SearchAppResult;
use routee_compass_core::algorithm::search::SearchInstance;

/// Performs some kind of post-processing on a search result. The result JSON is available
/// to the plugin as a reference which was potentially modified upstream by another output
/// plugin. The plugin will output a modified version of the JSON as a result.
///
/// A simple No-operation [`OutputPlugin`] would simply clone its JSON argument.
///
/// # Default OutputPlugins
///
/// The following default set of output plugin builders are found in the [`super::default`] module:
///
/// * [summary] - simple plugin appends cost and distance to result
/// * [traversal] - fully-featured plugin for traversal outputs in different formats
/// * [uuid] - attach the original graph ids to a result
///
/// [summary]: super::default::summary::builder::SummaryOutputPluginBuilder
/// [traversal]: super::default::traversal::builder::TraversalPluginBuilder
/// [uuid]: super::default::uuid::builder::UUIDOutputPluginBuilder
pub trait OutputPlugin: Send + Sync {
    /// unique name for this plugin, used in logging and metrics.
    fn name(&self) -> &str;

    /// Applies this [`OutputPlugin`] to a search result, passing along a JSON
    /// that will replace the `output` JSON argument.
    ///
    /// # Arguments
    ///
    /// * `output` - the search result passed to this plugin
    /// * `result` - the result of the search via the [internal representation].
    ///   this is passed as a `Result` as the search may have failed.
    ///
    /// # Returns
    ///
    /// The modified JSON or an error
    ///
    /// [internal representation]: crate::app::search::SearchAppResult
    fn process(
        &self,
        output: &mut serde_json::Value,
        result: &Result<(SearchAppResult, SearchInstance), CompassAppError>,
    ) -> Result<(), OutputPluginError>;
}
