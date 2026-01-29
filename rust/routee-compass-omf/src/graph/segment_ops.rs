//! functions mapped onto [TransportationSegmentRecord] rows to create [SegmentSplit] values

use crate::{
    collection::{
        record::{SegmentAccessRestriction, SegmentHeading},
        OvertureMapsCollectionError, SegmentAccessRestrictionWhen, TransportationSegmentRecord,
    },
    graph::{segment_split::SegmentSplit, ConnectorInSegment},
};
use itertools::Itertools;

/// creates simple connector splits from a record.
pub fn process_simple_connector_splits(
    segment: &TransportationSegmentRecord,
    when: Option<&SegmentAccessRestrictionWhen>,
) -> Result<Vec<SegmentSplit>, OvertureMapsCollectionError> {
    let headings = get_headings(segment, when)?;
    let result = segment
        .connectors
        .as_ref()
        .ok_or(OvertureMapsCollectionError::InvalidSegmentConnectors(
            format!("connectors is empty for segment record '{}'", segment.id),
        ))?
        .iter()
        .tuple_windows()
        .flat_map(|(src, dst)| {
            headings.iter().cloned().map(|heading| {
                let src =
                    ConnectorInSegment::new(segment.id.clone(), src.connector_id.clone(), src.at);
                let dst =
                    ConnectorInSegment::new(segment.id.clone(), dst.connector_id.clone(), dst.at);
                SegmentSplit::SimpleConnectorSplit { src, dst, heading }
            })
        })
        .collect::<Vec<SegmentSplit>>();
    Ok(result)
}

/// determines the headings over a segment that are supported. optionally matched to some
/// set of user-provided restrictions.
pub fn get_headings(
    segment: &TransportationSegmentRecord,
    when: Option<&SegmentAccessRestrictionWhen>,
) -> Result<Vec<SegmentHeading>, OvertureMapsCollectionError> {
    // If both when and access_restrictions are None/empty, return both headings
    let access_restrictions = segment.access_restrictions.as_ref();

    let no_restrictions = access_restrictions.map(|r| r.is_empty()).unwrap_or(true);
    if when.is_none() && (access_restrictions.is_none() || no_restrictions) {
        return Ok(vec![SegmentHeading::Forward, SegmentHeading::Backward]);
    }

    // Collect valid headings based on access restrictions
    let mut valid_headings = Vec::new();
    let when_heading = when.and_then(|w| w.heading.clone());
    let (test_fwd, test_bwd) = match when_heading {
        None => (true, true),
        Some(SegmentHeading::Forward) => (true, false),
        Some(SegmentHeading::Backward) => (false, true),
    };

    // Check Forward heading
    if test_fwd && is_heading_valid(SegmentHeading::Forward, when, access_restrictions) {
        valid_headings.push(SegmentHeading::Forward);
    }

    // Check Backward heading
    if test_bwd && is_heading_valid(SegmentHeading::Backward, when, access_restrictions) {
        valid_headings.push(SegmentHeading::Backward);
    }

    Ok(valid_headings)
}

/// Helper function to check if a heading is valid given the when constraint and access restrictions
///
/// Access restrictions are evaluated in order, where:
/// - Multiple restrictions can combine (e.g., "Denied all" + "Allowed specific" = "allowed only for specific")
/// - A restriction applies if its heading and when conditions match the query
/// - The final decision is: allowed if any Allowed restriction applies AND no Denied restriction applies
fn is_heading_valid(
    heading: SegmentHeading,
    when: Option<&SegmentAccessRestrictionWhen>,
    access_restrictions: Option<&Vec<SegmentAccessRestriction>>,
) -> bool {
    use crate::collection::record::SegmentAccessType as SAT;

    let Some(restrictions) = access_restrictions else {
        return true;
    };

    let is_denied = |r: &&SegmentAccessRestriction| r.access_type == SAT::Denied;

    // Partition applicable restrictions by heading-specificity
    let (heading_specific, general): (Vec<_>, Vec<_>) = restrictions
        .iter()
        .filter(|r| restriction_applies_to(r, &heading, when))
        .partition(|r| r.when.as_ref().and_then(|w| w.heading.as_ref()).is_some());

    // Further partition each group by denied/allowed. each of these becomes a set of which
    // emptiness proves a certain fact about the validity of this heading.
    let (heading_denied, heading_allowed): (Vec<_>, Vec<_>) =
        heading_specific.into_iter().partition(is_denied);
    let (general_denied, general_allowed): (Vec<_>, Vec<_>) =
        general.into_iter().partition(is_denied);
    let has_heading_denied = !heading_denied.is_empty();
    let has_heading_allowed = !heading_allowed.is_empty();
    let no_general_denied = general_denied.is_empty();
    let has_general_allowed = !general_allowed.is_empty();

    // Heading-specific denial takes priority - can only be overridden by heading-specific allowance
    if has_heading_denied {
        has_heading_allowed
    } else {
        // No heading-specific denial: allow if any allowance exists, or no denial exists
        has_heading_allowed || has_general_allowed || no_general_denied
    }
}

