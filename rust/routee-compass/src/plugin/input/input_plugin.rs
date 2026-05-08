use std::sync::Arc;

use crate::app::search::SearchApp;

use super::InputPluginError;

/// Performs some kind of pre-processing on a user query input. The input JSON is available
/// to the plugin as a reference. The plugin produces a vector of zero to many JSON objects that will
/// replace the input JSON.
///
/// A simple No-operation [`InputPlugin`] would simply clone the input and place it in a `Vec`.
///
/// # Default InputPlugins
///
/// The following default set of input plugin builders are found in the [`super::default`] module:
///
/// * [debug] - logs the (current) status of each query to the logging system
/// * [grid search] - duplicates a query based on a list of user-defined values
/// * [inject] - mechanism to inject values into the queries
/// * [load balancer] - uses weighting heuristics to balance query loads across threads
///
/// [debug]: super::default::debug::debug_builder::DebugInputPluginBuilder
/// [grid search]: super::default::grid_search::GridSearchBuilder
/// [inject]: super::default::inject::inject_builder::InjectPluginBuilder
/// [load balancer]: super::default::load_balancer::builder::LoadBalancerBuilder
///
pub trait InputPlugin: Send + Sync {
    /// unique name for this plugin, used in logging and metrics.
    fn name(&self) -> &str;

    /// Applies this [`InputPlugin`] to a user query input, passing along a `Vec` of input
    /// queries as a result which will replace the input.
    ///
    /// # Arguments
    ///
    /// * `input` - the user query input passed to this plugin
    /// * `search_app` - a reference to the search app with all loaded assets
    ///
    /// # Returns
    ///
    /// A `Vec` of JSON values to replace the input JSON, or an error
    fn process(
        &self,
        input: &mut serde_json::Value,
        search_app: Arc<SearchApp>,
    ) -> Result<(), InputPluginError>;
}
