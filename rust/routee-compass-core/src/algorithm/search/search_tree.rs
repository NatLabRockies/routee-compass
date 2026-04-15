use super::TreeStorage;
use super::{EdgeTraversal, SearchTreeNode};
use crate::model::network::{EdgeId, EdgeListId, Graph, NetworkError, VertexId};
use crate::model::unit::Cost;
use crate::{algorithm::search::Direction, model::label::Label};
use allocative::Allocative;
use std::collections::HashSet;
use std::sync::Arc;

/// A search tree that supports efficient lookups and bi-directional parent/child traversal
/// Designed for route planning algorithms that need both indexing and backtracking capabilities
#[derive(Clone, Debug, Allocative)]
pub struct SearchTree {
    storage: TreeStorage,
    /// The root node (None if empty tree)
    root: Option<Label>,
    /// Tree orientation for bi-directional search support
    direction: Direction,
}

impl Default for SearchTree {
    fn default() -> Self {
        Self::new_stateful(Direction::Forward)
    }
}

impl SearchTree {
    /// Create a new empty search tree with the specified orientation that
    /// supports Labels that contain state.
    pub fn new_stateful(direction: Direction) -> Self {
        Self {
            storage: TreeStorage::new_stateful(),
            root: None,
            direction,
        }
    }

    /// Create a new empty search tree with the specified orientation that
    /// supports Labels that have no state ([Label::Vertex]).
    pub fn new_vertex_oriented(direction: Direction) -> Self {
        Self {
            storage: TreeStorage::new_vertex_oriented(),
            root: None,
            direction,
        }
    }

    /// Create a new search tree with the given root node.
    pub fn with_root(root_label: Label, orientation: Direction) -> Result<Self, SearchTreeError> {
        let mut tree = match root_label {
            Label::Vertex(_) => Self::new_vertex_oriented(orientation),
            _ => Self::new_stateful(orientation),
        };
        tree.set_root(root_label)?;
        Ok(tree)
    }

    /// Set the root node of the tree
    pub fn set_root(&mut self, root_label: Label) -> Result<(), SearchTreeError> {
        // If the tree is completely empty, upgrade/morph the storage into the correct variant
        if self.is_empty() {
            self.storage = match root_label {
                Label::Vertex(_) => TreeStorage::new_vertex_oriented(),
                _ => TreeStorage::new_stateful(),
            };
        }
        let root_node = SearchTreeNode::new_root(self.direction);
        self.storage.insert_node(root_label.clone(), root_node)?;
        self.root = Some(root_label);
        Ok(())
    }

