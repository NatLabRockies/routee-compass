//! Locates RouteE Powertrain V2 model files. Assumes that any storage directory mirrors the scaffolding in
//! [model library](https://huggingface.co/NatLabRockies/routee-powertrain-model-library):
//! `<repository_directory>/v2/<make>/<vehicle_slug>/<year>/<config_slug>/v<N>/metadata.json`
use crate::model::model_identifier::ModelIdentifier;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const LIBRARY_ROOT: &str = "v2";
const METADATA_FILE: &str = "metadata.json";
const DEFAULT_HUGGING_FACE_REPO_ID: &str = "NatLabRockies/routee-powertrain-model-library";

/// a local or huggingface location where prediction models are stored
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PredictionModelStorage {
    /// a local directory containing a `v2/` model library
    Local { repository_directory: String },
    /// a Hugging Face model library. not yet supported; skipped during lookup.
    HuggingFace {
        repo_id: Option<String>,
        revision: Option<String>,
        cache_dir: Option<String>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum PredictionModelStorageError {
    #[error("invalid model identifier '{0}': {1}")]
    InvalidIdentifier(String, String),
    #[error("model '{identifier}' not found in any storage location: [{}]", searched.join(", "))]
    NotFound {
        identifier: String,
        searched: Vec<String>,
    },
    #[error("failed to read '{}': {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// returns the path to the model's `metadata.json` from the first storage location containing it.
/// if the identifier has no version, the *latest* version in that location is used.
pub fn find_model_metadata_path(
    identifier: &ModelIdentifier,
    storages: &[PredictionModelStorage],
) -> Result<PathBuf, PredictionModelStorageError> {
    let library_path = LibraryPath::try_from(identifier)?;
    let mut searched = vec![];
    for storage in storages {
        match storage {
            PredictionModelStorage::Local {
                repository_directory,
            } => {
                if let Some(path) = find_local(&library_path, Path::new(repository_directory))? {
                    return Ok(path);
                }
                searched.push(format!("local '{repository_directory}'"));
            }
            PredictionModelStorage::HuggingFace { repo_id, .. } => {
                // TODO: check the Hugging Face cache, then download the model into it.
                let repo_id = repo_id.as_deref().unwrap_or(DEFAULT_HUGGING_FACE_REPO_ID);
                log::warn!("skipping Hugging Face source '{repo_id}': not yet supported");
                searched.push(format!("hugging face '{repo_id}' (not yet supported)"));
            }
        }
    }
    Err(PredictionModelStorageError::NotFound {
        identifier: identifier.to_string(),
        searched,
    })
}

/// a model's location relative to the `v2/` library root
struct LibraryPath {
    /// make, vehicle_slug, year, config_slug
    segments: [String; 4],
    version: Option<u64>,
}

impl TryFrom<&ModelIdentifier> for LibraryPath {
    type Error = PredictionModelStorageError;

    fn try_from(identifier: &ModelIdentifier) -> Result<Self, Self::Error> {
        let invalid = |msg: &str| {
            PredictionModelStorageError::InvalidIdentifier(identifier.to_string(), msg.to_string())
        };
        let (segments, version) = match identifier {
            ModelIdentifier::FullyQualifiedId(id) => {
                let mut parts: Vec<String> = id.split('/').map(String::from).collect();
                let version = if parts.len() == 5 { parts.pop() } else { None };
                (parts, version)
            }
            ModelIdentifier::StructuredId {
                make,
                model,
                year,
                variant,
                version,
            } => {
                let variant = variant.clone().ok_or_else(|| invalid("missing variant"))?;
                let segments = vec![make.clone(), model.clone(), year.to_string(), variant];
                (segments, version.clone())
            }
        };
        // identifiers are user input, so keep them from escaping the library directory
        if segments
            .iter()
            .any(|s| s.is_empty() || s == "." || s == ".." || s.contains(['/', '\\', ':']))
        {
            return Err(invalid("path segments must be non-empty file names"));
        }
        let segments = segments
            .try_into()
            .map_err(|_| invalid("expected 'make/vehicle_slug/year/config_slug[/v<N>]'"))?;
        let version = version
            .map(|v| v.strip_prefix('v').unwrap_or(&v).parse().ok())
            .map(|v| v.ok_or_else(|| invalid("version must be 'v<N>' or '<N>'")))
            .transpose()?;
        Ok(LibraryPath { segments, version })
    }
}

fn find_local(
    library_path: &LibraryPath,
    repository_directory: &Path,
) -> Result<Option<PathBuf>, PredictionModelStorageError> {
    let mut config_dir = repository_directory.join(LIBRARY_ROOT);
    config_dir.extend(&library_path.segments);
    let metadata_path = |version: u64| config_dir.join(format!("v{version}")).join(METADATA_FILE);

    if let Some(version) = library_path.version {
        let path = metadata_path(version);
        return Ok(path.is_file().then_some(path));
    }
    let entries = match std::fs::read_dir(&config_dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        result => result.map_err(|source| PredictionModelStorageError::Io {
            path: config_dir.clone(),
            source,
        })?,
    };
    let latest = entries
        .filter_map(|e| {
            e.ok()?
                .file_name()
                .to_str()?
                .strip_prefix('v')?
                .parse()
                .ok()
        })
        .filter(|version| metadata_path(*version).is_file())
        .max();
    Ok(latest.map(metadata_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;

    const CONFIG: &str = "toyota/camry_ice/2016/rf_base_67ae9982";

    fn fq(id: &str) -> ModelIdentifier {
        ModelIdentifier::FullyQualifiedId(id.to_string())
    }

    fn local(dir: &tempfile::TempDir) -> PredictionModelStorage {
        let repository_directory = dir.path().to_string_lossy().into_owned();
        PredictionModelStorage::Local {
            repository_directory,
        }
    }

    fn metadata_path(dir: &tempfile::TempDir, version: u64) -> PathBuf {
        let version_dir = dir
            .path()
            .join("v2")
            .join(CONFIG)
            .join(format!("v{version}"));
        version_dir.join(METADATA_FILE)
    }

    /// creates a local model library containing the given versions of [`CONFIG`]
    fn local_library(versions: &[u64]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for version in versions {
            let path = metadata_path(&dir, *version);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "{}").unwrap();
        }
        dir
    }

    #[test]
    fn test_library_path_from_identifier() {
        let structured = ModelIdentifier::StructuredId {
            make: "toyota".to_string(),
            model: "camry_ice".to_string(),
            year: NonZeroU64::new(2016).unwrap(),
            variant: Some("rf_base_67ae9982".to_string()),
            version: Some("3".to_string()),
        };
        for (identifier, version) in [(fq(&format!("{CONFIG}/v2")), 2), (structured, 3)] {
            let path = LibraryPath::try_from(&identifier).unwrap();
            assert_eq!(
                (path.segments.join("/"), path.version),
                (CONFIG.to_string(), Some(version))
            );
        }
    }

    #[test]
    fn test_library_path_rejects_invalid_identifiers() {
        for id in [
            "toyota/camry_ice/2016",
            "toyota/../2016/rf_base_67ae9982",
            &format!("{CONFIG}/latest"),
            &format!("{CONFIG}/v1/extra"),
        ] {
            let result = LibraryPath::try_from(&fq(id));
            assert!(
                matches!(
                    result,
                    Err(PredictionModelStorageError::InvalidIdentifier(..))
                ),
                "{id}"
            );
        }
    }

    #[test]
    fn test_find_model_metadata_path_selects_version() {
        let dir = local_library(&[1, 2, 10]);
        // an incomplete version directory without metadata.json is skipped
        let incomplete = metadata_path(&dir, 11).with_file_name("model.onnx"); // replace v11/metadata.json with v11/model.onnx
        std::fs::create_dir_all(incomplete.parent().unwrap()).unwrap();
        std::fs::write(incomplete, "").unwrap();
        let storages = [local(&dir)];
        let latest = find_model_metadata_path(&fq(CONFIG), &storages).unwrap();
        assert_eq!(
            latest,
            metadata_path(&dir, 10),
            "incomplete v11 directory should be skipped"
        );
        let pinned = find_model_metadata_path(&fq(&format!("{CONFIG}/v2")), &storages).unwrap();
        assert_eq!(pinned, metadata_path(&dir, 2));
    }

    #[test]
    fn test_find_model_metadata_path_searches_storage_in_order() {
        let (empty, first, second) = (local_library(&[]), local_library(&[1]), local_library(&[2]));
        let hugging_face = PredictionModelStorage::HuggingFace {
            repo_id: None,
            revision: None,
            cache_dir: None,
        };
        let storages = [hugging_face, local(&empty), local(&first), local(&second)];
        let path = find_model_metadata_path(&fq(CONFIG), &storages).unwrap();
        assert_eq!(path, metadata_path(&first, 1));
    }

    #[test]
    fn test_find_model_metadata_path_not_found() {
        let dir = local_library(&[1]);
        match find_model_metadata_path(&fq(&format!("{CONFIG}/v2")), &[local(&dir)]) {
            Err(PredictionModelStorageError::NotFound { searched, .. }) => {
                assert_eq!(searched.len(), 1)
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
