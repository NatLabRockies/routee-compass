use super::error::ConstraintModelError;
use crate::model::{
    network::Edge,
    state::{StateModel, StateVariable},
    traversal::EdgeFrontierContext,
};

/// Validates edge and traversal states. Provides an API for removing edges from
/// the frontier in a way that could be more efficient than modifying the [TraversalModel].
/// This may be desireable when a traversal model has complex cost logic but an edge
/// may not be traversable for this query, such as due to height restrictions.
///
/// [TraversalModel]: crate::model::traversal::model::TraversalModel
pub trait ConstraintModel: Send + Sync {
    /// Validates an edge before allowing it to be added to the search frontier.
    ///
    /// # Arguments
    ///
    /// * `ctx` - the traversal context
    /// * `state` - the state of the traversal at the beginning of this edge
    /// * `state_model` - provides operations on the state vector
    ///
    /// # Returns
    ///
    /// True if the edge is a valid part of the frontier, false otherwise
    fn valid_frontier(
        &self,
        ctx: &EdgeFrontierContext,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError>;

    /// Validates an edge independent of a search state, noting whether it
    /// is simply impassable with this ConstraintModel configuration. Can be
    /// called by valid_frontier as a cheaper first-pass operation. Also
    /// used by MapModel during query map matching.
    ///
    /// # Arguments
    ///
    /// * `edge` - the edge to test for validity
    ///
    /// # Returns
    ///
    /// True if the edge is valid
    fn valid_edge(&self, edge: &Edge) -> Result<bool, ConstraintModelError>;
}
