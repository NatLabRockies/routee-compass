use std::str::FromStr;

use routee_compass_core::model::state::InputFeature;
use routee_compass_core::model::unit::{DistanceUnit, SpeedUnit, TimeUnit};
/// The minimal set of metadata needed from routee-powertrain v2 to
/// integrate with Compass's prediction models.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vehicle {
    pub mass_lbs: f64,
    pub powertrain_type: PowertrainType,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub feature_set: Vec<Feature>,
    pub real_world_adjustment_factor: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Estimator {
    pub model_file: String,
    pub estimator_type: EstimatorType,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub units: String,
    pub dtype: String,
    pub constraints: Constraints, // { lower: Option<f64>, upper: Option<f64> }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Constraints {
    pub lower: Option<f64>,
    pub upper: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PowertrainType {
    Undefined,   // from "UNDEFINED"
    Ice,         // from "ICE"
    Hev,         // from "HEV"
    Bev,         // from "BEV"
    PhevEvMode,  // from "PHEV_EV_MODE"
    PhevHevMode, // from "PHEV_HEV_MODE"
    HeavyDuty,   // from "HEAVY_DUTY"
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EstimatorType {
    // ONNX PowertrainV2 models are supported
    ONNXEstimator,
    // If this variant is deserialized, error out. cannot support stochastic models yet.
    NGBoostEstimator,
}

/// Attempt to convert a `Feature` from the routee-powertrain v2 metadata
/// into an `InputFeature` used by Compass's prediction models.
impl TryFrom<&Feature> for InputFeature {
    type Error = String;

    fn try_from(value: &Feature) -> Result<Self, Self::Error> {
        let name_lower = value.name.to_lowercase();

        if name_lower.contains("distance") {
            Ok(InputFeature::Distance {
                name: value.name.clone(),
                unit: Some(DistanceUnit::from_str(&value.units)?),
            })
        } else if name_lower.contains("speed") {
            Ok(InputFeature::Speed {
                name: value.name.clone(),
                unit: Some(SpeedUnit::from_str(&value.units)?),
            })
        } else if name_lower.contains("time") {
            Ok(InputFeature::Time {
                name: value.name.clone(),
                unit: Some(TimeUnit::from_str(&value.units)?),
            })
        } else {
            Err(format!("Input feature {} not supported.", value.name))
        }
    }
}
