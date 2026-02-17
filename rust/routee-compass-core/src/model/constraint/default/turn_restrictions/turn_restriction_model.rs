use crate::model::{
    constraint::{ConstraintModel, ConstraintModelError},
    network::Edge,
    state::{StateModel, StateVariable},
};
use std::sync::Arc;

use super::{RestrictionRecord, TurnRestrictionFrontierService};

pub struct TurnRestrictionConstraintModel {
    pub service: Arc<TurnRestrictionFrontierService>,
}

impl ConstraintModel for TurnRestrictionConstraintModel {
    fn valid_frontier(
        &self,
        edge: &Edge,
        previous_edge: Option<&Edge>,
        _state: &[StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        match previous_edge {
            Some(previous_edge) => {
                let edge_pair = RestrictionRecord {
                    prev_edge_id: previous_edge.edge_id,
                    next_edge_id: edge.edge_id,
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
