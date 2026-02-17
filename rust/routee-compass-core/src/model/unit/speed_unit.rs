use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::str::FromStr;
use uom::si::f64::Velocity;

#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash, PartialOrd, Default, JsonSchema)]
pub enum SpeedUnit {
    KPH,
    #[default]
    MPH,
    MPS,
}

impl SpeedUnit {
    pub fn to_uom(&self, value: f64) -> Velocity {
        match self {
            Self::KPH => Velocity::new::<uom::si::velocity::kilometer_per_hour>(value),
            Self::MPH => Velocity::new::<uom::si::velocity::mile_per_hour>(value),
            Self::MPS => Velocity::new::<uom::si::velocity::meter_per_second>(value),
        }
    }

    pub fn from_uom(&self, value: Velocity) -> f64 {
        match self {
            Self::KPH => value.get::<uom::si::velocity::kilometer_per_hour>(),
            Self::MPH => value.get::<uom::si::velocity::mile_per_hour>(),
            Self::MPS => value.get::<uom::si::velocity::meter_per_second>(),
        }
    }
}

impl std::fmt::Display for SpeedUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpeedUnit::KPH => write!(f, "kilometers/hour"),
            SpeedUnit::MPH => write!(f, "miles/hour"),
            SpeedUnit::MPS => write!(f, "meters/second"),
        }
    }
}

impl FromStr for SpeedUnit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mph" => Ok(SpeedUnit::MPH),
            "kph" => Ok(SpeedUnit::KPH),
            "mps" => Ok(SpeedUnit::MPS),
            "miles/hour" => Ok(SpeedUnit::MPH),
            "kilometers/hour" => Ok(SpeedUnit::KPH),
            "meters/second" => Ok(SpeedUnit::MPS),
            _ => Err(format!(
                "expected speed unit as 'kph', 'mph', 'mps', or in the format '<distance unit>/<time unit>', found: {s}"
            )),
        }
    }
}

impl<'de> Deserialize<'de> for SpeedUnit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for SpeedUnit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.to_string())
    }
}
