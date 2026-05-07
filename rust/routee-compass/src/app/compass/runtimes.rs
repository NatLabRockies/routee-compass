use chrono::{DateTime, Local, TimeDelta};
use routee_compass_core::model::unit::TimeUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::Time;

/// accumulator that collects the runtimes of Compass components. collected in the target
/// time unit so that Runtimes is idempotent across JSON serialization round-trips.
/// uses the proportional input plugin runtime as the value to contribute to the total
/// time, and the un-proportioned input plugin runtime to contribute to the wall time.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Runtimes {
    /// proportional time spent running input plugins
    input: f64,
    /// wall time spent running input plugins
    input_wall: f64,
    /// time spent running graph search
    search: f64,
    /// time spent running output plugins
    output: f64,
    /// wall time spent running input plugins
    #[serde(skip_serializing_if = "Vec::is_empty")]
    input_plugins_wall: Vec<PluginRuntime>,
    /// proportional time spent running input plugins
    #[serde(skip_serializing_if = "Vec::is_empty")]
    input_plugins: Vec<PluginRuntime>,
    /// time spent running output plugins
    #[serde(skip_serializing_if = "Vec::is_empty")]
    output_plugins: Vec<PluginRuntime>,
    /// total as a sum of search + proportional input plugin + output plugin time.
    total: f64,
    /// total as a sum of search + wall input plugin + output plugin time.
    wall: f64,
    /// time unit used for recording time values.
    time_unit: TimeUnit,
}

#[derive(Default, Clone, Debug)]
pub struct InputPluginRuntimes {
    /// time required to run each input plugin.
    pub runtimes: Vec<(String, TimeDelta)>,
    /// proportion of time this row contributed to each input plugin runtime.
    pub runtimes_proportioned: Vec<(String, TimeDelta)>,
}

/// records the name and runtime of a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRuntime {
    /// the plugin's name.
    name: String,
    /// the runtime of the plugin.
    time: f64,
}

impl Runtimes {
    /// create a new [Runtimes] accumulator, collecting times in the given [TimeUnit].
    /// for an accumulator that uses the [Default] [TimeUnit], use [Runtimes::Default].
    pub fn new(time_unit: TimeUnit) -> Self {
        Self {
            input: 0.0,
            input_wall: 0.0,
            search: 0.0,
            output: 0.0,
            input_plugins_wall: vec![],
            input_plugins: vec![],
            output_plugins: vec![],
            total: 0.0,
            wall: 0.0,
            time_unit,
        }
    }

    /// adds the search runtime value to this accumulator.
    pub fn add_search_runtime(&mut self, td: TimeDelta) {
        let time = to_serializable(&td, &self.time_unit);
        self.total += time;
        self.wall += time;
        self.search = time;
    }

    /// adds the runtimes associated with running input plugins to this accumulator.
    pub fn add_input_plugin_runtimes(&mut self, ipr: &InputPluginRuntimes) {
        for (name, td) in ipr.runtimes.iter() {
            let time = to_serializable(td, &self.time_unit);
            self.wall += time;
            self.input_wall += time;
            self.input_plugins_wall.push(PluginRuntime {
                name: name.clone(),
                time,
            });
        }
        for (name, td) in ipr.runtimes_proportioned.iter() {
            let time = to_serializable(td, &self.time_unit);
            self.total += time;
            self.input += time;
            self.input_plugins.push(PluginRuntime {
                name: name.clone(),
                time,
            });
        }
    }

    /// adds the next output plugin runtime to this accumulator. should be called in the
    /// order of output plugins so that the first runtime pushed corresponds to the output
    /// plugin with index 0 (the 1st plugin).
    pub fn push_output_plugin_runtime(&mut self, name: &str, start_time: DateTime<Local>) {
        let duration = chrono::Local::now() - start_time;
        let time = to_serializable(&duration, &self.time_unit);
        self.total += time;
        self.wall += time;
        self.output += time;
        self.output_plugins.push(PluginRuntime {
            name: name.to_string(),
            time,
        });
    }
}

impl InputPluginRuntimes {
    /// records a runtime for an input plugin. will generate proportional runtimes
    /// based on the number of result rows of the plugin.
    pub fn record(&mut self, name: &str, start_time: DateTime<Local>, n_results: usize) {
        let duration = chrono::Local::now() - start_time;
        // denominator sanitized for both TimeDelta and f64::Div operations.
        let denom = if n_results == 0 {
            1
        } else if (i32::MAX as usize) < n_results {
            i32::MAX
        } else {
            n_results as i32
        };
        let dur_prop = duration.checked_div(denom).unwrap_or_default();
        self.runtimes.push((name.to_string(), duration));
        self.runtimes_proportioned
            .push((name.to_string(), dur_prop));
    }
}

const NANOS_PER_SEC: f64 = 1_000_000_000.0;

/// helper to convert a [TimeDelta] to the count of seconds as a floating point value.
fn to_fractional_seconds(td: &TimeDelta) -> f64 {
    let nanos_f64 = td.num_nanoseconds().unwrap_or(0) as f64;
    nanos_f64 / NANOS_PER_SEC
}

/// helper to serialize a [TimeDelta] into a count of the given [TimeUnit].
fn to_serializable(value: &TimeDelta, time_unit: &TimeUnit) -> f64 {
    let secs_uom: Time = TimeUnit::Seconds.to_uom(to_fractional_seconds(value));

    time_unit.from_uom(secs_uom)
}
