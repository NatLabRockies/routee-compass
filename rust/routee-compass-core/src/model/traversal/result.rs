use crate::model::{state::StateVariable, unit::Cost};

/// The state of a search after completing an edge traversal, along
/// with the cost of traversing that edge.
pub struct TraversalResult {
    pub total_cost: Cost,
    pub updated_state: Vec<StateVariable>,
}
