use serde::{Deserialize, Serialize};

use crate::collection::record::during_expression::DuringExpression;

use super::{
    mode::{SegmentHeading, SegmentMode, SegmentRecognized, SegmentUsing},
    vehicle::{SegmentUnit, SegmentVehicleComparator, SegmentVehicleDimension},
};

fn default_none<T>() -> Option<T> {
    None
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SegmentAccessRestrictionWhen {
    /// Time span or time spans during which something is open or active, specified
    /// in the OSM opening hours specification:
    /// see <https://wiki.openstreetmap.org/wiki/Key:opening_hours/specification>
    #[serde(skip_serializing_if = "Option::is_none", default = "default_none")]
    pub during: Option<DuringExpression>,
    /// Enumerates possible travel headings along segment geometry.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub heading: Option<SegmentHeading>,
    /// Reason why a person or entity travelling on the transportation network is
    /// using a particular location.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub using: Option<Vec<SegmentUsing>>,
    /// Status of the person or entity travelling as recognized by authorities
    /// controlling the particular location
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub recognized: Option<Vec<SegmentRecognized>>,
    /// Enumerates possible travel modes. Some modes represent groups of modes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mode: Option<Vec<SegmentMode>>,
    /// Vehicle attributes for which the rule applies
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vehicle: Option<Vec<SegmentAccessRestrictionWhenVehicle>>,
}

impl SegmentAccessRestrictionWhen {
    pub fn contains_mode(&self, mode: &SegmentMode) -> bool {
        self.mode
            .as_ref()
            .map(|m| m.contains(mode))
            .unwrap_or_default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentAccessRestrictionWhenVehicle {
    pub dimension: SegmentVehicleDimension,
    pub comparison: SegmentVehicleComparator,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unit: Option<SegmentUnit>,
}

impl SegmentAccessRestrictionWhenVehicle {
    /// returns true if the when provided would pass the restriction
    /// based on the comparison logic
    pub fn is_valid(&self, when: &SegmentAccessRestrictionWhenVehicle) -> bool {
        use SegmentUnit as SU;

        if when.dimension != self.dimension {
            return false;
        }

        match (&self.unit, &when.unit) {
            (Some(SU::Length(this_unit)), Some(SU::Length(other_unit))) => {
                let this_value_f64 = this_unit.to_uom(self.value).get::<uom::si::length::meter>();
                let other_value_f64 = other_unit
                    .to_uom(when.value)
                    .get::<uom::si::length::meter>();
                self.comparison.apply(other_value_f64, this_value_f64)
            }
            (Some(SU::Weight(this_unit)), Some(SU::Weight(other_unit))) => {
                let this_value_f64 = this_unit
                    .to_uom(self.value)
                    .get::<uom::si::mass::kilogram>();
                let other_value_f64 = other_unit
                    .to_uom(when.value)
                    .get::<uom::si::mass::kilogram>();
                self.comparison.apply(other_value_f64, this_value_f64)
            }

            // Should be handled by the if statement checking the dimension but
            // just to be sure
            (Some(SU::Weight(_)), Some(SU::Length(_))) => false,
            (Some(SU::Length(_)), Some(SU::Weight(_))) => false,

            // If we miss any unit, check the raw values
            _ => self.comparison.apply(when.value, self.value),
        }
    }
}
