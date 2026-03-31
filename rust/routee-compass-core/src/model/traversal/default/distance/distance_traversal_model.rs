use uom::si::f64::Length;
use uom::ConstZero;

use crate::algorithm::search::SearchGraph;
use crate::model::network::{Edge, Vertex};
use crate::model::state::StateModel;
use crate::model::state::StateVariable;
use crate::model::state::{InputFeature, StateVariableConfig};
use crate::model::traversal::default::fieldname;
use crate::model::traversal::traversal_model::TraversalModel;
use crate::model::traversal::traversal_model_error::TraversalModelError;
use crate::model::unit::DistanceUnit;
use crate::util::geo::haversine;

/// a model for traversing edges based on distance.
pub struct DistanceTraversalModel {
    pub distance_unit: DistanceUnit,
    pub include_trip_distance: bool,
}

impl DistanceTraversalModel {
    pub fn new(distance_unit: DistanceUnit, include_trip_distance: bool) -> DistanceTraversalModel {
        Self {
            distance_unit,
            include_trip_distance,
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
        trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let (_, edge, _) = trajectory;

        state_model.add_distance(state, fieldname::EDGE_DISTANCE, &edge.distance)?;
        if self.include_trip_distance {
            state_model.add_distance(state, fieldname::TRIP_DISTANCE, &edge.distance)?;
        }
        Ok(())
    }

    /// uses a haversine distance to estimate the distance between two vertices.
    fn estimate_traversal(
        &self,
        od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let (src, dst) = od;
        let distance =
            haversine::coord_distance(&src.coordinate, &dst.coordinate).map_err(|e| {
                TraversalModelError::TraversalModelFailure(format!(
                    "could not compute haversine distance between {src} and {dst}: {e}"
                ))
            })?;
        if self.include_trip_distance {
            state_model.add_distance(state, fieldname::TRIP_DISTANCE, &distance)?;
        }
        state_model.add_distance(state, fieldname::EDGE_DISTANCE, &distance)?;
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
                output_unit: Some(self.distance_unit),
            },
        )];
        if self.include_trip_distance {
            features.push((
                String::from(fieldname::TRIP_DISTANCE),
                StateVariableConfig::Distance {
                    initial: Length::ZERO,
                    accumulator: true,
                    output_unit: Some(self.distance_unit),
                },
            ));
        }
        features
    }
}
