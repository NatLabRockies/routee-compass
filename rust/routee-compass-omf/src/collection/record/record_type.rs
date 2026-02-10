use arrow::array::RecordBatch;
use serde::de::DeserializeOwned;

use crate::collection::{OvertureMapsCollectionError, OvertureRecord};

pub enum OvertureRecordType {
    Places,
    Buildings,
    Segment,
    Connector,
}

impl OvertureRecordType {
    pub fn format_url(&self, release_str: &str) -> String {
        match self {
            OvertureRecordType::Places => {
                format!("release/{release_str}/theme=places/type=place/").to_owned()
            }
            OvertureRecordType::Buildings => {
                format!("release/{release_str}/theme=buildings/type=building/").to_owned()
            }
            OvertureRecordType::Segment => {
                format!("release/{release_str}/theme=transportation/type=segment/").to_owned()
            }
            OvertureRecordType::Connector => {
                format!("release/{release_str}/theme=transportation/type=connector/").to_owned()
            }
        }
    }

    /// processes an arrow [RecordBatch] into an [OvertureRecord] collection,
    /// deserializing into the underlying row type struct along the way.
    pub fn process_batch<R>(
        &self,
        record_batch: &RecordBatch,
    ) -> Result<Vec<OvertureRecord>, OvertureMapsCollectionError>
    where
        R: DeserializeOwned + Into<OvertureRecord>,
    {
        let as_rows: Vec<R> = serde_arrow::from_record_batch(record_batch).map_err(|e| {
            OvertureMapsCollectionError::DeserializeError(format!("Serde error: {e}"))
        })?;
        let as_result: Vec<OvertureRecord> = as_rows.into_iter().map(Into::into).collect();
        Ok(as_result)
    }
}

impl std::fmt::Display for OvertureRecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Places => write!(f, "Places"),
            Self::Buildings => write!(f, "Buildings"),
            Self::Segment => write!(f, "Segment"),
            Self::Connector => write!(f, "Connector"),
        }
    }
}
