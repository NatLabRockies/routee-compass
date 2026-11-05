use crate::model::model_identifier::ModelIdentifier;
use routee_compass_core::model::traversal::{
    TraversalModel, TraversalModelError, TraversalModelService,
};
use std::collections::HashMap;
use std::sync::Arc;

/// query key used to select a vehicle model from the library
pub const MODEL_KEY: &str = "model_key";
/// query key that was renamed to [`MODEL_KEY`]; only used to produce a migration hint
const RENAMED_QUERY_KEY: &str = "model_name";

/// holds a library of vehicle models as TraversalModelServices and selects one
/// based on the model_key field of the incoming query.
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
        let model_key = parameters.get(MODEL_KEY).ok_or_else(|| {
            let hint = if parameters.get(RENAMED_QUERY_KEY).is_some() {
                format!(
                    " ('{RENAMED_QUERY_KEY}' was renamed to '{MODEL_KEY}' for RouteE Powertrain V2 models; values are now V2 model keys, e.g. 'toyota/camry_ice/2016/rf_base_67ae9982')"
                )
            } else {
                String::new()
            };
            TraversalModelError::BuildError(format!("query missing '{MODEL_KEY}' field{hint}"))
        })?;

        let model_identifier: ModelIdentifier =
            serde_json::from_value(model_key.clone()).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize '{MODEL_KEY}' field from query into ModelIdentifier: {e}"
                ))
            })?;

        let service = self.vehicle_library.get(&model_identifier).ok_or_else(|| {
            TraversalModelError::BuildError(format!(
                "unknown vehicle model {model_identifier} from `{MODEL_KEY}` field in query"
            ))
        })?;

        let model = service.build(parameters)?;
        Ok(model)
    }
}
