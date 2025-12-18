use crate::collection::record::{TransportationConnectorRecord, TransportationSegmentRecord};

use super::{BuildingsRecord, PlacesRecord};

#[derive(Debug)]
pub enum OvertureRecord {
    Places(PlacesRecord),
    Buildings(BuildingsRecord),
    Segment(TransportationSegmentRecord),
    Connector(TransportationConnectorRecord),
}

impl From<PlacesRecord> for OvertureRecord {
    fn from(value: PlacesRecord) -> Self {
        Self::Places(value)
    }
}

impl From<BuildingsRecord> for OvertureRecord {
    fn from(value: BuildingsRecord) -> Self {
        Self::Buildings(value)
    }
}

impl From<TransportationSegmentRecord> for OvertureRecord {
    fn from(value: TransportationSegmentRecord) -> Self {
        Self::Segment(value)
    }
}

impl From<TransportationConnectorRecord> for OvertureRecord {
    fn from(value: TransportationConnectorRecord) -> Self {
        Self::Connector(value)
    }
}
