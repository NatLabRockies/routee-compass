use super::{Turn, TurnDelayModelConfig};
use std::collections::HashMap;
use uom::si::f64::Time;

pub enum TurnDelayModel {
    TabularDiscrete { table: HashMap<Turn, Time> },
}

impl From<TurnDelayModelConfig> for TurnDelayModel {
    fn from(config: TurnDelayModelConfig) -> Self {
        match config {
            TurnDelayModelConfig::TabularDiscrete { table, time_unit } => {
                let table = table
                    .into_iter()
                    .map(|(turn, delay)| (turn, time_unit.to_uom(delay)))
                    .collect();
                TurnDelayModel::TabularDiscrete { table }
            }
        }
    }
}
