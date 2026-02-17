use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uom::si::f64::ThermodynamicTemperature;

use crate::model::unit::TemperatureUnit;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct AmbientTemperatureConfig {
    value: f64,
    unit: TemperatureUnit,
}

impl AmbientTemperatureConfig {
    pub fn to_uom(&self) -> ThermodynamicTemperature {
        self.unit.to_uom(self.value)
    }
}
