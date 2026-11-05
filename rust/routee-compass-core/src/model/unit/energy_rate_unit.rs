use itertools::Itertools;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
use std::str::FromStr;

use crate::model::unit::EnergyUnit;

#[derive(Debug, Clone, Eq, PartialEq, Copy, Hash, PartialOrd)]
pub enum EnergyRateUnit {
    GGPM,
    GDPM,
    KWHPM,
    KWHPKM,
}

impl EnergyRateUnit {
    pub fn to_uom(&self, _value: f64) -> String {
        todo!()
    }

    pub fn associated_energy_unit(&self) -> EnergyUnit {
        match self {
            EnergyRateUnit::GGPM => EnergyUnit::GallonsGasolineEquivalent,
            EnergyRateUnit::GDPM => EnergyUnit::GallonsDieselEquivalent,
            EnergyRateUnit::KWHPM => EnergyUnit::KilowattHours,
            EnergyRateUnit::KWHPKM => EnergyUnit::KilowattHours,
        }
    }
}

impl std::fmt::Display for EnergyRateUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnergyRateUnit::GGPM => write!(f, "gallons gasoline/mile"),
            EnergyRateUnit::GDPM => write!(f, "gallons diesel/mile"),
            EnergyRateUnit::KWHPM => write!(f, "kilowatt hour/mile"),
            EnergyRateUnit::KWHPKM => write!(f, "kilowatt hour/kilometer"),
        }
    }
}

impl FromStr for EnergyRateUnit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split("/").collect_vec()[..] {
            // Powertrain V1
            ["gallons gasoline", "mile"] => Ok(EnergyRateUnit::GGPM),
            ["gallons diesel", "mile"] => Ok(EnergyRateUnit::GDPM),
            ["kilowatt hour", "mile"] => Ok(EnergyRateUnit::KWHPM),
            ["kilowatt hour", "kilometer"] => Ok(EnergyRateUnit::KWHPKM),
            ["kWh", "mile"] => Ok(EnergyRateUnit::KWHPM),
            ["kWh", "kilometer"] => Ok(EnergyRateUnit::KWHPKM),
            // Powertrain V2 just adds an "s" to the unit
            ["gallons gasoline", "miles"] => Ok(EnergyRateUnit::GGPM),
            ["gallons diesel", "miles"] => Ok(EnergyRateUnit::GDPM),
            ["kilowatt-hour", "miles"] => Ok(EnergyRateUnit::KWHPM),
            ["kilowatt-hour", "kilometers"] => Ok(EnergyRateUnit::KWHPKM),
            _ => Err(format!(
                "expected energy rate unit in the format '<energy>/<distance>', found: {s}"
            )),
        }
    }
}

struct StrVisitor;

impl Visitor<'_> for StrVisitor {
    type Value = EnergyRateUnit;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string value in the form '<energy_unit>/<distance_unit>'")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let result = FromStr::from_str(v);
        result.map_err(|e| {
            serde::de::Error::custom(format!(
                "while attempting to deserialize value '{v}', had the following error: {e}"
            ))
        })
    }
}

impl<'de> Deserialize<'de> for EnergyRateUnit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StrVisitor)
    }
}

impl Serialize for EnergyRateUnit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::EnergyRateUnit as ERU;
    use serde_json::{self as sj, json};

    #[test]
    fn test_mpg_from_str() {
        let result = ERU::from_str("gallons gasoline/mile");
        let expected = ERU::GGPM;
        assert_eq!(result, Ok(expected))
    }

    #[test]
    fn test_gdpm_from_str() {
        let result = ERU::from_str("gallons diesel/mile");
        let expected = ERU::GDPM;
        assert_eq!(result, Ok(expected))
    }

    #[test]
    fn test_gpm_from_json() {
        let result: Result<ERU, String> =
            sj::from_value(json!("gallons gasoline/mile")).map_err(|e| e.to_string());
        let expected = ERU::GGPM;
        assert_eq!(result, Ok(expected))
    }
}
