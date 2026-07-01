use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::fmt::Display;
use std::num::NonZeroU64;
use std::str::FromStr;

use crate::model::model_identifier::ModelIdentifier::FullyQualifiedId;

/// The `ModelIdentifier` is an `enum` deserialized from a RouteE Powertrain .json file's "name" field,
/// and details the vehicle energy model.
///
/// The variants are either a `String` detailing the `FullyQualifiedID`:
/// ```json
/// {
///     "name": "BMW/328d_4cyl_2WD/2016",
/// }
/// ```
///
/// or a structured json object detailing the `StructuredID`:
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
/// Note: variant and version fields are both of type `Option<String>`, so they
/// are not required.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(untagged)] // serde tries each variant
pub enum ModelIdentifier {
    FullyQualifiedId(String),
    StructuredId {
        make: String,
        model: String,
        year: NonZeroU64,
        variant: Option<String>,
        version: Option<String>,
    },
}

/// `ModelIdentifierError` variants
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelIdentifierError {
    #[error("fully qualified id '{0}' must have at least make/model/year fields")]
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
                let mut attributes = id.split('/').map(|s| s.to_string());

                let (make, model, year) =
                    match (attributes.next(), attributes.next(), attributes.next()) {
                        (Some(a), Some(b), Some(c)) => (a, b, c),
                        _ => return Err(ModelIdentifierError::MissingFields(id.clone())),
                    };

                let year = NonZeroU64::from_str(&year)
                    .map_err(|_| ModelIdentifierError::InvalidYear(year))?;

                let (variant, version) = match (attributes.next(), attributes.next()) {
                    (Some(var), Some(ver)) => (Some(var), Some(ver)),
                    (Some(var), None) => {
                        return Err(ModelIdentifierError::OneOfVariantOrVersion(
                            "variant".to_string(),
                            var.to_string(),
                        ))
                    }
                    (None, Some(ver)) => {
                        return Err(ModelIdentifierError::OneOfVariantOrVersion(
                            "version".to_string(),
                            ver.to_string(),
                        ))
                    }
                    (None, None) => (None, None),
                };

                Ok(ModelIdentifier::StructuredId {
                    make,
                    model,
                    year,
                    variant,
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
                model,
                year,
                variant,
                version,
            } => match (variant, version) {
                (Some(var), Some(ver)) => Ok(FullyQualifiedId(format!(
                    "{}/{}/{}/{}/{}",
                    make, model, year, var, ver
                ))),
                (Some(var), None) => Err(ModelIdentifierError::OneOfVariantOrVersion(
                    "variant".to_string(),
                    var.to_string(),
                )),
                (None, Some(ver)) => Err(ModelIdentifierError::OneOfVariantOrVersion(
                    "version".to_string(),
                    ver.to_string(),
                )),
                (None, None) => Ok(FullyQualifiedId(format!("{}/{}/{}", make, model, year))),
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
                model,
                year,
                variant,
                version,
            } => {
                // if the variant or version are None type, prints "none"
                let opt_to_val = |o: &Option<String>| {
                    Value::String(o.clone().unwrap_or_else(|| "none".to_string()))
                };

                let obj = json!({
                    "make": make,
                    "model": model,
                    "year": year,
                    "variant": opt_to_val(variant),
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
            r#"{"make":"Ford","model":"Quadricycle","year":1896,"variant":"none","version":"none"}"#,
            format!("{}", model_identifier)
        );

        Ok(())
    }

    // Ensures that FullyQualifiedID converts to StructuredID when printing.
    #[test]
    fn test_display_fully_qualified() -> Result<(), TraversalModelError> {
        let vehicle_json_as_str = r#"{
            "name": "Ford/Quadricycle/1896",
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

        assert_eq!(r#"Ford/Quadricycle/1896"#, format!("{}", model_identifier));

        Ok(())
    }
}
