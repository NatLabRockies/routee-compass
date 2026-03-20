mod plugin;
mod builder;
mod config;

pub mod ops;
mod operation;
pub use builder::EvalOutputPluginBuilder;
pub use config::EvalOutputPluginConfig;
pub use plugin::EvalOutputPlugin;
pub use operation::Operation;