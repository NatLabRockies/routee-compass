use object_store::{aws::AmazonS3Builder, ObjectStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::OvertureMapsCollectionError;

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum ObjectStoreSource {
    AmazonS3,
    Azure,
    FileSystem,
}

impl ObjectStoreSource {
    pub fn build(&self) -> Result<Arc<dyn ObjectStore>, OvertureMapsCollectionError> {
        match self {
            ObjectStoreSource::AmazonS3 => {
                let object_store = AmazonS3Builder::new()
                    .with_region("us-west-2")
                    .with_skip_signature(true)
                    .with_url("s3://overturemaps-us-west-2/")
                    .build()
                    .map_err(|e| OvertureMapsCollectionError::ConnectionError(e.to_string()))?;

                Ok(Arc::new(object_store))
            }
            ObjectStoreSource::Azure => todo!(),
            ObjectStoreSource::FileSystem => todo!(),
        }
    }
}
