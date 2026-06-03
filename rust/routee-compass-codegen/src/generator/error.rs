use std::path::PathBuf;

use crate::generator::CodegenComponentType;

#[derive(thiserror::Error, Debug)]
pub enum CodegenError {
    #[error("internal error: path to Cargo manifest {0} has no parent")]
    RepoLayout(String),
    #[error("error reading {component} {path} {file}: {source}")]
    TemplateReadError {
        component: CodegenComponentType,
        path: PathBuf,
        file: String,
        source: std::io::Error,
    },
}
