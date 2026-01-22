use chrono::NaiveDate;
use futures::stream::{self, StreamExt};
use itertools::Itertools;
use object_store::{path::Path, ListResult, ObjectMeta, ObjectStore};
use parquet::arrow::arrow_reader::ArrowPredicate;
use parquet::arrow::arrow_reader::ArrowReaderOptions;
use parquet::arrow::async_reader::ParquetObjectReader;
use parquet::arrow::async_reader::ParquetRecordBatchStream;
use parquet::arrow::async_reader::ParquetRecordBatchStreamBuilder;
use parquet::file::metadata::RowGroupMetaData;
use parquet::file::statistics::Statistics;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;

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
    batch_size: usize,
}

impl TryFrom<OvertureMapsCollectorConfig> for OvertureMapsCollector {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: OvertureMapsCollectorConfig) -> Result<Self, Self::Error> {
        value.build()
    }
}

impl OvertureMapsCollector {
    pub fn new(object_store: Arc<dyn ObjectStore>, batch_size: usize) -> Self {
        Self {
            obj_store: object_store,
            batch_size,
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
        let row_filter = if let Some(row_filter_config) = &row_filter_config {
            row_filter_config.validate_unique_variant()?;
            Some(RowFilter::try_from(row_filter_config.clone())?)
        } else {
            None
        };

        // Instantiate Stream Builders
        let streams = meta_objects.into_iter().map(|meta| {
            log::debug!("File Name: {}, Size: {}", meta.location, meta.size);

            // Clone required references
            // let object_store = self.obj_store.clone();
            let row_filter_ref = row_filter.clone();
            let io_handle = io_runtime.handle().clone();

            // Parquet objects in charge of processing the incoming stream
            let opts = ArrowReaderOptions::new().with_page_index(true);
            let reader = ParquetObjectReader::new(self.obj_store.clone(), meta.location)
                .with_runtime(io_handle);

            // The return of this block is the future returned by the non-async closure
            async move {
                let builder = ParquetRecordBatchStreamBuilder::new_with_options(reader, opts)
                    .await
                    .map_err(|e| OvertureMapsCollectionError::ArrowReaderError { source: e })?;

                // Implement the required query filters
                // For this we need the schema of each file so we get that from the builder
                let parquet_metadata = builder.metadata();
                let file_metadata = parquet_metadata.file_metadata();

                // Prune row groups using a bbox if available. This optimization
                // could be extended to other kinds of filters in the future.
                let row_group_indices = opt_bbox_filter.as_ref().map(|bbox| {
                    let indices = prune_row_groups_by_bbox(parquet_metadata.row_groups(), bbox);

                    log::debug!(
                        "Pruned to {}/{} row groups",
                        indices.len(),
                        parquet_metadata.num_row_groups()
                    );

                    indices
                });

                // Build Arrow filters from RowFilter enum
                let predicates: Vec<Box<dyn ArrowPredicate>> = if let Some(filter) = &row_filter_ref
                {
                    filter.build(file_metadata)?
                } else {
                    vec![]
                };

                // Check if row group indices is not empty (further optimization)
                // if not empty, apply them to the builder
                let builder = if let Some(indices) = row_group_indices {
                    if indices.is_empty() {
                        return Ok::<_, OvertureMapsCollectionError>(None);
                    }
                    builder.with_row_groups(indices)
                } else {
                    builder
                };

                let row_filter = parquet::arrow::arrow_reader::RowFilter::new(predicates);

                let stream: ParquetRecordBatchStream<ParquetObjectReader> =
                    builder
                        .with_row_filter(row_filter)
                        .with_batch_size(self.batch_size)
                        .build()
                        .map_err(|e| {
                            OvertureMapsCollectionError::ParquetRecordBatchStreamError { source: e }
                        })?;

                Ok::<_, OvertureMapsCollectionError>(Some(stream))
            }
        });

        const CONCURRENCY_LIMIT: usize = 64;

        log::info!("Started collection");
        let start_collection = Instant::now();
        let result_vec = runtime.block_on(async {
            stream::iter(streams)
                .buffer_unordered(CONCURRENCY_LIMIT)
                .collect::<Vec<_>>()
                .await
        });
        log::info!("Collection time {:?}", start_collection.elapsed());

        // Unpack record batches
        let record_streams: Vec<_> = result_vec
            .into_iter()
            .collect::<Result<Vec<_>, OvertureMapsCollectionError>>()?
            .into_iter()
            .flatten()
            .collect();

        let record_batches = runtime
            .block_on(
                stream::iter(record_streams)
                    .flatten_unordered(None)
                    .collect::<Vec<_>>(),
            )
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| OvertureMapsCollectionError::RecordBatchRetrievalError { source: e })?;

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

/// Prune row groups based on bounding box statistics
/// Returns indices of row groups that MAY contain matching rows
fn prune_row_groups_by_bbox(
    row_groups: &[RowGroupMetaData],
    bbox: &crate::collection::Bbox,
) -> Vec<usize> {
    row_groups
        .iter()
        .enumerate()
        .filter(|(_, rg)| {
            // Find the bbox column statistics
            // Overture uses a 'bbox' struct with xmin, xmax, ymin, ymax
            // Check if row group's min/max intersects query bbox

            // Get statistics for bbox.xmin, bbox.xmax, bbox.ymin, bbox.ymax columns
            // If row_group.max_xmin > query.xmax, skip (no intersection)
            // If row_group.min_xmax < query.xmin, skip (no intersection)
            // Similarly for y coordinates

            // look for column paths that are bbox.xmin, bbox.xmax ...

            let mut min_xmin: Option<f32> = None;
            let mut min_ymin: Option<f32> = None;
            let mut max_xmax: Option<f32> = None;
            let mut max_ymax: Option<f32> = None;
            for cc_meta in rg.columns() {
                let column_path = cc_meta.column_path();
                let name_parts = column_path.parts();

                // Ignore columns that are not length 2
                if name_parts.len() != 2 {
                    continue;
                }
                // and those that don't start with bbox
                if name_parts[0] != "bbox" {
                    continue;
                }

                let element = &name_parts[1];
                if element == "xmin" {
                    min_xmin = cc_meta.statistics().and_then(|ss| match ss {
                        Statistics::Float(value) => value.min_opt().copied(),
                        Statistics::Double(value) => value.min_opt().copied().map(|v| v as f32),
                        _ => None,
                    });
                } else if element == "xmax" {
                    max_xmax = cc_meta.statistics().and_then(|ss| match ss {
                        Statistics::Float(value) => value.max_opt().copied(),
                        Statistics::Double(value) => value.max_opt().copied().map(|v| v as f32),
                        _ => None,
                    });
                } else if element == "ymin" {
                    min_ymin = cc_meta.statistics().and_then(|ss| match ss {
                        Statistics::Float(value) => value.min_opt().copied(),
                        Statistics::Double(value) => value.min_opt().copied().map(|v| v as f32),
                        _ => None,
                    });
                } else if element == "ymax" {
                    max_ymax = cc_meta.statistics().and_then(|ss| match ss {
                        Statistics::Float(value) => value.max_opt().copied(),
                        Statistics::Double(value) => value.max_opt().copied().map(|v| v as f32),
                        _ => None,
                    });
                }
            }

            let condition_1 = max_xmax.map(|xmax| xmax >= bbox.xmin).unwrap_or(true);
            let condition_2 = min_xmin.map(|xmin| bbox.xmax >= xmin).unwrap_or(true);
            let condition_3 = max_ymax.map(|ymax| ymax >= bbox.ymin).unwrap_or(true);
            let condition_4 = min_ymin.map(|ymin| bbox.ymax >= ymin).unwrap_or(true);

            condition_1 && condition_2 && condition_3 && condition_4
        })
        .map(|(idx, _)| idx)
        .collect()
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
        OvertureMapsCollectorConfig::new(ObjectStoreSource::AmazonS3, 512)
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
