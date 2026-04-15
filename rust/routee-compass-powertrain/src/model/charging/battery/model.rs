use routee_compass_core::model::{
    constraint::{ConstraintModel, ConstraintModelError},
    network::Edge,
    state::{StateModel, StateVariable},
    traversal::EdgeFrontierContext,
};
use uom::si::f64::Ratio;

use crate::model::fieldname;

#[derive(Clone)]
pub struct BatteryFilter {
    pub soc_lower_bound: Ratio,
}

impl ConstraintModel for BatteryFilter {
    fn valid_frontier(
        &self,
        _ctx: &EdgeFrontierContext,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        if !state_model.contains_key(fieldname::TRIP_SOC) {
            // if we don't have the trip_soc, then this frontier is valid
            return Ok(true);
        }
        let soc: Ratio = state_model.get_ratio(state, fieldname::TRIP_SOC).map_err(|_| {
            ConstraintModelError::ConstraintModelError(
                "BatteryFrontier constraint model requires the state variable 'trip_soc' but not found".to_string(),
            )
        })?;
        Ok(soc > self.soc_lower_bound)
    }

    fn valid_edge(&self, _edge: &Edge) -> Result<bool, ConstraintModelError> {
        Ok(true)
    }
}
