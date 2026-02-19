use std::path::Path;

use clap::{Parser, Subcommand};
use config::{Config, File};
use serde::{Deserialize, Serialize};

use crate::{
    app::{
        cli_bbox::parse_bbox,
        network::{IslandDetectionAlgorithmConfiguration, NetworkEdgeListConfiguration},
        CliBoundingBox,
    },
    collection::OvertureMapsCollectionError,
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
    Network {
        /// descriptive user-provided name for this import region.
        #[arg(short, long)]
        name: String,

        /// configuration file defining how the network is imported and separated
        /// into mode-specific edge lists.
        #[arg(short, long)]
        configuration_file: String,

        /// location on disk to write output files. if not provided,
        /// use the current working directory.
        #[arg(short, long)]
        output_directory: Option<String>,

        /// use a stored raw data export from a previous run of OmfOperation::Network
        /// which is a JSON file containing a TransportationCollection.
        #[arg(short, long)]
        local_source: Option<String>,
        
        /// write the raw OMF dataset as a JSON blob to the output directory.
        #[arg(short, long)]
        store_raw: bool,

        /// bounding box to filter data (format: xmin,xmax,ymin,ymax)
        #[arg(short, long, value_parser = parse_bbox, allow_hyphen_values(true))]
        bbox: Option<CliBoundingBox>,

        /// write the list of segment and connector IDs for each edge created
        #[arg(long)]
        omf_ids: bool,
    },
}

impl OmfOperation {
    pub fn run(&self) -> Result<(), OvertureMapsCollectionError> {
        match self {
            OmfOperation::Network {
                name,
                configuration_file,
                output_directory,
                local_source,
                store_raw,
                bbox,
                omf_ids,
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
                let island_algorithm_configuration = config
                    .get::<Option<IslandDetectionAlgorithmConfiguration>>(
                        "island_algorithm_configuration",
                    )
                    .map_err(|e| {
                        let msg = format!(
                            "error reading 'island_algorithm_configuration' key in '{configuration_file}': {e}"
                        );
                        OvertureMapsCollectionError::InvalidUserInput(msg)
                    })?;
                let outdir = match output_directory {
                    Some(out) => Path::new(out),
                    None => Path::new(""),
                };
                let local = local_source.as_ref().map(Path::new);
                crate::app::network::run(
                    name,
                    bbox.as_ref(),
                    &network_config,
                    outdir,
                    local,
                    *store_raw,
                    island_algorithm_configuration,
                    *omf_ids,
                )
            }
        }
    }
}
