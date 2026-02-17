use std::fmt::Display;

use super::{custom_variable_type::CustomVariableType, state_model_error::StateModelError};
use crate::model::state::StateVariable;
use ordered_float::OrderedFloat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// codec between StateVar values and basic Rust types.
/// all stateful information in Compass is encoded as a vector of floating
/// point values. a codec provides a mapping to and from types in order
/// to represent different real-numbered and discrete information types.
///
/// because Rust does not support dependent types and trait objects cannot
/// return "Self"s, we are limited in how we generalize over unit types.
/// these codecs substitute for the limitations in the type system, providing
/// a validation check on encoding and decoding to/from StateVar instances
/// and also a codec value that can be instantiated from configuration for a
/// given StateModel.
///
/// warning: this could backfire, but probably in extreme cases that can be avoided.
/// for example, if the user selects esoteric integer values that are not well-represented
/// in floating point, then an encode -> decode roundtrip may not be idempotent.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum CustomVariableConfig {
    FloatingPoint { initial: OrderedFloat<f64> },
    SignedInteger { initial: i64 },
    UnsignedInteger { initial: u64 },
    Boolean { initial: bool },
}

impl Default for CustomVariableConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Display for CustomVariableConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let initial = self
            .initial()
            .map(|i| format!("{i}"))
            .unwrap_or_else(|_| String::from("<invalid initial argument>"));
        write!(f, "{}: {}", self.name(), initial)
    }
}

impl JsonSchema for CustomVariableConfig {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CustomVariableConfig".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let create_schema = |name: &str, unit: &str| {
            json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "const": name
                    },
                    "initial": { "type": unit },
                },
                "required": vec!["type", "initial"],
            })
        };
        let schema_value = schemars::json_schema!({
            "oneOf": [
                create_schema("floating_point", "number"),
                create_schema("signed_integer", "integer"),
                create_schema("unsigned_integer", "integer"),
                create_schema("boolean", "boolean"),
            ]
        });
        schema_value
    }
}

impl CustomVariableConfig {
    pub const DEFAULT: Self = Self::FloatingPoint {
        initial: OrderedFloat(0.0),
    };

    pub fn name(&self) -> String {
        match self {
            CustomVariableConfig::FloatingPoint { initial: _ } => {
                CustomVariableType::FloatingPoint.to_string()
            }
            CustomVariableConfig::SignedInteger { initial: _ } => {
                CustomVariableType::SignedInteger.to_string()
            }
            CustomVariableConfig::UnsignedInteger { initial: _ } => {
                CustomVariableType::UnsignedInteger.to_string()
            }
            CustomVariableConfig::Boolean { initial: _ } => CustomVariableType::Boolean.to_string(),
        }
    }

    pub fn initial(&self) -> Result<StateVariable, StateModelError> {
        match self {
            CustomVariableConfig::FloatingPoint { initial } => self.encode_f64(initial),
            CustomVariableConfig::SignedInteger { initial } => self.encode_i64(initial),
            CustomVariableConfig::UnsignedInteger { initial } => self.encode_u64(initial),
            CustomVariableConfig::Boolean { initial } => self.encode_bool(initial),
        }
    }

    pub fn encode_f64(&self, value: &f64) -> Result<StateVariable, StateModelError> {
        match self {
            CustomVariableConfig::FloatingPoint { initial: _ } => Ok(StateVariable(*value)),
            _ => Err(StateModelError::EncodeError(
                CustomVariableType::FloatingPoint.to_string(),
                self.name(),
            )),
        }
    }

    pub fn encode_i64(&self, value: &i64) -> Result<StateVariable, StateModelError> {
        match self {
            CustomVariableConfig::SignedInteger { initial: _ } => Ok(StateVariable(*value as f64)),
            _ => Err(StateModelError::EncodeError(
                CustomVariableType::SignedInteger.to_string(),
                self.name(),
            )),
        }
    }

    pub fn encode_u64(&self, value: &u64) -> Result<StateVariable, StateModelError> {
        match self {
            CustomVariableConfig::UnsignedInteger { initial: _ } => {
                Ok(StateVariable(*value as f64))
            }
            _ => Err(StateModelError::EncodeError(
                CustomVariableType::UnsignedInteger.to_string(),
                self.name(),
            )),
        }
    }

    pub fn encode_bool(&self, value: &bool) -> Result<StateVariable, StateModelError> {
        match self {
            CustomVariableConfig::Boolean { initial: _ } => {
                if *value {
                    Ok(StateVariable(1.0))
                } else {
                    Ok(StateVariable(0.0))
                }
            }
            _ => Err(StateModelError::EncodeError(
                CustomVariableType::Boolean.to_string(),
                self.name(),
            )),
        }
    }

    pub fn decode_f64(&self, value: &StateVariable) -> Result<f64, StateModelError> {
        match self {
            CustomVariableConfig::FloatingPoint { initial: _ } => Ok(value.0),
            _ => Err(StateModelError::DecodeError(
                *value,
                CustomVariableType::FloatingPoint.to_string(),
                self.name(),
            )),
        }
    }
    pub fn decode_i64(&self, value: &StateVariable) -> Result<i64, StateModelError> {
        match self {
            CustomVariableConfig::SignedInteger { initial: _ } => Ok(value.0 as i64),
            _ => Err(StateModelError::DecodeError(
                *value,
                CustomVariableType::SignedInteger.to_string(),
                self.name(),
            )),
        }
    }
    pub fn decode_u64(&self, value: &StateVariable) -> Result<u64, StateModelError> {
        match self {
            CustomVariableConfig::UnsignedInteger { initial: _ } => {
                if value < &StateVariable::ZERO {
                    Err(StateModelError::ValueError(
                        *value,
                        CustomVariableType::UnsignedInteger.to_string(),
                    ))
                } else {
                    Ok(value.0 as u64)
                }
            }
            _ => Err(StateModelError::DecodeError(
                *value,
                CustomVariableType::UnsignedInteger.to_string(),
                self.name(),
            )),
        }
    }
    pub fn decode_bool(&self, value: &StateVariable) -> Result<bool, StateModelError> {
        match self {
            CustomVariableConfig::Boolean { initial: _ } => {
                let is_false = value.0 == 0.0;
                if is_false {
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            _ => Err(StateModelError::DecodeError(
                *value,
                CustomVariableType::Boolean.to_string(),
                self.name(),
            )),
        }
    }
}
