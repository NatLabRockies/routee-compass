use super::{EdgeTraversal, SearchTreeNode};
use crate::model::network::{EdgeId, EdgeListId, Graph, NetworkError, VertexId};
use crate::model::unit::AsF64;
use crate::{algorithm::search::Direction, model::label::Label};
use allocative::Allocative;
use ordered_float::OrderedFloat;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A search tree that supports efficient lookups and bi-directional parent/child traversal
/// Designed for route planning algorithms that need both indexing and backtracking capabilities
#[derive(Clone, Debug, Allocative)]
pub struct SearchTree {
    /// Fast lookup by label
    nodes: HashMap<Label, SearchTreeNode>,
    /// Fast Label lookup by VertexId
    labels: HashMap<VertexId, HashSet<Label>>,
    /// The root node (None if empty tree)
    root: Option<Label>,
    /// Tree orientation for bi-directional search support
    direction: Direction,
}

impl Default for SearchTree {
    fn default() -> Self {
        Self::new(Direction::Forward)
    }
}

impl SearchTree {
    /// Create a new empty search tree with the specified orientation
    pub fn new(direction: Direction) -> Self {
        Self {
            nodes: HashMap::new(),
            labels: HashMap::new(),
            root: None,
            direction,
        }
    }

    /// Create a new search tree with the given root node.
    pub fn with_root(root_label: Label, orientation: Direction) -> Self {
        let mut tree = Self::new(orientation);
        tree.set_root(root_label);
        tree
    }

    /// Set the root node of the tree
    pub fn set_root(&mut self, root_label: Label) {
        let root_node = SearchTreeNode::new_root(self.direction);
        self.nodes.insert(root_label.clone(), root_node);
        if root_label.needs_vertex_map_storage() {
            self.labels
                .entry(*root_label.vertex_id())
                .and_modify(|l| {
                    let _ = l.insert(root_label.clone());
                })
                .or_insert(HashSet::from([root_label.clone()]));
        }
        self.root = Some(root_label);
    }

