use crate::model::{
    constraint::{ConstraintModel, ConstraintModelError},
    network::Edge,
    state::{StateModel, StateVariable},
    traversal::EdgeTraversalContext,
};
use std::sync::Arc;

use super::turn_restriction_service::{RestrictedEdgePair, TurnRestrictionFrontierService};

pub struct TurnRestrictionConstraintModel {
    pub service: Arc<TurnRestrictionFrontierService>,
}

impl ConstraintModel for TurnRestrictionConstraintModel {
    fn valid_frontier(
        &self,
        ctx: &EdgeTraversalContext,
        _state: &[StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        let previous_edge = ctx.previous_edge_traversal()?;
        match previous_edge {
            Some(previous_edge) => {
                let edge_pair = RestrictedEdgePair {
                    prev_edge_id: previous_edge.edge_id,
                    next_edge_id: ctx.edge.edge_id,
                };
                if self.service.restricted_edge_pairs.contains(&edge_pair) {
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            None => Ok(true),
        }
    }

    fn valid_edge(&self, _edge: &Edge) -> Result<bool, ConstraintModelError> {
        Ok(true)
    }
}
