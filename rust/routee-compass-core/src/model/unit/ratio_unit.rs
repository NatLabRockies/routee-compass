use crate::util::serde::serde_ops::string_deserialize;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uom::si::f64::Ratio;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy, Hash, PartialOrd, Default, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RatioUnit {
    Percent,
    #[default]
    Decimal,
    Millis,
}

impl RatioUnit {
    pub fn to_uom(&self, value: f64) -> Ratio {
        use RatioUnit as G;
        match self {
            G::Percent => Ratio::new::<uom::si::ratio::percent>(value),
            G::Decimal => Ratio::new::<uom::si::ratio::ratio>(value),
            G::Millis => Ratio::new::<uom::si::ratio::per_mille>(value),
        }
    }

    pub fn from_uom(&self, value: Ratio) -> f64 {
        use RatioUnit as G;
        match self {
            G::Percent => value.get::<uom::si::ratio::percent>(),
            G::Decimal => value.get::<uom::si::ratio::ratio>(),
            G::Millis => value.get::<uom::si::ratio::per_mille>(),
        }
    }
}

impl std::fmt::Display for RatioUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .replace('\"', "");
        write!(f, "{s}")
    }
}

impl FromStr for RatioUnit {
    type Err = serde_json::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        string_deserialize(s)
    }
}
