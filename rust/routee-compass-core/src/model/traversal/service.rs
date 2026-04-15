use super::{error::TraversalModelError, model::TraversalModel};
use std::sync::Arc;

/// A [`TraversalModelService`] is a persistent builder of [TraversalModel] instances.
/// Building a [`TraversalModelService`] may be an expensive operation and often includes
/// file IO on the order of the size of the road network edge list.
/// The service then builds a [TraversalModel] instance for each route query.
/// [`TraversalModelService`] must be read across the thread pool and so it implements
/// Send and Sync.
///
/// [TraversalModel]: compass_core::model::traversal::model::TraversalModel
pub trait TraversalModelService: Send + Sync {
    /// Builds a [TraversalModel] for the incoming query, used as parameters for this
    /// build operation.
    ///
    /// The query is passed as parameters to this operation so that any query-time
    /// coefficients may be applied to the [TraversalModel].
    ///
    /// # Arguments
    ///
    /// * `query` - the incoming query which may contain parameters for building the [TraversalModel]
    ///
    /// # Returns
    ///
    /// The [TraversalModel] instance for this query, or an error
    ///
    /// [TraversalModel]: compass_core::model::traversal::model::TraversalModel
    fn build(
        &self,
        query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError>;
}
