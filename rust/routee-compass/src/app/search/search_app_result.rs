use allocative::Allocative;

use chrono::{DateTime, Local, TimeDelta};
use routee_compass_core::algorithm::search::{EdgeTraversal, SearchTree};

#[derive(Allocative)]
pub struct SearchAppResult {
    pub routes: Vec<Vec<EdgeTraversal>>,
    pub trees: Vec<SearchTree>,
    #[allocative(skip)]
    pub search_executed_time: DateTime<Local>,
    #[allocative(skip)]
    pub search_runtime: TimeDelta,
    pub iterations: u64,
    pub terminated: Option<String>,
}
