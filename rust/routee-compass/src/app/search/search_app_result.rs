use allocative::Allocative;

use routee_compass_core::algorithm::search::{EdgeTraversal, SearchGraph};

use std::time::Duration;

#[derive(Allocative)]
pub struct SearchAppResult {
    pub routes: Vec<Vec<EdgeTraversal>>,
    pub trees: Vec<SearchGraph>,
    pub search_executed_time: String,
    pub search_runtime: Duration,
    pub iterations: u64,
    pub terminated: Option<String>,
}
