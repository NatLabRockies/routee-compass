use serde::{Deserialize, Serialize};

use super::{
    access_restriction_when::SegmentAccessRestrictionWhen,
    mode::SegmentMode,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentAccessType {
    Allowed,
    Denied,
    Designated,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentAccessRestriction {
    pub access_type: SegmentAccessType,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub when: Option<SegmentAccessRestrictionWhen>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vehicle: Option<String>,
}

impl SegmentAccessRestriction {
    pub fn contains_mode(&self, mode: &SegmentMode) -> bool {
        self.when
            .as_ref()
            .and_then(|w| w.mode.as_ref())
            .map(|m| m.contains(mode))
            .unwrap_or_default()
    }
}