    /// Insert the trajectory (parent) -[edge]-> (child) as a node in the tree
    pub fn insert(
        &mut self,
        parent_label: Label,
        edge_traversal: EdgeTraversal,
        child_label: Label,
    ) -> Result<(), SearchTreeError> {
        // Verify parent exists
        // If parent doesn't exist but tree is empty, make parent the root
        if !self.nodes.contains_key(&parent_label) {
            if self.is_empty() {
                self.set_root(parent_label.clone());
            } else {
                return Err(SearchTreeError::ParentNotFound(parent_label));
            }
        }

        // Create the new node
        let new_node =
            SearchTreeNode::new_child(edge_traversal, parent_label.clone(), self.direction);

        // Insert the new node
        self.nodes.insert(child_label.clone(), new_node);
        if child_label.needs_vertex_map_storage() {
            self.labels
                .entry(*child_label.vertex_id())
                .and_modify(|l| {
                    let _ = l.insert(child_label.clone());
                })
                .or_insert(HashSet::from([child_label.clone()]));
        }

        Ok(())
    }

    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (&'a Label, &'a SearchTreeNode)> + 'a> {
        Box::new(self.nodes.iter())
    }

    pub fn keys<'a>(&'a self) -> Box<dyn Iterator<Item = &'a Label> + 'a> {
        Box::new(self.nodes.keys())
    }

    pub fn values<'a>(&'a self) -> Box<dyn Iterator<Item = &'a SearchTreeNode> + 'a> {
        Box::new(self.nodes.values())
    }

    /// Get a node by its label
    pub fn get(&self, label: &Label) -> Option<&SearchTreeNode> {
        self.nodes.get(label)
    }

    /// gets the label with the minimum cost associated with a vertex
    pub fn get_min_cost_label(&self, vertex: VertexId) -> Option<&Label> {
        self.get_label_by(vertex, min_cost_ordering, true)
    }

    /// Find labels for the given vertex ID
    pub fn get_labels(&self, vertex: VertexId) -> Box<dyn Iterator<Item = Label> + '_> {
        // we always perform a lookup for the Vertex label, as it is excluded from the labels map
        let vertex_label = Label::Vertex(vertex);
        let vertex_iter = std::iter::once(vertex_label);

        match self.labels.get(&vertex) {
            Some(labels) => Box::new(vertex_iter.chain(labels.iter().cloned())),
            None => Box::new(vertex_iter),
        }
    }

    /// finds a single label by picking the one that is maximal/minimal wrt some comparison function.
    /// for most cases, using the method get_min_cost_label is the correct choice.
    ///
    /// # Arguments
    ///
    /// * `vertex` - the vertex expected to match some label
    /// * `compare` - a comparison function
    /// * `min` - if true, find the minimal value according to the ordering function F, otherwise, the max
    pub fn get_label_by<F>(&self, vertex: VertexId, mut compare: F, min: bool) -> Option<&Label>
    where
        F: FnMut(&(&Label, Option<&EdgeTraversal>)) -> OrderedFloat<f64>,
    {
        let label_edge_iter = self.get_labels(vertex).filter_map(|label| {
            let (stored_label, node) = self.nodes.get_key_value(&label)?;
            let edge_traversal = node.incoming_edge();
            Some((stored_label, edge_traversal))
        });

        let found = if min {
            label_edge_iter.min_by_key(|item| compare(item))
        } else {
            label_edge_iter.max_by_key(|item| compare(item))
        };

        found.map(|(label, _)| label)
    }

    /// Get a mutable reference to a node by its label
    pub fn get_mut(&mut self, label: &Label) -> Option<&mut SearchTreeNode> {
        self.nodes.get_mut(label)
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
        self.nodes.contains_key(label)
    }

    /// Get the number of nodes in the tree
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
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
        let target_label = self
            .get_label_by(leaf_vertex, min_cost_ordering, true)
            .ok_or(SearchTreeError::VertexNotFound(leaf_vertex))?;

        self.reconstruct_path(target_label, Some(depth))
    }

    /// Backtrack from a leaf vertex to construct a path using the tree's inherent direction
    ///
    /// # Arguments
    /// * `leaf_vertex` - The vertex ID to backtrack from
    ///
    /// # Returns
    /// A path of EdgeTraversals from root to leaf (forward) or leaf to root (reverse)
    pub fn backtrack(&self, leaf_vertex: VertexId) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        let target_label = self
            .get_label_by(leaf_vertex, min_cost_ordering, true)
            .ok_or(SearchTreeError::VertexNotFound(leaf_vertex))?;

        self.reconstruct_path(target_label, None)
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
            if steps > self.nodes.len() as u64 {
                return Err(SearchTreeError::InvalidBranchStructure(format!(
                    "Exceeded tree size {} while backtracking from {}",
                    self.nodes.len(),
                    target_label
                )));
            }

            let exceeds_depth = depth.map(|l| steps >= l).unwrap_or_default();
            if exceeds_depth {
                break;
            }
            let current_node = self
                .get(current_label)
                .ok_or_else(|| SearchTreeError::LabelNotFound(current_label.clone()))?;

            // If this is the root, we're done, otherwise traverse path
            match current_node {
                SearchTreeNode::Root { .. } => break,
                SearchTreeNode::Branch {
                    incoming_edge,
                    parent,
                    ..
                } => {
                    path.push(incoming_edge.clone());
                    current_label = parent;
                }
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

    /// Get all labels in the tree
    pub fn labels(&self) -> impl Iterator<Item = &Label> {
        self.nodes.keys()
    }

    /// Get all nodes in the tree
    pub fn nodes(&self) -> impl Iterator<Item = &SearchTreeNode> {
        self.nodes.values()
    }

    /// Get the incoming edge for a vertex by finding its minimum cost label.
    /// This is an optimized version for getting just the parent edge without full backtracking.
    ///
    /// # Arguments
    /// * `vertex` - The vertex ID to get the incoming edge for
    ///
    /// # Returns
    /// The incoming EdgeTraversal if the vertex exists and is not the root, None otherwise
    pub fn get_incoming_edge(&self, vertex: VertexId) -> Option<&EdgeTraversal> {
        let label = self.get_label_by(vertex, min_cost_ordering, true)?;
        let node = self.get(label)?;
        node.incoming_edge()
    }
}

