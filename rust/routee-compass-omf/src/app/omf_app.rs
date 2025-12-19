use std::path::Path;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use crate::{
    collection::{
        ObjectStoreSource, OvertureMapsCollectionError, OvertureMapsCollectorConfig,
        ReleaseVersion, RowFilterConfig, TransportationCollection,
    },
    graph::OmfGraphVectorized,
};

/// Command line tool for batch downloading and summarizing of OMF (Overture Maps Foundation) data
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct OmfApp {
    #[command(subcommand)]
    pub op: OmfOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Subcommand)]
pub enum OmfOperation {
    /// download all of the OMF transportation data
    Download,
}

impl OmfOperation {
    pub fn run(self) -> Result<(), OvertureMapsCollectionError> {
        match self {
            OmfOperation::Download => {
                let collector =
                    OvertureMapsCollectorConfig::new(ObjectStoreSource::AmazonS3, 128).build()?;
                let release = ReleaseVersion::Latest;
                let row_filter_config = RowFilterConfig::Bbox {
                    xmin: -105.254,
                    xmax: -105.197,
                    ymin: 39.733,
                    ymax: 39.784,
                };

                let collection = TransportationCollection::try_from_collector(
                    collector,
                    release,
                    Some(row_filter_config),
                )?;
                let vectorized_graph = OmfGraphVectorized::try_from_collection(collection, 0)?;
                vectorized_graph.write_compass(Path::new("./"), true)?;

                Ok(())
            }
        }
    }
}
