//! configuration for the [`super::MapModel`].
//! this model is responsible for:
//!   - map matching queries to valid Graph Edges/Vertices (via [`super::MatchingType`])
//!   - lookup of geometries from EdgeListId/EdgeId combinations across Compass
use crate::{
    config::OneOrMany,
    model::{map::SpatialIndexType, unit::DistanceUnit},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uom::si::f64::Length;

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct MapModelConfig {
    /// distance from coordinate to the nearest vertex required for map matching
    pub tolerance: Option<DistanceTolerance>,
    /// geometries to place in the spatial index used for map matching.
    pub spatial_index_type: Option<SpatialIndexType>,
    /// the [`MatchingType`]s supported
    pub matching_type: Option<Vec<String>>,
    /// for each edge list, geometry configuration
    pub geometry: OneOrMany<MapModelGeometryConfig>,
    /// allow source-only queries for shortest path tree outputs
    pub queries_without_destinations: bool,
}

/// for a given EdgeList, the source of its geometries. this can be
///   - simply constructed by drawing lines between the vertices
///     used by each edge in this edgelist (from_vertices)
///   - a file containing LineStrings (from_linestrings)
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapModelGeometryConfig {
    FromVertices,
    FromLinestrings {
        /// file containing edge geometries for this [`EdgeList`]
        geometry_input_file: String,
    },
}

impl Default for MapModelConfig {
    fn default() -> Self {
        Self {
            tolerance: Default::default(),
            matching_type: Default::default(),
            spatial_index_type: Default::default(),
            geometry: OneOrMany::One(MapModelGeometryConfig::FromVertices),
            queries_without_destinations: Default::default(),
        }
    }
}

/// deserialize an Optional MapModel from configuration
impl TryFrom<Option<&Value>> for MapModelConfig {
    type Error = String;

    fn try_from(value: Option<&Value>) -> Result<Self, Self::Error> {
        match value {
            None => Ok(MapModelConfig::default()),
            Some(json) => {
                let map_model_config: MapModelConfig =
                    serde_json::from_value(json.clone()).map_err(|e| {
                        let map_model_str = serde_json::to_string_pretty(&json).unwrap_or_default();
                        format!(
                            "unable to deserialize map model configuration section due to '{e}'. input data: \n{map_model_str}"
                        )
                    })?;
                Ok(map_model_config)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct DistanceTolerance {
    pub distance: f64,
    pub unit: DistanceUnit,
}

impl DistanceTolerance {
    pub fn to_uom(&self) -> Length {
        self.unit.to_uom(self.distance)
    }
}
