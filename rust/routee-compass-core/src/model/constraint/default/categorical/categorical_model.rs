use super::categorical_service::CategoricalModelService;
use crate::model::constraint::{ConstraintModel, ConstraintModelError};
use crate::model::network::Edge;
use crate::model::state::StateModel;
use crate::model::state::StateVariable;
use crate::model::traversal::EdgeFrontierContext;
use std::collections::HashSet;
use std::sync::Arc;
pub struct CategoricalConstraintModel {
    pub service: Arc<CategoricalModelService>,
    pub query_categories: Option<HashSet<u8>>,
}

/// Constrains the search at query time to edges belonging to an allowed set of
/// category values.
///
/// Instances of this model are built by the `CategoricalModelService`. For a
/// configured category such as `road_class`, a query supplies the allowed values:
///
/// ```json
/// {
///     "road_class": ["footpath", "sidewalk", "staircase", "service", "residential"]
/// }
/// ```
///
/// Only edges whose `road_class` is in this list are considered during the search.
impl ConstraintModel for CategoricalConstraintModel {
    // returns true if the frontier context is valid
    fn valid_frontier(
        &self,
        ctx: &EdgeFrontierContext,
        _state: &[StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        self.valid_edge(ctx.edge)
    }

    // returns true if the edge is in the set of categories
    fn valid_edge(&self, edge: &Edge) -> Result<bool, ConstraintModelError> {
        match &self.query_categories {
            None => Ok(true),
            Some(encoding) => self
                .service
                .category_by_edge
                .get(edge.edge_id.0)
                .ok_or_else(|| {
                    ConstraintModelError::ConstraintModelError(format!(
                        "edge id {} missing from constraint model file",
                        edge.edge_id
                    ))
                })
                .map(|id| encoding.contains(id)),
        }
    }
}
