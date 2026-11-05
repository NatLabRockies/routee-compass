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
use std::sync::{Arc, Mutex, Weak};

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
        key: Box<ModelIdentifier>,
        error: ort::Error,
    },
    #[error("Failed to commit ONNX model {key} from memory: {error}")]
    CommitFromMemory {
        key: Box<ModelIdentifier>,
        error: ort::Error,
    },
    #[error("Failed to commit ONNX model {key} from file location {}: {error}", path.display())]
    CommitFromFile {
        key: Box<ModelIdentifier>,
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
        token: Arc<ThreadLocalPoolToken>,
        max_sessions_per_thread: usize,
    },
}

/// Shared ownership token tracking the lifetime of a `ThreadLocal` [`SessionPool`].
/// When the last handle (and lease) for a pool is dropped, this token cleans up the
/// current thread's entry immediately and causes all other worker threads' `Weak`
/// references to expire so they are pruned on next access.
///
/// in reality, this won't be a problem for CLI runs, but without this, a thread-local
/// SessionPool would leak resources if the user initializes Compass/Bambam multiple times
/// in Python because of how Rayon manages a global thread pool.
#[derive(Debug)]
pub struct ThreadLocalPoolToken {
    pool_id: usize,
}

impl Drop for ThreadLocalPoolToken {
    fn drop(&mut self) {
        let _ = THREAD_POOLS.try_with(|pools| {
            if let Ok(mut map) = pools.try_borrow_mut() {
                map.remove(&self.pool_id);
            }
        });
    }
}

struct ThreadLocalPoolEntry {
    token: Weak<ThreadLocalPoolToken>,
    pool: LruResourcePool<ModelIdentifier, ort::session::Session>,
}

