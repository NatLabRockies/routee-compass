use crate::{algorithm::search::{EdgeTraversal, SearchTree, SearchTreeError}, model::{label::Label, network::{Edge, Vertex}}};

/// context for an edge traversal from (src)->[edge]->(dst) that will be
/// added to the tree.
pub struct EdgeTraversalContext<'a> {
    /// [Label] associated with the src [Vertex]
    pub parent_label: &'a Label,
    /// source of the trajectory
    pub src: &'a Vertex,
    /// edge along this trajectory
    pub edge: &'a Edge,
    /// destination of the trajectory
    pub dst: &'a Vertex,
    /// search tree state before adding this traversal
    pub tree: &'a SearchTree
}

impl<'a> EdgeTraversalContext<'a> {
    /// create a new context for an edge traversal.
    pub fn new(
        parent_label: &'a Label,
        src: &'a Vertex,
        edge: &'a Edge,
        dst: &'a Vertex,
        tree: &'a SearchTree
    ) -> Self {
        Self { parent_label, src, edge, dst, tree }
    }

    /// convenience method to backtrack from the parent label toward the tree root. 
    /// 
    /// # Arguments
    /// * `depth` - if provided, the number of steps to take toward the root, otherwise returns the complete path to the root.
    /// 
    pub fn backtrack(&self, depth: Option<u64>) -> Result<Vec<EdgeTraversal>, SearchTreeError> {
        self.tree.reconstruct_path(self.parent_label, depth)
    }

    /// convenience method to access the previous edge, exactly one step of backtracking into the tree.
    /// if there is no previous edge (we are at the search origin/source), then the result is None.
    pub fn previous_edge_traversal(&self) -> Result<Option<&EdgeTraversal>, SearchTreeError> {
        let pred = self.tree.predecessor(self.parent_label)?;
        match pred {
            Some((edge_traversal, _)) => Ok(Some(edge_traversal)),
            None => Ok(None),
        }
    }
}