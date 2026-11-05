//! a session pool for hydraded ort model sessions.
//!
//! the user is intended to use this as follows in EnergyModelService:
//!   1. retrieve model_key from query
//!   2. use another service to ensure we have the model downloaded to disk. this becomes the path of a [CommitFromSource::File] instance.
//!   3. call SessionPool::acquire with the key and the fallback CommitFromSource value, receiving a [SessionLease].
//!   4. pass the [SessionLease] to the TraversalModel.
//!   5. traversal model uses the session via `self.lease.with_session(|s| s.run(...))`.
//!   6. when the TraversalModel is Dropped, the lease is [Drop]ped, which returns the Session to the pool.
//!
use crate::model::model_identifier::ModelIdentifier;
use crate::model::prediction::onnx::SessionLease;
use crate::util::LruResourcePool;
use ort::session::Session;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Configures how model sessions are pooled and scheduled across threads.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionPoolConfig {
    /// A single shared LRU pool across all threads protected by a Mutex.
    /// Best for random/high-cardinality model demand and bounded total memory.
    Global { max_sessions: usize },
    /// Each worker thread maintains its own lock-free thread-local LRU cache.
    /// Best for single-model or small grid-search studies with zero lock overhead.
    ThreadLocal { max_sessions_per_thread: usize },
}

/// when a cache miss happens, we can build a Session instance from this source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommitFromSource {
    /// we have a path to a local .onnx file.
    File { path: PathBuf },
    /// we have the onnx file stored as bytes in memory for quick rehydration.
    Memory { bytes: Vec<u8> },
}

impl Default for SessionPoolConfig {
    fn default() -> Self {
        Self::Global { max_sessions: 128 }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SessionPoolError {
    #[error("Failed to create ONNX SessionBuilder for {key}: {error}")]
    SessionBuilder {
        key: ModelIdentifier,
        error: ort::Error,
    },
    #[error("Failed to commit ONNX model {key} from memory: {error}")]
    CommitFromMemory {
        key: ModelIdentifier,
        error: ort::Error,
    },
    #[error("Failed to commit ONNX model {key} from file location {path}: {error}")]
    CommitFromFile {
        key: ModelIdentifier,
        path: PathBuf,
        error: ort::Error,
    },
}

/// lightweight handle for a pool of onnx Session instances that can be used.
/// this can be cloned and owned by multiple values.
#[derive(Clone)]
pub enum SessionPool {
    Global {
        pool: Arc<Mutex<LruResourcePool<ModelIdentifier, ort::session::Session>>>,
    },
    ThreadLocal {
        pool_id: usize,
        max_sessions_per_thread: usize,
    },
}

/// identifiers used across threads for the id of a thread-local session pool.
static NEXT_POOL_ID: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Per-thread storage keyed by pool_id -> LruResourcePool
    static THREAD_POOLS: RefCell<HashMap<usize, LruResourcePool<ModelIdentifier, ort::session::Session>>> =
        RefCell::new(HashMap::new());
}

impl SessionPool {
    /// create a new [`SessionPool`] using the given configuration.
    pub fn new(config: &SessionPoolConfig) -> Self {
        match config {
            SessionPoolConfig::Global { max_sessions } => Self::Global {
                pool: Arc::new(Mutex::new(LruResourcePool::new(*max_sessions))),
            },
            SessionPoolConfig::ThreadLocal {
                max_sessions_per_thread,
            } => Self::ThreadLocal {
                pool_id: NEXT_POOL_ID.fetch_add(1, Ordering::Relaxed),
                max_sessions_per_thread: *max_sessions_per_thread,
            },
        }
    }

    /// retrieve an instance of a Session for some key from the pool. if none are available, returns None.
    pub fn checkout(&self, key: &ModelIdentifier) -> Option<ort::session::Session> {
        match self {
            Self::Global { pool } => pool.lock().ok()?.checkout(key),
            Self::ThreadLocal {
                pool_id,
                max_sessions_per_thread,
            } => THREAD_POOLS.with(|pools| {
                let mut map = pools.borrow_mut();
                let table = map
                    .entry(*pool_id)
                    .or_insert_with(|| LruResourcePool::new(*max_sessions_per_thread));
                table.checkout(key)
            }),
        }
    }

    /// checks back in a session for some key.
    pub fn checkin(&self, key: ModelIdentifier, session: ort::session::Session) {
        match self {
            Self::Global { pool } => {
                if let Ok(mut guard) = pool.lock() {
                    guard.checkin(key, session);
                }
            }
            Self::ThreadLocal {
                pool_id,
                max_sessions_per_thread,
            } => {
                THREAD_POOLS.with(|pools| {
                    let mut map = pools.borrow_mut();
                    let table = map
                        .entry(*pool_id)
                        .or_insert_with(|| LruResourcePool::new(*max_sessions_per_thread));
                    table.checkin(key, session);
                });
            }
        }
    }

    /// Acquires a [`SessionLease`] for the duration of a query.
    /// On a cache hit, reuses an idle [`Session`] from the pool.
    /// On a cache miss, builds a new [`Session`] from `model_path` outside the pool lock.
    pub fn acquire(
        &self,
        key: &ModelIdentifier,
        commit_from: &CommitFromSource,
    ) -> Result<SessionLease, SessionPoolError> {
        let session = match self.checkout(key) {
            Some(cached_session) => Ok(cached_session),
            None => {
                // Cache miss: build a new Session.
                let mut builder =
                    Session::builder().map_err(|e| SessionPoolError::SessionBuilder {
                        key: key.clone(),
                        error: e,
                    })?;

                match commit_from {
                    CommitFromSource::File { path } => {
                        builder.commit_from_file(path).map_err(|error| {
                            SessionPoolError::CommitFromFile {
                                key: key.clone(),
                                path: path.to_path_buf(),
                                error,
                            }
                        })
                    }
                    CommitFromSource::Memory { bytes } => builder
                        .commit_from_memory(bytes)
                        .map_err(|e| SessionPoolError::CommitFromMemory {
                            key: key.clone(),
                            error: e,
                        }),
                }
            }
        }?;

        Ok(SessionLease::new(
            key.clone(),
            Mutex::new(Some(session)),
            self.clone(),
        ))
    }
}

impl From<SessionPoolConfig> for SessionPool {
    fn from(value: SessionPoolConfig) -> Self {
        Self::new(&value)
    }
}

impl From<&SessionPoolConfig> for SessionPool {
    fn from(value: &SessionPoolConfig) -> Self {
        Self::new(value)
    }
}
