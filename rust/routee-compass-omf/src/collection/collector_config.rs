use serde::{Deserialize, Serialize};

use super::ObjectStoreSource;
use super::OvertureMapsCollectionError;
use super::OvertureMapsCollector;

/// Serializable configuration for OvertureMapsCollector Object.
/// Builds to a [`OvertureMapsCollector`]
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct OvertureMapsCollectorConfig {
    obj_store_type: ObjectStoreSource,
    // Number of row groups to schedule for each process. Defaults to 4
    rg_chunk_size: Option<usize>,
    // Limit to the number of files to process simultaneously. Defaults to 64
    file_concurrency_limit: Option<usize>,
}

impl Default for OvertureMapsCollectorConfig {
    fn default() -> Self {
        Self {
            obj_store_type: ObjectStoreSource::AmazonS3,
            rg_chunk_size: Some(4),
            file_concurrency_limit: Some(64),
        }
    }
}

impl OvertureMapsCollectorConfig {
    pub fn new(
        obj_store_type: ObjectStoreSource,
        rg_chunk_size: Option<usize>,
        file_concurrency_limit: Option<usize>,
    ) -> Self {
        Self {
            obj_store_type,
            rg_chunk_size,
            file_concurrency_limit,
        }
    }

    pub fn build(&self) -> Result<OvertureMapsCollector, OvertureMapsCollectionError> {
        Ok(OvertureMapsCollector::new(
            self.obj_store_type.build()?,
            self.rg_chunk_size.unwrap_or(4),
            self.file_concurrency_limit.unwrap_or(64),
        ))
    }
}
