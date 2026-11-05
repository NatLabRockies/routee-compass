use crate::model::energy_model_service::{EnergyModelService, MODEL_KEY};
use crate::model::model_identifier::ModelIdentifier;
use crate::model::prediction::routee_powertrain_v2_metadata::PowertrainType;
use crate::model::BevEnergyModel;
use crate::model::IceEnergyModel;
use crate::model::PhevEnergyModel;
use config::Config;
use routee_compass_core::config::ops::strip_type_from_config;
use routee_compass_core::config::ConfigJsonExtensions;
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

            resolve_estimator_model_file(&mut vehicle_json, Path::new(vehicle_file))?;

            let model_identifier = read_model_identifier(&vehicle_json, vehicle_file)?;

            let (vehicle_json_stripped, vehicle_type) = match vehicle_json.get("type") {
                Some(_) => strip_type_from_config(&vehicle_json)
                    .map_err(|e| TraversalModelError::BuildError(e.to_string()))?,
                None => {
                    let vehicle_type = infer_vehicle_type(&vehicle_json, vehicle_file)?;
                    (vehicle_json, vehicle_type)
                }
            };

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

            vehicle_library.insert(model_identifier, service);
        }

        let service = EnergyModelService::new(vehicle_library)?;

        Ok(Arc::new(service))
    }
}

/// reads the vehicle's [`ModelIdentifier`] from the RouteE Powertrain V2 `model_key` field.
fn read_model_identifier(
    vehicle_json: &serde_json::Value,
    vehicle_file: &str,
) -> Result<ModelIdentifier, TraversalModelError> {
    let value = vehicle_json.get(MODEL_KEY).ok_or_else(|| {
        let hint = if vehicle_json.get("name").is_some() {
            format!(
                " (vehicle 'name' was replaced by '{MODEL_KEY}'; RouteE Powertrain V2 metadata.json files provide it)"
            )
        } else {
            String::new()
        };
        TraversalModelError::BuildError(format!(
            "vehicle model missing '{MODEL_KEY}' field in '{vehicle_file}'{hint}"
        ))
    })?;
    serde_json::from_value(value.clone()).map_err(|e| {
        TraversalModelError::BuildError(format!(
            "Could not build ModelIdentifier from '{MODEL_KEY}' field in vehicle config file '{vehicle_file}': {e}"
        ))
    })
}

/// RouteE Powertrain V2 metadata has no Compass `type` field, so the vehicle model type
/// is inferred from `vehicle.powertrain_type`.
fn infer_vehicle_type(
    vehicle_json: &serde_json::Value,
    vehicle_file: &str,
) -> Result<String, TraversalModelError> {
    let powertrain_type_json = vehicle_json
        .get("vehicle")
        .and_then(|vehicle| vehicle.get("powertrain_type"))
        .ok_or_else(|| {
            TraversalModelError::BuildError(format!(
                "vehicle config file '{vehicle_file}' must have either a 'type' field or a 'vehicle.powertrain_type' field"
            ))
        })?;
    let powertrain_type: PowertrainType = serde_json::from_value(powertrain_type_json.clone())
        .map_err(|e| {
            TraversalModelError::BuildError(format!(
                "invalid 'vehicle.powertrain_type' in vehicle config file '{vehicle_file}': {e}"
            ))
        })?;
    match powertrain_type {
        PowertrainType::Ice | PowertrainType::Hev => Ok(String::from("ice")),
        PowertrainType::Bev => Ok(String::from("bev")),
        PowertrainType::PhevEvMode | PowertrainType::PhevHevMode => Err(TraversalModelError::BuildError(format!(
            "vehicle config file '{vehicle_file}' describes a single PHEV operating mode; PHEV vehicles require a config with type 'phev' that provides both charge sustaining and charge depleting models"
        ))),
        PowertrainType::Undefined | PowertrainType::HeavyDuty => Err(TraversalModelError::BuildError(format!(
            "unsupported 'vehicle.powertrain_type' {powertrain_type_json} in vehicle config file '{vehicle_file}'; add a 'type' field to select a vehicle model"
        ))),
    }
}

