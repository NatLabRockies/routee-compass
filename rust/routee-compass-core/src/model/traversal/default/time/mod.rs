//! simple stateless model that calculates leg time from
//! leg speed and leg distance, also appending to the overall trip time.

mod builder;
mod config;
mod model;

pub use builder::TimeTraversalBuilder;
pub use config::TimeTraversalConfig;
pub use model::TimeTraversalModel;
