use super::search_error::SearchError;
use super::SearchInstance;
use crate::algorithm::search::SearchTree;
use crate::model::cost::{CostModel, TraversalCost};
use crate::model::network::{Edge, EdgeId, EdgeListId, Vertex};
use crate::model::state::{StateModel, StateVariable};
use crate::model::traversal::TraversalModel;
use allocative::Allocative;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Debug, Serialize, Deserialize, Allocative)]
pub struct EdgeTraversal {
    pub edge_list_id: EdgeListId,
    pub edge_id: EdgeId,
    pub cost: TraversalCost,
    pub result_state: Vec<StateVariable>,
}

impl Display for EdgeTraversal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "edge {} cost:{} state:{:?}",
            self.edge_id, self.cost.total_cost, self.result_state
        )
    }
}

impl EdgeTraversal {
    /// traverses an edge, possibly after traversing some previous edge,
    /// collecting the access and traversal costs. returns the
    /// accumulated cost and updated search state.
    ///
    /// The traversal and access models for the destination edge list are used
    /// over the following graph trajectory:
    ///   `(v1) -[e1]-> (v2) -[e2]-> (v3)`
    ///     - start_state: at location (v2)
    ///     - next_edge: -[e2]->
    ///     - prev_edge: -[e1]-> (optional, is None at start of a forward search)
    ///     - access model applied from e1 to e2
    ///     - traverse model applied to e2
    ///
    /// # Arguments
    ///
    /// * `next_edge`     - the edge to traverse
    /// * `prev_edge`     - the previously traversed edge, if exists, for access costs
    /// * `prev_state`    - the state before traversal, positioned closer to the destination
    /// * `si`            - the search assets for this query
    ///
    /// # Returns
    ///
    /// An edge traversal summarizing the costs and result state of accessing and traversing the next edge.
    pub fn new(
        next_edge: (EdgeListId, EdgeId),
        tree: &SearchTree,
        prev_state: &[StateVariable],
        si: &SearchInstance,
    ) -> Result<EdgeTraversal, SearchError> {
        // find this traversal in the graph
        let (edge_list_id, edge_id) = next_edge;
        let trajectory = si.graph.edge_triplet(&edge_list_id, &edge_id)?;
        let tm = si.get_traversal_model(&edge_list_id)?;
        Self::new_local(
            trajectory,
            tree,
            prev_state,
            &si.state_model.clone(),
            tm.clone().as_ref(),
            &si.cost_model.clone(),
        )
    }

    /// executes a traversal from some source vertex and source state through some edge,
    /// producing the result state and cost.
    ///
    /// this function signature makes uses lower-level constructs than the associated [`EdgeTraversal::new`]
    /// method and does not require [`Arc`]-wrapped types.
    pub fn new_local(
        trajectory: (&Vertex, &Edge, &Vertex),
        tree: &SearchTree,
        prev_state: &[StateVariable],
        state_model: &StateModel,
        traversal_model: &dyn TraversalModel,
        cost_model: &CostModel,
    ) -> Result<EdgeTraversal, SearchError> {
        let (_, edge, _) = trajectory;
        let mut result_state = state_model.initial_state(Some(prev_state))?;

        traversal_model.traverse_edge(trajectory, &mut result_state, tree, state_model)?;

        let cost =
            cost_model.traversal_cost(trajectory, prev_state, &result_state, tree, state_model)?;

        let result = EdgeTraversal {
            edge_list_id: edge.edge_list_id,
            edge_id: edge.edge_id,
            cost,
            result_state,
        };

        Ok(result)
    }
}
