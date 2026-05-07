use std::sync::Arc;

use chrono::{DateTime, Local};
use serde_json::Value;

use crate::{app::compass::runtimes::InputPluginRuntimes, plugin::input::InputPluginError};

/// wrapper for queries passing through the input plugin processing phase of a run.
/// after completing all input plugins, this record type contains the final row value,
/// runtimes for each plugin used, and for each input plugin, the proportion of runtime
/// that this row contributed, in the case that the input plugin generated more rows than it started with.
#[derive(Debug, Clone, Default)]
pub struct InputPluginPayload {
    /// current/final state of the query being processed.
    pub row: Value,
    /// error result along the way.
    pub error: Option<Arc<InputPluginError>>,
    /// runtime metrics for the input plugin processing.
    pub runtimes: InputPluginRuntimes,
}

impl InputPluginPayload {
    /// lift a Value into an instance of a [InputPluginResult] before running
    /// any input plugin processing, to set its initial state.
    pub fn new(initial: Value) -> Self {
        Self {
            row: initial,
            error: None,
            runtimes: Default::default(),
        }
    }

    /// creates a new [InputPluginResult] with a new row, moving existing
    /// runtimes data.
    pub fn update_row(self, new_row: Value) -> Self {
        Self {
            row: new_row,
            error: None,
            runtimes: self.runtimes,
        }
    }

    /// clones existing runtimes data and inserts a new row. used when
    /// a plugin produces more than one row.
    pub fn create_child_row(&self, new_row: Value) -> Self {
        Self {
            row: new_row,
            error: None,
            runtimes: self.runtimes.clone(),
        }
    }

    /// records a runtime for an input plugin. will generate proportional runtimes
    /// based on the number of result rows of the plugin.
    pub fn record_input_plugin_runtime(
        &mut self,
        name: &str,
        start_time: DateTime<Local>,
        n_results: usize,
    ) {
        self.runtimes.record(name, start_time, n_results);
    }
}
