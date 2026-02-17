use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uom::si::f64::Energy;

// Extending the SI system to include liquid fuel units
// https://www.eia.gov/energyexplained/units-and-calculators/energy-conversion-calculators.php
unit! {
    system: uom::si;
    quantity: uom::si::energy;

    // This is assuming joules as base unit
    @gal_gas: 1.268_329_84E8; "GGE", "Gallons gasoline equivalent", "Gal gasoline equivalent";
    @gal_diesel: 1.449_452E8; "GDE", "Gallons diesel equivalent", "Gal diesel equivalent";
    @liter_gas: 3.350_554E7; "LGE", "Liters gasoline equivalent", "L gasoline equivalent";
    @liter_diesel: 3.829_023_6E7; "LDE", "Liters diesel equivalent", "L diesel equivalent";
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy, Hash, PartialOrd, Default, JsonSchema,
)]
#[serde(rename_all = "snake_case", try_from = "String")]
pub enum EnergyUnit {
    KilowattHours,
    #[default]
    GallonsGasolineEquivalent,
    GallonsDieselEquivalent,
    LitersGasolineEquivalent,
    LitersDieselEquivalent,
}

impl EnergyUnit {
    pub fn to_uom(&self, value: f64) -> Energy {
        // TODO: This should be updated when we have gas and diesel units in UOM.
        match self {
            Self::KilowattHours => Energy::new::<uom::si::energy::kilowatt_hour>(value),
            Self::GallonsGasolineEquivalent => Energy::new::<gal_gas>(value),
            Self::GallonsDieselEquivalent => Energy::new::<gal_diesel>(value),
            Self::LitersGasolineEquivalent => Energy::new::<liter_gas>(value),
            Self::LitersDieselEquivalent => Energy::new::<liter_diesel>(value),
        }
    }
    pub fn from_uom(&self, value: Energy) -> f64 {
        match self {
            Self::KilowattHours => value.get::<uom::si::energy::kilowatt_hour>(),
            Self::GallonsGasolineEquivalent => value.get::<gal_gas>(),
            Self::GallonsDieselEquivalent => value.get::<gal_diesel>(),
            Self::LitersGasolineEquivalent => value.get::<liter_gas>(),
            Self::LitersDieselEquivalent => value.get::<liter_diesel>(),
        }
    }
}

impl std::fmt::Display for EnergyUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self)
            .map_err(|_| std::fmt::Error)?
            .replace('\"', "");
        write!(f, "{s}")
    }
}

impl FromStr for EnergyUnit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use EnergyUnit as E;
        match s
            .trim()
            .to_lowercase()
            .replace("_", " ")
            .replace(" ", "")
            .as_str()
        {
            "gallonsdiesel" | "gallonsdieselequivalent" | "gde" => Ok(E::GallonsDieselEquivalent),
            "kilowatthours" | "kilowatthour" | "kwh" => Ok(E::KilowattHours),
            "litersgasoline" => Ok(E::LitersGasolineEquivalent),
            "litersdiesel" => Ok(E::LitersDieselEquivalent),
            "gallonsgasoline" | "gallonsgasolineequivalent" | "gge" => {
                Ok(E::GallonsGasolineEquivalent)
            }
            _ => Err(format!("unknown energy unit '{s}'")),
        }
    }
}

impl TryFrom<String> for EnergyUnit {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}
