//! Locates RouteE Powertrain V2 model files. Assumes that any `repository_directory` mirrors the scaffolding in
//! [model library](https://huggingface.co/NatLabRockies/routee-powertrain-model-library):
//! `<repository_directory>/v2/<make>/<vehicle_slug>/<year>/<config_slug>/v<N>/metadata.json`
use crate::model::model_identifier::ModelIdentifier;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

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
    let relative_model_path = RelativeModelPath::try_from(identifier)?;
    let mut searched = vec![];

    for storage in storages {
        match storage {
            PredictionModelStorage::Local {
                repository_directory,
            } => {
                if let Some(path) =
                    find_local(Path::new(repository_directory), &relative_model_path)?
                {
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
struct RelativeModelPath {
    to_config_slug: PathBuf, // `<make>/<vehicle_slug>/<year>/<config_slug>`
    version: Option<u64>, // the `<to_config_slug>/v<N>` directory. `None` selects the latest version
}

impl TryFrom<&ModelIdentifier> for RelativeModelPath {
    type Error = PredictionModelStorageError;

    fn try_from(identifier: &ModelIdentifier) -> Result<Self, Self::Error> {
        // convert the identifier to its `FullyQualifiedId` representation & then to a PathBuf.
        let path = match identifier.to_fully_qualified().map_err(|e| {
            PredictionModelStorageError::InvalidIdentifier(identifier.to_string(), e.to_string())
        })? {
            ModelIdentifier::FullyQualifiedId(id) => PathBuf::from(id),
            ModelIdentifier::StructuredId { .. } => {
                return Err(PredictionModelStorageError::InvalidIdentifier(
                    identifier.to_string(),
                    "could not convert to a fully qualified id".to_string(),
                ))
            }
        };

        if !path.components().all(|c| matches!(c, Component::Normal(_))) {
            return Err(PredictionModelStorageError::InvalidIdentifier(
                identifier.to_string(),
                "must be a relative path of file names".to_string(),
            ));
        }

        let (path, version) = match path.components().count() {
            4 => (path.as_path(), None), // no version specified
            5 => (
                path.parent().unwrap_or(&path),
                path.file_name().and_then(|name| name.to_str()),
            ), // version specified
            _ => {
                return Err(PredictionModelStorageError::InvalidIdentifier(
                    identifier.to_string(),
                    "expected 'make/vehicle_slug/year/config_slug[/v<N>]'".to_string(),
                ))
            }
        };

        let version = version
            .map(|v| {
                v.strip_prefix('v').unwrap_or(v).parse().map_err(|_| {
                    PredictionModelStorageError::InvalidIdentifier(
                        identifier.to_string(),
                        "version must be 'v<N>' or '<N>'".to_string(),
                    )
                })
            })
            .transpose()?;

        Ok(RelativeModelPath {
            to_config_slug: path.to_path_buf(),
            version,
        })
    }
}

/// returns the model's `metadata.json` path within a local storage, or `None` if it is not present.
fn find_local(
    repository_directory: &Path, // The root of a `Local` storage; the directory containing `v2/<to_config_slug>/<vN>`
    relative_model_path: &RelativeModelPath, // model location relative to `v2/`
) -> Result<Option<PathBuf>, PredictionModelStorageError> {
    let config_slug_dir = repository_directory
        .join(LIBRARY_ROOT) // .../v2/
        .join(&relative_model_path.to_config_slug); // .../<to_config_slug>

    let metadata_path = |version: u64| {
        config_slug_dir
            .join(format!("v{version}"))
            .join(METADATA_FILE)
    };

    if let Some(version) = relative_model_path.version {
        let path = metadata_path(version);
        return Ok(path.is_file().then_some(path)); // return Ok(None) if the path to the version folder isn't found, and proceed to other storages.
    }

    // check which version folders are available in the config_slug_dir
    let entries = match std::fs::read_dir(&config_slug_dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        result => result.map_err(|source| PredictionModelStorageError::Io {
            path: config_slug_dir.clone(),
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
        .max(); // max takes the latest.

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
    fn test_relative_model_path_from_identifier() {
        let structured = ModelIdentifier::StructuredId {
            make: "toyota".to_string(),
            vehicle_slug: "camry_ice".to_string(),
            year: NonZeroU64::new(2016).unwrap(),
            config_slug: "rf_base_67ae9982".to_string(),
            version: Some("3".to_string()),
        };
        for (identifier, version) in [(fq(&format!("{CONFIG}/v2")), 2), (structured, 3)] {
            let path = RelativeModelPath::try_from(&identifier).unwrap();
            assert_eq!(
                (path.to_config_slug, path.version),
                (PathBuf::from(CONFIG), Some(version))
            );
        }
    }

    #[test]
    fn test_relative_model_path_rejects_invalid_identifiers() {
        for id in [
            "toyota/camry_ice/2016",
            "toyota/../2016/rf_base_67ae9982",
            &format!("{CONFIG}/.."),
            &format!("/{CONFIG}"),
            &format!("{CONFIG}/latest"),
            &format!("{CONFIG}/v1/extra"),
        ] {
            let result = RelativeModelPath::try_from(&fq(id));
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