    /// Insert the trajectory (parent) -[edge]-> (child) as a node in the tree. it is
    /// assumed that the incoming edge traversal has already been deemed dominant over
    /// any matching trajectory already existing in the tree.
    pub fn insert_trajectory(
        &mut self,
        parent_label: Label,
        edge_traversal: EdgeTraversal,
        child_label: Label,
    ) -> Result<(), SearchTreeError> {
        // Verify parent exists - special case on empty tree
        // If parent doesn't exist but tree is empty, make parent the root
        if !self.storage.contains_key(&parent_label) {
            if self.is_empty() {
                self.set_root(parent_label.clone())?;
            } else {
                return Err(SearchTreeError::ParentNotFound(parent_label));
            }
        }
        let new_node =
            SearchTreeNode::new_child(edge_traversal, parent_label.clone(), self.direction);
        self.storage.insert_node(child_label, new_node)?;

        Ok(())
    }

    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (Label, &'a SearchTreeNode)> + 'a> {
        Box::new(self.storage.branch_iter())
    }

    pub fn keys<'a>(&'a self) -> Box<dyn Iterator<Item = Label> + 'a> {
        Box::new(self.storage.label_iter())
    }

    pub fn values<'a>(&'a self) -> Box<dyn Iterator<Item = &'a SearchTreeNode> + 'a> {
        Box::new(self.storage.node_iter())
    }

    /// Get a node by its label
    pub fn get(&self, label: &Label) -> Option<&SearchTreeNode> {
        self.storage.get(label)
    }

    /// gets the label with the minimum accumulated tree cost associated with the
    /// destination vertex of a search.
    pub fn get_min_accumulated_cost_label(
        &self,
        destination_vertex: VertexId,
    ) -> Result<(Label, Cost), SearchTreeError> {
        let labels: Vec<Label> = self.get_labels(destination_vertex).collect();
        if labels.is_empty() {
            return Err(SearchTreeError::VertexNotFound(destination_vertex));
        }

        let mut min_cost: Option<(Label, Cost)> = None;
        for label in labels.iter() {
            // performs tree backtracking to calculate total objective cost for this label
            let cost = self.calculate_total_objective_cost(label)?;
            let improves_cost = min_cost.as_ref().map(|(_, c)| cost < *c).unwrap_or(true);
            if improves_cost {
                min_cost = Some((label.clone(), cost));
            }
        }

        min_cost.ok_or(SearchTreeError::MissingLabelsForDestinationVertex(
            destination_vertex,
        ))
    }

    /// Find labels for the given vertex ID
    pub fn get_labels(&self, vertex: VertexId) -> Box<dyn Iterator<Item = Label> + '_> {
        self.storage.get_labels(vertex)
    }

    // /// finds a single label by picking the one that is maximal/minimal wrt some comparison function.
    // /// for most cases, using the method get_min_cost_label is the correct choice.
    // ///
    // /// # Arguments
    // ///
    // /// * `vertex` - the vertex expected to match some label
    // /// * `compare` - a comparison function
    // /// * `min` - if true, find the minimal value according to the ordering function F, otherwise, the max
    // pub fn get_label_by<F>(&self, vertex: VertexId, mut compare: F, min: bool) -> Option<&Label>
    // where
    //     F: FnMut(&(&Label, Option<&EdgeTraversal>)) -> OrderedFloat<f64>,
    // {
    //     let label_edge_iter = self.get_labels(vertex).filter_map(|label| {
    //         let (stored_label, node) = self.nodes.get_key_value(&label)?;
    //         let edge_traversal = node.incoming_edge();
    //         Some((stored_label, edge_traversal))
    //     });

    //     let found = if min {
    //         label_edge_iter.min_by_key(|item| compare(item))
    //     } else {
    //         label_edge_iter.max_by_key(|item| compare(item))
    //     };

    //     found.map(|(label, _)| label)
    // }

    /// walk up the tree from the given label to the root, summing the marginal objective costs.
    /// fails if the tree is malformed by counting labels visited and comparing to the count of
    /// [Label]s in the tree.
    ///
    /// # Arguments
    /// * `label` - trip destination label to backtrack from and calculate cost
    pub fn calculate_total_objective_cost(&self, label: &Label) -> Result<Cost, SearchTreeError> {
        let mut total = Cost::ZERO;
        let mut current_label = label;
        let mut steps: usize = 0;
        let max_steps = self.storage.len();

        loop {
            // Guard against infinite loops in malformed trees
            if steps > max_steps {
                return Err(SearchTreeError::InvalidBranchStructure(format!(
                    "Cycle detected: exceeded tree size {} while calculating cost from {}",
                    max_steps, label
                )));
            }

            let node = self
                .get(current_label)
                .ok_or_else(|| SearchTreeError::LabelNotFound(current_label.clone()))?;

            match node {
                SearchTreeNode::Root { .. } => break,
                SearchTreeNode::Branch {
                    incoming_edge,
                    parent,
                    ..
                } => {
                    // add this objective cost and update the node cursor to the branch's parent
                    total += incoming_edge.cost.objective_cost;
                    current_label = parent;
                }
            }

            steps += 1;
        }

        Ok(total)
    }

    /// Get a mutable reference to a node by its label
    pub fn get_mut(&mut self, label: &Label) -> Option<&mut SearchTreeNode> {
        self.storage.get_mut(label)
    }

    /// Get the root label
    pub fn root(&self) -> Option<&Label> {
        self.root.as_ref()
    }

    /// Get the parent of a node
    pub fn get_parent(&self, label: &Label) -> Option<&SearchTreeNode> {
        let node = self.get(label)?;
        let parent_label = node.parent_label()?;
        self.get(parent_label)
    }

    /// Check if the tree contains a node with the given label
    pub fn contains(&self, label: &Label) -> bool {
        self.storage.contains_key(label)
    }

    /// Get the number of nodes in the tree
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Get the tree orientation
    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Backtrack from a leaf vertex to construct a path using the tree's inherent direction
    /// and limit the backtracking depth to some nonzero count of edges.
    ///
    /// # Arguments
    /// * `leaf_vertex` - The vertex ID to backtrack from. this should be the destination vertex `dst` of the graph triplet (src)-[edge]->(dst).
    /// * `depth` - max number of edges to find for the path starting at leaf_vertex
    ///
    /// # Returns
    /// A path of EdgeTraversals from root to leaf (forward) or leaf to root (reverse)
    pub fn backtrack_with_depth(
        &self,
        leaf_vertex: VertexId,
        depth: u64,
    ) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        let (target_label, _) = self.get_min_accumulated_cost_label(leaf_vertex)?;

        self.reconstruct_path(&target_label, Some(depth))
    }

    /// Backtrack from a leaf vertex to construct a path using the tree's inherent direction
    ///
    /// # Arguments
    /// * `leaf_vertex` - The vertex ID to backtrack from
    ///
    /// # Returns
    /// A path of EdgeTraversals from root to leaf (forward) or leaf to root (reverse)
    pub fn backtrack(&self, leaf_vertex: VertexId) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        let (target_label, _) = self.get_min_accumulated_cost_label(leaf_vertex)?;

        self.reconstruct_path(&target_label, None)
    }

    /// convenience method to backtrack from some label.
    pub fn backtrack_label(&self, label: &Label) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        self.reconstruct_path(label, None)
    }

    /// convenience method to backtrack from some label to some depth.
    pub fn backtrack_label_with_depth(
        &self,
        label: &Label,
        depth: u64,
    ) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        self.reconstruct_path(label, Some(depth))
    }

    /// backtrack for edge-oriented search, begins from source vertex of target edge.
    pub fn backtrack_edge_oriented_route(
        &self,
        target: (EdgeListId, EdgeId),
        graph: Arc<Graph>,
    ) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        let (d_el, d_e) = target;
        let d_v = graph.src_vertex_id(&d_el, &d_e)?;
        self.backtrack(d_v)
    }

    /// Reconstruct a path from root to the given target label
    /// This is the primary backtracking method for route reconstruction
    /// If depth is provided, the path will be limited to a specified number of EdgeTraversals.
    pub fn reconstruct_path(
        &self,
        target_label: &Label,
        depth: Option<u64>,
    ) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        let mut path = Vec::new();
        let mut current_label = target_label;
        let mut steps: u64 = 0;
        let mut visited = HashSet::new();

        // Walk up from target to root
        loop {
            // detect cycles
            if !visited.insert(current_label.clone()) {
                return Err(SearchTreeError::InvalidBranchStructure(format!(
                    "Cycle detected at label: {}",
                    current_label
                )));
            }

            // extra sanity check which should never be true given the cycle
            // check above, but, we always want to be defensive against infinite loops.
            if steps > self.storage.len() as u64 {
                return Err(SearchTreeError::InvalidBranchStructure(format!(
                    "Exceeded tree size {} while backtracking from {}",
                    self.storage.len(),
                    target_label
                )));
            }

            let exceeds_depth = depth.map(|l| steps >= l).unwrap_or_default();
            if exceeds_depth {
                break;
            }

            let predecessor = self.predecessor(current_label)?;
            match predecessor {
                Some((incoming_edge, parent)) => {
                    path.push(incoming_edge.clone());
                    current_label = parent;
                }
                None => break,
            }

            steps += 1;
        }

        // For forward search, reverse the path to go from root to target
        // For reverse search, keep the path as-is (it's already from target to source)
        match self.direction {
            Direction::Forward => {
                path.reverse();
                Ok(path)
            }
            Direction::Reverse => Ok(path),
        }
    }

    /// get the edge traversal and parent [Label] that leads to some child [Label], aka,
    /// where (parent) -[incoming_edge]-> (child)
    pub fn predecessor(
        &self,
        child: &Label,
    ) -> Result<Option<(&EdgeTraversal, &Label)>, SearchTreeError> {
        let current_node = self
            .get(child)
            .ok_or_else(|| SearchTreeError::LabelNotFound(child.clone()))?;

        match current_node {
            SearchTreeNode::Root { .. } => Ok(None),
            SearchTreeNode::Branch {
                incoming_edge,
                parent,
                ..
            } => Ok(Some((incoming_edge, parent))),
        }
    }

    /// Get all labels in the tree
    pub fn labels(&self) -> impl Iterator<Item = Label> + '_ {
        self.storage.label_iter()
    }

    /// Get all nodes in the tree
    pub fn nodes(&self) -> impl Iterator<Item = &SearchTreeNode> {
        self.storage.node_iter()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SearchTreeError {
    #[error("parent not found for label {0}")]
    ParentNotFound(Label),
    #[error("Label not found in tree: {0}")]
    LabelNotFound(Label),
    #[error("Label '{0}' exists in tree without matching SearchTreeNode")]
    MissingNodeForLabel(Label),
    #[error("Destination vertex '{0}' has no labels at end of search")]
    MissingLabelsForDestinationVertex(VertexId),
    #[error("Node is missing parent reference: {0}")]
    MissingParent(Label),
    #[error("Invalid branch structure: {0}")]
    InvalidBranchStructure(String),
    #[error("Vertex not found in tree: {0}")]
    VertexNotFound(VertexId),
    #[error("while backtracking from '{0}', {1}")]
    BacktrackingError(Label, String),
    #[error("Search tree error while interacting with Graph: {source}")]
    NetworkError {
        #[from]
        source: NetworkError,
    },
    #[error("LabelModel must produce a consistent Label type, but produced {1} after initially producing {0}")]
    HeterogeneousLabelTypes(String, String),
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::model::{
        cost::TraversalCost,
        network::{EdgeId, EdgeListId, VertexId},
        unit::Cost,
    };

    #[test]
    fn test_new_empty_tree() {
        let tree = SearchTree::new_stateful(Direction::Forward);
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.direction(), Direction::Forward);
        assert!(tree.root().is_none());
    }

    #[test]
    fn test_tree_with_root() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        assert!(!tree.is_empty());
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.root(), Some(&root_label));
        assert!(tree.contains(&root_label));

        let root_node = tree.get(&root_label).unwrap();
        assert!(root_node.is_root());
    }

    #[test]
    fn test_insert_child_nodes() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Insert first child
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        // Insert second child
        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(
            root_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        assert_eq!(tree.len(), 3);

        // Verify child nodes
        let child1_node = tree.get(&child1_label).unwrap();
        assert!(!child1_node.is_root());
        assert_eq!(child1_node.parent_label(), Some(&root_label));
        assert_eq!(child1_node.incoming_edge().unwrap().edge_id, EdgeId(1));

        let child2_node = tree.get(&child2_label).unwrap();
        assert!(!child2_node.is_root());
        assert_eq!(child2_node.parent_label(), Some(&root_label));
        assert_eq!(child2_node.incoming_edge().unwrap().edge_id, EdgeId(2));
    }

    #[test]
    fn test_insert_with_nonexistent_parent() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label, Direction::Forward).unwrap();

        let child_label = create_test_label(1);
        let child_traversal = create_test_edge_traversal(1, 10.0);
        let nonexistent_parent = create_test_label(99);

        let result =
            tree.insert_trajectory(nonexistent_parent.clone(), child_traversal, child_label);
        assert!(matches!(result, Err(SearchTreeError::ParentNotFound(_))));
    }

    #[test]
    fn test_get_parent() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        let child_label = create_test_label(1);
        let child_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child_traversal, child_label.clone())
            .unwrap();

        // Root has no parent
        assert!(tree.get_parent(&root_label).is_none());

        // Child has root as parent
        let parent = tree.get(&child_label).unwrap().parent_label().unwrap();
        assert_eq!(parent, &root_label);
    }

    #[test]
    fn test_reconstruct_path_forward_orientation() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(
            child2_label.clone(),
            child3_traversal.clone(),
            child3_label.clone(),
        )
        .unwrap();

        // Reconstruct path to child3
        let path = tree.reconstruct_path(&child3_label, None).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].edge_id, EdgeId(1)); // root -> 1
        assert_eq!(path[1].edge_id, EdgeId(2)); // 1 -> 2
        assert_eq!(path[2].edge_id, EdgeId(3)); // 2 -> 3
    }

    #[test]
    fn test_reconstruct_path_reverse_orientation() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(
            child2_label.clone(),
            child3_traversal.clone(),
            child3_label.clone(),
        )
        .unwrap();

        // Reconstruct path to child3 (reverse orientation keeps natural order)
        let path = tree.reconstruct_path(&child3_label, None).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].edge_id, EdgeId(3)); // 3 -> 2
        assert_eq!(path[1].edge_id, EdgeId(2)); // 2 -> 1
        assert_eq!(path[2].edge_id, EdgeId(1)); // 1 -> root
    }

    #[test]
    fn test_reconstruct_path_nonexistent_label() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label, Direction::Forward).unwrap();

        let nonexistent_label = create_test_label(99);
        let result = tree.reconstruct_path(&nonexistent_label, None);
        assert!(matches!(result, Err(SearchTreeError::LabelNotFound(_))));
    }

    #[test]
    fn test_iterators() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(root_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        // Test labels iterator
        let labels: HashSet<_> = tree.labels().collect();
        assert_eq!(labels.len(), 3);
        assert!(labels.contains(&root_label));
        assert!(labels.contains(&child1_label));
        assert!(labels.contains(&child2_label));

        // Test nodes iterator
        let node_count = tree.nodes().count();
        assert_eq!(node_count, 3);

        let vertex_ids: HashSet<_> = tree.labels().map(|l| *l.vertex_id()).collect();
        assert_eq!(vertex_ids.len(), 3);
        assert!(vertex_ids.contains(&VertexId(0)));
        assert!(vertex_ids.contains(&VertexId(1)));
        assert!(vertex_ids.contains(&VertexId(2)));
    }

    #[test]
    fn test_backtrack_forward_tree() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(
            child2_label.clone(),
            child3_traversal.clone(),
            child3_label.clone(),
        )
        .unwrap();

        // Backtrack from vertex 3 using tree's inherent direction (Forward)
        let path = tree.backtrack(VertexId(3)).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].edge_id, EdgeId(1)); // root -> 1
        assert_eq!(path[1].edge_id, EdgeId(2)); // 1 -> 2
        assert_eq!(path[2].edge_id, EdgeId(3)); // 2 -> 3
    }

    #[test]
    fn test_backtrack_reverse_tree() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(
            child2_label.clone(),
            child3_traversal.clone(),
            child3_label.clone(),
        )
        .unwrap();

        // Backtrack from vertex 3 using tree's inherent direction (Reverse)
        let path = tree.backtrack(VertexId(3)).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].edge_id, EdgeId(3)); // 3 -> 2
        assert_eq!(path[1].edge_id, EdgeId(2)); // 2 -> 1
        assert_eq!(path[2].edge_id, EdgeId(1)); // 1 -> root
    }

    #[test]
    fn test_backtrack_nonexistent_vertex() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label, Direction::Forward).unwrap();

        let result = tree.backtrack(VertexId(99));
        assert!(matches!(
            result,
            Err(SearchTreeError::VertexNotFound(VertexId(99)))
        ));
    }

    #[test]
    fn test_backtrack_root_vertex() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Backtracking from root should return empty path
        let path = tree.backtrack(VertexId(0)).unwrap();
        assert_eq!(path.len(), 0);
    }

    // #[test]
    // fn test_find_label_for_vertex() {
    //     let root_label = create_test_label(0);
    //     let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

    //     let child1_label = create_test_label(1);
    //     let child1_traversal = create_test_edge_traversal(1, 10.0);
    //     tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
    //         .unwrap();

    //     // Test finding existing vertex
    //     let found_label = tree.get_min_accumulated_cost_label(VertexId(1));
    //     assert_eq!(found_label, Some(&child1_label));

    //     // Test finding non-existent vertex
    //     let not_found = tree.get_min_accumulated_cost_label(VertexId(99));
    //     assert_eq!(not_found, None);
    // }

    #[test]
    fn test_auto_root_creation() {
        let mut tree = SearchTree::new_stateful(Direction::Forward);
        assert!(tree.is_empty());
        assert!(tree.root().is_none());

        // Insert first node - parent should become root automatically
        let parent_label = create_test_label(0);
        let child_label = create_test_label(1);
        let edge_traversal = create_test_edge_traversal(1, 10.0);

        tree.insert_trajectory(
            parent_label.clone(),
            edge_traversal.clone(),
            child_label.clone(),
        )
        .unwrap();

        // Verify root was created automatically
        assert!(!tree.is_empty());
        assert_eq!(tree.len(), 2); // root + child
        assert_eq!(tree.root(), Some(&parent_label));

        // Verify structure
        let root_node = tree.get(&parent_label).unwrap();
        assert!(root_node.is_root());
        // Verify automatic root creation logic via label presence in nodes map
        assert!(tree.storage.contains_key(&parent_label));

        let child_node = tree.get(&child_label).unwrap();
        assert!(!child_node.is_root());
        assert_eq!(child_node.parent_label(), Some(&parent_label));
        assert_eq!(child_node.incoming_edge().unwrap().edge_id, EdgeId(1));
    }

    #[test]
    fn test_auto_root_creation_chain() {
        let mut tree = SearchTree::new_stateful(Direction::Forward);

        // Build a chain: 0 -> 1 -> 2 -> 3 by only calling insert
        let label0 = create_test_label(0);
        let label1 = create_test_label(1);
        let label2 = create_test_label(2);
        let label3 = create_test_label(3);

        // First insert creates root automatically
        tree.insert_trajectory(
            label0.clone(),
            create_test_edge_traversal(1, 10.0),
            label1.clone(),
        )
        .unwrap();

        // Subsequent inserts work normally
        tree.insert_trajectory(
            label1.clone(),
            create_test_edge_traversal(2, 15.0),
            label2.clone(),
        )
        .unwrap();
        tree.insert_trajectory(
            label2.clone(),
            create_test_edge_traversal(3, 20.0),
            label3.clone(),
        )
        .unwrap();

        // Verify final structure
        assert_eq!(tree.len(), 4);
        assert_eq!(tree.root(), Some(&label0));

        // Verify backtracking works
        let path = tree.backtrack(VertexId(3)).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].edge_id, EdgeId(1)); // 0 -> 1
        assert_eq!(path[1].edge_id, EdgeId(2)); // 1 -> 2
        assert_eq!(path[2].edge_id, EdgeId(3)); // 2 -> 3
    }

    #[test]
    fn test_insert_without_auto_root_when_parent_exists() {
        let mut tree = SearchTree::new_stateful(Direction::Forward);
        let root_label = create_test_label(0);

        // Manually create root first
        tree.set_root(root_label.clone()).unwrap();

        // Insert should work normally without creating a new root
        let child_label = create_test_label(1);
        let edge_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), edge_traversal, child_label.clone())
            .unwrap();

        // Root should still be the same
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.root(), Some(&root_label));

        // Trying to insert with non-existent parent should still fail
        let orphan_label = create_test_label(99);
        let nonexistent_parent = create_test_label(999);
        let result = tree.insert_trajectory(
            orphan_label,
            create_test_edge_traversal(99, 5.0),
            nonexistent_parent.clone(),
        );
        assert!(matches!(result, Err(SearchTreeError::ParentNotFound(_))));
    }

    #[test]
    fn test_backtrack_with_depth_forward_tree_full_path() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert_trajectory(child3_label.clone(), child4_traversal, child4_label.clone())
            .unwrap();

        // Backtrack with depth equal to total path length
        let path = tree.backtrack_with_depth(VertexId(4), 4).unwrap();

        assert_eq!(path.len(), 4);
        assert_eq!(path[0].edge_id, EdgeId(1)); // root -> 1
        assert_eq!(path[1].edge_id, EdgeId(2)); // 1 -> 2
        assert_eq!(path[2].edge_id, EdgeId(3)); // 2 -> 3
        assert_eq!(path[3].edge_id, EdgeId(4)); // 3 -> 4
    }

    #[test]
    fn test_backtrack_with_depth_forward_tree_limited_depth() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert_trajectory(child3_label.clone(), child4_traversal, child4_label.clone())
            .unwrap();

        // Backtrack with depth less than total path length
        let path = tree.backtrack_with_depth(VertexId(4), 2).unwrap();

        // Should only get the last 2 edges (limited by depth)
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].edge_id, EdgeId(3)); // 2 -> 3
        assert_eq!(path[1].edge_id, EdgeId(4)); // 3 -> 4
    }

    #[test]
    fn test_backtrack_with_depth_forward_tree_depth_one() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        // Backtrack with depth of 1
        let path = tree.backtrack_with_depth(VertexId(3), 1).unwrap();

        // Should only get the last edge
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].edge_id, EdgeId(3)); // 2 -> 3
    }

    #[test]
    fn test_backtrack_with_depth_reverse_tree_full_path() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert_trajectory(child3_label.clone(), child4_traversal, child4_label.clone())
            .unwrap();

        // Backtrack with depth equal to total path length (reverse orientation)
        let path = tree.backtrack_with_depth(VertexId(4), 4).unwrap();

        assert_eq!(path.len(), 4);
        // In reverse orientation, path is not reversed, so it goes from target to root
        assert_eq!(path[0].edge_id, EdgeId(4)); // 4 -> 3
        assert_eq!(path[1].edge_id, EdgeId(3)); // 3 -> 2
        assert_eq!(path[2].edge_id, EdgeId(2)); // 2 -> 1
        assert_eq!(path[3].edge_id, EdgeId(1)); // 1 -> root
    }

    #[test]
    fn test_backtrack_with_depth_reverse_tree_limited_depth() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse).unwrap();

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert_trajectory(child3_label.clone(), child4_traversal, child4_label.clone())
            .unwrap();

        // Backtrack with depth less than total path length
        let path = tree.backtrack_with_depth(VertexId(4), 2).unwrap();

        // Should only get the first 2 edges from the target
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].edge_id, EdgeId(4)); // 4 -> 3
        assert_eq!(path[1].edge_id, EdgeId(3)); // 3 -> 2
    }

    #[test]
    fn test_backtrack_with_depth_from_root() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Backtracking from root with any depth should return empty path
        let path = tree.backtrack_with_depth(VertexId(0), 5).unwrap();
        assert_eq!(path.len(), 0);
    }

    #[test]
    fn test_backtrack_with_depth_nonexistent_vertex() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label, Direction::Forward).unwrap();

        let result = tree.backtrack_with_depth(VertexId(99), 1);
        assert!(matches!(
            result,
            Err(SearchTreeError::VertexNotFound(VertexId(99)))
        ));
    }

    #[test]
    fn test_backtrack_with_depth_exceeds_available_path() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Build a short path: 0 -> 1 -> 2
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        // Request more depth than available
        let path = tree.backtrack_with_depth(VertexId(2), 10).unwrap();

        // Should return the entire available path (2 edges)
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].edge_id, EdgeId(1)); // root -> 1
        assert_eq!(path[1].edge_id, EdgeId(2)); // 1 -> 2
    }

    #[test]
    fn test_backtrack_with_depth_branching_tree() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        // Build a branching tree:
        //     0
        //   /   \
        //  1     2
        //  |     |
        //  3     4
        //        |
        //        5

        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert_trajectory(root_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert_trajectory(child1_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert_trajectory(child2_label.clone(), child4_traversal, child4_label.clone())
            .unwrap();

        let child5_label = create_test_label(5);
        let child5_traversal = create_test_edge_traversal(5, 30.0);
        tree.insert_trajectory(child4_label.clone(), child5_traversal, child5_label.clone())
            .unwrap();

        // Test backtrack from leaf node 3 with depth 1
        let path = tree.backtrack_with_depth(VertexId(3), 1).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].edge_id, EdgeId(3)); // 1 -> 3

        // Test backtrack from leaf node 5 with depth 2
        let path = tree.backtrack_with_depth(VertexId(5), 2).unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0].edge_id, EdgeId(4)); // 2 -> 4
        assert_eq!(path[1].edge_id, EdgeId(5)); // 4 -> 5

        // Test backtrack from leaf node 5 with full depth
        let path = tree.backtrack_with_depth(VertexId(5), 3).unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].edge_id, EdgeId(2)); // root -> 2
        assert_eq!(path[1].edge_id, EdgeId(4)); // 2 -> 4
        assert_eq!(path[2].edge_id, EdgeId(5)); // 4 -> 5
    }

    fn create_test_edge_traversal(edge_id: usize, cost: f64) -> EdgeTraversal {
        EdgeTraversal {
            edge_id: EdgeId(edge_id),
            edge_list_id: EdgeListId(0),
            cost: TraversalCost::mock(Cost::new(cost), Cost::new(cost)),
            result_state: vec![],
        }
    }

    fn create_test_label(vertex_id: usize) -> Label {
        Label::Vertex(VertexId(vertex_id))
    }

    #[test]
    fn test_backtrack_mixed_labels_bug() {
        // Reproduction of bug where mixed label types cause Label::Vertex lookup to fail
        // Now, we strictly forbid mixed label types in the tree.
        let mut tree = SearchTree::new_stateful(Direction::Forward);

        let root_label = Label::Vertex(VertexId(0));
        let child_label = Label::VertexWithIntState {
            vertex_id: VertexId(1),
            state: 1,
        };

        let result = tree.insert_trajectory(
            root_label.clone(),
            create_test_edge_traversal(1, 10.0),
            child_label.clone(),
        );

        assert!(
            matches!(result, Err(SearchTreeError::HeterogeneousLabelTypes(_, _))),
            "Tree must reject heterogeneous label types"
        );
    }

    // #[test]
    // fn test_get_incoming_edge() {
    //     // Test the optimized get_incoming_edge method
    //     let root_label = create_test_label(0);
    //     let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

    //     // Build a linear path: 0 -> 1 -> 2 -> 3
    //     let child1_label = create_test_label(1);
    //     let child1_traversal = create_test_edge_traversal(1, 10.0);
    //     tree.insert_trajectory(root_label.clone(), child1_traversal, child1_label.clone())
    //         .unwrap();

    //     let child2_label = create_test_label(2);
    //     let child2_traversal = create_test_edge_traversal(2, 15.0);
    //     tree.insert_trajectory(child1_label.clone(), child2_traversal, child2_label.clone())
    //         .unwrap();

    //     let child3_label = create_test_label(3);
    //     let child3_traversal = create_test_edge_traversal(3, 20.0);
    //     tree.insert_trajectory(child2_label.clone(), child3_traversal, child3_label.clone())
    //         .unwrap();

    //     // Test: get incoming edge for vertex 1 (should be edge 1: 0->1)
    //     let edge1 = tree.get_incoming_edge(VertexId(1));
    //     assert!(edge1.is_some());
    //     assert_eq!(edge1.unwrap().edge_id, EdgeId(1));

    //     // Test: get incoming edge for vertex 2 (should be edge 2: 1->2)
    //     let edge2 = tree.get_incoming_edge(VertexId(2));
    //     assert!(edge2.is_some());
    //     assert_eq!(edge2.unwrap().edge_id, EdgeId(2));

    //     // Test: get incoming edge for vertex 3 (should be edge 3: 2->3)
    //     let edge3 = tree.get_incoming_edge(VertexId(3));
    //     assert!(edge3.is_some());
    //     assert_eq!(edge3.unwrap().edge_id, EdgeId(3));

    //     // Test: root has no incoming edge
    //     let edge_root = tree.get_incoming_edge(VertexId(0));
    //     assert!(edge_root.is_none());

    //     // Test: nonexistent vertex returns None
    //     let edge_none = tree.get_incoming_edge(VertexId(99));
    //     assert!(edge_none.is_none());
    // }

    #[test]
    fn test_reconstruct_path_cycle_detection() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward).unwrap();

        let child_label = create_test_label(1);
        let traversal = create_test_edge_traversal(1, 10.0);

        tree.insert_trajectory(root_label.clone(), traversal.clone(), child_label.clone())
            .unwrap();

        // Force a cycle: make root point back to child, creating a loop
        let bad_root_node =
            SearchTreeNode::new_child(traversal, child_label.clone(), Direction::Forward);
        tree.storage
            .insert_node(root_label.clone(), bad_root_node)
            .unwrap();

        // Attempt backtrack from child which hits root, which bounces back to child
        let result = tree.backtrack(VertexId(1));
        assert!(matches!(
            result,
            Err(SearchTreeError::InvalidBranchStructure(_))
        ));
    }
}
