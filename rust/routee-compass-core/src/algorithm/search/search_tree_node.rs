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
    pub fn new_root(orientation: Direction) -> Self {
        Self::Root {
            direction: orientation,
        }
    }

    pub fn new_child(edge_traversal: EdgeTraversal, parent: Label, direction: Direction) -> Self {
        Self::Branch {
            incoming_edge: edge_traversal,
            parent,
            direction,
        }
    }

    pub fn parent_label(&self) -> Option<&Label> {
        match self {
            SearchTreeNode::Root { .. } => None,
            SearchTreeNode::Branch { parent, .. } => Some(parent),
        }
    }

    pub fn incoming_edge(&self) -> Option<&EdgeTraversal> {
        match self {
            SearchTreeNode::Root { .. } => None,
            SearchTreeNode::Branch { incoming_edge, .. } => Some(incoming_edge),
        }
    }

    pub fn is_root(&self) -> bool {
        match self {
            SearchTreeNode::Root { .. } => true,
            SearchTreeNode::Branch { .. } => false,
        }
    }

    pub fn direction(&self) -> Direction {
        match self {
            SearchTreeNode::Root { direction } => *direction,
            SearchTreeNode::Branch { direction, .. } => *direction,
        }
    }

    pub fn traversal_cost(&self) -> Option<&TraversalCost> {
        match self {
            SearchTreeNode::Root { .. } => None,
            SearchTreeNode::Branch { incoming_edge, .. } => Some(&incoming_edge.cost),
        }
    }
}
