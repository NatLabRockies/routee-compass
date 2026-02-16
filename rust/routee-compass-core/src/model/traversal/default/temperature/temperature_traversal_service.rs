use std::sync::Arc;

use crate::model::traversal::{
    default::temperature::{
        temperature_traversal_config::AmbientTemperatureConfig, TemperatureTraversalModel,
    },
    TraversalModel, TraversalModelError, TraversalModelService,
};

pub struct TemperatureTraversalService {
    pub default_ambient_temperature: Option<AmbientTemperatureConfig>,
}

impl TraversalModelService for TemperatureTraversalService {
    fn build(
        &self,
        query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let ambient_temp_config: AmbientTemperatureConfig = match query.get("ambient_temperature") {
            Some(value) => {
                serde_json::from_value(
                    value.clone()
                ).map_err(|_| TraversalModelError::BuildError("Could not parse ambient_temperature key from query. Expected a JSON object with a value and unit key.".to_string()))
            }
            None => match &self.default_ambient_temperature {
                Some(config) => Ok(config.clone()),
                None => Err(TraversalModelError::BuildError("No ambient_temperature key provided in query and no default set.".to_string())),
            }
        }?;

        Ok(Arc::new(TemperatureTraversalModel {
            ambient_temperature: ambient_temp_config.to_uom(),
        }))
    }
}
