use arrow::array::RecordBatch;
use chrono::NaiveDate;
use futures::stream::{self, StreamExt};
use futures::{TryFutureExt, TryStreamExt};
use itertools::Itertools;
use object_store::{path::Path, ListResult, ObjectMeta, ObjectStore};
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;

use crate::collection::collector_ops::{process_meta_obj_into_tasks, RowGroupTask};
use crate::collection::record::TransportationConnectorRecord;
use crate::collection::record::TransportationSegmentRecord;
use crate::collection::BuildingsRecord;
use crate::collection::PlacesRecord;

use super::record::OvertureRecord;
use super::record::OvertureRecordType;
use super::OvertureMapsCollectionError;
use super::OvertureMapsCollectorConfig;
use super::ReleaseVersion;
use super::RowFilter;
use super::RowFilterConfig;

/// Stores the initialized object store and allows to collect
/// records by `collect_from_release` and `collect_from_path`
#[derive(Debug)]
pub struct OvertureMapsCollector {
    obj_store: Arc<dyn ObjectStore>,
    rg_chunk_size: usize,
    file_concurrency_limit: usize,
}

impl TryFrom<OvertureMapsCollectorConfig> for OvertureMapsCollector {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: OvertureMapsCollectorConfig) -> Result<Self, Self::Error> {
        value.build()
    }
}

impl OvertureMapsCollector {
    pub fn new(
        object_store: Arc<dyn ObjectStore>,
        rg_chunk_size: usize,
        file_concurrency_limit: usize,
    ) -> Self {
        Self {
            obj_store: object_store,
            rg_chunk_size,
            file_concurrency_limit,
        }
    }

    fn get_latest_release(&self) -> Result<String, OvertureMapsCollectionError> {
        // Get runtime to consume async functions
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                OvertureMapsCollectionError::TokioError(format!(
                    "failure creating async rust tokio runtime: {e}"
                ))
            })?;

        // Get the folder names for all releases
        let common_path = Path::from("release/");
        let ListResult {
            common_prefixes, ..
        } = runtime
            .block_on(self.obj_store.list_with_delimiter(Some(&common_path)))
            .map_err(|e| {
                OvertureMapsCollectionError::ConnectionError(format!(
                    "Could not retrieve list of folders to get latest OvertureMaps release: {e}"
                ))
            })?;

        // Process all common prefixes to find latest date
        let mut version_tuples: Vec<(NaiveDate, String)> = common_prefixes
            .iter()
            .filter_map(|p| {
                let clean_str = p.to_string().strip_prefix("release/")?.to_string();

                let mut string_parts = clean_str.split(".");
                let date_part = NaiveDate::parse_from_str(string_parts.next()?, "%Y-%m-%d").ok()?;

                Some((date_part, clean_str))
            })
            .collect();

        // Get the latest date from tuples
        version_tuples.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        version_tuples
            .pop()
            .ok_or(OvertureMapsCollectionError::ConnectionError(String::from(
                "No version tuples generated while getting latest version string",
            )))
            .map(|(_, v)| v)
    }

    pub fn collect_from_path(
        &self,
        path: Path,
        record_type: &OvertureRecordType,
        row_filter_config: Option<RowFilterConfig>,
    ) -> Result<Vec<OvertureRecord>, OvertureMapsCollectionError> {
        let filemeta_stream = self.obj_store.list(Some(&path));

        let io_runtime = tokio::runtime::Runtime::new()
            .map_err(|e| OvertureMapsCollectionError::TokioError(e.to_string()))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                OvertureMapsCollectionError::TokioError(format!(
                    "failure creating async rust tokio runtime: {e}"
                ))
            })?;

        // Collect all metadata to create streams, synchronously
        let meta_objects = runtime
            .block_on(filemeta_stream.collect::<Vec<_>>())
            .into_iter()
            .collect::<Result<Vec<ObjectMeta>, _>>()
            .map_err(|e| OvertureMapsCollectionError::MetadataError(e.to_string()))?;

        // Prepare the filter predicates
        let opt_bbox_filter = row_filter_config
            .as_ref()
            .and_then(|f| f.get_bbox_filter_if_exists());
        
        // validate provided bbox
        if let Some(bbox) = opt_bbox_filter.as_ref() {
            bbox.validate()?
        };

        // build rest of the filters
        let row_filter = if let Some(row_filter_config) = &row_filter_config {
            row_filter_config.validate_unique_variant()?;
            Some(RowFilter::try_from(row_filter_config.clone())?)
        } else {
            None
        };

        log::info!("Started collection");
        let start_collection = Instant::now();
        // Process each all metadata object into a flat vector of tasks that
        // each take a small number of row_groups. Inside the `process_meta_obj_into_tasks`
        // function we also prune based on the bounding box
        let row_group_tasks: Vec<RowGroupTask> = runtime.block_on(async {
            Ok(stream::iter(meta_objects)
                .map(|meta| {
                    process_meta_obj_into_tasks(
                        meta,
                        self.obj_store.clone(),
                        Some(io_runtime.handle().clone()),
                        opt_bbox_filter,
                        Some(self.rg_chunk_size),
                    )
                })
                .buffer_unordered(self.file_concurrency_limit)
                .try_collect::<Vec<Vec<RowGroupTask>>>()
                .await?
                .into_iter()
                .flatten()
                .collect())
        })?;

        // Build and collect streams
        let streams = row_group_tasks
            .into_iter()
            .map(|rgt| {
                rgt.build_stream(
                    row_filter.as_ref(),
                    self.obj_store.clone(),
                    io_runtime.handle().clone(),
                )
            })
            .collect::<Result<Vec<_>, OvertureMapsCollectionError>>()?;

        let record_batches = runtime.block_on(
            stream::iter(streams)
                .flatten_unordered(self.file_concurrency_limit)
                .try_collect::<Vec<RecordBatch>>()
                .map_err(|e| OvertureMapsCollectionError::RecordBatchRetrievalError { source: e }),
        )?;
        log::info!("Collection time {:?}", start_collection.elapsed());

        // Deserialize the batches into Records
        let start_deserialization = Instant::now();
        let records: Vec<OvertureRecord> = record_batches
            .par_iter()
            .map(|batch| match record_type {
                OvertureRecordType::Places => record_type.process_batch::<PlacesRecord>(batch),
                OvertureRecordType::Buildings => {
                    record_type.process_batch::<BuildingsRecord>(batch)
                }
                OvertureRecordType::Segment => {
                    record_type.process_batch::<TransportationSegmentRecord>(batch)
                }
                OvertureRecordType::Connector => {
                    record_type.process_batch::<TransportationConnectorRecord>(batch)
                }
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect_vec();

        log::info!("Deserialization time {:?}", start_deserialization.elapsed());
        log::info!("Total time {:?}", start_collection.elapsed());
        Ok(records)
    }

    pub fn collect_from_release(
        &self,
        release: ReleaseVersion,
        record_type: &OvertureRecordType,
        row_filter_config: Option<RowFilterConfig>,
    ) -> Result<Vec<OvertureRecord>, OvertureMapsCollectionError> {
        let release_str = match release {
            ReleaseVersion::Latest => self.get_latest_release()?,
            other => String::from(other),
        };
        log::info!("Collecting OvertureMaps {record_type} records from release {release_str}");
        let path = Path::from(record_type.format_url(release_str));
        self.collect_from_path(path, record_type, row_filter_config)
    }
}

