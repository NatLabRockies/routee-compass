use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TimeUnit {
    Hours,
    #[default]
    Minutes,
    Seconds,
    Milliseconds,
}

impl TimeUnit {
    pub fn to_uom(&self, value: f64) -> uom::si::f64::Time {
        match self {
            TimeUnit::Hours => uom::si::f64::Time::new::<uom::si::time::hour>(value),
            TimeUnit::Minutes => uom::si::f64::Time::new::<uom::si::time::minute>(value),
            TimeUnit::Seconds => uom::si::f64::Time::new::<uom::si::time::second>(value),
            TimeUnit::Milliseconds => uom::si::f64::Time::new::<uom::si::time::millisecond>(value),
        }
    }
    pub fn from_uom(&self, value: uom::si::f64::Time) -> f64 {
        match self {
            TimeUnit::Hours => value.get::<uom::si::time::hour>(),
            TimeUnit::Minutes => value.get::<uom::si::time::minute>(),
            TimeUnit::Seconds => value.get::<uom::si::time::second>(),
            TimeUnit::Milliseconds => value.get::<uom::si::time::millisecond>(),
        }
    }
}

impl std::fmt::Display for TimeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .replace('\"', "");
        write!(f, "{s}")
    }
}

impl FromStr for TimeUnit {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hour" | "hours" | "hr" | "hrs" | "h" => Ok(TimeUnit::Hours),
            "minute" | "minutes" | "min" | "mins" | "m" => Ok(TimeUnit::Minutes),
            "second" | "seconds" | "sec" | "secs" | "s" => Ok(TimeUnit::Seconds),
            "millisecond" | "milliseconds" | "ms" => Ok(TimeUnit::Milliseconds),
            _ => Err(format!("unknown time unit '{s}'")),
        }
    }
}

impl TryFrom<String> for TimeUnit {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}
