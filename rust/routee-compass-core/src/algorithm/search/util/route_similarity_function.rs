use crate::{
    algorithm::search::{edge_traversal::EdgeTraversal, search_error::SearchError, SearchInstance},
    model::network::{EdgeId, EdgeListId},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// finds the similarity between two paths during a ksp algorithm.
#[derive(Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RouteSimilarityFunction {
    #[default]
    AcceptAll,
    /// each edge id match between paths is counted. a path is "similar" if
    /// match count / total edges is greater than threshold.
    EdgeIdCosineSimilarity { threshold: f64 },
    /// weights each match count by the edge distance. a path is "similar" if
    /// match distances / total distances is greater than threshold.
    DistanceWeightedCosineSimilarity { threshold: f64 },
}

type DistanceFunction<'a> =
    Box<dyn Fn(&'_ EdgeListId, &'_ EdgeId) -> Result<f64, SearchError> + 'a>;

impl RouteSimilarityFunction {
    /// tests for similarity between two paths.
    ///
    /// # Arguments
    /// * `a`  - one route
    /// * `b`  - another route
    /// * `si` - search instance for these routes
    ///
    /// # Returns
    /// true if the routes are "similar"
    pub fn test_similarity(
        self,
        a: &[&EdgeTraversal],
        b: &[&EdgeTraversal],
        si: &SearchInstance,
    ) -> Result<bool, SearchError> {
        let similarity = self.rank_similarity(a, b, si)?;
        Ok(self.is_similar(similarity))
    }

    /// compares a similarity rank against similarity criteria.
    ///
    /// # Arguments
    /// * `similarity` - output of this rank_similarity function
    ///
    /// # Result
    ///
    /// true if the ranking meets the similarity criteria
    pub fn is_similar(&self, similarity: f64) -> bool {
        match self {
            RouteSimilarityFunction::AcceptAll => true,
            RouteSimilarityFunction::EdgeIdCosineSimilarity { threshold } => {
                similarity >= *threshold
            }
            RouteSimilarityFunction::DistanceWeightedCosineSimilarity { threshold } => {
                similarity >= *threshold
            }
        }
    }

    /// ranks the similarity of two routes using this similarity function.
    ///
    /// # Arguments
    /// * `a`  - one route
    /// * `b`  - another route
    /// * `si` - search instance for these routes
    ///
    /// # Returns
    /// the similarity ranking of these routes
    pub fn rank_similarity(
        &self,
        a: &[&EdgeTraversal],
        b: &[&EdgeTraversal],
        si: &SearchInstance,
    ) -> Result<f64, SearchError> {
        match self {
            RouteSimilarityFunction::AcceptAll => Ok(0.0),
            RouteSimilarityFunction::EdgeIdCosineSimilarity { threshold: _ } => {
                let unit_dist_fn = Box::new(|_: &EdgeListId, _: &EdgeId| Ok(1.0));
                cos_similarity(a, b, unit_dist_fn)
            }
            RouteSimilarityFunction::DistanceWeightedCosineSimilarity { threshold: _ } => {
                let dist_fn = Box::new(|edge_list_id: &EdgeListId, edge_id: &EdgeId| {
                    si.graph
                        .get_edge(edge_list_id, edge_id)
                        .map(|edge| edge.distance.get::<uom::si::length::meter>())
                        .map_err(SearchError::from)
                });
                cos_similarity(a, b, dist_fn)
            }
        }
    }
}

/// computes the cosine similarity of two routes using the provided distance function
///
/// # Arguments
///
/// * `a`       - this route
/// * `b`       - that route
/// * `dist_fn` - mapping from EdgeIds to some distance value
///
/// # Returns
///
/// the cosine similarity of the routes, a value from -1 to 1
fn cos_similarity(
    a: &[&EdgeTraversal],
    b: &[&EdgeTraversal],
    dist_fn: DistanceFunction<'_>,
) -> Result<f64, SearchError> {
    let a_map = a
        .iter()
        .map(|e| dist_fn(&e.edge_list_id, &e.edge_id).map(|dist| (e.edge_id, dist)))
        .collect::<Result<HashMap<_, _>, _>>()?;
    let b_map = b
        .iter()
        .map(|e| dist_fn(&e.edge_list_id, &e.edge_id).map(|dist| (e.edge_id, dist)))
        .collect::<Result<HashMap<_, _>, _>>()?;

    let numer: f64 = a_map
        .keys()
        .collect::<HashSet<_>>()
        .union(&b_map.keys().collect::<HashSet<_>>())
        .map(|edge_id| {
            let a_dist = a_map.get(edge_id).cloned().unwrap_or_default();
            let b_dist = b_map.get(edge_id).cloned().unwrap_or_default();
            a_dist * b_dist
        })
        .sum();

    let denom_a: f64 = a_map.values().map(|d| d * d).sum();
    let denom_b: f64 = b_map.values().map(|d| d * d).sum();
    let denom = denom_a.sqrt() * denom_b.sqrt();
    let cos_sim = numer / denom;
    Ok(cos_sim)
}