/// helper function to construct the min cost ordering
fn min_cost_ordering(pair: &(&Label, Option<&EdgeTraversal>)) -> OrderedFloat<f64> {
    let (_, et) = pair;
    match et {
        None => OrderedFloat(f64::MAX),
        Some(e) => OrderedFloat(e.cost.total_cost.as_f64()),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SearchTreeError {
    #[error("parent not found for label {0}")]
    ParentNotFound(Label),
    #[error("Label not found in tree: {0}")]
    LabelNotFound(Label),
    #[error("Node is missing parent reference: {0}")]
    MissingParent(Label),
    #[error("Invalid branch structure: {0}")]
    InvalidBranchStructure(String),
    #[error("Vertex not found in tree: {0}")]
    VertexNotFound(VertexId),
    #[error("Cycle detected: {0}")]
    CycleDetected(String),
    #[error("Search tree error while interacting with Graph: {source}")]
    NetworkError {
        #[from]
        source: NetworkError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        cost::TraversalCost,
        network::{EdgeId, EdgeListId, VertexId},
        unit::Cost,
    };

    #[test]
    fn test_new_empty_tree() {
        let tree = SearchTree::new(Direction::Forward);
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.direction(), Direction::Forward);
        assert!(tree.root().is_none());
    }

    #[test]
    fn test_tree_with_root() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Insert first child
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        // Insert second child
        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(
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
        let mut tree = SearchTree::with_root(root_label, Direction::Forward);

        let child_label = create_test_label(1);
        let child_traversal = create_test_edge_traversal(1, 10.0);
        let nonexistent_parent = create_test_label(99);

        let result = tree.insert(nonexistent_parent.clone(), child_traversal, child_label);
        assert!(matches!(result, Err(SearchTreeError::ParentNotFound(_))));
    }

    #[test]
    fn test_get_parent() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        let child_label = create_test_label(1);
        let child_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child_traversal, child_label.clone())
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse);

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(
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
        let tree = SearchTree::with_root(root_label, Direction::Forward);

        let nonexistent_label = create_test_label(99);
        let result = tree.reconstruct_path(&nonexistent_label, None);
        assert!(matches!(result, Err(SearchTreeError::LabelNotFound(_))));
    }

    #[test]
    fn test_iterators() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(root_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        // Test labels iterator
        let labels: HashSet<_> = tree.labels().cloned().collect();
        assert_eq!(labels.len(), 3);
        assert!(labels.contains(&root_label));
        assert!(labels.contains(&child1_label));
        assert!(labels.contains(&child2_label));

        // Test nodes iterator
        let node_count = tree.nodes().count();
        assert_eq!(node_count, 3);

        let vertex_ids: HashSet<_> = tree.labels().map(|l| l.vertex_id()).collect();
        assert_eq!(vertex_ids.len(), 3);
        assert!(vertex_ids.contains(&VertexId(0)));
        assert!(vertex_ids.contains(&VertexId(1)));
        assert!(vertex_ids.contains(&VertexId(2)));
    }

    #[test]
    fn test_backtrack_forward_tree() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse);

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(
            root_label.clone(),
            child1_traversal.clone(),
            child1_label.clone(),
        )
        .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(
            child1_label.clone(),
            child2_traversal.clone(),
            child2_label.clone(),
        )
        .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(
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
        let tree = SearchTree::with_root(root_label, Direction::Forward);

        let result = tree.backtrack(VertexId(99));
        assert!(matches!(
            result,
            Err(SearchTreeError::VertexNotFound(VertexId(99)))
        ));
    }

    #[test]
    fn test_backtrack_root_vertex() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Backtracking from root should return empty path
        let path = tree.backtrack(VertexId(0)).unwrap();
        assert_eq!(path.len(), 0);
    }

    #[test]
    fn test_find_label_for_vertex() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        // Test finding existing vertex
        let found_label = tree.get_min_cost_label(VertexId(1));
        assert_eq!(found_label, Some(&child1_label));

        // Test finding non-existent vertex
        let not_found = tree.get_min_cost_label(VertexId(99));
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_auto_root_creation() {
        let mut tree = SearchTree::new(Direction::Forward);
        assert!(tree.is_empty());
        assert!(tree.root().is_none());

        // Insert first node - parent should become root automatically
        let parent_label = create_test_label(0);
        let child_label = create_test_label(1);
        let edge_traversal = create_test_edge_traversal(1, 10.0);

        tree.insert(
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
        assert!(tree.nodes.contains_key(&parent_label));

        let child_node = tree.get(&child_label).unwrap();
        assert!(!child_node.is_root());
        assert_eq!(child_node.parent_label(), Some(&parent_label));
        assert_eq!(child_node.incoming_edge().unwrap().edge_id, EdgeId(1));
    }

    #[test]
    fn test_auto_root_creation_chain() {
        let mut tree = SearchTree::new(Direction::Forward);

        // Build a chain: 0 -> 1 -> 2 -> 3 by only calling insert
        let label0 = create_test_label(0);
        let label1 = create_test_label(1);
        let label2 = create_test_label(2);
        let label3 = create_test_label(3);

        // First insert creates root automatically
        tree.insert(
            label0.clone(),
            create_test_edge_traversal(1, 10.0),
            label1.clone(),
        )
        .unwrap();

        // Subsequent inserts work normally
        tree.insert(
            label1.clone(),
            create_test_edge_traversal(2, 15.0),
            label2.clone(),
        )
        .unwrap();
        tree.insert(
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
        let mut tree = SearchTree::new(Direction::Forward);
        let root_label = create_test_label(0);

        // Manually create root first
        tree.set_root(root_label.clone());

        // Insert should work normally without creating a new root
        let child_label = create_test_label(1);
        let edge_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), edge_traversal, child_label.clone())
            .unwrap();

        // Root should still be the same
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.root(), Some(&root_label));

        // Trying to insert with non-existent parent should still fail
        let orphan_label = create_test_label(99);
        let nonexistent_parent = create_test_label(999);
        let result = tree.insert(
            orphan_label,
            create_test_edge_traversal(99, 5.0),
            nonexistent_parent.clone(),
        );
        assert!(matches!(result, Err(SearchTreeError::ParentNotFound(_))));
    }

    #[test]
    fn test_backtrack_with_depth_forward_tree_full_path() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert(child3_label.clone(), child4_traversal, child4_label.clone())
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert(child3_label.clone(), child4_traversal, child4_label.clone())
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(child2_label.clone(), child3_traversal, child3_label.clone())
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse);

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert(child3_label.clone(), child4_traversal, child4_label.clone())
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Reverse);

        // Build a linear path: 0 -> 1 -> 2 -> 3 -> 4
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert(child3_label.clone(), child4_traversal, child4_label.clone())
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
        let tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Backtracking from root with any depth should return empty path
        let path = tree.backtrack_with_depth(VertexId(0), 5).unwrap();
        assert_eq!(path.len(), 0);
    }

    #[test]
    fn test_backtrack_with_depth_nonexistent_vertex() {
        let root_label = create_test_label(0);
        let tree = SearchTree::with_root(root_label, Direction::Forward);

        let result = tree.backtrack_with_depth(VertexId(99), 1);
        assert!(matches!(
            result,
            Err(SearchTreeError::VertexNotFound(VertexId(99)))
        ));
    }

    #[test]
    fn test_backtrack_with_depth_exceeds_available_path() {
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Build a short path: 0 -> 1 -> 2
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(child1_label.clone(), child2_traversal, child2_label.clone())
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
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

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
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(root_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(child1_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        let child4_label = create_test_label(4);
        let child4_traversal = create_test_edge_traversal(4, 25.0);
        tree.insert(child2_label.clone(), child4_traversal, child4_label.clone())
            .unwrap();

        let child5_label = create_test_label(5);
        let child5_traversal = create_test_edge_traversal(5, 30.0);
        tree.insert(child4_label.clone(), child5_traversal, child5_label.clone())
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
            cost: TraversalCost {
                total_cost: Cost::new(cost),
                objective_cost: Cost::new(cost),
                #[cfg(feature = "detailed_costs")]
                cost_component: std::collections::HashMap::new(),
            },
            result_state: vec![],
        }
    }

    fn create_test_label(vertex_id: usize) -> Label {
        Label::Vertex(VertexId(vertex_id))
    }

    #[test]
    fn test_backtrack_mixed_labels_bug() {
        // Reproduction of bug where mixed label types cause Label::Vertex lookup to fail
        let mut tree = SearchTree::new(Direction::Forward);

        let root_label = Label::Vertex(VertexId(0));
        let child_label = Label::VertexWithIntState {
            vertex_id: VertexId(1),
            state: 1,
        };

        // This will set root as Label::Vertex(0)
        // Label::Vertex is NOT added to self.labels
        tree.insert(
            root_label.clone(),
            create_test_edge_traversal(1, 10.0),
            child_label.clone(),
        )
        .unwrap();

        // child_label IS added to self.labels because it is not Label::Vertex

        // Now self.labels is NOT empty (contains key VertexId(1))

        // Try to backtrack from root.
        // We expect this to SUCCEED (return empty path for root), but currently it might fail
        // with VertexNotFound(0) because get_labels skips Label::Vertex when self.labels is populated.
        let result = tree.backtrack(VertexId(0));
        assert!(
            result.is_ok(),
            "Backtracking from root Vertex label should succeed even if tree has mixed labels"
        );
    }

    #[test]
    fn test_vertex_label_model_optimization_correctness() {
        // This test verifies that the specialized handling for Label::Vertex (skipping the aux labels map)
        // works correctly with backtracking.
        let mut tree = SearchTree::new(Direction::Forward);

        // 1. Setup a tree with only Vertex labels (simulating VertexLabelModel)
        let root_id = VertexId(0);
        let child_id = VertexId(1);

        let root_label = Label::Vertex(root_id);
        let child_label = Label::Vertex(child_id);

        // Create an edge traversal
        let et = create_test_edge_traversal(1, 10.0);

        // Insert (root -> child)
        // This trigger's root creation and child insertion
        tree.insert(root_label.clone(), et, child_label.clone())
            .unwrap();

        // 2. Verify OPTIMIZATION: labels map should be EMPTY
        // Because Label::Vertex is optimized to NOT be stored in the secondary index
        assert!(
            tree.labels.is_empty(),
            "Tree labels map should be empty for pure Vertex labels"
        );

        // 3. Verify backtracking works despite empty labels map
        // This confirms get_labels correctly synthesizes the Vertex label lookup
        let result = tree.backtrack(child_id);
        assert!(
            result.is_ok(),
            "Backtracking failed for Vertex label: {:?}",
            result.err()
        );

        let path = result.unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].edge_id, EdgeId(1));

        // 4. Verify backtracking from root
        let root_result = tree.backtrack(root_id);
        assert!(root_result.is_ok());
        assert_eq!(root_result.unwrap().len(), 0);
    }

    #[test]
    fn test_get_incoming_edge() {
        // Test the optimized get_incoming_edge method
        let root_label = create_test_label(0);
        let mut tree = SearchTree::with_root(root_label.clone(), Direction::Forward);

        // Build a linear path: 0 -> 1 -> 2 -> 3
        let child1_label = create_test_label(1);
        let child1_traversal = create_test_edge_traversal(1, 10.0);
        tree.insert(root_label.clone(), child1_traversal, child1_label.clone())
            .unwrap();

        let child2_label = create_test_label(2);
        let child2_traversal = create_test_edge_traversal(2, 15.0);
        tree.insert(child1_label.clone(), child2_traversal, child2_label.clone())
            .unwrap();

        let child3_label = create_test_label(3);
        let child3_traversal = create_test_edge_traversal(3, 20.0);
        tree.insert(child2_label.clone(), child3_traversal, child3_label.clone())
            .unwrap();

        // Test: get incoming edge for vertex 1 (should be edge 1: 0->1)
        let edge1 = tree.get_incoming_edge(VertexId(1));
        assert!(edge1.is_some());
        assert_eq!(edge1.unwrap().edge_id, EdgeId(1));

        // Test: get incoming edge for vertex 2 (should be edge 2: 1->2)
        let edge2 = tree.get_incoming_edge(VertexId(2));
        assert!(edge2.is_some());
        assert_eq!(edge2.unwrap().edge_id, EdgeId(2));

        // Test: get incoming edge for vertex 3 (should be edge 3: 2->3)
        let edge3 = tree.get_incoming_edge(VertexId(3));
        assert!(edge3.is_some());
        assert_eq!(edge3.unwrap().edge_id, EdgeId(3));

        // Test: root has no incoming edge
        let edge_root = tree.get_incoming_edge(VertexId(0));
        assert!(edge_root.is_none());

        // Test: nonexistent vertex returns None
        let edge_none = tree.get_incoming_edge(VertexId(99));
        assert!(edge_none.is_none());
    }
}
