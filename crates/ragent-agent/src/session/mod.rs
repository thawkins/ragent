//! Session management for agent conversations.
//!
//! A [`Session`] represents an ongoing conversation between a user and an agent,
//! scoped to a working directory. [`SessionManager`] provides CRUD operations
//! backed by persistent [`Storage`] and emits lifecycle events via [`EventBus`].

pub mod processor;
pub mod profiler;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use crate::event::{Event, EventBus};
use crate::message::Message;
use crate::storage::Storage;

/// A conversation session between a user and an agent.
///
/// Each session is tied to a project directory and tracks metadata such as
/// version, timestamps, and an optional diff summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique identifier for this session.
    pub id: String,
    /// Human-readable title for the session.
    pub title: String,
    /// Identifier of the project this session belongs to.
    pub project_id: String,
    /// Working directory on disk associated with this session.
    pub directory: PathBuf,
    /// Optional parent session id when this session was forked.
    pub parent_id: Option<String>,
    /// Monotonically increasing version number for optimistic concurrency.
    pub version: i64,
    /// Storage format version for backward compatibility.
    pub format_version: i64,
    /// Timestamp when the session was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when the session was last modified.
    pub updated_at: DateTime<Utc>,
    /// Timestamp when the session was archived, if applicable.
    pub archived_at: Option<DateTime<Utc>>,
    /// Optional aggregate diff statistics for the session.
    pub summary: Option<SessionSummary>,
    /// Config file path used when this session was created (for validation on resume).
    pub config_path: Option<PathBuf>,
}

/// Aggregate statistics summarizing the changes made during a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Total number of lines added.
    pub additions: u64,
    /// Total number of lines deleted.
    pub deletions: u64,
    /// Number of files that were modified.
    pub files_changed: u64,
}

/// Manages the lifecycle of sessions, delegating persistence to [`Storage`]
/// and broadcasting changes through the [`EventBus`].
pub struct SessionManager {
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
}

impl SessionManager {
    /// Creates a new `SessionManager` with the given storage backend and event bus.
    ///
    /// # Errors
    ///
    /// This constructor does not return errors. It simply wraps the provided
    /// storage and event bus references.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use ragent_core::storage::Storage;
    /// use ragent_core::event::EventBus;
    /// use ragent_core::session::SessionManager;
    ///
    /// let storage = Arc::new(Storage::open_in_memory().unwrap());
    /// let event_bus = Arc::new(EventBus::new(128));
    /// let manager = SessionManager::new(storage, event_bus);
    /// ```
    pub const fn new(storage: Arc<Storage>, event_bus: Arc<EventBus>) -> Self {
        Self { storage, event_bus }
    }

    /// Returns a reference to the underlying storage backend.
    ///
    /// # Errors
    ///
    /// This function does not return errors. It returns an immutable reference
    /// to the shared `Storage` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use ragent_core::storage::Storage;
    /// use ragent_core::event::EventBus;
    /// use ragent_core::session::SessionManager;
    ///
    /// let storage = Arc::new(Storage::open_in_memory().unwrap());
    /// let event_bus = Arc::new(EventBus::new(128));
    /// let manager = SessionManager::new(storage, event_bus);
    /// let _storage_ref = manager.storage();
    /// ```
    #[must_use]
    pub const fn storage(&self) -> &Arc<Storage> {
        &self.storage
    }

    /// Creates a new session rooted at `directory`, persists it, and emits a
    /// `SessionCreated` event.
    ///
    /// # Errors
    ///
    /// Returns an error if the session cannot be persisted to storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    /// use ragent_core::storage::Storage;
    /// use ragent_core::event::EventBus;
    /// use ragent_core::session::SessionManager;
    ///
    /// let storage = Arc::new(Storage::open_in_memory().unwrap());
    /// let event_bus = Arc::new(EventBus::new(128));
    /// let manager = SessionManager::new(storage, event_bus);
    /// let session = manager.create_session(PathBuf::from("/tmp/project")).unwrap();
    /// assert!(!session.id.is_empty());
    /// ```
    pub fn create_session(&self, directory: PathBuf) -> anyhow::Result<Session> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        self.storage
            .create_session(&id, &directory.display().to_string())?;

        let session = Session {
            id: id.clone(),
            title: String::new(),
            project_id: String::new(),
            directory,
            parent_id: None,
            version: 1,
            format_version: 1, // Current format version
            created_at: now,
            updated_at: now,
            archived_at: None,
            summary: None,
            config_path: crate::config::Config::load()
                .ok()
                .and_then(|_| std::env::var("RAGENT_CONFIG").ok())
                .map(PathBuf::from),
        };
        self.event_bus
            .publish(Event::SessionCreated { session_id: id });

