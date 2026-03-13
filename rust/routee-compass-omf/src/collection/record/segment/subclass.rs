use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSubclass {
    Link,
    Sidewalk,
    Crosswalk,
    ParkingAisle,
    Driveway,
    Alley,
    CycleCrossing,
}

impl fmt::Display for SegmentSubclass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SegmentSubclass::Link => "link",
            SegmentSubclass::Sidewalk => "sidewalk",
            SegmentSubclass::Crosswalk => "crosswalk",
            SegmentSubclass::ParkingAisle => "parking_aisle",
            SegmentSubclass::Driveway => "driveway",
            SegmentSubclass::Alley => "alley",
            SegmentSubclass::CycleCrossing => "cycle_crossing",
        };
        f.write_str(s)
    }
}
