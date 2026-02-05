use crate::model::{
    state::{CustomVariableConfig, StateModelError, StateVariable},
    unit::{DistanceUnit, EnergyUnit, RatioUnit, SpeedUnit, TemperatureUnit, TimeUnit},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Display;
use uom::si::f64::*;

#[derive(Clone, PartialEq, PartialOrd, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StateVariableConfig {
    Distance {
        initial: Length,
        accumulator: bool,
        output_unit: Option<DistanceUnit>,
    },
    Time {
        initial: Time,
        accumulator: bool,
        output_unit: Option<TimeUnit>,
    },
    Speed {
        initial: Velocity,
        accumulator: bool,
        output_unit: Option<SpeedUnit>,
    },
    Energy {
        initial: Energy,
        accumulator: bool,
        output_unit: Option<EnergyUnit>,
    },
    Ratio {
        initial: Ratio,
        accumulator: bool,
        output_unit: Option<RatioUnit>,
    },
    Temperature {
        initial: ThermodynamicTemperature,
        accumulator: bool,
        output_unit: Option<TemperatureUnit>,
    },
    Custom {
        custom_type: String,
        value: CustomVariableConfig,
        accumulator: bool,
    },
}

impl StateVariableConfig {
    pub fn initial_value(&self) -> Result<StateVariable, StateModelError> {
        match self {
            StateVariableConfig::Distance { initial, .. } => {
                Ok(StateVariable(DistanceUnit::default().from_uom(*initial)))
            }
            StateVariableConfig::Time { initial, .. } => {
                Ok(StateVariable(TimeUnit::default().from_uom(*initial)))
            }
            StateVariableConfig::Speed { initial, .. } => {
                Ok(StateVariable(SpeedUnit::default().from_uom(*initial)))
            }
            StateVariableConfig::Energy { initial, .. } => {
                Ok(StateVariable(EnergyUnit::default().from_uom(*initial)))
            }
            StateVariableConfig::Ratio { initial, .. } => {
                Ok(StateVariable(RatioUnit::default().from_uom(*initial)))
            }
            StateVariableConfig::Temperature { initial, .. } => {
                Ok(StateVariable(TemperatureUnit::default().from_uom(*initial)))
            }
            StateVariableConfig::Custom { value, .. } => value.initial(),
        }
    }
    pub fn is_accumulator(&self) -> bool {
        match self {
            StateVariableConfig::Distance { accumulator, .. } => *accumulator,
            StateVariableConfig::Time { accumulator, .. } => *accumulator,
            StateVariableConfig::Speed { accumulator, .. } => *accumulator,
            StateVariableConfig::Energy { accumulator, .. } => *accumulator,
            StateVariableConfig::Ratio { accumulator, .. } => *accumulator,
            StateVariableConfig::Temperature { accumulator, .. } => *accumulator,
            StateVariableConfig::Custom { accumulator, .. } => *accumulator,
        }
    }
    pub fn get_custom_feature_format(&self) -> Result<&CustomVariableConfig, StateModelError> {
        match self {
            StateVariableConfig::Custom { value, .. } => Ok(value),
            _ => Err(StateModelError::UnexpectedFeatureType(
                "Expected Custom feature type".to_string(),
                format!("Got: {self:?}"),
            )),
        }
    }

    /// the stringified name of the variable's output unit, if set by user.
    /// if None, it implies the output unit is the Default implementation of the Unit type.
    pub fn output_unit_name(&self) -> Option<String> {
        match self {
            StateVariableConfig::Distance { output_unit, .. } => {
                output_unit.map(|u| format!("{}", u))
            }
            StateVariableConfig::Time { output_unit, .. } => output_unit.map(|u| format!("{}", u)),
            StateVariableConfig::Speed { output_unit, .. } => output_unit.map(|u| format!("{}", u)),
            StateVariableConfig::Energy { output_unit, .. } => {
                output_unit.map(|u| format!("{}", u))
            }
            StateVariableConfig::Ratio { output_unit, .. } => output_unit.map(|u| format!("{}", u)),
            StateVariableConfig::Temperature { output_unit, .. } => {
                output_unit.map(|u| format!("{}", u))
            }
            StateVariableConfig::Custom { custom_type, .. } => Some(custom_type.clone()),
        }
    }

    pub fn get_unit_name(&self) -> String {
        match self {
            StateVariableConfig::Distance { output_unit, .. } => output_unit.map_or_else(
                || format!("{}", DistanceUnit::default()),
                |u| format!("{}", u),
            ),
            StateVariableConfig::Time { output_unit, .. } => {
                output_unit.map_or_else(|| format!("{}", TimeUnit::default()), |u| format!("{}", u))
            }
            StateVariableConfig::Speed { output_unit, .. } => output_unit
                .map_or_else(|| format!("{}", SpeedUnit::default()), |u| format!("{}", u)),
            StateVariableConfig::Energy { output_unit, .. } => output_unit.map_or_else(
                || format!("{}", EnergyUnit::default()),
                |u| format!("{}", u),
            ),
            StateVariableConfig::Ratio { output_unit, .. } => output_unit
                .map_or_else(|| format!("{}", RatioUnit::default()), |u| format!("{}", u)),
            StateVariableConfig::Temperature { output_unit, .. } => output_unit.map_or_else(
                || format!("{}", TemperatureUnit::default()),
                |u| format!("{}", u),
            ),
            StateVariableConfig::Custom { custom_type, .. } => custom_type.clone(),
        }
    }

