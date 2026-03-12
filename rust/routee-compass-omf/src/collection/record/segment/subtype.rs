use std::fmt;

use serde::{Deserialize, Serialize};

use super::{class::SegmentClass, subclass::SegmentSubclass};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSubtype {
    Road,
    Rail,
    Water,
}

impl fmt::Display for SegmentSubtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SegmentSubtype::Road => "road",
            SegmentSubtype::Rail => "rail",
            SegmentSubtype::Water => "water",
        };
        f.write_str(s)
    }
}

/// Fully qualified segment type including type, class and subclass. E.g. road-service-driveway
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SegmentFullType(
    pub SegmentSubtype,
    pub SegmentClass,
    pub Option<SegmentSubclass>,
);

impl SegmentFullType {
    pub fn has_subclass(&self) -> bool {
        self.2.is_some()
    }

    pub fn with_subclass(&self, subclass: SegmentSubclass) -> Self {
        Self(self.0.clone(), self.1.clone(), Some(subclass))
    }

    pub fn as_str(&self) -> String {
        match self.2.as_ref() {
            Some(subclass) => format!("{}-{}-{}", self.0, self.1, subclass),
            None => format!("{}-{}", self.0, self.1),
        }
    }
}
