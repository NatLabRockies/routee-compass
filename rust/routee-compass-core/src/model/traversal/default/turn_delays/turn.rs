use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::model::traversal::TraversalModelError;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Turn {
    NoTurn,
    SlightRight,
    SlightLeft,
    Right,
    Left,
    SharpRight,
    SharpLeft,
    UTurn,
}

impl Display for Turn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", serde_json::to_string(self).unwrap_or_default())
    }
}

impl Turn {
    pub fn from_angle(angle: i16) -> Result<Self, TraversalModelError> {
        match angle {
            -180..=-160 => Ok(Turn::UTurn),
            -159..=-135 => Ok(Turn::SharpLeft),
            -134..=-45 => Ok(Turn::Left),
            -44..=-20 => Ok(Turn::SlightLeft),
            -19..=19 => Ok(Turn::NoTurn),
            20..=44 => Ok(Turn::SlightRight),
            45..=134 => Ok(Turn::Right),
            135..=159 => Ok(Turn::SharpRight),
            160..=180 => Ok(Turn::UTurn),
            _ => Err(TraversalModelError::TraversalModelFailure(format!(
                "Angle {angle} out of range of -180 to 180"
            ))),
        }
    }
}
