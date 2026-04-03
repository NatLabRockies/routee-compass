use serde::{Deserialize, Serialize};

/// sets the policy for assignment of cost in the search, based on the family of
/// search algorithm used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum CostConstraint {
    /// Forces costs to be strictly positive to prevent cycles (e.g., Dijkstra, A*)
    StrictlyPositive,
    /// Allows unconstrained values, including negative costs (e.g., Bellman-Ford)
    Unconstrained,
}
