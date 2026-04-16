use super::{error::TraversalModelError, service::TraversalModelService};
use std::sync::Arc;

/// A [`TraversalModelBuilder`] takes a JSON object describing the configuration of a
/// traversal model and builds a [`TraversalModelService`].
///
/// A [`TraversalModelBuilder`] instance should be an empty struct that implements
/// this trait.
pub trait TraversalModelBuilder {
    /// Builds a [`TraversalModelService`] from configuration.
    ///
    /// # Arguments
    ///
    /// * `parameters` - the contents of the "traversal" TOML config section
    ///
    /// # Returns
    ///
    /// A [`TraversalModelService`] designed to persist the duration of the CompassApp.
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError>;
}
