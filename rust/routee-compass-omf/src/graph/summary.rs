use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    app::CliBoundingBox,
    collection::OvertureMapsCollectionError,
    graph::{
        omf_graph::{OmfEdgeList, GLOBAL_AVG_SPEED_KEY},
        OmfGraphVectorized,
    },
};

/// summarizes an OMF import of a network.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OmfGraphSummary {
    /// information describing how this dataset was generated
    pub source: OmfGraphSource,
    ///
    pub stats: OmfGraphStats,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct OmfGraphSource {
    /// location of imported OMF dataset. this should either be
    /// an official OMF release identifier or a local file path.
    pub release: String,
    /// user-provided name for the network
    pub study_region: String,
    /// date and time this network was created
    pub created: String,
    /// bounding box query used when run
    pub bbox: Option<CliBoundingBox>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct OmfGraphStats {
    /// number of vertices in the network
    pub vertices: usize,
    /// details for each edge list.
    pub edge_list: IndexMap<String, EdgeListStats>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct EdgeListStats {
    /// number of edges in the network
    pub edges: usize,
    /// sum of all miles of roadways
    pub miles: f64,
    /// average speed of all segments in this edge list
    pub avg_speed_mph: Option<f64>,
    /// count and mileage of roadways by road class
    pub road_class_stats: IndexMap<String, ClassStats>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ClassStats {
    /// number of segments
    pub count: usize,
    /// total miles of counted segments
    pub distance_miles: f64,
    /// average speed observed over the counted segments
    pub avg_speed_mph: Option<f64>,
}

struct ClassStatsAcc {
    /// number of segments
    pub count: usize,
    /// total miles of counted segments
    pub sum_distance: uom::si::f64::Length,
}

impl OmfGraphSource {
    pub fn new(release: &str, study_region: &str, bbox: Option<&CliBoundingBox>) -> Self {
        let created = chrono::Utc::now().to_rfc3339();
        Self {
            release: release.to_string(),
            study_region: study_region.to_string(),
            created,
            bbox: bbox.cloned(),
        }
    }
}

impl TryFrom<&OmfGraphVectorized> for OmfGraphStats {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: &OmfGraphVectorized) -> Result<Self, Self::Error> {
        let edge_list_iter = value.edge_list_config.iter().zip(value.edge_lists.iter());
        let mut edge_list = IndexMap::new();
        for (c, e) in edge_list_iter {
            let key = c.mode.clone();
            let value = EdgeListStats::try_from(e)?;
            let _ = edge_list.insert(key, value);
        }
        Ok(OmfGraphStats {
            vertices: value.vertices.len(),
            edge_list,
        })
    }
}

impl TryFrom<&OmfEdgeList> for EdgeListStats {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: &OmfEdgeList) -> Result<Self, Self::Error> {
        let edges = value.edges.len();
        let miles = if edges == 0 {
            0.0
        } else {
            value
                .edges
                .0
                .iter()
                .map(|e| e.distance.get::<uom::si::length::mile>())
                .sum()
        };

        let mut class_stats_accumulators: HashMap<String, ClassStatsAcc> = HashMap::new();
        let edge_iter = value.edges.0.iter().zip(value.classes.iter());
        for (edge, class_full_type) in edge_iter {
            let road_class = class_full_type.as_str().to_string();
            match class_stats_accumulators.get_mut(&road_class) {
                Some(cnt) => {
                    cnt.add(edge.distance);
                }
                None => {
                    let acc = ClassStatsAcc::new(edge.distance);
                    class_stats_accumulators.insert(road_class.clone(), acc);
                }
            }
        }
        let road_class_stats: IndexMap<String, ClassStats> = class_stats_accumulators
            .into_iter()
            .map(|(k, v)| {
                // this fully-qualified road class label may or may not be represented in the
                // collected speed lookup table.
                let avg_speed = value.speed_lookup.get(&k).cloned();
                (k, ClassStats::new(v, avg_speed))
            })
            .collect();
        let avg_speed_mph = value.speed_lookup.get(GLOBAL_AVG_SPEED_KEY).cloned();

        Ok(Self {
            edges,
            miles,
            road_class_stats,
            avg_speed_mph,
        })
    }
}

impl ClassStats {
    fn new(acc: ClassStatsAcc, avg_speed_mph: Option<f64>) -> Self {
        Self {
            count: acc.count,
            distance_miles: acc.sum_distance.get::<uom::si::length::mile>(),
            avg_speed_mph,
        }
    }
}

impl ClassStatsAcc {
    pub fn new(distance: uom::si::f64::Length) -> Self {
        Self {
            count: 1,
            sum_distance: distance,
        }
    }
    pub fn add(&mut self, distance: uom::si::f64::Length) {
        self.count += 1;
        self.sum_distance += distance;
    }
}
