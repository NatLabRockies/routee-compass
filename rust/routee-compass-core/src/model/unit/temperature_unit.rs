use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uom::si::f64::ThermodynamicTemperature;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy, Hash, PartialOrd, Default, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TemperatureUnit {
    Fahrenheit,
    Celsius,
    #[default]
    Kelvin,
}

impl TemperatureUnit {
    pub fn to_uom(&self, value: f64) -> ThermodynamicTemperature {
        use TemperatureUnit as G;
        match self {
            G::Fahrenheit => ThermodynamicTemperature::new::<
                uom::si::thermodynamic_temperature::degree_fahrenheit,
            >(value),
            G::Celsius => ThermodynamicTemperature::new::<
                uom::si::thermodynamic_temperature::degree_celsius,
            >(value),
            G::Kelvin => {
                ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(value)
            }
        }
    }
    pub fn from_uom(&self, value: ThermodynamicTemperature) -> f64 {
        use TemperatureUnit as G;
        match self {
            G::Fahrenheit => value.get::<uom::si::thermodynamic_temperature::degree_fahrenheit>(),
            G::Celsius => value.get::<uom::si::thermodynamic_temperature::degree_celsius>(),
            G::Kelvin => value.get::<uom::si::thermodynamic_temperature::kelvin>(),
        }
    }
}

impl std::fmt::Display for TemperatureUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .replace('\"', "");
        write!(f, "{s}")
    }
}

impl FromStr for TemperatureUnit {
    type Err = serde_json::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::util::serde::serde_ops::string_deserialize(s)
    }
}
