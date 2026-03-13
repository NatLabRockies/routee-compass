use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::collection::OvertureMapsCollectionError;

/// This function takes a [`Vec<f64>`] and returns `a` and `b` if and only
/// if the vector has exactly two elements and the second one is higher than the
/// first one. Otherwise it returns an error.
pub(super) fn validate_between_vector(
    b_vector: &Vec<f64>,
) -> Result<(&f64, &f64), OvertureMapsCollectionError> {
    let [low, high] = b_vector.as_slice() else {
        return Err(OvertureMapsCollectionError::InvalidBetweenVector(
            "Between vector has length != 2".to_string(),
        ));
    };

    if high < low {
        return Err(OvertureMapsCollectionError::InvalidBetweenVector(format!(
            "`high` is lower than `low`: [{low}, {high}]"
        )));
    }

    Ok((low, high))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentValueBetween<T> {
    #[serde(skip_serializing_if = "Option::is_none", default = "default_none")]
    pub value: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub between: Option<Vec<f64>>,
}

fn default_none<T>() -> Option<T> {
    None
}

impl<T: Debug> SegmentValueBetween<T> {
    /// Used to filter limits based on a linear reference segment.
    /// Returns `true` if the open interval `(between[0], between[1])`
    /// overlaps with the open interval `(start, end)`.
    ///
    /// # Examples
    ///
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
    /// // (20, 30) does not overlap with open interval (10, 20)
    /// assert!(!limit.check_open_intersection(20.0, 30.0).unwrap());
    /// ```
    pub fn check_open_intersection(
        &self,
        start: f64,
        end: f64,
    ) -> Result<bool, OvertureMapsCollectionError> {
        let b_vector =
            self.between
                .as_ref()
                .ok_or(OvertureMapsCollectionError::InvalidBetweenVector(format!(
                    "`between` vector is empty: {self:?}"
                )))?;
        let (low, high) = validate_between_vector(b_vector)?;

        Ok(start < *high && end > *low)
    }
}
