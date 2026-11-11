use super::SessionPool;
use crate::model::model_identifier::ModelIdentifier;
use ort::session::Session;
use routee_compass_core::model::traversal::TraversalModelError;
use std::sync::Mutex;

/// RAII guard that holds an ONNX [`Session`] for a single query and automatically
/// checks it back into the [`SessionPool`] when the query finishes and drops the model.
pub struct SessionLease {
    key: ModelIdentifier,
    session: Mutex<Option<Session>>,
    pool: SessionPool,
}

impl SessionLease {
    /// start a new session lease. the session is automagically returned to the pool when [Drop] is
    /// called.
    pub fn new(key: ModelIdentifier, session: Mutex<Option<Session>>, pool: SessionPool) -> Self {
        Self { key, session, pool }
    }

    /// Provides mutable access to the leased [`Session`] for running inference.
    pub fn with_session<F, R>(&self, f: F) -> Result<R, TraversalModelError>
    where
        F: FnOnce(&mut Session) -> Result<R, TraversalModelError>,
    {
        let mut guard = self.session.lock().map_err(|e| {
            TraversalModelError::TraversalModelFailure(format!("ONNX session lock poisoned: {e}"))
        })?;
        let session = guard.as_mut().ok_or_else(|| {
            TraversalModelError::TraversalModelFailure(
                "ONNX session accessed after lease was dropped".to_string(),
            )
        })?;
        f(session)
    }
}

impl Drop for SessionLease {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.session.lock() {
            if let Some(session) = guard.take() {
                self.pool.checkin(self.key.clone(), session);
            }
        }
    }
}
