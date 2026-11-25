use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::fmt::Display;
use std::num::NonZeroU64;
use std::str::FromStr;

use crate::model::model_identifier::ModelIdentifier::FullyQualifiedId;

/// The `ModelIdentifier` is an `enum` deserialized from a RouteE Powertrain metadata.json file's "model_key" field,
/// and details the vehicle energy model.
///
/// The variants are either a `String` detailing the `FullyQualifiedID`:
/// ```json
/// {
///     "model_identifier": "BMW/328d_4cyl_2WD/2016",
/// }
/// ```
///
/// or a structured json object detailing the `StructuredID`:
/// ```json
/// {
///     "model_identifier": {
///         "make": "BMW",
///         "vehicle_slug": "328d_4cyl_2WD",
///         "year": 2016,
///     },
/// }
/// ```
///
/// Note: variant and version fields are both of type `Option<String>`, so they
/// are not required.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(untagged)] // serde tries each variant
pub enum ModelIdentifier {
    FullyQualifiedId(String),
    StructuredId {
        make: String,
        vehicle_slug: String,
        year: NonZeroU64,
        config_slug: String,
        version: Option<String>,
    },
}

/// `ModelIdentifierError` variants
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelIdentifierError {
    #[error(
        "fully qualified id '{0}' must have at least make/vehicle_slug/year/config_slug fields"
    )]
    MissingFields(String),
    #[error("invalid year '{0}' in fully qualified id")]
    InvalidYear(String),
    #[error("version and variant must appear together, found only '{0}' = '{1}'")]
    OneOfVariantOrVersion(String, String),
}

impl ModelIdentifier {
    /// Creates a copy of the `ModelIdentifier` from it's current variant (either `FullyQualifiedId` or `StructuredId`)
    /// to `StructuredId`
    pub fn to_structured(&self) -> Result<Self, ModelIdentifierError> {
        match self {
            Self::StructuredId { .. } => Ok(self.clone()),
            Self::FullyQualifiedId(id) => {
                let mut id_fields = id.split('/').map(|s| s.to_string());

                let (make, vehicle_slug, year, config_slug) = match (
                    id_fields.next(),
                    id_fields.next(),
                    id_fields.next(),
                    id_fields.next(),
                ) {
                    (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
                    _ => return Err(ModelIdentifierError::MissingFields(id.clone())),
                };

                let year = NonZeroU64::from_str(&year)
                    .map_err(|_| ModelIdentifierError::InvalidYear(year))?;

                let version = id_fields.next();

                Ok(ModelIdentifier::StructuredId {
                    make,
                    vehicle_slug,
                    year,
                    config_slug,
                    version,
                })
            }
        }
    }

    /// Converts the `ModelIdentifier` from it's current variant (either `FullyQualifiedId` or `StructuredId`)
    /// to `FullyQualifiedId`
    pub fn to_fully_qualified(&self) -> Result<Self, ModelIdentifierError> {
        match self {
            Self::StructuredId {
                make,
                vehicle_slug,
                year,
                config_slug,
                version,
            } => match version {
                Some(ver) => Ok(FullyQualifiedId(format!(
                    "{}/{}/{}/{}/{}",
                    make, vehicle_slug, year, config_slug, ver
                ))),
                None => Ok(FullyQualifiedId(format!(
                    "{}/{}/{}/{}",
                    make, vehicle_slug, year, config_slug
                ))),
            },
            Self::FullyQualifiedId(_) => Ok(self.clone()),
        }
    }
}

impl Display for ModelIdentifier {
    /// Default `Display` implementation of `fmt()` for `ModelIdentifier`
    /// prints the `ModelIdentifier` as the original variant.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelIdentifier::FullyQualifiedId(id) => write!(f, "{id}"),
            ModelIdentifier::StructuredId {
                make,
                vehicle_slug,
                year,
                config_slug,
                version,
            } => {
                // if the variant or version are None type, prints "none"
                let opt_to_val = |o: &Option<String>| {
                    Value::String(o.clone().unwrap_or_else(|| "none".to_string()))
                };

                let obj = json!({
                    "make": make,
                    "vehicle_slug": vehicle_slug,
                    "year": year,
                    "config_slug": config_slug,
                    "version": opt_to_val(version),
                });

                write!(f, "{obj}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use routee_compass_core::model::traversal::TraversalModelError;

    // Testing deserialization of vehicle_json["model_key"] into ModelIdentifier::StructuredID
    #[test]
    fn test_model_identifier_structured() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "model_key": {
                "make": "Ford",
                "vehicle_slug": "Quadricycle",
                "year": 1896,
                "config_slug": "ffffffff"
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
            .get("model_key")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `model_key`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `model_key` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!(
            model_identifier,
            ModelIdentifier::StructuredId {
                make: "Ford".to_string(),
                vehicle_slug: "Quadricycle".to_string(),
                year: NonZeroU64::new(1896).unwrap(),
                config_slug: "ffffffff".to_string(),
                version: None
            },
        );

        Ok(())
    }

    // Tests deserializing vehicle_json["model_key"] into ModelIdentifier::FullyQualifiedID
    #[test]
    fn test_model_identifier_fully_qualified() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "model_key": "1896_Ford_Quadricycle_ffffffff",
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
            .get("model_key")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `model_key`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `model_key` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!(
            model_identifier,
            ModelIdentifier::FullyQualifiedId("1896_Ford_Quadricycle_ffffffff".to_string())
        );

        Ok(())
    }

    // Ensure that if one non-optional field is redacted, we receive an error when attempting to deserialize a ModelIdentifier.
    #[test]
    fn test_model_identifier_rejects_invalid() {
        let bad = serde_json::json!({ "make": "Ford" }); // no model, no year
        assert!(ModelIdentifier::deserialize(&bad).is_err());
    }

    // Ensures that structuredID is printed as structuredID.
    #[test]
    fn test_display_structured() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "model_key": {
                "make": "Ford",
                "vehicle_slug": "Quadricycle",
                "year": 1896,
                "config_slug": "ffffffff"
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
            .get("model_key")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `model_key`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `model_key` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!(
            r#"{"make":"Ford","vehicle_slug":"Quadricycle","year":1896,"config_slug":"ffffffff","version":"none"}"#,
            format!("{}", model_identifier)
        );

        Ok(())
    }

    // Ensures that FullyQualifiedID converts to StructuredID when printing.
    #[test]
    fn test_display_fully_qualified() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "model_key": "Ford/Quadricycle/1896/ffffffff",
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
            .get("model_key")
            .ok_or(TraversalModelError::BuildError(
                "vehicle_json does not have field `model_key`".to_string(),
            ))?;
        let model_identifier: ModelIdentifier =
            ModelIdentifier::deserialize(name_field).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Could not deserialize `model_key` field into ModelIdentifier because of {e}"
                ))
            })?;

        assert_eq!(
            r#"Ford/Quadricycle/1896/ffffffff"#,
            format!("{}", model_identifier)
        );

        Ok(())
    }
}
