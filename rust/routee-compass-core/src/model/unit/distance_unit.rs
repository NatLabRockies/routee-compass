use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uom::si::f64::Length;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Default, JsonSchema)]
#[serde(rename_all = "snake_case", try_from = "String")]
pub enum DistanceUnit {
    Meters,
    Kilometers,
    #[default]
    Miles,
    Inches,
    Feet,
}

impl DistanceUnit {
    pub fn to_uom(&self, value: f64) -> Length {
        match self {
            DistanceUnit::Meters => Length::new::<uom::si::length::meter>(value),
            DistanceUnit::Kilometers => Length::new::<uom::si::length::kilometer>(value),
            DistanceUnit::Miles => Length::new::<uom::si::length::mile>(value),
            DistanceUnit::Inches => Length::new::<uom::si::length::inch>(value),
            DistanceUnit::Feet => Length::new::<uom::si::length::foot>(value),
        }
    }
    pub fn from_uom(&self, value: Length) -> f64 {
        match self {
            DistanceUnit::Meters => value.get::<uom::si::length::meter>(),
            DistanceUnit::Kilometers => value.get::<uom::si::length::kilometer>(),
            DistanceUnit::Miles => value.get::<uom::si::length::mile>(),
            DistanceUnit::Inches => value.get::<uom::si::length::inch>(),
            DistanceUnit::Feet => value.get::<uom::si::length::foot>(),
        }
    }
}

impl std::fmt::Display for DistanceUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .replace('\"', "");
        write!(f, "{s}")
    }
}

impl FromStr for DistanceUnit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use DistanceUnit as D;
        match s.trim().to_lowercase().as_str() {
            "meters" | "meter" => Ok(D::Meters),
            "km" | "kilometers" | "kilometer" => Ok(D::Kilometers),
            "miles" | "mile" => Ok(D::Miles),
            "inches" | "inch" | "in" => Ok(D::Inches),
            "feet" | "ft" => Ok(D::Feet),
            _ => Err(format!("unknown distance unit '{s}'")),
        }
    }
}

impl TryFrom<String> for DistanceUnit {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}
