use object_store::{ObjectMeta, ObjectStore};
use parquet::{
    arrow::{
        arrow_reader::{ArrowReaderMetadata, ArrowReaderOptions},
        async_reader::{ParquetObjectReader, ParquetRecordBatchStream},
        ParquetRecordBatchStreamBuilder,
    },
    file::{metadata::RowGroupMetaData, statistics::Statistics},
};
use std::sync::Arc;
use tokio::runtime::Handle;

use crate::collection::{Bbox, OvertureMapsCollectionError, RowFilter};

pub struct RowGroupTask {
    pub obj_meta: Arc<ObjectMeta>,
    pub row_groups: Vec<usize>,
    pub parquet_metadata: ArrowReaderMetadata,
}

/// a [`RowGroupTask`] represents a fully determined retrieval operation
/// at the rowgroup level (including file location and metadata).
/// With a [`RowGroupTask`] object constructed, we have all the information
/// needed to build a stream object that produces a vector of RecordBatch.
///
/// A successful call to `build_stream`, you retrieve a [`ParquetRecordBatchStream`] that
/// when consumed, returns the record batches associated with a row group.
impl RowGroupTask {
    pub fn build_stream(
        self,
        row_filter: Option<&RowFilter>,
        obj_store: Arc<dyn ObjectStore>,
        io_handle: Handle,
    ) -> Result<ParquetRecordBatchStream<ParquetObjectReader>, OvertureMapsCollectionError> {
        let built_predicates = row_filter
            .map(|f| f.build(self.parquet_metadata.metadata().file_metadata()))
            .unwrap_or(Ok(vec![]))?;

        let reader = ParquetObjectReader::new(obj_store, self.obj_meta.location.clone())
            .with_runtime(io_handle);

        ParquetRecordBatchStreamBuilder::new_with_metadata(reader, self.parquet_metadata)
            .with_row_groups(self.row_groups)
            .with_row_filter(parquet::arrow::arrow_reader::RowFilter::new(
                built_predicates,
            ))
            .build()
            .map_err(|e| OvertureMapsCollectionError::ArrowReaderError { source: e })
    }
}

/// this auxliary function takes an [`ObjectMeta`]
/// pointing to a specific file in an object store
/// and process it into a [`RowGroupTask`] following
/// the configuration provided. During this process,
/// we perform IO to retrieve the file's metadata and
/// prune it according to an Optional bbox.
pub async fn process_meta_obj_into_tasks(
    meta: ObjectMeta,
    store: Arc<dyn ObjectStore>,
    io_handle: Option<Handle>,
    bbox_prune: Option<Bbox>,
    row_group_chunk_size: Option<usize>,
) -> Result<Vec<RowGroupTask>, OvertureMapsCollectionError> {
    // readers are cheap to build
    let opts = ArrowReaderOptions::new().with_page_index(true);
    let mut reader = if let Some(handle) = io_handle {
        ParquetObjectReader::new(store, meta.location.clone()).with_runtime(handle)
    } else {
        ParquetObjectReader::new(store, meta.location.clone())
    };

    // This goes out and gets the metadata to build a stream
    let arrow_metadata = ArrowReaderMetadata::load_async(&mut reader, opts)
        .await
        .map_err(|e| OvertureMapsCollectionError::ArrowReaderError { source: e })?;
    let parquet_metadata = arrow_metadata.metadata();

    // Prune row groups using a bbox if available. This optimization
    // could be extended to other kinds of filters in the future.
    let row_group_indices = bbox_prune
        .as_ref()
        .map(|bbox| {
            let indices = prune_row_groups_by_bbox(parquet_metadata.row_groups(), bbox);

            log::debug!(
                "Pruned to {}/{} row groups",
                indices.len(),
                parquet_metadata.num_row_groups()
            );

            indices
        })
        .unwrap_or_else(|| (0..parquet_metadata.num_row_groups()).collect());

    let meta_arc = Arc::new(meta);
    Ok(row_group_indices
        .chunks(row_group_chunk_size.unwrap_or(4))
        .map(|indices| RowGroupTask {
            obj_meta: meta_arc.clone(),
            row_groups: indices.to_vec(),
            parquet_metadata: arrow_metadata.clone(),
        })
        .collect())
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
