use crate::generator::CodegenComponentType;

#[derive(thiserror::Error, Debug)]
pub enum CodegenError {
    #[error("internal error: path to Cargo manifest {0} has no parent")]
    RepoLayout(String),
    #[error("error reading {component} {file}: {source}")]
    TemplateReadError {
        component: CodegenComponentType,
        file: String,
        source: std::io::Error,
    },
}
