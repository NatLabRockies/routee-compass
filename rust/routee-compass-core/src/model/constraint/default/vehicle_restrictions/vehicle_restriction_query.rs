use super::VehicleParameterConfig;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VehicleRestrictionQuery {
    pub vehicle_parameters: Vec<VehicleParameterConfig>,
}
