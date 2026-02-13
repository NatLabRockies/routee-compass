use crate::{
    algorithm::search::{SearchError, SearchTree},
    model::{
        label::Label,
        network::{EdgeId, EdgeListId, VertexId},
        state::StateVariable,
        unit::ReverseCost,
    },
    util::priority_queue::InternalPriorityQueue,
};

pub struct FrontierInstance {
    pub prev_label: Label,
    pub prev_edge: Option<(EdgeListId, EdgeId)>,
    pub prev_state: Vec<StateVariable>,
}

impl FrontierInstance {
    /// creates a new FrontierInstance by popping the next pair from the frontier.
    ///
    /// grabs the previous label, but handle some other termination conditions
    /// based on the state of the priority queue and optional search destination
    /// - we reach the destination                                       (Ok)
    /// - if the set is ever empty and there's no destination            (Ok)
    /// - if the set is ever empty and there's a destination             (Err)
    ///
    /// # Arguments
    /// * `frontier` - queue of priority-ranked labels for exploration
    /// * `source` - search source vertex
    /// * `target` - optional search destination
    /// * `solution` - current working search tree
    /// * `initial_state` - state vector at origin of search
    ///
    /// # Results
    /// A record representing the next label to explore. None if the queue has been exhausted in a search with no
    /// destination, or we have reached our destination.
    /// An error if no path exists for a search that includes a destination.
    pub fn pop_new(
        frontier: &mut InternalPriorityQueue<Label, ReverseCost>,
        source: VertexId,
        target: Option<VertexId>,
        solution: &SearchTree,
        initial_state: &[StateVariable],
    ) -> Result<Option<FrontierInstance>, SearchError> {
        loop {
            match (frontier.pop(), target) {
                (None, Some(target_vertex_id)) => {
                    return Err(SearchError::NoPathExistsBetweenVertices(
                        source,
                        target_vertex_id,
                        solution.len(),
                    ))
                }
                (None, None) => return Ok(None),
                (Some((prev_label, _)), Some(target_v)) if prev_label.vertex_id() == &target_v => {
                    return Ok(None)
                }
                (Some((prev_label, _)), _) => {
                    let node_opt = solution.get(&prev_label);
                    if node_opt.is_none() && !solution.is_empty() {
                        // this label was pruned from the search tree while it was in the frontier.
                        // skip it and continue to the next label in the frontier.
                        continue;
                    }
                    let prev_edge_traversal_opt = node_opt.and_then(|n| n.incoming_edge()).cloned();

                    // grab the current state from the solution, or get initial state if we are at the search root
                    let prev_edge = prev_edge_traversal_opt
                        .as_ref()
                        .map(|et| (et.edge_list_id, et.edge_id));
                    let prev_state = match prev_edge_traversal_opt.as_ref() {
                        None => initial_state.to_vec(),
                        Some(et) => et.result_state.clone(),
                    };

                    let result = FrontierInstance {
                        prev_label,
                        prev_edge,
                        prev_state,
                    };

                    return Ok(Some(result));
                }
            }
        }
    }
}
