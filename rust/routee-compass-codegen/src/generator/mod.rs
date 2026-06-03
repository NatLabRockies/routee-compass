//! template modules are intentionally left unimplemented as they are used
//! for code generation.

mod component_type;

mod error;
pub mod run;

#[allow(unused)]
mod constraint;
#[allow(unused)]
mod input_plugin;
#[allow(unused)]
mod output_plugin;
#[allow(unused)]
mod traversal;

pub use component_type::CodegenComponentType;
pub use error::CodegenError;
