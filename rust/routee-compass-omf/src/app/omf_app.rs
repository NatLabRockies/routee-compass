use std::path::Path;

use clap::{Parser, Subcommand};
use config::{Config, File};
use serde::{Deserialize, Serialize};

use crate::{app::network::NetworkEdgeListConfiguration, collection::OvertureMapsCollectionError};

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
    Network {
        /// configuration file defining how the network is imported and separated
        /// into mode-specific edge lists.
        #[arg(short, long)]
        configuration_file: String,
        /// location on disk to write output files. if not provided,
        /// use the current working directory.
        #[arg(short, long)]
        output_directory: Option<String>,
    },
}

impl OmfOperation {
    pub fn run(&self) -> Result<(), OvertureMapsCollectionError> {
        match self {
            OmfOperation::Network {
                configuration_file,
                output_directory,
            } => {
                let filepath = Path::new(configuration_file);
                let config = Config::builder()
                    .add_source(File::from(filepath))
                    .build()
                    .map_err(|e| {
                        let msg = format!("file '{configuration_file}' produced error: {e}");
                        OvertureMapsCollectionError::InvalidUserInput(msg)
                    })?;
                let network_config = config
                    .get::<Vec<NetworkEdgeListConfiguration>>("edge_lists")
                    .map_err(|e| {
                        let msg = format!(
                            "error reading 'edge_lists' key in '{configuration_file}': {e}"
                        );
                        OvertureMapsCollectionError::InvalidUserInput(msg)
                    })?;
                let outdir = output_directory.as_ref().map(|s| s.as_str());
                crate::app::network::run(&network_config, outdir)
            }
        }
    }
}
