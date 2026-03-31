use allocative::Allocative;
use serde::Serialize;

use crate::{
    algorithm::search::{Direction, EdgeTraversal},
    model::{cost::TraversalCost, label::Label},
};

/// A node in the search graph, containing parent/child relationships and traversal data
#[derive(Debug, Clone, Allocative, Serialize)]
pub enum SearchGraphNode {
    Root {
        /// Tree orientation this node belongs to
        direction: Direction,
        /// Number of nodes in the graph that have this node as a parent
        child_count: usize,
    },
    Branch {
        /// The edge traversal that led to this node (None for root)
        incoming_edge: EdgeTraversal,
        /// Parent node label (None for root)
        parent: Label,
        /// Tree orientation this node belongs to
        direction: Direction,
        /// Number of nodes in the graph that have this node as a parent
        child_count: usize,
    },
}

impl SearchGraphNode {
    pub fn new_root(orientation: Direction) -> Self {
        Self::Root {
            direction: orientation,
            child_count: 0,
        }
    }

    pub fn new_child(edge_traversal: EdgeTraversal, parent: Label, direction: Direction) -> Self {
        Self::Branch {
            incoming_edge: edge_traversal,
            parent,
            direction,
            child_count: 0,
        }
    }

    pub fn parent_label(&self) -> Option<&Label> {
        match self {
            SearchGraphNode::Root { .. } => None,
            SearchGraphNode::Branch { parent, .. } => Some(parent),
        }
    }

    pub fn incoming_edge(&self) -> Option<&EdgeTraversal> {
        match self {
            SearchGraphNode::Root { .. } => None,
            SearchGraphNode::Branch { incoming_edge, .. } => Some(incoming_edge),
        }
    }

    pub fn is_root(&self) -> bool {
        match self {
            SearchGraphNode::Root { .. } => true,
            SearchGraphNode::Branch { .. } => false,
        }
    }

    pub fn direction(&self) -> Direction {
        match self {
            SearchGraphNode::Root { direction, .. } => *direction,
            SearchGraphNode::Branch { direction, .. } => *direction,
        }
    }

    pub fn traversal_cost(&self) -> Option<&TraversalCost> {
        match self {
            SearchGraphNode::Root { .. } => None,
            SearchGraphNode::Branch { incoming_edge, .. } => Some(&incoming_edge.cost),
        }
    }

    pub fn child_count(&self) -> usize {
        match self {
            SearchGraphNode::Root { child_count, .. } => *child_count,
            SearchGraphNode::Branch { child_count, .. } => *child_count,
        }
    }

    pub fn increment_child_count(&mut self) {
        match self {
            SearchGraphNode::Root { child_count, .. } => *child_count += 1,
            SearchGraphNode::Branch { child_count, .. } => *child_count += 1,
        }
    }

    pub fn decrement_child_count(&mut self) {
        match self {
            SearchGraphNode::Root { child_count, .. } => {
                if *child_count > 0 {
                    *child_count -= 1;
                }
            }
            SearchGraphNode::Branch { child_count, .. } => {
                if *child_count > 0 {
                    *child_count -= 1;
                }
            }
        }
    }

    pub fn is_prunable(&self) -> bool {
        self.child_count() == 0
    }
}
