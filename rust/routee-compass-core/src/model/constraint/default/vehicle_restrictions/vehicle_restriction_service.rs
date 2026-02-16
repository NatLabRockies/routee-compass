use super::{
    vehicle_restriction_model::VehicleRestrictionConstraintModel,
    vehicle_restriction_query::VehicleRestrictionQuery, VehicleParameter, VehicleParameterType,
    VehicleRestriction,
};
use crate::model::{
    constraint::{ConstraintModel, ConstraintModelError, ConstraintModelService},
    network::EdgeId,
    state::StateModel,
};
use indexmap::IndexMap;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct VehicleRestrictionFrontierService {
    pub vehicle_restriction_lookup:
        Arc<HashMap<EdgeId, IndexMap<VehicleParameterType, VehicleRestriction>>>,
}

impl ConstraintModelService for VehicleRestrictionFrontierService {
    fn build(
        &self,
        query: &serde_json::Value,
        _state_model: Arc<StateModel>,
    ) -> Result<Arc<dyn ConstraintModel>, ConstraintModelError> {
        let service: Arc<VehicleRestrictionFrontierService> = Arc::new(self.clone());
        let restriction_query: VehicleRestrictionQuery = serde_json::from_value(query.clone())
            .map_err(|e| {
                ConstraintModelError::BuildError(format!(
                    "Unable to deserialize vehicle restriction query: {e}"
                ))
            })?;
        let vehicle_parameters: Vec<VehicleParameter> = restriction_query
            .vehicle_parameters
            .into_iter()
            .map(|vpc| vpc.into())
            .collect();
        let model = VehicleRestrictionConstraintModel {
            service,
            vehicle_parameters,
        };

        Ok(Arc::new(model))
    }
}
