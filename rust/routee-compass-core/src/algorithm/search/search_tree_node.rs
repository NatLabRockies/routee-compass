use allocative::Allocative;
use serde::Serialize;

use crate::{
    algorithm::search::{Direction, EdgeTraversal},
    model::{cost::TraversalCost, label::Label},
};

/// A node in the search tree containing parent/child relationships and traversal data
#[derive(Debug, Clone, Allocative, Serialize)]
pub enum SearchTreeNode {
    Root {
        /// Tree orientation this node belongs to
        direction: Direction,
        /// Number of nodes in the tree that have this node as a parent
        child_count: usize,
    },
    Branch {
        /// The edge traversal that led to this node (None for root)
        incoming_edge: EdgeTraversal,
        /// Parent node label (None for root)
        parent: Label,
        /// Tree orientation this node belongs to
        direction: Direction,
    },
}

impl SearchTreeNode {
    /// create a new root node.
    pub fn new_root(direction: Direction) -> Self {
        Self::Root {
            direction,
            child_count: 0,
        }
    }

    /// create a new child node from some trajectory (parent) -> [incoming_edge] -> (), for some
    /// direction in the network.
    pub fn new_child(incoming_edge: EdgeTraversal, parent: Label, direction: Direction) -> Self {
        Self::Branch {
            incoming_edge,
            parent,
            direction,
        }
    }

    /// Retrieves the label of the parent node, if this node is not the root.
    pub fn parent_label(&self) -> Option<&Label> {
        match self {
            SearchTreeNode::Root { .. } => None,
            SearchTreeNode::Branch { parent, .. } => Some(parent),
        }
    }

    /// Retrieves the edge traversal that led to this node, if this node is not the root.
    pub fn incoming_edge(&self) -> Option<&EdgeTraversal> {
        match self {
            SearchTreeNode::Root { .. } => None,
            SearchTreeNode::Branch { incoming_edge, .. } => Some(incoming_edge),
        }
    }

    /// Returns `true` if this node is the root of the search tree.
    pub fn is_root(&self) -> bool {
        match self {
            SearchTreeNode::Root { .. } => true,
            SearchTreeNode::Branch { .. } => false,
        }
    }

    /// Retrieves the direction of the search tree this node belongs to.
    pub fn direction(&self) -> Direction {
        match self {
            SearchTreeNode::Root { direction, .. } => *direction,
            SearchTreeNode::Branch { direction, .. } => *direction,
        }
    }

    /// Retrieves the cost of the edge traversal that led to this node, if this node is not the root.
    pub fn traversal_cost(&self) -> Option<&TraversalCost> {
        match self {
            SearchTreeNode::Root { .. } => None,
            SearchTreeNode::Branch { incoming_edge, .. } => Some(&incoming_edge.cost),
        }
    }
}
