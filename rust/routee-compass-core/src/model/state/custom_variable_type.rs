use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CustomVariableType {
    FloatingPoint,
    SignedInteger,
    UnsignedInteger,
    Boolean,
}

impl Display for CustomVariableType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            CustomVariableType::FloatingPoint => String::from("floating_point"),
            CustomVariableType::SignedInteger => String::from("signed_integer"),
            CustomVariableType::UnsignedInteger => String::from("unsigned_integer"),
            CustomVariableType::Boolean => String::from("boolean"),
        };
        write!(f, "{msg}")
    }
}
