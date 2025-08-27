use serde::{Deserialize, Serialize};

use super::ObjectStoreSource;
use super::OvertureMapsCollectionError;
use super::OvertureMapsCollector;

/// Serializable configuration for OvertureMapsCollector Object.
/// Builds to a [`OvertureMapsCollector`]
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct OvertureMapsCollectorConfig {
    obj_store_type: ObjectStoreSource,
    batch_size: usize,
}

impl Default for OvertureMapsCollectorConfig {
    fn default() -> Self {
        Self {
            obj_store_type: ObjectStoreSource::AmazonS3,
            batch_size: 4096 * 32,
        }
    }
}

impl OvertureMapsCollectorConfig {
    pub fn new(obj_store_type: ObjectStoreSource, batch_size: usize) -> Self {
        Self {
            obj_store_type,
            batch_size,
        }
    }

    pub fn build(&self) -> Result<OvertureMapsCollector, OvertureMapsCollectionError> {
        Ok(OvertureMapsCollector::new(
            self.obj_store_type.build()?,
            // self.row_filter_config.clone(),
            self.batch_size,
        ))
    }
}
