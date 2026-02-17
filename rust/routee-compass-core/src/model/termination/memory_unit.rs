use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq, JsonSchema)]
pub enum MemoryUnit {
    #[default]
    #[serde(rename = "MB")]
    Megabytes,
    #[serde(rename = "GB")]
    Gigabytes,
}

impl MemoryUnit {
    pub const MEGABYTES_PER_BYTE: f64 = 0.00000095;
    pub const GIGABYTES_PER_BYTE: f64 = 0.000000000931323;

    /// converts the given value in bytes into this memory unit
    pub fn convert(&self, bytes: f64) -> f64 {
        match self {
            MemoryUnit::Megabytes => bytes * Self::MEGABYTES_PER_BYTE,
            MemoryUnit::Gigabytes => bytes * Self::GIGABYTES_PER_BYTE,
        }
    }
}

impl Display for MemoryUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MemoryUnit::Megabytes => "MB",
            MemoryUnit::Gigabytes => "GB",
        };
        write!(f, "{}", s)
    }
}
