use crate::model::{
    constraint::default::vehicle_restrictions::VehicleParameter,
    unit::{DistanceUnit, WeightUnit},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VehicleParameterConfig {
    Height { value: f64, unit: DistanceUnit },
    Width { value: f64, unit: DistanceUnit },
    TotalLength { value: f64, unit: DistanceUnit },
    TrailerLength { value: f64, unit: DistanceUnit },
    TotalWeight { value: f64, unit: WeightUnit },
    WeightPerAxle { value: f64, unit: WeightUnit },
}

impl From<VehicleParameterConfig> for VehicleParameter {
    fn from(val: VehicleParameterConfig) -> Self {
        match val {
            VehicleParameterConfig::Height { value, unit } => VehicleParameter::Height {
                value: unit.to_uom(value),
            },
            VehicleParameterConfig::Width { value, unit } => VehicleParameter::Width {
                value: unit.to_uom(value),
            },
            VehicleParameterConfig::TotalLength { value, unit } => VehicleParameter::TotalLength {
                value: unit.to_uom(value),
            },
            VehicleParameterConfig::TrailerLength { value, unit } => {
                VehicleParameter::TrailerLength {
                    value: unit.to_uom(value),
                }
            }
            VehicleParameterConfig::TotalWeight { value, unit } => VehicleParameter::TotalWeight {
                value: unit.to_uom(value),
            },
            VehicleParameterConfig::WeightPerAxle { value, unit } => {
                VehicleParameter::WeightPerAxle {
                    value: unit.to_uom(value),
                }
            }
        }
    }
}
