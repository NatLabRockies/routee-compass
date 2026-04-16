use std::{fmt::Display, str::FromStr};

use super::{ComparisonOperation, RestrictionRow, VehicleParameter, VehicleParameterType};
use crate::model::{
    constraint::ConstraintModelError,
    unit::{DistanceUnit, WeightUnit},
};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct VehicleRestriction {
    pub restriction_parameter: VehicleParameter,
    pub comparison_operation: ComparisonOperation,
}

impl VehicleRestriction {
    pub fn new(
        restriction_parameter: VehicleParameter,
        comparison_operation: ComparisonOperation,
    ) -> Self {
        VehicleRestriction {
            restriction_parameter,
            comparison_operation,
        }
    }

    pub fn vehicle_parameter_type(&self) -> &VehicleParameterType {
        self.restriction_parameter.vehicle_parameter_type()
    }

    /// compares this restriction against some query-time vehicle parameter using
    /// the restriction's comparison operator
    pub fn within_restriction(&self, query_parameter: &VehicleParameter) -> bool {
        self.comparison_operation
            .compare_parameters(query_parameter, &self.restriction_parameter)
    }
}

impl TryFrom<&RestrictionRow> for VehicleRestriction {
    type Error = ConstraintModelError;

    fn try_from(row: &RestrictionRow) -> Result<Self, Self::Error> {
        use VehicleParameterType as VPT;
        let vehicle_parameter = match row.name {
            VPT::Height => VehicleParameter::Height {
                value: DistanceUnit::from_str(&row.unit)
                    .map_err(|e| {
                        ConstraintModelError::BuildError(format!(
                            "Unable to parse height unit {}: {}",
                            row.unit, e
                        ))
                    })?
                    .to_uom(row.value),
            },
            VPT::Width => VehicleParameter::Width {
                value: DistanceUnit::from_str(&row.unit)
                    .map_err(|e| {
                        ConstraintModelError::BuildError(format!(
                            "Unable to parse height unit {}: {}",
                            row.unit, e
                        ))
                    })?
                    .to_uom(row.value),
            },
            VPT::TotalLength => VehicleParameter::TotalLength {
                value: DistanceUnit::from_str(&row.unit)
                    .map_err(|e| {
                        ConstraintModelError::BuildError(format!(
                            "Unable to parse height unit {}: {}",
                            row.unit, e
                        ))
                    })?
                    .to_uom(row.value),
            },
            VPT::TrailerLength => VehicleParameter::TrailerLength {
                value: DistanceUnit::from_str(&row.unit)
                    .map_err(|e| {
                        ConstraintModelError::BuildError(format!(
                            "Unable to parse height unit {}: {}",
                            row.unit, e
                        ))
                    })?
                    .to_uom(row.value),
            },
            VPT::TotalWeight => VehicleParameter::TotalWeight {
                value: WeightUnit::from_str(&row.unit)
                    .map_err(|e| {
                        ConstraintModelError::BuildError(format!(
                            "Unable to parse weight unit {}: {}",
                            row.unit, e
                        ))
                    })?
                    .to_uom(row.value),
            },
            VPT::WeightPerAxle => VehicleParameter::WeightPerAxle {
                value: WeightUnit::from_str(&row.unit)
                    .map_err(|e| {
                        ConstraintModelError::BuildError(format!(
                            "Unable to parse weight unit {}: {}",
                            row.unit, e
                        ))
                    })?
                    .to_uom(row.value),
            },
        };
        let comparison_operation = row.operation.clone();
        Ok(VehicleRestriction {
            restriction_parameter: vehicle_parameter,
            comparison_operation,
        })
    }
}

impl Display for VehicleRestriction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "query parameter is {} link restrictions matching {}",
            self.comparison_operation,
            self.restriction_parameter.vehicle_parameter_type()
        )
    }
}
