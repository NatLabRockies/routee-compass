use crate::model::{
    constraint::ConstraintModel,
    network::{Edge, EdgeId},
    traversal::EdgeFrontierContext,
};
use std::{collections::HashSet, sync::Arc};

/// A wrapper of the user-generated ConstraintModel which prohibits traversals
/// on selected edges. algorithms can create this wrapper with a set of "cut edges"
/// and the search will not allow traversal of these edges.
pub struct EdgeCutConstraintModel {
    pub underlying: Arc<dyn ConstraintModel>,
    cut_edges: HashSet<EdgeId>,
}

impl EdgeCutConstraintModel {
    pub fn new(
        underlying: Arc<dyn ConstraintModel>,
        cut_edges: HashSet<EdgeId>,
    ) -> EdgeCutConstraintModel {
        EdgeCutConstraintModel {
            underlying,
            cut_edges,
        }
    }
}

impl ConstraintModel for EdgeCutConstraintModel {
    fn valid_frontier(
        &self,
        ctx: &EdgeFrontierContext,
        state: &[crate::model::state::StateVariable],
        state_model: &crate::model::state::StateModel,
    ) -> Result<bool, crate::model::constraint::ConstraintModelError> {
        if self.cut_edges.contains(&ctx.edge.edge_id) {
            Ok(false)
        } else {
            self.underlying.valid_frontier(ctx, state, state_model)
        }
    }

    fn valid_edge(
        &self,
        edge: &Edge,
    ) -> Result<bool, crate::model::constraint::ConstraintModelError> {
        if self.cut_edges.contains(&edge.edge_id) {
            self.underlying.valid_edge(edge)
        } else {
            Ok(false)
        }
    }
}
