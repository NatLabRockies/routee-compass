use std::path::Path;

pub fn generate_output_plugin_module(
    _pascal_case_name: &str,
    _snake_case_name: &str,
    _path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("OutputPlugin generation not yet implemented".into())
}
