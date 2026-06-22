use crate::model::model_identifier::ModelIdentifier;
use routee_compass_core::model::traversal::{
    TraversalModel, TraversalModelError, TraversalModelService,
};
use std::collections::HashMap;
use std::sync::Arc;

/// holds a library of vehicle models as TraversalModelServices and selects one
/// based on the model_name field of the incoming query.
#[derive(Clone)]
pub struct EnergyModelService {
    pub vehicle_library: HashMap<ModelIdentifier, Arc<dyn TraversalModelService>>,
}

impl EnergyModelService {
    pub fn new(
        vehicle_library: HashMap<ModelIdentifier, Arc<dyn TraversalModelService>>,
    ) -> Result<Self, TraversalModelError> {
        Ok(EnergyModelService { vehicle_library })
    }
}

impl TraversalModelService for EnergyModelService {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let model_identifier: ModelIdentifier = serde_json::from_value(parameters.clone())
            .map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Query missing 'model_identifier' field\nFrom serde_json::{e}"
                ))
            })?;

        let service = self.vehicle_library.get(&model_identifier).ok_or_else(|| {
            TraversalModelError::BuildError(format!(
                "unknown vehicle model {:?}, must be one of [{}]",
                model_identifier,
                self.vehicle_library
                    .keys()
                    .map(|m| format!("{m:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
        let model = service.build(parameters)?;
        Ok(model)
    }
}
