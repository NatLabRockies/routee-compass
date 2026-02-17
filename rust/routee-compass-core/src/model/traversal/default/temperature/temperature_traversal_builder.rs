use crate::model::traversal::{
    default::temperature::{
        temperature_traversal_config::TemperatureTraversalConfig, TemperatureTraversalService,
    },
    TraversalModelBuilder, TraversalModelError, TraversalModelService,
};
use std::sync::Arc;

pub struct TemperatureTraversalBuilder {}

impl TraversalModelBuilder for TemperatureTraversalBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {
        let config: TemperatureTraversalConfig = serde_json::from_value(parameters.clone())
            .map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "failed to read temperature traversal configuration: {e}"
                ))
            })?;

        let service = Arc::new(TemperatureTraversalService {
            default_ambient_temperature: config.default_ambient_temperature,
        });
        Ok(service)
    }
}
