use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uom::si::f64::Mass;

#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Default, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WeightUnit {
    #[default]
    Pounds,
    Tons,
    Kg,
}

impl WeightUnit {
    pub fn to_uom(&self, value: f64) -> Mass {
        match self {
            Self::Pounds => uom::si::mass::Mass::new::<uom::si::mass::pound>(value),
            Self::Tons => uom::si::mass::Mass::new::<uom::si::mass::ton>(value),
            Self::Kg => uom::si::mass::Mass::new::<uom::si::mass::kilogram>(value),
        }
    }
}

impl std::fmt::Display for WeightUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .replace('\"', "");
        write!(f, "{s}")
    }
}

impl FromStr for WeightUnit {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pound" | "pounds" | "lb" | "lbs" => Ok(Self::Pounds),
            "ton" | "tons" => Ok(Self::Tons),
            "kilogram" | "kilograms" | "kg" | "kgs" => Ok(Self::Kg),
            _ => Err(format!("unknown weight unit '{s}'")),
        }
    }
}

impl TryFrom<String> for WeightUnit {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}