/// Check if a restriction applies to the given heading and when conditions
///
/// A restriction applies if:
/// 1. The heading matches (or restriction has no heading constraint)
/// 2. The when conditions match:
///    - If querying with when=None: restriction must have empty/minimal conditions (applies broadly)
///    - If querying with when=Some: the query conditions must be compatible with restriction
fn restriction_applies_to(
    restriction: &SegmentAccessRestriction,
    heading: &SegmentHeading,
    when: Option<&SegmentAccessRestrictionWhen>,
) -> bool {
    let restriction_when = restriction.when.as_ref();

    // Check if the restriction's heading matches or is unrestricted
    let heading_matches = restriction_when
        .and_then(|w| w.heading.as_ref())
        .map(|h| h == heading)
        .unwrap_or(true); // If no heading specified in restriction, it applies to all

    if !heading_matches {
        return false;
    }

    // If when is provided, check if the query conditions are compatible with the restriction
    if let Some(when) = when {
        when_is_compatible(when, restriction_when)
    } else {
        // No when constraint provided in query - we only match restrictions that apply
        // broadly (without mode/using/recognized constraints), or have no when clause at all.
        // This represents "what's allowed by default without specific conditions"
        restriction_when.is_none()
            || restriction_when.is_some_and(|rw| {
                // A restriction with specific conditions (mode, using, recognized) doesn't
                // apply to the "default" case
                rw.mode.is_none() && rw.using.is_none() && rw.recognized.is_none()
            })
    }
}

