use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::generator::CodegenError;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CodegenComponentType {
    Traversal,
    Constraint,
    InputPlugin,
    OutputPlugin,
}

impl CodegenComponentType {
    pub fn template_directory(&self) -> Result<PathBuf, CodegenError> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_dir = manifest_dir
            .parent()
            .ok_or_else(|| CodegenError::RepoLayout(manifest_dir.to_string_lossy().to_string()))?;
        match self {
            CodegenComponentType::Traversal => Ok(repo_dir
                .join("routee-compass-core")
                .join("src")
                .join("model")
                .join("traversal")
                .join("template")),
            CodegenComponentType::Constraint => Ok(repo_dir
                .join("routee-compass-core")
                .join("src")
                .join("model")
                .join("constraint")
                .join("template")),
            CodegenComponentType::InputPlugin => Ok(repo_dir
                .join("routee-compass")
                .join("src")
                .join("plugin")
                .join("input")
                .join("template")),
            CodegenComponentType::OutputPlugin => todo!(),
        }
    }

    pub fn retrieve_template(&self, file: &str) -> Result<String, CodegenError> {
        let dir = self.template_directory()?;
        let path = dir.join(file);
        std::fs::read_to_string(path).map_err(|source| CodegenError::TemplateReadError {
            component: self.clone(),
            file: file.to_string(),
            source,
        })
    }
}

impl std::fmt::Display for CodegenComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CodegenComponentType::Traversal => "Traversal",
            CodegenComponentType::Constraint => "Constraint",
            CodegenComponentType::InputPlugin => "InputPlugin",
            CodegenComponentType::OutputPlugin => "OutputPlugin",
        };
        write!(f, "{s}")
    }
}
