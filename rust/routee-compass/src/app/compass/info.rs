use crate::app::{compass::runtimes::Runtimes, search::SearchAppResult};
use serde::{Deserialize, Serialize};

pub const INFO_KEY: &str = "info";

/// framework info associated with this row of a Compass input collecting run statistics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Info {
    iterations: u64,
    tree_edges: usize,
    route_edges: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ram_mib: Option<f64>,
    terminated: String,
    runtime: Runtimes,
}

impl Info {
    /// creates a new record of the "info" section of a result. written to the output JSON at [INFO_KEY].
    pub fn new_from_success(result: &SearchAppResult, runtime: Runtimes, record_ram: bool) -> Self {
        let route_edges = match result.routes.as_slice() {
            [] => None,
            rs => {
                let count = rs.iter().map(|r| r.len()).sum::<usize>();
                Some(count)
            }
        };
        let tree_edges = result.trees.iter().map(|t| t.len()).sum::<usize>();
        let terminated = result
            .terminated
            .clone()
            .unwrap_or_else(|| "false".to_string());
        let ram_mib = if record_ram {
            let memory_bytes = allocative::size_of_unique(result) as f64;
            Some(memory_bytes / 1_048_576.0)
        } else {
            None
        };
        Self {
            iterations: result.iterations,
            tree_edges,
            route_edges,
            terminated,
            ram_mib,
            runtime,
        }
    }

    /// helper constructor for when we are creating an info section on a failed run.
    pub fn new_from_failure(runtime: Runtimes) -> Self {
        Self {
            iterations: 0,
            tree_edges: 0,
            route_edges: None,
            ram_mib: None,
            terminated: "false".to_string(),
            runtime,
        }
    }
}
