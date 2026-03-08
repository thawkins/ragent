pub mod processor;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use crate::event::{Event, EventBus};
use crate::message::Message;
use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub project_id: String,
    pub directory: PathBuf,
    pub parent_id: Option<String>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub summary: Option<SessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub additions: u64,
    pub deletions: u64,
    pub files_changed: u64,
}

pub struct SessionManager {
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
}

impl SessionManager {
    pub fn new(storage: Arc<Storage>, event_bus: Arc<EventBus>) -> Self {
        Self { storage, event_bus }
    }

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
            created_at: now,
            updated_at: now,
            archived_at: None,
            summary: None,
        };

        self.event_bus.publish(Event::SessionCreated {
            session_id: id,
        });

        Ok(session)
    }

    pub fn get_session(&self, id: &str) -> anyhow::Result<Option<Session>> {
        let row = self.storage.get_session(id)?;
        Ok(row.map(|r| row_to_session(r)))
    }

    pub fn list_sessions(&self) -> anyhow::Result<Vec<Session>> {
        let rows = self.storage.list_sessions()?;
        Ok(rows.into_iter().map(row_to_session).collect())
    }

    pub fn archive_session(&self, id: &str) -> anyhow::Result<()> {
        self.storage.archive_session(id)?;
        self.event_bus.publish(Event::SessionUpdated {
            session_id: id.to_string(),
        });
        Ok(())
    }

    pub fn get_messages(&self, session_id: &str) -> anyhow::Result<Vec<Message>> {
        self.storage.get_messages(session_id)
    }
}

fn row_to_session(row: crate::storage::SessionRow) -> Session {
    let created_at = DateTime::parse_from_rfc3339(&row.created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let archived_at = row.archived_at.and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });
    let summary = row.summary.and_then(|s| serde_json::from_str(&s).ok());

    Session {
        id: row.id,
        title: row.title,
        project_id: row.project_id,
        directory: PathBuf::from(row.directory),
        parent_id: row.parent_id,
        version: row.version,
        created_at,
        updated_at,
        archived_at,
        summary,
    }
}
