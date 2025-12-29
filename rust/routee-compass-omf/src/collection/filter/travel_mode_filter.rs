use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    collection::{
        record::{SegmentAccessType, SegmentClass, SegmentMode, SegmentSubclass, SegmentSubtype},
        TransportationSegmentRecord,
    },
    graph::SegmentSplit,
};

/// configures a predicate for testing whether a Segment belongs to a specific travel mode
/// [{ type = "subtype", value = "road"}]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum TravelModeFilter {
    /// filter a row based on its subtype. fails if not a match or value is not set.
    #[serde(rename = "subtype")]
    MatchesSubtype(SegmentSubtype),
    /// filter a row based on a class. fails if not a match, and optionally, if 'class'
    /// is unset on the row data.
    #[serde(rename = "class")]
    MatchesClasses {
        classes: HashSet<SegmentClass>,
        ignore_unset: bool,
    },
    /// filter a row based on a class with additional subclass(es). fails if not a match,
    /// and optionally, if 'class' or 'subclass' are unset.
    #[serde(rename = "class_with_subclasses")]
    MatchesClassesWithSubclasses {
        classes: HashMap<SegmentClass, Vec<SegmentSubclass>>,
        ignore_unset: bool,
    },

    /// filter a row based on the [SegmentMode]. if there is no blanket access
    /// restriction or the the [SegmentAccessType] is not "Denied" then accept the row,
    /// or, optionally, only when explicitly matching "Allowed" or "Designated".
    MatchesModeAccess {
        mode: SegmentMode,
        must_allow: Option<bool>,
        must_designate: Option<bool>,
    },

    Combined(Vec<Box<TravelModeFilter>>),
}

impl TravelModeFilter {
    /// test whether a given row and split combination match a travel mode filter.
    /// returns false if there is no match.
    pub fn matches_filter(
        &self,
        segment: &TransportationSegmentRecord,
        split: &SegmentSplit,
    ) -> bool {
        match self {
            TravelModeFilter::MatchesSubtype(subtype) => segment
                .subtype
                .as_ref()
                .map(|s| s == subtype)
                .unwrap_or_default(),

            TravelModeFilter::MatchesClasses {
                classes,
                ignore_unset: ignore_missing,
            } => segment
                .class
                .as_ref()
                .map(|c| classes.contains(c))
                .unwrap_or(*ignore_missing),

            TravelModeFilter::MatchesClassesWithSubclasses {
                classes,
                ignore_unset: ignore_missing,
            } => match (segment.class.as_ref(), segment.subclass.as_ref()) {
                (Some(cl), None) => classes.contains_key(cl),
                (Some(cl), Some(sc)) => match classes.get(cl) {
                    None => *ignore_missing,
                    Some(subclasses) => subclasses.contains(sc),
                },
                _ => *ignore_missing,
            },

            TravelModeFilter::MatchesModeAccess {
                mode,
                must_allow,
                must_designate,
            } => {
                let restrictions = segment
                    .access_restrictions
                    .as_ref()
                    .map(|rs| rs.iter())
                    .unwrap_or_default();

                for restriction in restrictions.into_iter() {
                    match (&restriction.access_type, must_allow, must_designate) {
                        // Confirm our mode is allowed when 'must_allow' is true
                        (SegmentAccessType::Allowed, Some(true), _) => {
                            if !restriction.contains_mode(mode) {
                                return false;
                            }
                        }
                        // Confirm our mode is designated when 'must_designate' is true
                        (SegmentAccessType::Designated, _, Some(true)) => {
                            if !restriction.contains_mode(mode) {
                                return false;
                            }
                        }
                        // Confirm our mode is NOT denied
                        (SegmentAccessType::Denied, _, _) => todo!(),
                        _ => {
                            if restriction.contains_mode(mode) {
                                return false;
                            }
                        }
                    }
                }
                true
            }
            TravelModeFilter::Combined(travel_mode_filters) => todo!(),
        }
    }

    /// number indicating what order this filter should appear in a sorted list when building a combined instance.
    /// used internally when building a combined instance.
    /// higher priority matching conditions (i.e. ones we want to test first) should have lower values.
    fn ordering_value(&self) -> u64 {
        use TravelModeFilter as T;
        match self {
            T::MatchesSubtype(..) => 0,
            T::MatchesClasses { .. } => 1,
            T::MatchesClassesWithSubclasses { .. } => 1,
            T::MatchesModeAccess { .. } => 2,
            T::Combined(..) => 999,
        }
    }
}

impl PartialEq for TravelModeFilter {
    fn eq(&self, other: &Self) -> bool {
        self.ordering_value().cmp(&other.ordering_value()).is_eq()
    }
}

impl PartialOrd for TravelModeFilter {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.ordering_value().cmp(&other.ordering_value()))
    }
}
