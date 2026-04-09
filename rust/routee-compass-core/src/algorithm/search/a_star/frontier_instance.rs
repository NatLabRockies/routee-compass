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
        match (frontier.pop(), target) {
            (None, Some(target_vertex_id)) => Err(SearchError::NoPathExistsBetweenVertices(
                source,
                target_vertex_id,
                solution.len(),
            )),
            (None, None) => Ok(None),
            (Some((prev_label, _)), Some(target_v)) if prev_label.vertex_id() == &target_v => {
                Ok(None)
            }
            (Some((prev_label, _)), _) => {
                let node_opt = solution.get(&prev_label);
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

                Ok(Some(result))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithm::search::Direction;
    use crate::model::{network::VertexId, unit::Cost};

    #[test]
    fn test_pop_new_empty_queue() {
        let mut frontier = InternalPriorityQueue::default();
        let solution = SearchTree::new(Direction::Forward);
        let initial_state = vec![StateVariable::ZERO];
        let result =
            FrontierInstance::pop_new(&mut frontier, VertexId(0), None, &solution, &initial_state)
                .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pop_new_no_path_exists() {
        let mut frontier = InternalPriorityQueue::default();
        let solution = SearchTree::new(Direction::Forward);
        let initial_state = vec![StateVariable::ZERO];
        let result = FrontierInstance::pop_new(
            &mut frontier,
            VertexId(0),
            Some(VertexId(1)),
            &solution,
            &initial_state,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_pop_new_returns_root_when_tree_empty() {
        let mut frontier = InternalPriorityQueue::default();
        let label = Label::Vertex(VertexId(0));
        frontier.push(label.clone(), ReverseCost::from(Cost::ZERO));
        let solution = SearchTree::new(Direction::Forward);
        let initial_state = vec![StateVariable::ZERO];
        let result =
            FrontierInstance::pop_new(&mut frontier, VertexId(0), None, &solution, &initial_state)
                .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().prev_label, label);
    }

    #[test]
    fn test_pop_new_reaches_target() {
        let mut frontier = InternalPriorityQueue::default();
        let target = VertexId(1);
        let label = Label::Vertex(target);
        frontier.push(label, ReverseCost::from(Cost::ZERO));

        let solution = SearchTree::new(Direction::Forward);
        let initial_state = vec![StateVariable::ZERO];

        // Reaching target vertex should return Ok(None)
        let result = FrontierInstance::pop_new(
            &mut frontier,
            VertexId(0),
            Some(target),
            &solution,
            &initial_state,
        )
        .unwrap();

        assert!(result.is_none());
    }
}
