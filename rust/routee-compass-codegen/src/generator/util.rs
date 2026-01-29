use std::fs;
use std::path::Path;

/// helper for file writing with overwrite check
pub fn write_file(
    path: &Path,
    contents: String,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path_exists = fs::exists(path)?;
    if path_exists && !force {
        let p_str = path.to_str().unwrap_or_default();
        Err(format!("path '{p_str}' already exists. to overwrite, use the --force flag").into())
    } else {
        fs::write(path, contents)?;
        Ok(())
    }
}
