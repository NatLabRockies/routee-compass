use std::path::Path;

use crate::collection::OvertureMapsCollectionError;

/// helper function to "mkdir -p path" - make all directories along a path
pub fn create_dirs<P>(path: P) -> Result<(), OvertureMapsCollectionError>
where
    P: AsRef<Path>,
{
    let dirspath = path.as_ref();
    if !dirspath.is_dir() {
        std::fs::create_dir_all(dirspath).map_err(|e| {
            let msg = format!(
                "error building output directory '{}': {e}",
                dirspath.to_str().unwrap_or_default()
            );
            OvertureMapsCollectionError::InvalidUserInput(msg)
        })
    } else {
        Ok(())
    }
}
