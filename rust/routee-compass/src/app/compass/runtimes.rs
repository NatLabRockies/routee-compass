use chrono::{DateTime, Local, TimeDelta};
use routee_compass_core::model::unit::TimeUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::Time;

/// accumulator that collects the runtimes of Compass components. collected in the target
/// time unit so that Runtimes is idempotent across JSON serialization round-trips.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Runtimes {
    search: f64,
    output_plugins: Vec<f64>,
    total: f64,
    time_unit: TimeUnit,
}

impl Runtimes {
    /// create a new [Runtimes] accumulator, collecting times in the given [TimeUnit].
    /// for an accumulator that uses the [Default] [TimeUnit], use [Runtimes::Default].
    pub fn new(time_unit: TimeUnit) -> Self {
        Self {
            search: 0.0,
            output_plugins: vec![],
            total: 0.0,
            time_unit,
        }
    }

    /// adds the search runtime value to this accumulator.
    pub fn add_search_runtime(&mut self, td: TimeDelta) {
        let time = to_serializable(&td, &self.time_unit);
        self.search = time;
        self.total += time;
    }

    /// adds the next output plugin runtime to this accumulator. should be called in the
    /// order of output plugins so that the first runtime pushed corresponds to the output
    /// plugin with index 0 (the 1st plugin).
    pub fn push_output_plugin_runtime(&mut self, start_time: DateTime<Local>) {
        let duration = chrono::Local::now() - start_time;
        let time = to_serializable(&duration, &self.time_unit);
        self.output_plugins.push(time);
        self.total += time;
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
    let out = time_unit.from_uom(secs_uom);
    out
}
