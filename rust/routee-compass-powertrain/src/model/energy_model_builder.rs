use crate::model::energy_model_service::EnergyModelService;
use crate::model::BevEnergyModel;
use crate::model::IceEnergyModel;
use crate::model::PhevEnergyModel;
use config::Config;
use routee_compass_core::config::ConfigJsonExtensions;
use routee_compass_core::config::ops::strip_type_from_config;
use routee_compass_core::model::traversal::{
    TraversalModelBuilder, TraversalModelError, TraversalModelService,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EnergyModelBuilderConfig {
    pub vehicle_input_files: Vec<String>,
    pub include_trip_energy: Option<bool>,
}

pub struct EnergyModelBuilder {}

impl TraversalModelBuilder for EnergyModelBuilder {
    fn build(
        &self,
        params: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {
        let config: EnergyModelBuilderConfig =
            serde_json::from_value(params.clone()).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "failure reading energy traversal model configuration: {e}"
                ))
            })?;

        // read all vehicle configurations from files
        let mut vehicle_library = HashMap::new();
        for vehicle_file in &config.vehicle_input_files {
            let vehicle_config = Config::builder()
                .add_source(config::File::with_name(vehicle_file))
                .build()
                .map_err(|e| {
                    TraversalModelError::BuildError(format!(
                        "failed to read vehicle config file '{}': {}",
                        vehicle_file, e
                    ))
                })?;

            let mut vehicle_json = vehicle_config
                .try_deserialize::<serde_json::Value>()
                .map_err(|e| {
                    TraversalModelError::BuildError(format!(
                        "failed to parse vehicle config file '{}': {}",
                        vehicle_file, e
                    ))
                })?
                .normalize_file_paths(Path::new(vehicle_file), None)
                .map_err(|e| {
                    TraversalModelError::BuildError(format!(
                        "failed to normalize file paths in vehicle config file '{}': {}",
                        vehicle_file, e
                    ))
                })?;

            // inject include_trip_energy if specified at the model level
            if let Some(include_trip_energy) = config.include_trip_energy {
                vehicle_json["include_trip_energy"] = serde_json::Value::Bool(include_trip_energy);
            }

            let model_name = vehicle_json
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    TraversalModelError::BuildError(format!(
                        "vehicle model missing 'name' field in '{}'",
                        vehicle_file
                    ))
                })?
                .to_string();
            
            let (vehicle_json_stripped, vehicle_type) = strip_type_from_config(&vehicle_json)
                .map_err(|e| TraversalModelError::BuildError(e.to_string()))?;

            let service: Arc<dyn TraversalModelService> = match vehicle_type.as_str() {
                "ice" => Arc::new(IceEnergyModel::try_from(&vehicle_json_stripped)?),
                "bev" => Arc::new(BevEnergyModel::try_from(&vehicle_json_stripped)?),
                "phev" => Arc::new(PhevEnergyModel::try_from(&vehicle_json_stripped)?),
                _ => {
                    return Err(TraversalModelError::BuildError(format!(
                        "unknown vehicle model type in '{}': {}",
                        vehicle_file, vehicle_type
                    )));
                }
            };

            vehicle_library.insert(model_name, service);
        }

        let service = EnergyModelService::new(vehicle_library)?;

        Ok(Arc::new(service))
    }
}
