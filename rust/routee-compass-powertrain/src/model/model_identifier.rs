use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;
use std::fmt::Display;
use std::num::NonZeroU64;

/// The ModelIdentifier is a type deserialized from a RouteE Powertrain .json file's "name" field,
/// and details the vehicle energy model.
///
/// The variants are either a fully qualified string ID:
/// ```json
/// {
///     "name": "2016_BMW_328d_4cyl_2WD",
/// }
/// ```
///
/// or a structured ID:
/// ```json
/// {
///     "name": {
///         "make": "BMW",
///         "model": "328d_4cyl_2WD",
///         "year": 2016,
///     },
/// }
/// ```
///
/// Note: variant and version fields are both of type Option<String>, so they
/// are not required.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(untagged)] // serde tries each variant
pub enum ModelIdentifier {
    FullyQualifiedId(String),
    StructuredId {
        make: String,
        model: String,
        year: NonZeroU64,
        #[serde(skip_serializing_if = "Option::is_none")]
        variant: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
    },
}

// standard display of a ModelIdentifier converts the object to its minified json,
// but a fully qualified id is represented as a raw string rather than a JSON string.
impl Display for ModelIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelIdentifier::FullyQualifiedId(id) => write!(f, "{id}"),
            ModelIdentifier::StructuredId { .. } => write!(
                f,
                "{}",
                serde_json::to_string(self).map_err(|_| fmt::Error)?
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use routee_compass_core::model::traversal::TraversalModelError;

    // Testing deserialization of vehicle_json["name"] into ModelIdentifier::StructuredID
    #[test]
    fn test_model_identifier_structured() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "name": {
                "make": "Ford",
                "model": "Quadricycle",
                "year": 1896
            },
            "type": "ice",
            "mass_estimate_lbs": 500,
            "model_input_file": "ford_quad_1896.bin",
            "energy_rate_unit": "gallons gasoline/mile",
            "real_world_energy_adjustment": 100.0,
            "a_star_heuristic_energy_rate": 0.001,
            "input_features": [
                {
                    "type": "speed",
                    "name": "edge_speed",
                    "unit": "mph"
                }
            ],
            "model_type": "onnx"
            }"#;
        let vehicle_json: serde_json::Value =
            serde_json::from_str(vehicle_json_as_str).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Couldn't load vehicle json because of {e}"
                ))
            })?;
        let name_field = vehicle_json
            .get("name")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `name`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `name` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!(
            model_identifier,
            ModelIdentifier::StructuredId {
                make: "Ford".to_string(),
                model: "Quadricycle".to_string(),
                year: NonZeroU64::new(1896).unwrap(),
                variant: None,
                version: None
            },
        );

        Ok(())
    }

    // Tests deserializing vehicle_json["name"] into ModelIdentifier::FullyQualifiedID
    #[test]
    fn test_model_identifier_fully_qualified() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "name": "1896_Ford_Quadricycle",
            "type": "ice",
            "mass_estimate_lbs": 500,
            "model_input_file": "ford_quad_1896.bin",
            "energy_rate_unit": "gallons gasoline/mile",
            "real_world_energy_adjustment": 100.0,
            "a_star_heuristic_energy_rate": 0.001,
            "input_features": [
                {
                    "type": "speed",
                    "name": "edge_speed",
                    "unit": "mph"
                }
            ],
            "model_type": "onnx"
            }"#;
        let vehicle_json: serde_json::Value =
            serde_json::from_str(vehicle_json_as_str).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Couldn't load vehicle json because of {e}"
                ))
            })?;
        let name_field = vehicle_json
            .get("name")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `name`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `name` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!(
            model_identifier,
            ModelIdentifier::FullyQualifiedId("1896_Ford_Quadricycle".to_string())
        );

        Ok(())
    }

    // ensure that if one non-optional field is redacted, we receive an error when attempting to deserialize
    #[test]
    fn test_model_identifier_rejects_invalid() {
        let bad = serde_json::json!({ "make": "Ford" }); // no model, no year
        assert!(ModelIdentifier::deserialize(&bad).is_err());
    }

    #[test]
    fn test_display_structured() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "name": {
                "make": "Ford",
                "model": "Quadricycle",
                "year": 1896
            },
            "type": "ice",
            "mass_estimate_lbs": 500,
            "model_input_file": "ford_quad_1896.bin",
            "energy_rate_unit": "gallons gasoline/mile",
            "real_world_energy_adjustment": 100.0,
            "a_star_heuristic_energy_rate": 0.001,
            "input_features": [
                {
                    "type": "speed",
                    "name": "edge_speed",
                    "unit": "mph"
                }
            ],
            "model_type": "onnx"
            }"#;
        let vehicle_json: serde_json::Value =
            serde_json::from_str(vehicle_json_as_str).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Couldn't load vehicle json because of {e}"
                ))
            })?;
        let name_field = vehicle_json
            .get("name")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `name`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `name` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!(
            r#"{"make":"Ford","model":"Quadricycle","year":1896}"#,
            format!("{}", model_identifier)
        );

        Ok(())
    }

    #[test]
    fn test_display_fully_qualified() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "name": "1896_Ford_Quadricycle",
            "type": "ice",
            "mass_estimate_lbs": 500,
            "model_input_file": "ford_quad_1896.bin",
            "energy_rate_unit": "gallons gasoline/mile",
            "real_world_energy_adjustment": 100.0,
            "a_star_heuristic_energy_rate": 0.001,
            "input_features": [
                {
                    "type": "speed",
                    "name": "edge_speed",
                    "unit": "mph"
                }
            ],
            "model_type": "onnx"
            }"#;
        let vehicle_json: serde_json::Value =
            serde_json::from_str(vehicle_json_as_str).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Couldn't load vehicle json because of {e}"
                ))
            })?;
        let name_field = vehicle_json
            .get("name")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `name`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `name` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!("1896_Ford_Quadricycle", format!("{}", model_identifier));

        Ok(())
    }
}
