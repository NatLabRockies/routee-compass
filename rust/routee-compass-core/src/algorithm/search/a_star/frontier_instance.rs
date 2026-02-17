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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithm::search::Direction;
    use crate::model::{
        cost::TraversalCost,
        label::{default::vertex_label_model::VertexLabelModel, LabelModel},
        network::{EdgeId, EdgeListId, VertexId},
        unit::Cost,
    };
    use std::sync::Arc;

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
    fn test_pop_new_skips_pruned_label() {
        let mut frontier = InternalPriorityQueue::default();
        let l1 = Label::Vertex(VertexId(1));
        let l2 = Label::Vertex(VertexId(2));

        // l2 has higher priority (lower cost) but is "pruned" (not in solution tree)
        frontier.push(l2.clone(), ReverseCost::from(Cost::new(5.0)));
        frontier.push(l1.clone(), ReverseCost::from(Cost::new(10.0)));

        let mut solution = SearchTree::new(Direction::Forward);
        let root = Label::Vertex(VertexId(0));
        solution.set_root(root.clone());

        // Add l1 to the tree
        let et = crate::algorithm::search::EdgeTraversal {
            edge_id: EdgeId(0),
            edge_list_id: EdgeListId(0),
            cost: TraversalCost {
                objective_cost: Cost::new(10.0),
                total_cost: Cost::new(10.0),
                ..Default::default()
            },
            result_state: vec![StateVariable::ZERO],
        };
        let label_model: Arc<dyn LabelModel> = Arc::new(VertexLabelModel {});
        solution.insert(root, et, l1.clone(), label_model).unwrap();

        let initial_state = vec![StateVariable::ZERO];

        // Should skip l2 and return l1
        let result =
            FrontierInstance::pop_new(&mut frontier, VertexId(0), None, &solution, &initial_state)
                .unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().prev_label, l1);
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

    #[test]
    fn test_pop_new_skips_pruned_state_label() {
        let mut frontier = InternalPriorityQueue::default();
        let v1 = VertexId(1);
        let l1 = Label::VertexWithIntState {
            vertex_id: v1,
            state: 100,
        };
        let l2 = Label::VertexWithIntState {
            vertex_id: v1,
            state: 50,
        }; // This label will not be in the tree

        // l2 has higher priority (lower cost) but is not in the tree
        frontier.push(l2.clone(), ReverseCost::from(Cost::new(5.0)));
        frontier.push(l1.clone(), ReverseCost::from(Cost::new(10.0)));

        let mut solution = SearchTree::new(Direction::Forward);
        let root = Label::Vertex(VertexId(0));
        solution.set_root(root.clone());

        // Manually insert l1 by using a compatible label model
        let et = crate::algorithm::search::EdgeTraversal {
            edge_id: EdgeId(0),
            edge_list_id: EdgeListId(0),
            cost: TraversalCost {
                objective_cost: Cost::new(10.0),
                total_cost: Cost::new(10.0),
                ..Default::default()
            },
            result_state: vec![StateVariable::ZERO],
        };

        // VertexLabelModel.compare returns Greater, so it won't prune anything.
        // That's fine for this test since we just want to ensure l1 is in the tree and l2 is not.
        let label_model: Arc<dyn LabelModel> = Arc::new(VertexLabelModel {});
        solution.insert(root, et, l1.clone(), label_model).unwrap();

        let initial_state = vec![StateVariable::ZERO];

        // Should skip l2 (not in tree) and return l1
        let result =
            FrontierInstance::pop_new(&mut frontier, VertexId(0), None, &solution, &initial_state)
                .unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap().prev_label, l1);
    }
}
