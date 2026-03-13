use routee_compass_core::model::unit::SpeedUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::Velocity;

use crate::collection::OvertureMapsCollectionError;

use super::{
    access_restriction_when::SegmentAccessRestrictionWhen, value_between::validate_between_vector,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentSpeedUnit {
    #[serde(rename = "km/h")]
    Kmh,
    #[serde(rename = "mph")]
    Mph,
}

impl SegmentSpeedUnit {
    pub fn to_uom(&self, value: f64) -> Velocity {
        match self {
            SegmentSpeedUnit::Kmh => SpeedUnit::KPH.to_uom(value),
            SegmentSpeedUnit::Mph => SpeedUnit::MPH.to_uom(value),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpeedLimitWithUnit {
    pub value: i32,
    pub unit: SegmentSpeedUnit,
}

impl SpeedLimitWithUnit {
    pub fn to_uom_value(&self) -> Velocity {
        self.unit.to_uom(self.value as f64)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentSpeedLimit {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub min_speed: Option<SpeedLimitWithUnit>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_speed: Option<SpeedLimitWithUnit>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub is_max_speed_variable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub when: Option<SegmentAccessRestrictionWhen>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub between: Option<Vec<f64>>,
}

impl SegmentSpeedLimit {
    /// Used to filter limits based on a linear reference segment.
    /// Returns `true` if the open interval `(between[0], between[1])`
    /// overlaps with the open interval `(start, end)`.
    ///
    /// # Examples
    ///
    /// Basic overlap:
    /// ```
    /// # use bambam_omf::collection::SegmentSpeedLimit;
    ///
    /// let limit = SegmentSpeedLimit {
    ///     min_speed: None,
    ///     max_speed: None,
    ///     is_max_speed_variable: None,
    ///     when: None,
    ///     between: Some(vec![10.0, 20.0]),
    /// };
    ///
    /// // (15, 25) overlaps with (10, 20)
    /// assert!(limit.check_open_intersection(15.0, 25.0).unwrap());
    /// ```
    ///
    /// No overlap:
    /// ```
    /// # use bambam_omf::collection::SegmentSpeedLimit;
    /// # let limit = SegmentSpeedLimit {
    /// #    min_speed: None,
    /// #    max_speed: None,
    /// #    is_max_speed_variable: None,
    /// #    when: None,
    /// #    between: Some(vec![10.0, 20.0]),
    /// # };
    ///
    /// // (20, 30) does not overlap with open interval (10, 20)
    /// assert!(!limit.check_open_intersection(20.0, 30.0).unwrap());
    /// ```
    ///
    /// No `between` restriction means always applicable:
    /// ```
    /// # use bambam_omf::collection::SegmentSpeedLimit;
    /// let limit = SegmentSpeedLimit {
    ///     min_speed: None,
    ///     max_speed: None,
    ///     is_max_speed_variable: None,
    ///     when: None,
    ///     between: None,
    /// };
    ///
    /// assert!(limit.check_open_intersection(100.0, 200.0).unwrap());
    /// ```
    pub fn check_open_intersection(
        &self,
        start: f64,
        end: f64,
    ) -> Result<bool, OvertureMapsCollectionError> {
        match self.between.as_ref() {
            Some(b_vector) => {
                let (low, high) = validate_between_vector(b_vector)?;
                Ok(start < *high && end > *low)
            }
            None => Ok(true),
        }
    }

    pub fn get_max_speed(&self) -> Option<SpeedLimitWithUnit> {
        self.max_speed.clone()
    }

    /// given a sub-segment linear reference (start, end), compute the total overlapping portion
    pub fn get_linear_reference_portion(
        &self,
        start: f64,
        end: f64,
    ) -> Result<f64, OvertureMapsCollectionError> {
        match self.between.as_ref() {
            Some(b_vector) => {
                let (low, high) = validate_between_vector(b_vector)?;

                Ok((high.min(end) - low.max(start)).max(0.))
            }
            None => Ok(end - start),
        }
    }
}
