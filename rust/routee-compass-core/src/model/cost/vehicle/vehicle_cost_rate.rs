use crate::model::{
    cost::CostModelError,
    state::{CustomVariableType, StateModel, StateVariable},
    unit::{Cost, DistanceUnit, EnergyUnit, RatioUnit, SpeedUnit, TemperatureUnit, TimeUnit},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uom::si::f64::*;
/// a mapping for how to transform vehicle state values into a Cost.
/// mappings can be a single instance of Raw, Factor, or Offset mapping.
///
/// when multiple mappings are specified they are applied sequentially (in user-defined order)
/// to the state value.
#[derive(Serialize, Deserialize, Clone, Default, Debug, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VehicleCostRate {
    /// no cost rate, effectively zeroes out a state variable's impact
    /// on the cost of traversal.
    #[default]
    Zero,
    Raw,
    /// use a distance value as a cost, optionally multiplied against a cost factor
    Distance {
        factor: f64,
        unit: DistanceUnit,
    },
    /// use a time value as a cost, optionally multiplied against a cost factor
    Time {
        factor: f64,
        unit: TimeUnit,
    },
    /// use a speed value as a cost, optionally multiplied against a cost factor
    Speed {
        factor: f64,
        unit: SpeedUnit,
    },
    /// use a energy value as a cost, optionally multiplied against a cost factor
    Energy {
        factor: f64,
        unit: EnergyUnit,
    },
    /// use a ratio value as a cost, optionally multiplied against a cost factor
    Ratio {
        factor: f64,
        unit: RatioUnit,
    },
    /// use a temperature value as a cost, optionally multiplied against a cost factor
    Temperature {
        factor: f64,
        unit: TemperatureUnit,
    },
    /// use a custom value as a cost, optionally multiplied against a cost factor
    Custom {
        variable_type: CustomVariableType,
        factor: Option<f64>,
    },
    // Combined(Vec<VehicleCostRate>),
    // leaving room for extension if we need to do any fancier math, maybe not needed
    // Poly2 { x0: f64, x1: f64 },
    // Exp { base: f64, exp_coefficient: f64 },
}

impl VehicleCostRate {
    /// computes the cost for a state variable based on a search state using
    /// this vehicle cost rate configuration.
    pub fn compute_cost(
        &self,
        name: &str,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<Cost, CostModelError> {
        let raw = self.get_raw(name, state, state_model)?;
        let cost_factor = self.get_factor();
        let cost = Cost::new(raw * cost_factor);
        Ok(cost)
    }

    pub fn get_unit_name(&self) -> Option<String> {
        match self {
            VehicleCostRate::Zero => None,
            VehicleCostRate::Raw => None,
            VehicleCostRate::Distance { unit, .. } => Some(unit.to_string()),
            VehicleCostRate::Time { unit, .. } => Some(unit.to_string()),
            VehicleCostRate::Speed { unit, .. } => Some(unit.to_string()),
            VehicleCostRate::Energy { unit, .. } => Some(unit.to_string()),
            VehicleCostRate::Ratio { unit, .. } => Some(unit.to_string()),
            VehicleCostRate::Temperature { unit, .. } => Some(unit.to_string()),
            VehicleCostRate::Custom { variable_type, .. } => Some(variable_type.to_string()),
        }
    }

    /// helper function to get the multiplicitive factor for a given [`VehicleCostRate`].
    pub fn get_factor(&self) -> f64 {
        match self {
            VehicleCostRate::Zero => 0.0,
            VehicleCostRate::Raw => 1.0,
            VehicleCostRate::Distance { factor, .. } => *factor,
            VehicleCostRate::Time { factor, .. } => *factor,
            VehicleCostRate::Speed { factor, .. } => *factor,
            VehicleCostRate::Energy { factor, .. } => *factor,
            VehicleCostRate::Ratio { factor, .. } => *factor,
            VehicleCostRate::Temperature { factor, .. } => *factor,
            VehicleCostRate::Custom { factor, .. } => factor.unwrap_or(1.0),
        }
    }

    /// helper function to get the raw state variable as an f64 which can be used to compute a cost.
    pub fn get_raw(
        &self,
        name: &str,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<f64, CostModelError> {
        match self {
            VehicleCostRate::Zero => Ok(0.0),
            VehicleCostRate::Raw => {
                let raw = state_model.get_raw_state_variable(state, name)?;
                Ok(raw.0)
            }
            VehicleCostRate::Distance { unit, .. } => {
                let value: Length = state_model.get_distance(state, name)?;
                let raw = unit.from_uom(value);
                Ok(raw)
            }
            VehicleCostRate::Time { unit, .. } => {
                let value: Time = state_model.get_time(state, name)?;
                let raw = unit.from_uom(value);
                Ok(raw)
            }
            VehicleCostRate::Speed { unit, .. } => {
                let value: Velocity = state_model.get_speed(state, name)?;
                let raw = unit.from_uom(value);
                Ok(raw)
            }
            VehicleCostRate::Energy { unit, .. } => {
                let value: Energy = state_model.get_energy(state, name)?;
                let raw = unit.from_uom(value);
                Ok(raw)
            }
            VehicleCostRate::Ratio { unit, .. } => {
                let value: Ratio = state_model.get_ratio(state, name)?;
                let raw = unit.from_uom(value);
                Ok(raw)
            }
            VehicleCostRate::Temperature { unit, .. } => {
                let value: ThermodynamicTemperature = state_model.get_temperature(state, name)?;
                let raw = unit.from_uom(value);
                Ok(raw)
            }
            VehicleCostRate::Custom { variable_type, .. } => match variable_type {
                CustomVariableType::FloatingPoint => {
                    let value = state_model.get_custom_f64(state, name)?;
                    Ok(value)
                }
                CustomVariableType::SignedInteger => {
                    let value = state_model.get_custom_i64(state, name)?;
                    Ok(value as f64)
                }
                CustomVariableType::UnsignedInteger => {
                    let value = state_model.get_custom_u64(state, name)?;
                    Ok(value as f64)
                }
                CustomVariableType::Boolean => {
                    let is_one = state_model.get_custom_bool(state, name)?;
                    if is_one {
                        Ok(1.0)
                    } else {
                        Ok(0.0)
                    }
                }
            },
        }
    }
}

impl std::fmt::Display for VehicleCostRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let factor = match self {
            VehicleCostRate::Zero => None,
            VehicleCostRate::Raw => None,
            VehicleCostRate::Distance { factor, .. } => Some(*factor),
            VehicleCostRate::Time { factor, .. } => Some(*factor),
            VehicleCostRate::Speed { factor, .. } => Some(*factor),
            VehicleCostRate::Energy { factor, .. } => Some(*factor),
            VehicleCostRate::Ratio { factor, .. } => Some(*factor),
            VehicleCostRate::Temperature { factor, .. } => Some(*factor),
            VehicleCostRate::Custom { factor, .. } => factor.to_owned(),
        };
        let s = match (factor, self.get_unit_name()) {
            (None, None) => String::from("zeroed"),
            (None, Some(unit)) => format!("cost per {unit}"),
            (Some(factor), None) => format!("cost times {factor}"),
            (Some(factor), Some(unit)) => format!("cost times {factor} per {unit}"),
        };
        write!(f, "{}", s)
    }
}