#[cfg(test)]
mod test {
    use crate::collection::{
        ObjectStoreSource, OvertureMapsCollector, OvertureMapsCollectorConfig, OvertureRecord,
        OvertureRecordType, ReleaseVersion, RowFilterConfig,
    };
    use chrono::NaiveDate;
    use std::str::FromStr;

    fn get_collector() -> OvertureMapsCollector {
        OvertureMapsCollectorConfig::new(ObjectStoreSource::AmazonS3, Some(4), Some(64))
            .build()
            .unwrap()
    }

    #[test]
    #[ignore]
    fn test_deserialization() {
        let collector = get_collector();

        // Roughly Golden, CO
        let row_filter = RowFilterConfig::Bbox {
            xmin: -105.254,
            xmax: -105.197,
            ymin: 39.733,
            ymax: 39.784,
        };

        // Connectors
        let connector_records = collector
            .collect_from_release(
                ReleaseVersion::Monthly {
                    datetime: NaiveDate::from_str("2025-12-17").unwrap(),
                    version: Some(0),
                },
                &OvertureRecordType::Connector,
                Some(row_filter.clone()),
            )
            .unwrap();

        println!("Records Length: {}", connector_records.len());

        assert_eq!(connector_records.len(), 6436);
        assert!(matches!(
            connector_records[0],
            OvertureRecord::Connector(..)
        ));

        // Segment
        let segment_records = collector
            .collect_from_release(
                ReleaseVersion::Monthly {
                    datetime: NaiveDate::from_str("2025-12-17").unwrap(),
                    version: Some(0),
                },
                &OvertureRecordType::Segment,
                Some(row_filter),
            )
            .unwrap();

        println!("Records Length: {}", segment_records.len());

        assert_eq!(segment_records.len(), 3771);
        assert!(matches!(segment_records[0], OvertureRecord::Segment(..)));
    }
}
