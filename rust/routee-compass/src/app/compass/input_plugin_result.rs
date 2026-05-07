use std::sync::Arc;

use chrono::{DateTime, Local, TimeDelta};
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

    /// records a runtime for an input plugin. will generate proportional runtimes
    /// based on the number of result rows of the plugin.
    pub fn record_input_plugin_runtime(&mut self, start_time: DateTime<Local>, n_results: usize) {
        self.runtimes.record(start_time, n_results);
    }
}

#[derive(Default, Clone, Debug)]
pub struct InputPluginRuntimes {
    /// time required to run each input plugin.
    pub runtimes: Vec<TimeDelta>,
    /// proportion of time this row contributed to each input plugin runtime.
    pub runtimes_proportioned: Vec<TimeDelta>,
    /// along the way, how many queries were split out due to input processing.
    pub proportional_contributions: Vec<f64>,
}

impl InputPluginRuntimes {
    /// records a runtime for an input plugin. will generate proportional runtimes
    /// based on the number of result rows of the plugin.
    pub fn record(&mut self, start_time: DateTime<Local>, n_results: usize) {
        let duration = chrono::Local::now() - start_time;
        let td_denom = if (i32::MAX as usize) < n_results {
            i32::MAX
        } else {
            n_results as i32
        };
        let dur_prop = duration.checked_div(td_denom).unwrap_or_default();
        let prop = 1.0 / td_denom as f64;
        self.runtimes.push(duration);
        self.runtimes_proportioned.push(dur_prop);
        self.proportional_contributions.push(prop);
    }
}

/// prevent exceeding system limits in denominator value for proportioning time deltas.
const MAX_RESULTS: usize = i32::MAX as usize;
