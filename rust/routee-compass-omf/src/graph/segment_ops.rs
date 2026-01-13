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

pub fn get_headings(
    segment: &TransportationSegmentRecord,
    when: Option<&SegmentAccessRestrictionWhen>,
) -> Result<Vec<SegmentHeading>, OvertureMapsCollectionError> {
    // If both when and access_restrictions are None/empty, return both headings
    let access_restrictions = segment.access_restrictions.as_ref();

    if when.is_none() && (access_restrictions.is_none() || access_restrictions.unwrap().is_empty())
    {
        return Ok(vec![SegmentHeading::Forward, SegmentHeading::Backward]);
    }

    // Collect valid headings based on access restrictions
    let mut valid_headings = Vec::new();

    // Check Forward heading
    if is_heading_valid(SegmentHeading::Forward, when, access_restrictions) {
        valid_headings.push(SegmentHeading::Forward);
    }

    // Check Backward heading
    if is_heading_valid(SegmentHeading::Backward, when, access_restrictions) {
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
    use crate::collection::record::SegmentAccessType;

    // If no access restrictions, the heading is valid
    let Some(restrictions) = access_restrictions else {
        return true;
    };

    if restrictions.is_empty() {
        return true;
    }

    // Collect applicable restrictions that match the heading and when conditions
    let mut has_allowed = false;
    let mut has_denied = false;

    for restriction in restrictions {
        if restriction_applies_to(restriction, &heading, when) {
            match restriction.access_type {
                SegmentAccessType::Allowed | SegmentAccessType::Designated => {
                    has_allowed = true;
                }
                SegmentAccessType::Denied => {
                    has_denied = true;
                }
            }
        }
    }

    // Decision logic:
    // - If no restrictions apply, default to allowed
    // - If any Denied applies, not allowed (unless overridden by Allowed)
    // - If any Allowed applies, allowed
    // The combination "Denied + Allowed" means the Allowed takes precedence (specific exception)
    if !has_allowed && !has_denied {
        // No applicable restrictions for this heading/when combination
        true
    } else if has_allowed {
        // At least one Allowed restriction applies - this is an explicit permission
        true
    } else {
        // Only Denied restrictions apply
        false
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
            || restriction_when.map_or(false, |rw| {
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
fn when_is_compatible(
    when: &SegmentAccessRestrictionWhen,
    segment_restrictions: Option<&SegmentAccessRestrictionWhen>,
) -> bool {
    // return early if no restrictions on segment
    let Some(restrictions) = segment_restrictions else {
        return true;
    };

    // Check heading compatibility
    if let Some(when_heading) = &when.heading {
        if let Some(restriction_heading) = &restrictions.heading {
            if restriction_heading != when_heading {
                return false;
            }
        }
        // If restriction has no heading, it applies to all headings - compatible
    }

    // compatibility checks
    // in the following blocks, for a given restriction:
    //   - if the restriction is not defined on the segment (None), we continue
    //   - if the restriction IS defined (Some), the "when" query must match it

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

    // If we got here, all specified fields in when are compatible
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::record::{
        OvertureMapsBbox, SegmentAccessType, SegmentMode, SegmentRecognized, SegmentUsing,
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
        let segment = create_test_segment(Some(vec![]));
        let result = get_headings(&segment, None).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&SegmentHeading::Forward));
        assert!(result.contains(&SegmentHeading::Backward));
    }

    #[test]
    fn test_segment_with_forward_only_restriction() {
        // Test: A segment with backward denied should only produce Forward heading
        let segment = create_test_segment(Some(vec![create_restriction_heading_only(
            SegmentAccessType::Denied,
            SegmentHeading::Backward,
        )]));

        let result = get_headings(&segment, None).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SegmentHeading::Forward);
    }

    #[test]
    fn test_segment_with_backward_only_restriction() {
        // Test: A segment with forward denied should only produce Backward heading
        let segment = create_test_segment(Some(vec![create_restriction_heading_only(
            SegmentAccessType::Denied,
            SegmentHeading::Forward,
        )]));

        let result = get_headings(&segment, None).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], SegmentHeading::Backward);
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

        assert_eq!(result.len(), 2); // Forward allowed for Car, Backward unrestricted
        assert!(result.contains(&SegmentHeading::Forward));
        assert!(result.contains(&SegmentHeading::Backward));
    }

    #[test]
    fn test_segment_with_mode_restriction_not_matching_when() {
        // Test: Denied all for forward + Allowed only Car
        // Query with Bicycle should deny Forward
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

        assert_eq!(result.len(), 1); // Only Backward valid
        assert!(result.contains(&SegmentHeading::Backward));
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

        assert_eq!(result.len(), 2); // Forward allowed with all conditions met, Backward unrestricted
        assert!(result.contains(&SegmentHeading::Forward));
        assert!(result.contains(&SegmentHeading::Backward));
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

        // Query without when - should match the Denied restriction
        let result_no_when = get_headings(&segment, None).unwrap();
        assert_eq!(result_no_when.len(), 1); // Only Backward is valid
        assert!(result_no_when.contains(&SegmentHeading::Backward));

        // Query with Car mode - should match the Allowed restriction
        let when_car = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Car]),
            None,
            None,
        );
        let result_car = get_headings(&segment, Some(&when_car)).unwrap();
        assert_eq!(result_car.len(), 2); // Forward allowed for cars, Backward has no restrictions
        assert!(result_car.contains(&SegmentHeading::Forward));
        assert!(result_car.contains(&SegmentHeading::Backward));

        // Query with Bicycle mode - should be denied
        let when_bicycle = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result_bicycle = get_headings(&segment, Some(&when_bicycle)).unwrap();
        assert_eq!(result_bicycle.len(), 1); // Only Backward
        assert!(result_bicycle.contains(&SegmentHeading::Backward));
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
        assert_eq!(result.len(), 2);
        assert!(result.contains(&SegmentHeading::Forward));
        assert!(result.contains(&SegmentHeading::Backward));
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
        // Test: Designated access type should be treated like Allowed
        let segment = create_test_segment(Some(vec![SegmentAccessRestriction {
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
        }]));

        let when_bicycle = create_when(
            SegmentHeading::Forward,
            Some(vec![SegmentMode::Bicycle]),
            None,
            None,
        );
        let result = get_headings(&segment, Some(&when_bicycle)).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&SegmentHeading::Forward));
        assert!(result.contains(&SegmentHeading::Backward));
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
}
