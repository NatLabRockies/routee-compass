use crate::collection::{
    OvertureMapsCollectionError, OvertureMapsCollector, OvertureRecord, OvertureRecordType,
    ReleaseVersion, RowFilterConfig, TransportationConnectorRecord, TransportationSegmentRecord,
};

pub struct TransportationCollection {
    pub connectors: Vec<TransportationConnectorRecord>,
    pub segments: Vec<TransportationSegmentRecord>,
}

impl TransportationCollection {
    /// Use a pre-built collector and download configuration to
    /// retrieve connectors and segments for a specified query
    pub fn try_from_collector(
        collector: OvertureMapsCollector,
        release: ReleaseVersion,
        row_filter_config: Option<RowFilterConfig>,
    ) -> Result<Self, OvertureMapsCollectionError> {
        let connectors = collector
            .collect_from_release(
                release.clone(),
                &OvertureRecordType::Connector,
                row_filter_config.clone(),
            )?
            .into_iter()
            .map(|record| match record {
                OvertureRecord::Connector(transportation_connector_record) => {
                    Ok(transportation_connector_record)
                }
                _ => Err(OvertureMapsCollectionError::DeserializeTypeError(format!(
                    "expected connector type, got {record:?}"
                ))),
            })
            .collect::<Result<Vec<TransportationConnectorRecord>, OvertureMapsCollectionError>>()?;

        let segments = collector
            .collect_from_release(
                release.clone(),
                &OvertureRecordType::Segment,
                row_filter_config.clone(),
            )?
            .into_iter()
            .map(|record| match record {
                OvertureRecord::Segment(transportation_segment_record) => {
                    Ok(transportation_segment_record)
                }
                _ => Err(OvertureMapsCollectionError::DeserializeTypeError(format!(
                    "expected segment type, got {record:?}"
                ))),
            })
            .collect::<Result<Vec<TransportationSegmentRecord>, OvertureMapsCollectionError>>()?;

        Ok(Self {
            connectors,
            segments,
        })
    }
}
