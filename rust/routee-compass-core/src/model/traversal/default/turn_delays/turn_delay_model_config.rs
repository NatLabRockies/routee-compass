use crate::model::unit::TimeUnit;
use std::collections::HashMap;

use super::Turn;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case", tag = "type", deny_unknown_fields)]
pub enum TurnDelayModelConfig {
    TabularDiscrete {
        /// table mapping fixed turn angles to delays in the provided time unit
        table: HashMap<Turn, f64>,
        /// time unit of delays
        time_unit: TimeUnit,
    },
}
