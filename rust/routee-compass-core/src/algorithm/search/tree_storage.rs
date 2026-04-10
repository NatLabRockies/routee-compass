use crate::algorithm::search::SearchTreeNode;
use crate::model::label::Label;
use crate::model::network::VertexId;
use allocative::Allocative;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Allocative)]
pub enum TreeStorage {
    VertexOnly(HashMap<VertexId, SearchTreeNode>),
    Stateful {
        nodes: HashMap<Label, SearchTreeNode>,
        labels: HashMap<VertexId, HashSet<Label>>,
    },
}

impl TreeStorage {
    pub fn new_vertex_oriented() -> Self {
        TreeStorage::VertexOnly(HashMap::new())
    }

    pub fn new_stateful() -> Self {
        TreeStorage::Stateful {
            nodes: HashMap::new(),
            labels: HashMap::new(),
        }
    }

    pub fn insert_node(&mut self, label: Label, node: SearchTreeNode) {
        match self {
            Self::VertexOnly(nodes) => {
                nodes.insert(*label.vertex_id(), node);
            }
            Self::Stateful { nodes, labels } => {
                let vertex_labels = labels.entry(*label.vertex_id()).or_default();
                vertex_labels.insert(label.clone());
                nodes.insert(label, node);
            }
        }
    }

    pub fn contains_key(&self, label: &Label) -> bool {
        match self {
            Self::VertexOnly(nodes) => nodes.contains_key(label.vertex_id()),
            Self::Stateful { nodes, .. } => nodes.contains_key(label),
        }
    }

    pub fn get(&self, label: &Label) -> Option<&SearchTreeNode> {
        match self {
            Self::VertexOnly(nodes) => nodes.get(label.vertex_id()),
            Self::Stateful { nodes, .. } => nodes.get(label),
        }
    }

    pub fn get_mut(&mut self, label: &Label) -> Option<&mut SearchTreeNode> {
        match self {
            Self::VertexOnly(nodes) => nodes.get_mut(label.vertex_id()),
            Self::Stateful { nodes, .. } => nodes.get_mut(label),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::VertexOnly(nodes) => nodes.len(),
            Self::Stateful { nodes, .. } => nodes.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::VertexOnly(nodes) => nodes.is_empty(),
            Self::Stateful { nodes, .. } => nodes.is_empty(),
        }
    }

    pub fn get_labels(&self, vertex: VertexId) -> Box<dyn Iterator<Item = Label> + '_> {
        match self {
            Self::VertexOnly(nodes) => {
                if nodes.contains_key(&vertex) {
                    Box::new(std::iter::once(Label::Vertex(vertex)))
                } else {
                    Box::new(std::iter::empty())
                }
            }
            Self::Stateful { labels, .. } => match labels.get(&vertex) {
                Some(vertex_labels) => Box::new(vertex_labels.iter().cloned()),
                None => Box::new(std::iter::empty()),
            },
        }
    }

    pub fn branch_iter<'a>(&'a self) -> Box<dyn Iterator<Item = (Label, &'a SearchTreeNode)> + 'a> {
        match self {
            Self::VertexOnly(store) => Box::new(store.iter().map(|(v, n)| (Label::Vertex(*v), n))),
            Self::Stateful { nodes, .. } => Box::new(nodes.iter().map(|(l, n)| (l.clone(), n))),
        }
    }

    pub fn label_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Label> + 'a> {
        match self {
            Self::VertexOnly(store) => Box::new(store.keys().map(|v| Label::Vertex(*v))),
            Self::Stateful { nodes, .. } => Box::new(nodes.keys().map(|l| l.clone())),
        }
    }

    pub fn node_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a SearchTreeNode> + 'a> {
        match self {
            Self::VertexOnly(nodes) => Box::new(nodes.values()),
            Self::Stateful { nodes, .. } => Box::new(nodes.values()),
        }
    }
}
