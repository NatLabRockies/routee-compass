use crate::{
    algorithm::search::{EdgeTraversal, SearchTree},
    model::label::Label,
};

/// during the search tree insert, a behavior for determining how we perform insert.
pub enum TrajectoryInsertBehavior {
    /// the previous label exists and its cost is dominant. cancel insertion.
    CancelInsertion,
    /// no previous label exists, we perform a full label + node insert
    InsertLabelAndNode,
    /// the previous label exists but its cost is dominated. insert new node
    /// for this trajectory.
    InsertNode,
}

impl TrajectoryInsertBehavior {
    /// pick the type of insert to run based on the existence of this label in the tree.
    pub fn new(
        tree: &mut SearchTree,
        next_label: &Label,
        traversal: &EdgeTraversal,
    ) -> TrajectoryInsertBehavior {
        match tree.get(next_label) {
            None => TrajectoryInsertBehavior::InsertLabelAndNode,
            Some(node) => {
                // node.traversal_cost of None means node is root, which by definition as a cost of zero
                let prev_cost = node
                    .traversal_cost()
                    .map(|tc| tc.objective_cost)
                    .unwrap_or_default();
                let next_cost = traversal.cost.objective_cost;
                if next_cost < prev_cost {
                    TrajectoryInsertBehavior::InsertNode
                } else {
                    TrajectoryInsertBehavior::CancelInsertion
                }
            }
        }
    }
}
