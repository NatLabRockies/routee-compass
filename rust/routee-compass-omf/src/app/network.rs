use std::path::Path;

use routee_compass_core::model::network::EdgeListId;
use serde::{Deserialize, Serialize};

use crate::{
    collection::{
        filter::TravelModeFilter, ObjectStoreSource, OvertureMapsCollectionError,
        OvertureMapsCollectorConfig, ReleaseVersion, RowFilterConfig, TransportationCollection,
    },
    graph::OmfGraphVectorized,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkEdgeListConfiguration {
    mode: String,
    filter: Vec<TravelModeFilter>,
}

/// runs an OMF network import using the provided configuration.
pub fn run(
    configuration: &[NetworkEdgeListConfiguration],
    output_directory: Option<&str>,
) -> Result<(), OvertureMapsCollectionError> {
    let collector = OvertureMapsCollectorConfig::new(ObjectStoreSource::AmazonS3, 128).build()?;
    let release = ReleaseVersion::Latest;
    let row_filter_config = RowFilterConfig::Bbox {
        xmin: -105.254,
        xmax: -105.197,
        ymin: 39.733,
        ymax: 39.784,
    };

    let collection =
        TransportationCollection::try_from_collector(collector, release, Some(row_filter_config))?;

    let edge_list_id = EdgeListId(0);
    let vectorized_graph = OmfGraphVectorized::new(collection, edge_list_id)?;
    let output_path = match output_directory {
        Some(o) => Path::new(o),
        None => Path::new(""),
    };
    vectorized_graph.write_compass(output_path, true)?;

    // for (index, edge_list) in configuration.iter().enumerate() {

    // }

    Ok(())
}
