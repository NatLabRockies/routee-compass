use super::turn_restriction_model::TurnRestrictionConstraintModel;
use crate::model::{
    constraint::{
        default::turn_restrictions::RestrictionRecord, ConstraintModel, ConstraintModelError,
        ConstraintModelService,
    },
    state::StateModel,
};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone)]
pub struct TurnRestrictionFrontierService {
    pub restricted_edge_pairs: Arc<HashSet<RestrictionRecord>>,
}

impl ConstraintModelService for TurnRestrictionFrontierService {
    fn build(
        &self,
        _query: &serde_json::Value,
        _state_model: Arc<StateModel>,
    ) -> Result<Arc<dyn ConstraintModel>, ConstraintModelError> {
        let service: Arc<TurnRestrictionFrontierService> = Arc::new(self.clone());
        let model = TurnRestrictionConstraintModel { service };
        Ok(Arc::new(model))
    }
}
