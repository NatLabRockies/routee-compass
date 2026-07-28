use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::model::{
    state::StateVariableConfig,
    unit::{DistanceUnit, EnergyUnit, RatioUnit, SpeedUnit, TemperatureUnit, TimeUnit},
};

/// defines the required input feature and its requested unit type for a given state variable
///
/// if a unit type is provided, then the state variable is provided in the requested unit to the model.
#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputFeature {
    Distance {
        name: String,
        unit: Option<DistanceUnit>,
    },
    Speed {
        name: String,
        unit: Option<SpeedUnit>,
    },
    Time {
        name: String,
        unit: Option<TimeUnit>,
    },
    Energy {
        name: String,
        unit: Option<EnergyUnit>,
    },
    Ratio {
        name: String,
        unit: Option<RatioUnit>,
    },
    Temperature {
        name: String,
        unit: Option<TemperatureUnit>,
    },
    Custom {
        name: String,
        unit: String,
    },
}

impl InputFeature {
    pub fn name(&self) -> String {
        match self {
            InputFeature::Distance { name, .. } => name.to_owned(),
            InputFeature::Speed { name, .. } => name.to_owned(),
            InputFeature::Time { name, .. } => name.to_owned(),
            InputFeature::Energy { name, .. } => name.to_owned(),
            InputFeature::Ratio { name, .. } => name.to_owned(),
            InputFeature::Temperature { name, .. } => name.to_owned(),
            InputFeature::Custom { name, .. } => name.to_owned(),
        }
    }
    pub fn from_state_variable_config(
        fieldname: &str,
        config: &StateVariableConfig,
    ) -> InputFeature {
        match config {
            StateVariableConfig::Distance { .. } => InputFeature::Distance {
                name: fieldname.to_string(),
                unit: config
                    .get_unit_name()
                    .and_then(|name| DistanceUnit::from_str(&name).ok()),
            },
            StateVariableConfig::Time { .. } => InputFeature::Time {
                name: fieldname.to_string(),
                unit: config
                    .get_unit_name()
                    .and_then(|name| TimeUnit::from_str(&name).ok()),
            },
            StateVariableConfig::Energy { .. } => InputFeature::Energy {
                name: fieldname.to_string(),
                unit: config
                    .get_unit_name()
                    .and_then(|name| EnergyUnit::from_str(&name).ok()),
            },
            StateVariableConfig::Speed { .. } => InputFeature::Speed {
                name: fieldname.to_string(),
                unit: config
                    .get_unit_name()
                    .and_then(|name| SpeedUnit::from_str(&name).ok()),
            },
            StateVariableConfig::Ratio { .. } => InputFeature::Ratio {
                name: fieldname.to_string(),
                unit: config
                    .get_unit_name()
                    .and_then(|name| RatioUnit::from_str(&name).ok()),
            },
            StateVariableConfig::Temperature { .. } => InputFeature::Temperature {
                name: fieldname.to_string(),
                unit: config
                    .get_unit_name()
                    .and_then(|name| TemperatureUnit::from_str(&name).ok()),
            },
            StateVariableConfig::Custom { .. } => InputFeature::Custom {
                name: fieldname.to_string(),
                unit: config.get_unit_name().unwrap_or_default(),
            },
        }
    }
}

impl std::fmt::Display for InputFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string_pretty(self).unwrap_or_default();
        write!(f, "{s}")
    }
}
