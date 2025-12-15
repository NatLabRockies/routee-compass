use crate::collection::record::{TransportationConnectorRecord, TransportationSegmentRecord};

use super::{BuildingsRecord, PlacesRecord};

#[derive(Debug)]
pub enum OvertureRecord {
    Places(PlacesRecord),
    Buildings(BuildingsRecord),
    Segment(TransportationSegmentRecord),
    Connector(TransportationConnectorRecord),
}