/// identifiers used across threads for the id of a thread-local session pool.
static NEXT_POOL_ID: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Per-thread storage keyed by pool_id -> ThreadLocalPoolEntry
    static THREAD_POOLS: RefCell<HashMap<usize, ThreadLocalPoolEntry>> =
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
            } => {
                let pool_id = NEXT_POOL_ID.fetch_add(1, Ordering::Relaxed);
                Self::ThreadLocal {
                    pool_id,
                    token: Arc::new(ThreadLocalPoolToken { pool_id }),
                    max_sessions_per_thread: *max_sessions_per_thread,
                }
            }
        }
    }

    /// Removes any thread-local session pools on the current worker thread whose
    /// [`SessionPool`] handles have all been dropped.
    pub fn prune_expired_thread_local_pools() {
        THREAD_POOLS.with(|pools| {
            pools
                .borrow_mut()
                .retain(|_, entry| entry.token.strong_count() > 0);
        });
    }

    /// retrieve an instance of a Session for some key from the pool. if none are available, returns None.
    pub fn checkout(&self, key: &ModelIdentifier) -> Option<ort::session::Session> {
        match self {
            Self::Global { pool } => pool.lock().ok()?.checkout(key),
            Self::ThreadLocal {
                pool_id,
                token,
                max_sessions_per_thread,
            } => THREAD_POOLS.with(|pools| {
                let mut map = pools.borrow_mut();
                map.retain(|_, entry| entry.token.strong_count() > 0);
                let entry = map.entry(*pool_id).or_insert_with(|| ThreadLocalPoolEntry {
                    token: Arc::downgrade(token),
                    pool: LruResourcePool::new(*max_sessions_per_thread),
                });
                entry.pool.checkout(key)
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
                token,
                max_sessions_per_thread,
            } => {
                THREAD_POOLS.with(|pools| {
                    let mut map = pools.borrow_mut();
                    map.retain(|_, entry| entry.token.strong_count() > 0);
                    let entry = map.entry(*pool_id).or_insert_with(|| ThreadLocalPoolEntry {
                        token: Arc::downgrade(token),
                        pool: LruResourcePool::new(*max_sessions_per_thread),
                    });
                    entry.pool.checkin(key, session);
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
                        key: Box::new(key.clone()),
                        error: e,
                    })?;

                match commit_from {
                    CommitFromSource::File { path } => {
                        builder.commit_from_file(path).map_err(|error| {
                            SessionPoolError::CommitFromFile {
                                key: Box::new(key.clone()),
                                path: path.to_path_buf(),
                                error,
                            }
                        })
                    }
                    CommitFromSource::Memory { bytes } => builder
                        .commit_from_memory(bytes)
                        .map_err(|e| SessionPoolError::CommitFromMemory {
                            key: Box::new(key.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr2;
    use routee_compass_core::model::traversal::TraversalModelError;

    fn camry_model_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("test")
            .join("Toyota_Camry.onnx")
    }

    fn sample_key(name: &str) -> ModelIdentifier {
        ModelIdentifier::FullyQualifiedId(name.to_string())
    }

    fn run_camry_inference(lease: &SessionLease) -> f32 {
        lease
            .with_session(|session| {
                let input = ort::value::Value::from_array(arr2(&[[50.0f32, 0.0f32]]))
                    .map_err(|e| TraversalModelError::TraversalModelFailure(e.to_string()))?;
                let outputs = session
                    .run(ort::inputs!["input" => input])
                    .map_err(|e| TraversalModelError::TraversalModelFailure(e.to_string()))?;
                let (_name, tensor) = outputs.iter().next().unwrap();
                let data = tensor
                    .try_extract_tensor::<f32>()
                    .map_err(|e| TraversalModelError::TraversalModelFailure(e.to_string()))?;
                Ok(data.1[0])
            })
            .unwrap()
    }

    #[test]
    fn test_global_pool_acquire_drop_and_reuse() {
        let pool = SessionPool::new(&SessionPoolConfig::Global { max_sessions: 2 });
        let key = sample_key("toyota/camry/2016");
        let source = CommitFromSource::File {
            path: camry_model_path(),
        };

        // Initially empty
        assert!(pool.checkout(&key).is_none());

        // Acquire builds a new session on cache miss
        let lease = pool.acquire(&key, &source).unwrap();
        let rate = run_camry_inference(&lease);
        assert!(rate > 0.0 && rate.is_finite());

        // While lease is held, pool has no idle session
        assert!(pool.checkout(&key).is_none());

        // Dropping the lease checks the session back into the pool
        drop(lease);

        let reused_session = pool.checkout(&key);
        assert!(reused_session.is_some());
        assert!(pool.checkout(&key).is_none());
    }

    #[test]
    fn test_acquire_from_memory_source() {
        let pool = SessionPool::new(&SessionPoolConfig::Global { max_sessions: 1 });
        let key = sample_key("toyota/camry_mem/2016");
        let bytes = std::fs::read(camry_model_path()).unwrap();
        let source = CommitFromSource::Memory { bytes };

        let lease = pool.acquire(&key, &source).unwrap();
        let rate = run_camry_inference(&lease);
        assert!(rate > 0.0 && rate.is_finite());
    }

    #[test]
    fn test_acquire_invalid_path_returns_error() {
        let pool = SessionPool::new(&SessionPoolConfig::Global { max_sessions: 1 });
        let key = sample_key("invalid/model/2020");
        let source = CommitFromSource::File {
            path: PathBuf::from("non_existent_model.onnx"),
        };

        let result = pool.acquire(&key, &source);
        assert!(matches!(
            result,
            Err(SessionPoolError::CommitFromFile { .. })
        ));
    }

    #[test]
    fn test_thread_local_pool_isolation() {
        let pool = SessionPool::new(&SessionPoolConfig::ThreadLocal {
            max_sessions_per_thread: 2,
        });
        let key = sample_key("toyota/camry_tl/2016");
        let source = CommitFromSource::File {
            path: camry_model_path(),
        };

        // Acquire and drop on main thread
        let lease = pool.acquire(&key, &source).unwrap();
        drop(lease);

        // Another thread should not see main thread's cached session
        let pool_clone = pool.clone();
        let key_clone = key.clone();
        let other_thread_hit =
            std::thread::spawn(move || pool_clone.checkout(&key_clone).is_some())
                .join()
                .unwrap();
        assert!(!other_thread_hit);

        // Main thread still has its cached session
        assert!(pool.checkout(&key).is_some());
    }

    #[test]
    fn test_thread_local_pool_cleanup_on_drop() {
        let key = sample_key("toyota/camry_tl_cleanup/2016");
        let source = CommitFromSource::File {
            path: camry_model_path(),
        };

        let pool_1 = SessionPool::new(&SessionPoolConfig::ThreadLocal {
            max_sessions_per_thread: 2,
        });
        let pool_1_id = match &pool_1 {
            SessionPool::ThreadLocal { pool_id, .. } => *pool_id,
            _ => panic!("Expected ThreadLocal pool"),
        };

        // Populate pool_1 on main thread
        let lease = pool_1.acquire(&key, &source).unwrap();
        drop(lease);
        assert!(THREAD_POOLS.with(|pools| pools.borrow().contains_key(&pool_1_id)));

        // Dropping the last handle on the current thread removes its entry immediately
        drop(pool_1);
        assert!(!THREAD_POOLS.with(|pools| pools.borrow().contains_key(&pool_1_id)));

        // Test cross-thread expiration: worker populates an entry, main thread drops the pool,
        // and the worker prunes the expired entry on next pool access.
        let pool_2 = SessionPool::new(&SessionPoolConfig::ThreadLocal {
            max_sessions_per_thread: 2,
        });
        let pool_2_id = match &pool_2 {
            SessionPool::ThreadLocal { pool_id, .. } => *pool_id,
            _ => panic!("Expected ThreadLocal pool"),
        };
        let pool_3 = SessionPool::new(&SessionPoolConfig::ThreadLocal {
            max_sessions_per_thread: 2,
        });
        let pool_3_id = match &pool_3 {
            SessionPool::ThreadLocal { pool_id, .. } => *pool_id,
            _ => panic!("Expected ThreadLocal pool"),
        };

        let (tx_populated, rx_populated) = std::sync::mpsc::channel();
        let (tx_dropped, rx_dropped) = std::sync::mpsc::channel();

        let pool_2_worker = pool_2.clone();
        let pool_3_worker = pool_3.clone();
        let key_worker = key.clone();
        let source_worker = source.clone();

        let worker = std::thread::spawn(move || {
            let lease = pool_2_worker.acquire(&key_worker, &source_worker).unwrap();
            drop(lease);
            drop(pool_2_worker);
            assert!(THREAD_POOLS.with(|pools| pools.borrow().contains_key(&pool_2_id)));
            tx_populated.send(()).unwrap();

            // Wait for main thread to drop its last handle to pool_2
            rx_dropped.recv().unwrap();

            // Accessing another pool on this worker should prune the expired pool_2 entry
            let _ = pool_3_worker.checkout(&key_worker);
            assert!(!THREAD_POOLS.with(|pools| pools.borrow().contains_key(&pool_2_id)));
            assert!(THREAD_POOLS.with(|pools| pools.borrow().contains_key(&pool_3_id)));
        });

        rx_populated.recv().unwrap();
        drop(pool_2);
        tx_dropped.send(()).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn test_concurrent_global_pool_contention() {
        let max_sessions = 3;
        let pool = SessionPool::new(&SessionPoolConfig::Global { max_sessions });
        let key = sample_key("toyota/camry_concurrent/2016");
        let source = Arc::new(CommitFromSource::File {
            path: camry_model_path(),
        });

        let handles: Vec<_> = (0..6)
            .map(|_| {
                let pool_clone = pool.clone();
                let key_clone = key.clone();
                let source_clone = Arc::clone(&source);
                std::thread::spawn(move || {
                    for _ in 0..5 {
                        let lease = pool_clone.acquire(&key_clone, &source_clone).unwrap();
                        let rate = run_camry_inference(&lease);
                        assert!(rate > 0.0);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        if let SessionPool::Global { pool: inner } = &pool {
            let guard = inner.lock().unwrap();
            assert!(guard.len() <= max_sessions);
            assert!(!guard.is_empty());
        } else {
            panic!("Expected Global pool");
        }
    }
}
