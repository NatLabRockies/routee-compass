use chrono::{DateTime, Local, TimeDelta};
use routee_compass_core::model::unit::TimeUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::Time;

use crate::app::compass::input_plugin_result::InputPluginRuntimes;

/// accumulator that collects the runtimes of Compass components. collected in the target
/// time unit so that Runtimes is idempotent across JSON serialization round-trips.
/// uses the proportional input plugin runtime as the value to contribute to the total
/// time, and the un-proportioned input plugin runtime to contribute to the wall time.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Runtimes {
    /// time spent running graph search
    search: f64,
    /// wall time spent running input plugins
    #[serde(skip_serializing_if = "Vec::is_empty")]
    input_plugins_wall: Vec<f64>,
    /// proportional time spent running input plugins
    #[serde(skip_serializing_if = "Vec::is_empty")]
    input_plugins_proportioned: Vec<f64>,
    /// time spent running output plugins
    #[serde(skip_serializing_if = "Vec::is_empty")]
    output_plugins: Vec<f64>,
    /// total as a sum of search + proportional input plugin + output plugin time.
    total: f64,
    /// total as a sum of search + wall input plugin + output plugin time.
    wall: f64,
    /// time unit used for recording time values.
    time_unit: TimeUnit,
}

impl Runtimes {
    /// create a new [Runtimes] accumulator, collecting times in the given [TimeUnit].
    /// for an accumulator that uses the [Default] [TimeUnit], use [Runtimes::Default].
    pub fn new(time_unit: TimeUnit) -> Self {
        Self {
            search: 0.0,
            input_plugins_wall: vec![],
            input_plugins_proportioned: vec![],
            output_plugins: vec![],
            total: 0.0,
            wall: 0.0,
            time_unit,
        }
    }

    /// adds the search runtime value to this accumulator.
    pub fn add_search_runtime(&mut self, td: TimeDelta) {
        let time = to_serializable(&td, &self.time_unit);
        self.search = time;
        self.total += time;
        self.wall += time;
    }

    /// adds the runtimes associated with running input plugins to this accumulator.
    pub fn add_input_plugin_runtimes(&mut self, ipr: &InputPluginRuntimes) {
        for td in ipr.runtimes.iter() {
            let time = to_serializable(td, &self.time_unit);
            self.input_plugins_wall.push(time);
            self.wall += time;
        }
        for td in ipr.runtimes_proportioned.iter() {
            let time = to_serializable(td, &self.time_unit);
            self.input_plugins_proportioned.push(time);
            self.total += time;
        }
    }

    /// adds the next output plugin runtime to this accumulator. should be called in the
    /// order of output plugins so that the first runtime pushed corresponds to the output
    /// plugin with index 0 (the 1st plugin).
    pub fn push_output_plugin_runtime(&mut self, start_time: DateTime<Local>) {
        let duration = chrono::Local::now() - start_time;
        let time = to_serializable(&duration, &self.time_unit);
        self.output_plugins.push(time);
        self.total += time;
        self.wall += time;
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