    pub fn get_feature_type(&self) -> String {
        match self {
            StateVariableConfig::Distance { .. } => "distance".to_string(),
            StateVariableConfig::Time { .. } => "time".to_string(),
            StateVariableConfig::Speed { .. } => "speed".to_string(),
            StateVariableConfig::Energy { .. } => "energy".to_string(),
            StateVariableConfig::Ratio { .. } => "ratio".to_string(),
            StateVariableConfig::Temperature { .. } => "temperature".to_string(),
            StateVariableConfig::Custom { .. } => "custom".to_string(),
        }
    }

    pub fn serialize_variable(
        &self,
        state_variable: &StateVariable,
    ) -> Result<serde_json::Value, StateModelError> {
        match self {
            StateVariableConfig::Distance { output_unit, .. } => {
                output_unit.map_or(Ok(json![state_variable]), |unit| {
                    let uom_value = DistanceUnit::default().to_uom(state_variable.into());
                    Ok(json![unit.from_uom(uom_value)])
                })
            }
            StateVariableConfig::Time { output_unit, .. } => {
                output_unit.map_or(Ok(json![state_variable]), |unit| {
                    let uom_value = TimeUnit::default().to_uom(state_variable.into());
                    Ok(json![unit.from_uom(uom_value)])
                })
            }
            StateVariableConfig::Speed { output_unit, .. } => {
                output_unit.map_or(Ok(json![state_variable]), |unit| {
                    let uom_value = SpeedUnit::default().to_uom(state_variable.into());
                    Ok(json![unit.from_uom(uom_value)])
                })
            }
            StateVariableConfig::Energy { output_unit, .. } => {
                output_unit.map_or(Ok(json![state_variable]), |unit| {
                    let uom_value = EnergyUnit::default().to_uom(state_variable.into());
                    Ok(json![unit.from_uom(uom_value)])
                })
            }
            StateVariableConfig::Ratio { output_unit, .. } => {
                output_unit.map_or(Ok(json![state_variable]), |unit| {
                    let uom_value = RatioUnit::default().to_uom(state_variable.into());
                    Ok(json![unit.from_uom(uom_value)])
                })
            }
            StateVariableConfig::Temperature { output_unit, .. } => {
                output_unit.map_or(Ok(json![state_variable]), |unit| {
                    let uom_value = TemperatureUnit::default().to_uom(state_variable.into());
                    Ok(json![unit.from_uom(uom_value)])
                })
            }
            StateVariableConfig::Custom { value, .. } => match value {
                CustomVariableConfig::FloatingPoint { .. } => {
                    value.decode_f64(state_variable).map(|v| json![v])
                }
                CustomVariableConfig::SignedInteger { .. } => {
                    value.decode_i64(state_variable).map(|v| json![v])
                }
                CustomVariableConfig::UnsignedInteger { .. } => {
                    value.decode_u64(state_variable).map(|v| json![v])
                }
                CustomVariableConfig::Boolean { .. } => {
                    value.decode_bool(state_variable).map(|v| json![v])
                }
            },
        }
    }
}

impl Display for StateVariableConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateVariableConfig::Distance {
                initial,
                accumulator,
                output_unit,
            } => {
                write!(
                    f,
                    "Distance: {initial:?} (Accumulator: {accumulator}, Output Unit: {output_unit:?})"
                )
            }
            StateVariableConfig::Time {
                initial,
                accumulator,
                output_unit,
            } => {
                write!(
                    f,
                    "Time: {initial:?} (Accumulator: {accumulator}, Output Unit: {output_unit:?})"
                )
            }
            StateVariableConfig::Speed {
                initial,
                accumulator,
                output_unit,
            } => {
                write!(
                    f,
                    "Speed: {initial:?} (Accumulator: {accumulator}, Output Unit: {output_unit:?})"
                )
            }
            StateVariableConfig::Energy {
                initial,
                accumulator,
                output_unit,
            } => {
                write!(
                    f,
                    "Energy: {initial:?} (Accumulator: {accumulator}, Output Unit: {output_unit:?})"
                )
            }
            StateVariableConfig::Ratio {
                initial,
                accumulator,
                output_unit,
            } => {
                write!(
                    f,
                    "Ratio: {initial:?} (Accumulator: {accumulator}, Output Unit: {output_unit:?})"
                )
            }
            StateVariableConfig::Temperature {
                initial,
                accumulator,
                output_unit,
            } => {
                write!(
                    f,
                    "Temperature: {initial:?} (Accumulator: {accumulator}, Output Unit: {output_unit:?})"
                )
            }
            StateVariableConfig::Custom {
                custom_type,
                value,
                accumulator,
            } => {
                write!(
                    f,
                    "Custom Type: {custom_type} (Value: {value} Accumulator: {accumulator})"
                )
            }
        }
    }
}
