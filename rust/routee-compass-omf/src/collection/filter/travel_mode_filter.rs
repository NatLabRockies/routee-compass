use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::collection::{
    record::{
        SegmentAccessRestriction, SegmentAccessType, SegmentClass, SegmentHeading, SegmentMode,
        SegmentSubclass, SegmentSubtype,
    },
    TransportationSegmentRecord,
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

    /// filter a row based on the [SegmentMode].
    ///
    /// # Other Modifiers
    ///   - if "heading" is present, it must be "forward"
    ///   - if "using" or "recognized" modifiers are present, returns false
    ///     - these imply some special user type, we want to ignore any of these for now
    ///   - "during", and "vehicle" modifiers are ignored.
    #[serde(rename = "access_mode")]
    MatchesModeAccess { modes: Vec<SegmentMode> },
}

/// helper struct used when processing [MatchesModeAccess] travel mode filters.
#[derive(Clone, Debug)]
struct ModeAccessAccumulator {
    pub modes: Vec<SegmentMode>,
    pub blanket_denial: bool,
    pub mode_denial: bool,
    pub mode_allowed: bool,
}

impl ModeAccessAccumulator {
    pub fn new(modes: &[SegmentMode]) -> Self {
        Self {
            modes: modes.iter().cloned().collect(),
            blanket_denial: false,
            mode_denial: false,
            mode_allowed: true,
        }
    }

    /// whether the restrictions recorded by this accumulator imply
    /// that the mode is supported on this segment.
    pub fn supports_mode(&self) -> bool {
        return match (self.blanket_denial, self.mode_denial, self.mode_allowed) {
            // blanket denial with exception
            (true, false, true) => true,
            // mode disallowed explicitly
            (_, true, _) => false,
            // mode disallowed implicitly
            (_, _, false) => false,
            // mode allowed implicitly
            _ => true,
        };
    }

    /// updates the accumulator with an additional restriction
    pub fn add_restriction(&mut self, r: &SegmentAccessRestriction) {
        // unpack values from the restriction relevant to this travel mode
        let has_mode = r
            .when
            .as_ref()
            .map(|x| {
                x.mode
                    .as_ref()
                    .map(|modes| modes.iter().any(|m| self.modes.contains(m)))
            })
            .flatten();
        let heading = r.when.as_ref().map(|x| x.heading.clone()).flatten();
        let mods = r
            .when
            .as_ref()
            .map(|x| x.recognized.is_some() || x.using.is_some());

        // match on cases that require a state update
        use SegmentAccessType as SAT;
        use SegmentHeading as SH;
        match (&r.access_type, has_mode, heading, mods) {
            (SAT::Denied, None, None, None) => {
                self.blanket_denial = true;
            }
            (SAT::Denied, Some(true), None | Some(SH::Forward), _) => {
                self.mode_denial = true;
                self.mode_allowed = false;
            }
            (SAT::Allowed | SAT::Designated, Some(true), None | Some(SH::Forward), None) => {
                self.mode_allowed = true;
            }
            (SAT::Allowed | SAT::Designated, Some(true), None | Some(SH::Forward), Some(true)) => {
                // currently not supporting the handling of "using" or "recognized"
                // modifications indicating this mode is only supported for a subset
                // of the population.
                self.mode_allowed = false;
            }
            _ => {}
        }
    }
}

impl TravelModeFilter {
    /// test whether a given row matches a travel mode filter.
    /// returns false if there is no match.
    pub fn matches_filter(&self, segment: &TransportationSegmentRecord) -> bool {
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

            TravelModeFilter::MatchesModeAccess { modes } => {
                let restrictions = segment
                    .access_restrictions
                    .as_ref()
                    .map(|rs| rs.iter())
                    .unwrap_or_default();

                let mut acc = ModeAccessAccumulator::new(modes);
                for r in restrictions {
                    acc.add_restriction(r);
                }
                acc.supports_mode()
            }
        }
    }

    /// number indicating what order this filter should appear in a sorted list.
    /// used internally to optimize performance.
    /// higher priority matching conditions (i.e. ones we want to test first) should have lower values.
    fn ordering_value(&self) -> u64 {
        use TravelModeFilter as T;
        match self {
            T::MatchesSubtype(..) => 0,
            T::MatchesClasses { .. } => 1,
            T::MatchesClassesWithSubclasses { .. } => 1,
            T::MatchesModeAccess { .. } => 2,
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

impl Eq for TravelModeFilter {}

impl Ord for TravelModeFilter {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordering_value().cmp(&other.ordering_value())
    }
}
