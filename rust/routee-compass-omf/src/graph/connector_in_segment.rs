use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

/// represents a connector found within a segment
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectorInSegment {
    pub segment_id: String,
    pub connector_id: String,
    pub linear_reference: OrderedFloat<f64>,
}

impl ConnectorInSegment {
    /// records an existing connector record as a connector within a segment.
    pub fn new(segment_id: String, connector_id: String, linear_reference: f64) -> Self {
        Self {
            segment_id,
            connector_id,
            linear_reference: OrderedFloat(linear_reference),
        }
    }

    /// creates a new connector within a segment by concatenating its segment id and reference.
    ///
    /// this follows the pattern described by OvertureMaps when assigning unique
    /// identifiers to sub-segments by their segment id along with linear reference ranges.
    /// see [[https://docs.overturemaps.org/guides/transportation/#transportation-splitter]]
    pub fn new_without_id(segment_id: String, linear_reference: f64) -> Self {
        let connector_id = format!("{}@{}", segment_id, linear_reference);
        Self {
            segment_id,
            connector_id,
            linear_reference: OrderedFloat(linear_reference),
        }
    }
}
