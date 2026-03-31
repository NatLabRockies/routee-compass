use super::traversal_model_error::TraversalModelError;
use crate::algorithm::search::SearchGraph;
use crate::model::network::{Edge, Vertex};
use crate::model::state::{InputFeature, StateModel, StateVariable, StateVariableConfig};

/// Dictates how state transitions occur while traversing a graph in a search algorithm.
///
/// see the [`super::default`] module for implementations bundled with RouteE Compass:
///   - [DistanceModel]: uses Edge distances to find the route with the shortest distance
///   - [SpeedLookupModel]: retrieves link speeds via lookup from a file
///
/// [DistanceModel]: super::default::distance::DistanceModel
/// [SpeedLookupModel]: super::default::speed_lookup_model::SpeedLookupModel
pub trait TraversalModel: Send + Sync {
    fn name(&self) -> String;
    /// list the state variables required as inputs to this traversal model. for
    /// example, if this traversal model uses a distance metric to compute time, then
    /// it should list the expected distance state variable here.
    fn input_features(&self) -> Vec<InputFeature>;

    /// lists the state variables produced by this traversal model. for example,
    /// if this traversal model produces leg distances, it should specify that here.
    fn output_features(&self) -> Vec<(String, StateVariableConfig)>;

    /// Updates the traversal state by traversing an edge.
    ///
    /// # Arguments
    ///
    /// * `trajectory` - source vertex, edge, and destination vertex
    /// * `state` - state of the search at the beginning of this trajectory
    /// * `tree` - the entire recorded search tree up to the point of traversing this trajectory
    /// * `state_model` - provides access to the state vector
    ///
    /// # Returns
    ///
    /// Either a traversal result or an error.
    fn traverse_edge(
        &self,
        trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        graph: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError>;

    /// Estimates the traversal state by traversing between two vertices without
    /// performing any graph traversals.
    ///
    /// # Arguments
    ///
    /// * `od` - source vertex and destination vertex
    /// * `state` - state of the search at the source vertex
    /// * `tree` - the entire recorded search tree up to the point of estimating this traversal
    /// * `state_model` - provides access to the state vector
    ///
    /// # Returns
    ///
    /// Either a traversal result or an error.
    fn estimate_traversal(
        &self,
        od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        graph: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError>;
}
