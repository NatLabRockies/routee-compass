mod builder;
pub mod default;
mod error;
mod model;
mod service;

/// template module is intentionally left unimplemented as it is used
/// for code generation.
#[allow(unused)]
mod template;

pub use builder::ConstraintModelBuilder;
pub use error::ConstraintModelError;
pub use model::ConstraintModel;
pub use service::ConstraintModelService;
