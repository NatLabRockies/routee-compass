use std::sync::Arc;

use chrono::TimeDelta;
use serde_json::Value;

use crate::plugin::input::InputPluginError;

/// the (successful) result of running input plugins for row processing.
/// contains the final row value, runtimes for each plugin used, and for each
/// input plugin, the proportion of runtime that this row contributed, in the case
/// that the input plugin generated more rows than it started with.
#[derive(Debug, Clone, Default)]
pub struct InputPluginResult {
    /// the resulting row, processed by all input plugins.
    pub row: Value,
    /// error result along the way
    pub error: Option<Arc<InputPluginError>>,
    /// runtime metrics for the input plugin processing
    pub runtimes: InputPluginRuntimes,
}

impl InputPluginResult {
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
}

#[derive(Default, Clone, Debug)]
pub struct InputPluginRuntimes {
    pub total: TimeDelta,
    /// time required to run each input plugin.
    pub runtimes: Vec<TimeDelta>,
    /// proportion of time this row contributed to each input plugin runtime.
    pub runtimes_proportioned: Vec<TimeDelta>,
    /// along the way, how many queries were split out due to input processing.
    pub proportional_contributions: Vec<f64>,
}