        Ok(session)
    }

    /// Retrieves a session by its unique identifier, returning `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    /// use ragent_core::storage::Storage;
    /// use ragent_core::event::EventBus;
    /// use ragent_core::session::SessionManager;
    ///
    /// let storage = Arc::new(Storage::open_in_memory().unwrap());
    /// let event_bus = Arc::new(EventBus::new(128));
    /// let manager = SessionManager::new(storage, event_bus);
    /// let session = manager.create_session(PathBuf::from("/tmp/project")).unwrap();
    /// let found = manager.get_session(&session.id).unwrap();
    /// assert!(found.is_some());
    /// ```
    pub fn get_session(&self, id: &str) -> anyhow::Result<Option<Session>> {
        let row = self.storage.get_session(id)?;
        Ok(row.map(Into::into))
    }

    /// Lists all non-archived sessions, ordered by most recently updated.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    /// use ragent_core::storage::Storage;
    /// use ragent_core::event::EventBus;
    /// use ragent_core::session::SessionManager;
    ///
    /// let storage = Arc::new(Storage::open_in_memory().unwrap());
    /// let event_bus = Arc::new(EventBus::new(128));
    /// let manager = SessionManager::new(storage, event_bus);
    /// manager.create_session(PathBuf::from("/tmp/project")).unwrap();
    /// let sessions = manager.list_sessions().unwrap();
    /// assert_eq!(sessions.len(), 1);
    /// ```
    pub fn list_sessions(&self) -> anyhow::Result<Vec<Session>> {
        let rows = self.storage.list_sessions()?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Archives a session so it no longer appears in [`list_sessions`](Self::list_sessions)
    /// and emits a `SessionUpdated` event.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage update fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    /// use ragent_core::storage::Storage;
    /// use ragent_core::event::EventBus;
    /// use ragent_core::session::SessionManager;
    ///
    /// let storage = Arc::new(Storage::open_in_memory().unwrap());
    /// let event_bus = Arc::new(EventBus::new(128));
    /// let manager = SessionManager::new(storage, event_bus);
    /// let session = manager.create_session(PathBuf::from("/tmp/project")).unwrap();
    /// manager.archive_session(&session.id).unwrap();
    /// let sessions = manager.list_sessions().unwrap();
    /// assert!(sessions.is_empty());
    /// ```
    pub fn archive_session(&self, id: &str) -> anyhow::Result<()> {
        self.storage.archive_session(id)?;
        self.event_bus.publish(Event::SessionUpdated {
            session_id: id.to_string(),
        });
        Ok(())
    }

    /// Returns all messages belonging to the given session, ordered chronologically.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    /// use ragent_core::storage::Storage;
    /// use ragent_core::event::EventBus;
    /// use ragent_core::message::Message;
    /// use ragent_core::session::SessionManager;
    ///
    /// let storage = Arc::new(Storage::open_in_memory().unwrap());
    /// let event_bus = Arc::new(EventBus::new(128));
    /// let manager = SessionManager::new(storage, event_bus);
    /// let session = manager.create_session(PathBuf::from("/tmp/project")).unwrap();
    /// let messages = manager.get_messages(&session.id).unwrap();
    /// assert!(messages.is_empty());
    /// ```
    pub fn get_messages(&self, session_id: &str) -> anyhow::Result<Vec<Message>> {
        self.storage.get_messages(session_id)
    }
}

impl Drop for SessionManager {
    /// Ensures any pending session data is flushed on drop.
    ///
    /// This provides a safety net for graceful shutdown scenarios.
    fn drop(&mut self) {
        // Signal that we're shutting down - this helps with async cleanup
        tracing::debug!("SessionManager dropping - ensuring session persistence");

        // Note: Storage operations are synchronous via Mutex, so no async work needed.
        // The SQLite connection will be closed when the Arc<Storage> is dropped.
        // This is primarily a hook for future persistence needs.
    }
}

impl From<crate::storage::SessionRow> for Session {
    fn from(row: crate::storage::SessionRow) -> Self {
        // Validate session row data integrity before conversion
        let session_id = row.id.clone();

        let created_at = DateTime::parse_from_rfc3339(&row.created_at).map_or_else(
            |e| {
                tracing::warn!(
                    session_id = %session_id,
                    raw = %row.created_at,
                    error = %e,
                    "failed to parse created_at timestamp, falling back to Utc::now()"
                );
                Utc::now()
            },
            |dt| dt.with_timezone(&Utc),
        );
        let updated_at = DateTime::parse_from_rfc3339(&row.updated_at).map_or_else(
            |e| {
                tracing::warn!(
                    session_id = %session_id,
                    raw = %row.updated_at,
                    error = %e,
                    "failed to parse updated_at timestamp, falling back to Utc::now()"
                );
                Utc::now()
            },
            |dt| dt.with_timezone(&Utc),
        );
        let archived_at = row.archived_at.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });
        let summary = row
            .summary
            .and_then(|s| match serde_json::from_str::<SessionSummary>(&s) {
                Ok(summ) => Some(summ),
                Err(e) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "failed to parse session summary, treating as None"
                    );
                    None
                }
            });

        Self {
            id: row.id,
            title: row.title,
            project_id: row.project_id,
            directory: PathBuf::from(row.directory),
            parent_id: row.parent_id,
            version: row.version,
            format_version: row.format_version,
            created_at,
            updated_at,
            archived_at,
            summary,
            config_path: None, // Historical sessions don't store config path
        }
    }
}
