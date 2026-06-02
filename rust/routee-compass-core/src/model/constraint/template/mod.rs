//! Template Constraint Model
//!
//! A stubbed version of a constraint model module that compiles. Used in codegen.
//! If code changes in Compass lead to compiler errors in this module, the changes
//! should get updated.

mod builder;
mod config;
mod engine;
mod model;
mod params;
mod service;

pub use builder::TemplateBuilder;
pub use config::TemplateConfig;
pub use engine::TemplateEngine;
pub use model::TemplateModel;
pub use params::TemplateParams;
pub use service::TemplateService;
