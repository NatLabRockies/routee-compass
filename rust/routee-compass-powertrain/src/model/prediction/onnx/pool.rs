use crate::model::model_identifier::ModelIdentifier;
use crate::model::prediction::onnx::SessionLease;
use crate::util::LruResourcePool;
use ort::session::Session;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
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

impl Default for SessionPoolConfig {
    fn default() -> Self {
        Self::Global { max_sessions: 128 }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SessionPoolError {
    #[error("Failed to create ONNX SessionBuilder for {key}: {e}")]
    SessionBuilder { key: ModelIdentifier, e: ort::Error },
    #[error("Failed to commit ONNX model {key} from memory: {e}")]
    CommitFromMemory { key: ModelIdentifier, e: ort::Error },
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
    /// On a cache miss, builds a new [`Session`] from `onnx_bytes` outside the pool lock.
    pub fn acquire(
        &self,
        key: &ModelIdentifier,
        onnx_bytes: &[u8],
    ) -> Result<SessionLease, SessionPoolError> {
        let session = match self.checkout(key) {
            Some(cached_session) => cached_session,
            None => {
                // Cache miss: build a new Session from the in-memory ONNX bytes
                Session::builder()
                    .map_err(|e| SessionPoolError::SessionBuilder {
                        key: key.clone(),
                        e,
                    })?
                    .commit_from_memory(onnx_bytes)
                    .map_err(|e| SessionPoolError::CommitFromMemory {
                        key: key.clone(),
                        e,
                    })?
            }
        };

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
