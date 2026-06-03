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
    pub fn template_directory(&self) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("generator")
            .join(self.to_string())
    }

    pub fn retrieve_template(&self, file: &str) -> Result<String, CodegenError> {
        let dir = self.template_directory();
        let path = dir.join(file);
        std::fs::read_to_string(&path).map_err(|source| CodegenError::TemplateReadError {
            component: self.clone(),
            path: path,
            file: file.to_string(),
            source,
        })
    }
}

impl std::fmt::Display for CodegenComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CodegenComponentType::Traversal => "traversal",
            CodegenComponentType::Constraint => "constraint",
            CodegenComponentType::InputPlugin => "input_plugin",
            CodegenComponentType::OutputPlugin => "output_plugin",
        };
        write!(f, "{s}")
    }
}
