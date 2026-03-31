use allocative::Allocative;

use super::edge_traversal::EdgeTraversal;
use crate::algorithm::search::SearchGraph;

#[derive(Default, Allocative)]
pub struct SearchAlgorithmResult {
    pub graphs: Vec<SearchGraph>,
    pub routes: Vec<Vec<EdgeTraversal>>,
    pub iterations: u64,
    pub terminated: Option<String>,
}
