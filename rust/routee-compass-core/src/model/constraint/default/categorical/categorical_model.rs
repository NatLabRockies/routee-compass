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

impl ConstraintModel for CategoricalConstraintModel {
    fn valid_frontier(
        &self,
        ctx: &EdgeFrontierContext,
        _state: &[StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        self.valid_edge(ctx.edge)
    }

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