/// Check if the when constraint is compatible with (contained by) the restriction when
///
/// Returns true if the query 'when' is compatible with the restriction 'when'.
/// A restriction with None for a field means it applies broadly (to all values of that field).
/// A restriction with Some([values]) means it only applies to those specific values.
///
/// # Arguments
/// * `when` - Query conditions (e.g., "Car mode")
/// * `segment_restrictions` - Restriction conditions (e.g., "Car and Bicycle modes" or None for all modes)
///
/// Note: Heading compatibility is handled by `restriction_applies_to`, not here.
fn when_is_compatible(
    when: &SegmentAccessRestrictionWhen,
    segment_restrictions: Option<&SegmentAccessRestrictionWhen>,
) -> bool {
    // return early if no restrictions on segment
    let Some(restrictions) = segment_restrictions else {
        return true;
    };

    // compatibility checks
    // in the following blocks, for a given restriction:
    //   - if the restriction is not defined on the segment (None), we continue
    //   - if the restriction IS defined (Some), the "when" query must match it
    // headings are NOT tested here as they have already been tested in get_headings

    // Check mode compatibility
    if let Some(restriction_modes) = &restrictions.mode {
        if let Some(when_modes) = &when.mode {
            if !when_modes.iter().all(|m| restriction_modes.contains(m)) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Check using compatibility
    if let Some(restriction_using) = &restrictions.using {
        if let Some(when_using) = &when.using {
            if !when_using.iter().all(|u| restriction_using.contains(u)) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Check recognized compatibility
    if let Some(restriction_recognized) = &restrictions.recognized {
        if let Some(when_recognized) = &when.recognized {
            if !when_recognized
                .iter()
                .all(|r| restriction_recognized.contains(r))
            {
                return false;
            }
        } else {
            return false;
        }
    }

    // Check vehicle dimension filters
    if let Some(restrictions_vehicle) = &restrictions.vehicle {
        let all_restrictions_apply_to_all_vehicles = restrictions_vehicle.iter().all(|r_vehicle| {
            if let Some(when_vehicles) = &when.vehicle {
                when_vehicles
                    .iter()
                    .all(|w_vehicle| r_vehicle.is_valid(w_vehicle))
            } else {
                false
            }
        });

        if !all_restrictions_apply_to_all_vehicles {
            return false;
        }
    }

    // If we got here, all specified fields in when are compatible
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::{
        filter::{MatchBehavior, TravelModeFilter},
        record::{
            OvertureMapsBbox, SegmentAccessRestrictionWhenVehicle, SegmentAccessType,
            SegmentLengthUnit, SegmentMode, SegmentRecognized, SegmentUnit, SegmentUsing,
            SegmentVehicleComparator, SegmentVehicleDimension,
        },
        SegmentClass,
    };

    #[test]
    fn test_segment_without_access_restrictions_both_headings() {
        // Test: A segment without access restrictions should produce both headings
        let segment = create_test_segment(None);
        let result = get_headings(&segment, None).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&SegmentHeading::Forward));
        assert!(result.contains(&SegmentHeading::Backward));
    }

    #[test]
    fn test_segment_with_empty_access_restrictions_both_headings() {
        // Test: A segment with empty access restrictions should produce both headings
        // when the user passes no constraints.
        let segment = create_test_segment(Some(vec![]));
        let result = get_headings(&segment, None).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&SegmentHeading::Forward));
        assert!(result.contains(&SegmentHeading::Backward));
    }

    #[test]
    fn test_segment_with_forward_only_restriction() {
        // Test: A segment with backward denied should only produce Forward heading
        // when the user passes no constraints.
        let segment = create_test_segment(Some(vec![create_restriction_heading_only(
            SegmentAccessType::Denied,
            SegmentHeading::Backward,
        )]));

        let result = get_headings(&segment, None).unwrap();

        assert_eq!(result, vec![SegmentHeading::Forward]);
    }

    #[test]
    fn test_segment_with_backward_only_restriction() {
        // Test: A segment with forward denied should only produce Backward heading
        // when the user passes no constraints.
        let segment = create_test_segment(Some(vec![create_restriction_heading_only(
            SegmentAccessType::Denied,
            SegmentHeading::Forward,
        )]));

        let result = get_headings(&segment, None).unwrap();

        assert_eq!(result, vec![SegmentHeading::Backward]);
    }

    #[test]
    fn test_segment_with_mode_restriction_matching_when() {
        // Test: Denied all modes for forward, then Allowed for Car/Bicycle
        // Query with Car should allow Forward
        let segment = create_test_segment(Some(create_denied_all_allowed_specific(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car, SegmentMode::Bicycle]),
            None,
            None,
        )));

        let when = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car]),
            None,
            None,
        );
        let result = get_headings(&segment, Some(&when)).unwrap();

        assert_eq!(result, vec![SegmentHeading::Forward]);
    }

    #[test]
    fn test_segment_with_mode_restriction_not_matching_when() {
        // Test: Denied all for forward + Allowed only Car
        // Query with Forward, Bicycle should deny both forward and backward
        let segment = create_test_segment(Some(create_denied_all_allowed_specific(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car]),
            None,
            None,
        )));

        let when = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result = get_headings(&segment, Some(&when)).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_segment_with_multiple_fields_matching() {
        // Test: Denied all + Allowed with multiple field constraints, all matching
        let segment = create_test_segment(Some(create_denied_all_allowed_specific(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car, SegmentMode::Bicycle]),
            Some(vec![SegmentUsing::AsCustomer]),
            Some(vec![SegmentRecognized::AsEmployee]),
        )));

        let when = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car]),
            Some(vec![SegmentUsing::AsCustomer]),
            Some(vec![SegmentRecognized::AsEmployee]),
        );
        let result = get_headings(&segment, Some(&when)).unwrap();

        assert_eq!(result, vec![SegmentHeading::Forward]); // Forward allowed with all conditions met
    }

    #[test]
    fn test_denied_all_then_allowed_specific() {
        // Test: "Denied all" followed by "Allowed for cars" should allow only cars
        // This is the classic "deny by default, allow exceptions" pattern
        let segment = create_test_segment(Some(create_denied_all_allowed_specific(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car]),
            None,
            None,
        )));

        // Query without when - should match the Denied restriction, accept any valid heading
        let result_no_when = get_headings(&segment, None).unwrap();
        assert_eq!(result_no_when, vec![SegmentHeading::Backward]); // Only Backward is valid

        // Query with Car mode - should match the Allowed restriction
        let when_car = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car]),
            None,
            None,
        );
        let result_car = get_headings(&segment, Some(&when_car)).unwrap();
        assert_eq!(result_car, vec![SegmentHeading::Forward]);

        // Query with Bicycle mode - should be denied
        let when_bicycle = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_bicycle = get_headings(&segment, Some(&when_bicycle)).unwrap();
        assert_eq!(result_bicycle.len(), 0);
    }

    #[test]
    fn test_allowed_overrides_denied_same_heading() {
        // Test: When both Denied and Allowed apply to the same conditions,
        // Allowed takes precedence (specific exception pattern)
        let segment = create_test_segment(Some(vec![
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Denied,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(SegmentHeading::Forward),
                    using: None,
                    recognized: None,
                    mode: Some(vec![SegmentMode::Car, SegmentMode::Bicycle]),
                    vehicle: None,
                }),
                vehicle: None,
            },
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Allowed,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(SegmentHeading::Forward),
                    using: None,
                    recognized: None,
                    mode: Some(vec![SegmentMode::Car]),
                    vehicle: None,
                }),
                vehicle: None,
            },
        ]));

        // Car should be allowed (Allowed overrides Denied)
        let when_car = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car]),
            None,
            None,
        );
        let result = get_headings(&segment, Some(&when_car)).unwrap();
        assert_eq!(result, vec![SegmentHeading::Forward]);
    }

    #[test]
    fn test_multiple_denied_restrictions() {
        // Test: Multiple Denied restrictions - all should be respected
        let segment = create_test_segment(Some(vec![
            create_restriction_heading_only(SegmentAccessType::Denied, SegmentHeading::Forward),
            create_restriction_heading_only(SegmentAccessType::Denied, SegmentHeading::Backward),
        ]));

        let result = get_headings(&segment, None).unwrap();

        // Both directions denied
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_designated_treated_as_allowed() {
        // Test: Blanket denial with Designated mode access type: should be treated like Allowed
        let segment = create_test_segment(Some(vec![
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Denied,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(SegmentHeading::Forward),
                    using: None,
                    recognized: None,
                    mode: None,
                    vehicle: None,
                }),
                vehicle: None,
            },
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Designated,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(SegmentHeading::Forward),
                    using: None,
                    recognized: None,
                    mode: Some(vec![SegmentMode::Bicycle]),
                    vehicle: None,
                }),
                vehicle: None,
            },
        ]));

        let when_bicycle = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result = get_headings(&segment, Some(&when_bicycle)).unwrap();

        assert_eq!(result, vec![SegmentHeading::Forward]);
    }

    #[test]
    fn test_restriction_with_mode_does_not_apply_when_query_has_no_mode() {
        // Test: A restriction that specifies a mode constraint should NOT apply
        // when the query doesn't specify a mode at all
        // This exercises the fix: we check if restriction.mode is Some, not if when.mode is Some
        let segment = create_test_segment(Some(vec![
            // Deny all forward traffic (no mode constraint)
            create_restriction_heading_only(SegmentAccessType::Denied, SegmentHeading::Forward),
            // Allow forward for bicycles only (mode constraint)
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Allowed,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(SegmentHeading::Forward),
                    using: None,
                    recognized: None,
                    mode: Some(vec![SegmentMode::Bicycle]),
                    vehicle: None,
                }),
                vehicle: None,
            },
        ]));

        // Query with when=None (no mode specified)
        // The Denied restriction applies (no mode constraint, applies broadly)
        // The Allowed restriction should NOT apply (has mode constraint, but query has none)
        // Expected: Forward is denied because only Denied applies
        let result = get_headings(&segment, None).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains(&SegmentHeading::Backward));
        assert!(!result.contains(&SegmentHeading::Forward));
    }

    #[test]
    fn test_from_data_1() {
        // test case from OMF data, a segment with
        //   - blanket denial for heading backward
        //   - opts in designated HGV travel, no heading specified
        // when
        //   - heading forward with motor_vehicle, car, truck, motorcycle: reject
        //   - heading backward with "": reject
        //   - heading forward with HGV: accept
        let segment = create_test_segment(Some(vec![
            create_restriction_heading_only(SegmentAccessType::Denied, SegmentHeading::Backward),
            create_restriction_mode(SegmentAccessType::Designated, vec![SegmentMode::Hgv]),
        ]));

        let mode = vec![
            SegmentMode::MotorVehicle,
            SegmentMode::Car,
            SegmentMode::Truck,
            SegmentMode::Motorcycle,
        ];

        // forward has no blanket denial, only optional designation, so we accept
        let when1 = create_when(SegmentHeading::Forward, Some(mode.clone()), None, None);
        let result1 = get_headings(&segment, Some(&when1)).unwrap();
        assert_eq!(result1.len(), 1);

        // blanket backward denial on a backward-oriented when query -> empty result
        let when2 = create_when(SegmentHeading::Backward, Some(mode.clone()), None, None);
        let result2 = get_headings(&segment, Some(&when2)).unwrap();
        assert_eq!(result2.len(), 0);
    }

    #[test]
    fn test_from_data_2() {
        // a segment has a blanket denial of backward access, and a designation for bicycle.
        // this should allow any mode traveling forward but no mode traveling backward.

        let segment = create_test_segment(Some(vec![
            create_restriction_heading_only(SegmentAccessType::Denied, SegmentHeading::Backward),
            create_restriction_mode(SegmentAccessType::Designated, vec![SegmentMode::Bicycle]),
        ]));

        // case 1: forward traversal should be accepted
        let when1 = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result1 = get_headings(&segment, Some(&when1)).unwrap();
        assert_eq!(result1, vec![SegmentHeading::Forward]);

        // case 2: backward traversal should be denied
        let when2 = create_when(
            SegmentHeading::Backward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result2 = get_headings(&segment, Some(&when2)).unwrap();

        assert_eq!(result2.len(), 0);
    }

    #[test]
    fn test_from_data_3() {
        let access_restrictions = vec![
            create_restriction_mode(SegmentAccessType::Designated, vec![SegmentMode::Hgv]),
            create_restriction_when_vehicle(
                SegmentAccessType::Denied,
                SegmentAccessRestrictionWhenVehicle {
                    dimension: SegmentVehicleDimension::Height,
                    comparison: SegmentVehicleComparator::GreaterThan,
                    value: 13.0,
                    unit: Some(SegmentUnit::Length(SegmentLengthUnit::Feet)),
                },
            ),
        ];

        let segment = create_test_segment(Some(access_restrictions));

        // Should pass simple drive filter
        let filter1 = create_simple_drive_filter();
        assert!(filter1.matches_filter(&segment));

        // Should be denied when vehicle height is greater than 13 ft
        let when1 = SegmentAccessRestrictionWhen {
            during: None,
            heading: Some(SegmentHeading::Forward),
            using: None,
            recognized: None,
            mode: None,
            vehicle: Some(vec![SegmentAccessRestrictionWhenVehicle {
                dimension: SegmentVehicleDimension::Height,
                comparison: SegmentVehicleComparator::GreaterThan,
                value: 100.0,
                unit: Some(SegmentUnit::Length(SegmentLengthUnit::Feet)),
            }]),
        };
        let result1 = get_headings(&segment, Some(&when1)).unwrap();
        assert_eq!(result1.len(), 0);

        // But should be allowed in any other case
        let when2 = SegmentAccessRestrictionWhen {
            during: None,
            heading: Some(SegmentHeading::Forward),
            using: None,
            recognized: None,
            mode: None,
            vehicle: Some(vec![SegmentAccessRestrictionWhenVehicle {
                dimension: SegmentVehicleDimension::Height,
                comparison: SegmentVehicleComparator::GreaterThan,
                value: 10.,
                unit: Some(SegmentUnit::Length(SegmentLengthUnit::Feet)),
            }]),
        };
        let result2 = get_headings(&segment, Some(&when2)).unwrap();
        assert_eq!(result2, vec![SegmentHeading::Forward]);

        // also when no vehicle is specified
        let when3 = SegmentAccessRestrictionWhen {
            during: None,
            heading: Some(SegmentHeading::Forward),
            using: None,
            recognized: None,
            mode: None,
            vehicle: None,
        };
        let result3 = get_headings(&segment, Some(&when3)).unwrap();
        assert_eq!(result3, vec![SegmentHeading::Forward]);
    }

    // Helper to represent a typical drive mode filter
    fn create_simple_drive_filter() -> TravelModeFilter {
        let classes = vec![
            SegmentClass::Motorway,
            SegmentClass::Trunk,
            SegmentClass::Primary,
            SegmentClass::Secondary,
            SegmentClass::Tertiary,
            SegmentClass::Residential,
            SegmentClass::LivingStreet,
            SegmentClass::Unclassified,
            SegmentClass::Service,
            SegmentClass::Unknown,
        ]
        .into_iter()
        .collect();

        TravelModeFilter::MatchesClasses {
            classes,
            behavior: MatchBehavior::Include,
            allow_unset: true,
        }
    }

    /// Helper to create a minimal segment for testing
    fn create_test_segment(
        access_restrictions: Option<Vec<SegmentAccessRestriction>>,
    ) -> TransportationSegmentRecord {
        // Create a minimal valid bbox using serde deserialization
        let bbox: OvertureMapsBbox =
            serde_json::from_str(r#"{"xmin": 0.0, "xmax": 1.0, "ymin": 0.0, "ymax": 1.0}"#)
                .expect("test invariant failed, unable to mock bbox of record");
        let mut record = TransportationSegmentRecord::default();
        record.access_restrictions = access_restrictions;
        record.bbox = bbox;
        record
    }

    /// Helper to create a simple access restriction with only heading constraint
    fn create_restriction_heading_only(
        access_type: SegmentAccessType,
        heading: SegmentHeading,
    ) -> SegmentAccessRestriction {
        SegmentAccessRestriction {
            access_type,
            when: Some(SegmentAccessRestrictionWhen {
                during: None,
                heading: Some(heading),
                using: None,
                recognized: None,
                mode: None,
                vehicle: None,
            }),
            vehicle: None,
        }
    }

    /// Helper to create a simple access restriction with only mode constraint
    fn create_restriction_mode(
        access_type: SegmentAccessType,
        modes: Vec<SegmentMode>,
    ) -> SegmentAccessRestriction {
        SegmentAccessRestriction {
            access_type,
            when: Some(SegmentAccessRestrictionWhen {
                during: None,
                heading: None,
                using: None,
                recognized: None,
                mode: Some(modes),
                vehicle: None,
            }),
            vehicle: None,
        }
    }

    /// Helper to create a simple access restriction with heading + modes constraint
    fn create_restriction_heading_mode(
        access_type: SegmentAccessType,
        heading: SegmentHeading,
        modes: Vec<SegmentMode>,
    ) -> SegmentAccessRestriction {
        SegmentAccessRestriction {
            access_type,
            when: Some(SegmentAccessRestrictionWhen {
                during: None,
                heading: Some(heading),
                using: None,
                recognized: None,
                mode: Some(modes),
                vehicle: None,
            }),
            vehicle: None,
        }
    }

    /// Helper to create a simple access restriction with vehicle detail
    fn create_restriction_when_vehicle(
        access_type: SegmentAccessType,
        vehicle: SegmentAccessRestrictionWhenVehicle,
    ) -> SegmentAccessRestriction {
        SegmentAccessRestriction {
            access_type,
            when: Some(SegmentAccessRestrictionWhen {
                during: None,
                heading: None,
                using: None,
                recognized: None,
                mode: None,
                vehicle: Some(vec![vehicle]),
            }),
            vehicle: None,
        }
    }

    /// Helper to create "Denied all + Allowed specific" pattern for a heading
    fn create_denied_all_allowed_specific(
        heading: SegmentHeading,
        allowed_modes: Option<Vec<SegmentMode>>,
        allowed_using: Option<Vec<SegmentUsing>>,
        allowed_recognized: Option<Vec<SegmentRecognized>>,
    ) -> Vec<SegmentAccessRestriction> {
        vec![
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Denied,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(heading.clone()),
                    using: None,
                    recognized: None,
                    mode: None,
                    vehicle: None,
                }),
                vehicle: None,
            },
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Allowed,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(heading),
                    using: allowed_using,
                    recognized: allowed_recognized,
                    mode: allowed_modes,
                    vehicle: None,
                }),
                vehicle: None,
            },
        ]
    }

    /// Helper to create a query when object
    fn create_when(
        heading: SegmentHeading,
        mode: Option<Vec<SegmentMode>>,
        using: Option<Vec<SegmentUsing>>,
        recognized: Option<Vec<SegmentRecognized>>,
    ) -> SegmentAccessRestrictionWhen {
        SegmentAccessRestrictionWhen {
            during: None,
            heading: Some(heading),
            using,
            recognized,
            mode,
            vehicle: None,
        }
    }

    #[test]
    fn test_general_denial_blocks_both_directions() {
        // Test: A denial with no heading specified should block both directions
        let segment = create_test_segment(Some(vec![SegmentAccessRestriction {
            access_type: SegmentAccessType::Denied,
            when: Some(SegmentAccessRestrictionWhen {
                during: None,
                heading: None, // No heading = applies to all
                using: None,
                recognized: None,
                mode: None,
                vehicle: None,
            }),
            vehicle: None,
        }]));

        let result = get_headings(&segment, None).unwrap();
        assert_eq!(
            result.len(),
            0,
            "General denial should block both directions"
        );
    }

    #[test]
    fn test_general_allowance_overrides_general_denial() {
        // Test: A general allowance (no heading) should override a general denial (no heading)
        // for a specific mode
        let segment = create_test_segment(Some(vec![
            // General denial for all
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Denied,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: None,
                    using: None,
                    recognized: None,
                    mode: None,
                    vehicle: None,
                }),
                vehicle: None,
            },
            // General allowance for bicycles (no heading specified)
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Allowed,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: None,
                    using: None,
                    recognized: None,
                    mode: Some(vec![SegmentMode::Bicycle]),
                    vehicle: None,
                }),
                vehicle: None,
            },
        ]));

        // Query with bicycle mode - should be allowed in both directions
        let when_fwd = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_fwd = get_headings(&segment, Some(&when_fwd)).unwrap();
        assert_eq!(result_fwd, vec![SegmentHeading::Forward]);

        let when_bwd = create_when(
            SegmentHeading::Backward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_bwd = get_headings(&segment, Some(&when_bwd)).unwrap();
        assert_eq!(result_bwd, vec![SegmentHeading::Backward]);
    }

    #[test]
    fn test_heading_specific_allowance_without_denial() {
        // Test: A heading-specific allowance without any denial should allow that heading
        // (and the other heading should default to allowed since no denial)
        let segment = create_test_segment(Some(vec![SegmentAccessRestriction {
            access_type: SegmentAccessType::Allowed,
            when: Some(SegmentAccessRestrictionWhen {
                during: None,
                heading: Some(SegmentHeading::Forward),
                using: None,
                recognized: None,
                mode: Some(vec![SegmentMode::Bicycle]),
                vehicle: None,
            }),
            vehicle: None,
        }]));

        // Query with bicycle forward - should be allowed (explicit allowance)
        let when_fwd = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_fwd = get_headings(&segment, Some(&when_fwd)).unwrap();
        assert_eq!(result_fwd, vec![SegmentHeading::Forward]);

        // Query with bicycle backward - should also be allowed (no denial, defaults to allowed)
        let when_bwd = create_when(
            SegmentHeading::Backward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_bwd = get_headings(&segment, Some(&when_bwd)).unwrap();
        assert_eq!(result_bwd, vec![SegmentHeading::Backward]);
    }

    #[test]
    fn test_restrictions_exist_but_none_apply() {
        // Test: Restrictions exist but none apply to the query (different mode)
        // Should default to allowed
        let segment = create_test_segment(Some(vec![create_restriction_heading_mode(
            SegmentAccessType::Denied,
            SegmentHeading::Forward,
            vec![SegmentMode::Car],
        )]));

        // Query with Bicycle - the Car denial shouldn't apply
        let when_bicycle = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result = get_headings(&segment, Some(&when_bicycle)).unwrap();
        assert_eq!(
            result,
            vec![SegmentHeading::Forward],
            "Denial for Car shouldn't affect Bicycle"
        );
    }

    #[test]
    fn test_heading_specific_allowance_overrides_general_denial() {
        // Test: A heading-specific allowance should override a general (non-heading) denial
        let segment = create_test_segment(Some(vec![
            // General denial (no heading)
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Denied,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: None,
                    using: None,
                    recognized: None,
                    mode: None,
                    vehicle: None,
                }),
                vehicle: None,
            },
            // Heading-specific allowance for forward + bicycle
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Allowed,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: Some(SegmentHeading::Forward),
                    using: None,
                    recognized: None,
                    mode: Some(vec![SegmentMode::Bicycle]),
                    vehicle: None,
                }),
                vehicle: None,
            },
        ]));

        // Forward bicycle should be allowed (heading-specific allowance)
        let when_fwd = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_fwd = get_headings(&segment, Some(&when_fwd)).unwrap();
        assert_eq!(result_fwd, vec![SegmentHeading::Forward]);

        // Backward bicycle should be denied (general denial, no allowance)
        let when_bwd = create_when(
            SegmentHeading::Backward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_bwd = get_headings(&segment, Some(&when_bwd)).unwrap();
        assert_eq!(result_bwd.len(), 0);
    }

    #[test]
    fn test_general_denial_with_heading_specific_denial_same_heading() {
        // Test: Both general denial and heading-specific denial for same heading
        // The heading-specific denial takes priority, requires heading-specific allowance
        let segment = create_test_segment(Some(vec![
            // General denial
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Denied,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: None,
                    using: None,
                    recognized: None,
                    mode: None,
                    vehicle: None,
                }),
                vehicle: None,
            },
            // Heading-specific denial for forward
            create_restriction_heading_only(SegmentAccessType::Denied, SegmentHeading::Forward),
            // General allowance for bicycle (should NOT override heading-specific denial)
            SegmentAccessRestriction {
                access_type: SegmentAccessType::Allowed,
                when: Some(SegmentAccessRestrictionWhen {
                    during: None,
                    heading: None,
                    using: None,
                    recognized: None,
                    mode: Some(vec![SegmentMode::Bicycle]),
                    vehicle: None,
                }),
                vehicle: None,
            },
        ]));

        // Forward bicycle - general allowance should NOT override heading-specific denial
        let when_fwd = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_fwd = get_headings(&segment, Some(&when_fwd)).unwrap();
        assert_eq!(
            result_fwd.len(),
            0,
            "General allowance should not override heading-specific denial"
        );

        // Backward bicycle - only general denial, general allowance should override
        let when_bwd = create_when(
            SegmentHeading::Backward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_bwd = get_headings(&segment, Some(&when_bwd)).unwrap();
        assert_eq!(result_bwd, vec![SegmentHeading::Backward]);
    }
}
