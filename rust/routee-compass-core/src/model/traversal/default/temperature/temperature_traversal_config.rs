use crate::model::unit::TemperatureUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::ThermodynamicTemperature;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TemperatureTraversalConfig {
    #[serde(rename = "type")]
    pub r#type: String,
    pub default_ambient_temperature: Option<AmbientTemperatureConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmbientTemperatureConfig {
    pub value: f64,
    pub unit: TemperatureUnit,
}

impl AmbientTemperatureConfig {
    pub fn to_uom(&self) -> ThermodynamicTemperature {
        self.unit.to_uom(self.value)
    }
}
