use std::sync::Arc;

use uom::si::f64::Length;
use uom::ConstZero;

use crate::algorithm::search::SearchTree;
use crate::model::network::Vertex;
use crate::model::state::StateModel;
use crate::model::state::StateVariable;
use crate::model::state::{InputFeature, StateVariableConfig};
use crate::model::traversal::default::distance::DistanceTraversalConfig;
use crate::model::traversal::default::fieldname;
use crate::model::traversal::error::TraversalModelError;
use crate::model::traversal::model::TraversalModel;
use crate::util::geo::haversine;

/// a model for traversing edges based on distance.
pub struct DistanceTraversalModel {
    pub config: Arc<DistanceTraversalConfig>,
}

impl DistanceTraversalModel {
    pub fn new(config: Arc<DistanceTraversalConfig>) -> DistanceTraversalModel {
        Self { config }
    }
}

impl Default for DistanceTraversalModel {
    fn default() -> Self {
        Self {
            config: Arc::new(Default::default()),
        }
    }
}

impl TraversalModel for DistanceTraversalModel {
    fn name(&self) -> String {
        String::from("Distance Traversal Model")
    }
    /// traverses a graph edge and updates the state vector with the distance.
    /// the distance values are directly available on the [`Graph`] model edges.
    ///
    /// [Graph]: crate::model::network::Graph
    fn traverse_edge(
        &self,
        ctx: &crate::model::traversal::EdgeFrontierContext,
        state: &mut Vec<StateVariable>,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        state_model.add_distance(state, &self.config.distance_name, &ctx.edge.distance)?;
        if self.config.include_trip_distance {
            state_model.add_distance(state, &self.config.trip_distance_name, &ctx.edge.distance)?;
        }
        Ok(())
    }

    /// uses a haversine distance to estimate the distance between two vertices.
    fn estimate_traversal(
        &self,
        od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let (src, dst) = od;
        let distance =
            haversine::coord_distance(&src.coordinate, &dst.coordinate).map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "could not compute haversine distance between {src} and {dst}: {e}"
                ))
            })?;
        state_model.add_distance(state, &self.config.distance_name, &distance)?;
        if self.config.include_trip_distance {
            state_model.add_distance(state, &self.config.trip_distance_name, &distance)?;
        }
        Ok(())
    }

    fn input_features(&self) -> Vec<InputFeature> {
        vec![]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        let mut features = vec![(
            String::from(fieldname::EDGE_DISTANCE),
            StateVariableConfig::Distance {
                initial: Length::ZERO,
                accumulator: false,
                output_unit: Some(self.config.distance_unit),
            },
        )];
        if self.config.include_trip_distance {
            features.push((
                String::from(fieldname::TRIP_DISTANCE),
                StateVariableConfig::Distance {
                    initial: Length::ZERO,
                    accumulator: true,
                    output_unit: Some(self.config.distance_unit),
                },
            ));
        }
        features
    }
}