/// resolves each relative `estimator.model_file` path against the vehicle config file's
/// directory, since RouteE Powertrain V2 metadata stores the model file name relative to itself.
fn resolve_estimator_model_file(
    vehicle_json: &mut serde_json::Value,
    vehicle_file: &Path,
) -> Result<(), TraversalModelError> {
    match vehicle_json {
        serde_json::Value::Object(obj) => {
            for (key, value) in obj.iter_mut() {
                if key == "estimator" {
                    if let Some(serde_json::Value::String(model_file)) = value.get_mut("model_file")
                    {
                        *model_file = resolve_model_file_path(model_file, vehicle_file)?;
                    }
                }
                resolve_estimator_model_file(value, vehicle_file)?;
            }
        }
        serde_json::Value::Array(arr) => {
            for value in arr.iter_mut() {
                resolve_estimator_model_file(value, vehicle_file)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn resolve_model_file_path(
    model_file: &str,
    vehicle_file: &Path,
) -> Result<String, TraversalModelError> {
    let path = Path::new(model_file);
    let parent = vehicle_file.parent().unwrap_or_else(|| Path::new(""));
    let resolved = parent.join(path);
    if resolved.is_file() {
        Ok(resolved.to_string_lossy().into_owned())
    } else if path.is_file() {
        Ok(model_file.to_string())
    } else {
        Err(TraversalModelError::BuildError(format!(
            "estimator model file '{}' not found as given or relative to vehicle config file '{}'",
            model_file,
            vehicle_file.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const RF_METADATA: &str = "src/model/prediction/test/2016_camry_ice_rf/metadata.json";
    const CNN_METADATA: &str = "src/model/prediction/test/2016_camry_ice_cnn/metadata.json";
    const RF_MODEL_KEY: &str = "toyota/camry_ice/2016/rf_base_67ae9982";
    const CNN_MODEL_KEY: &str = "toyota/camry_ice/2016/cnn_base_67ae9982";

    fn build_service(vehicle_input_files: &[&str]) -> Arc<dyn TraversalModelService> {
        EnergyModelBuilder {}
            .build(&json!({ "vehicle_input_files": vehicle_input_files }))
            .expect("failed to build energy model service from v2 metadata")
    }

    #[test]
    fn test_build_from_v2_metadata() {
        let service = build_service(&[RF_METADATA, CNN_METADATA]);

        let rf = service
            .build(&json!({ "model_key": RF_MODEL_KEY }))
            .unwrap();
        assert_eq!(rf.name(), format!("ICE Energy Model: {RF_MODEL_KEY}"));

        let cnn = service
            .build(&json!({ "model_key": CNN_MODEL_KEY }))
            .unwrap();
        assert_eq!(cnn.name(), format!("ICE Energy Model: {CNN_MODEL_KEY}"));
    }

    #[test]
    fn test_read_model_identifier() {
        let v2 = json!({ "model_key": RF_MODEL_KEY });
        assert_eq!(
            read_model_identifier(&v2, "f").unwrap(),
            ModelIdentifier::FullyQualifiedId(RF_MODEL_KEY.to_string())
        );
        let renamed = json!({ "name": "2016_TOYOTA_Camry_4cyl_2WD" });
        let error = read_model_identifier(&renamed, "f")
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("missing 'model_key'"), "{error}");
        assert!(error.contains("replaced by 'model_key'"), "{error}");
        let missing = read_model_identifier(&json!({}), "f")
            .err()
            .unwrap()
            .to_string();
        assert!(!missing.contains("replaced by"), "{missing}");
    }

    #[test]
    fn test_infer_vehicle_type() {
        let infer = |powertrain_type: &str| {
            infer_vehicle_type(
                &json!({ "vehicle": { "powertrain_type": powertrain_type } }),
                "f",
            )
        };
        assert_eq!(infer("ICE").unwrap(), "ice");
        assert_eq!(infer("HEV").unwrap(), "ice");
        assert_eq!(infer("BEV").unwrap(), "bev");
        for unsupported in [
            "PHEV_EV_MODE",
            "PHEV_HEV_MODE",
            "UNDEFINED",
            "HEAVY_DUTY",
            "nope",
        ] {
            assert!(infer(unsupported).is_err(), "{unsupported}");
        }
        assert!(infer_vehicle_type(&json!({}), "f").is_err());
    }

    #[test]
    fn test_resolve_estimator_model_file() {
        let mut nested = json!({
            "charge_sustaining": { "estimator": { "model_file": "model.onnx" } },
            "charge_depleting": { "estimator": { "model_file": "model.onnx" } },
        });
        resolve_estimator_model_file(&mut nested, Path::new(RF_METADATA)).unwrap();
        let expected = Path::new(RF_METADATA)
            .parent()
            .unwrap()
            .join("model.onnx")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            nested["charge_sustaining"]["estimator"]["model_file"],
            expected
        );
        assert_eq!(
            nested["charge_depleting"]["estimator"]["model_file"],
            expected
        );

        let mut missing = json!({ "estimator": { "model_file": "missing.onnx" } });
        assert!(resolve_estimator_model_file(&mut missing, Path::new(RF_METADATA)).is_err());
    }
}
