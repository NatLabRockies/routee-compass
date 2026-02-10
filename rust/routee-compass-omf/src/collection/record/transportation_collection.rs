use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::collection::{
    OvertureMapsCollectionError, OvertureMapsCollector, OvertureRecord, OvertureRecordType,
    ReleaseVersion, RowFilterConfig, TransportationConnectorRecord, TransportationSegmentRecord,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransportationCollection {
    pub release: String,
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
        let uri = match &release {
            ReleaseVersion::Latest => collector.get_latest_release()?,
            other => String::from(other),
        };
        let connectors = collector
            .collect_from_release(
                &uri,
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
                &uri,
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
            release: uri,
            connectors,
            segments,
        })
    }

    /// write this collection to disk as JSON.
    pub fn to_json(&self, output_directory: &Path) -> Result<(), OvertureMapsCollectionError> {
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            let msg = format!("failure while serializing OMF data as JSON: {e}");
            OvertureMapsCollectionError::SerializationError(msg)
        })?;
        let filepath = output_directory.join("omf-raw.json");
        std::fs::write(filepath, &json).map_err(|e| {
            let msg = format!(
                "failure while writing OMF data as JSON to disk at {}: {e}",
                output_directory.to_str().unwrap_or_default()
            );
            OvertureMapsCollectionError::SerializationError(msg)
        })
    }
}
