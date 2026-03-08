use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

use crate::message::{Message, MessagePart, Role};

/// SQLite-backed storage for sessions, messages, and provider credentials.
pub struct Storage {
    conn: Mutex<Connection>,
}

/// Acquires the database connection lock, mapping a poisoned mutex to an anyhow error.
macro_rules! lock_conn {
    ($self:expr) => {
        $self.conn.lock().map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))
    };
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at {}", path.display()))?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&self) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                project_id TEXT NOT NULL DEFAULT '',
                directory TEXT NOT NULL,
                parent_id TEXT,
                version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                summary TEXT
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                parts TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE INDEX IF NOT EXISTS idx_messages_session
                ON messages(session_id, created_at);

            CREATE TABLE IF NOT EXISTS provider_auth (
                provider_id TEXT PRIMARY KEY,
                api_key TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS mcp_servers (
                id TEXT PRIMARY KEY,
                config TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'disabled',
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS snapshots (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                data TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            ",
        )?;
        Ok(())
    }

    // ── Session CRUD ──────────────────────────────────────────────

    pub fn create_session(&self, id: &str, directory: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, directory, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, directory, now, now],
        )?;
        Ok(())
    }

    pub fn get_session(
        &self,
        id: &str,
    ) -> Result<Option<SessionRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, title, project_id, directory, parent_id, version, \
             created_at, updated_at, archived_at, summary FROM sessions WHERE id = ?1",
        )?;
        let row = stmt
            .query_row(params![id], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    directory: row.get(3)?,
                    parent_id: row.get(4)?,
                    version: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    archived_at: row.get(8)?,
                    summary: row.get(9)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, title, project_id, directory, parent_id, version, \
             created_at, updated_at, archived_at, summary \
             FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    directory: row.get(3)?,
                    parent_id: row.get(4)?,
                    version: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    archived_at: row.get(8)?,
                    summary: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn update_session(&self, id: &str, title: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )?;
        Ok(())
    }

    pub fn archive_session(&self, id: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET archived_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    // ── Message CRUD ──────────────────────────────────────────────

    pub fn create_message(&self, msg: &Message) -> Result<()> {
        let conn = lock_conn!(self)?;
        let parts_json = serde_json::to_string(&msg.parts)?;
        let role_str = msg.role.to_string();
        let created = msg.created_at.to_rfc3339();
        let updated = msg.updated_at.to_rfc3339();
        conn.execute(
            "INSERT INTO messages (id, session_id, role, parts, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![msg.id, msg.session_id, role_str, parts_json, created, updated],
        )?;
        // Touch session updated_at
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
            params![now, msg.session_id],
        )?;
        Ok(())
    }

    pub fn get_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, parts, created_at, updated_at \
             FROM messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                let id: String = row.get(0)?;
                let sid: String = row.get(1)?;
                let role_str: String = row.get(2)?;
                let parts_json: String = row.get(3)?;
                let created_str: String = row.get(4)?;
                let updated_str: String = row.get(5)?;
                Ok((id, sid, role_str, parts_json, created_str, updated_str))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut messages = Vec::with_capacity(rows.len());
        for (id, sid, role_str, parts_json, created_str, updated_str) in rows {
            let role = match role_str.as_str() {
                "user" => Role::User,
                _ => Role::Assistant,
            };
            let parts: Vec<MessagePart> = serde_json::from_str(&parts_json)
                .unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            messages.push(Message {
                id,
                session_id: sid,
                role,
                parts,
                created_at,
                updated_at,
            });
        }
        Ok(messages)
    }

    pub fn update_message(&self, msg: &Message) -> Result<()> {
        let conn = lock_conn!(self)?;
        let parts_json = serde_json::to_string(&msg.parts)?;
        let updated = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE messages SET parts = ?1, updated_at = ?2 WHERE id = ?3",
            params![parts_json, updated, msg.id],
        )?;
        Ok(())
    }

    // ── Provider Auth ─────────────────────────────────────────────

    pub fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO provider_auth (provider_id, api_key, updated_at) \
             VALUES (?1, ?2, ?3)",
            params![provider_id, api_key, now],
        )?;
        Ok(())
    }

    pub fn get_provider_auth(&self, provider_id: &str) -> Result<Option<String>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT api_key FROM provider_auth WHERE provider_id = ?1",
        )?;
        let key = stmt
            .query_row(params![provider_id], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(key)
    }
}

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub project_id: String,
    pub directory: String,
    pub parent_id: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
    pub summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_roundtrip() {
        let storage = Storage::open_in_memory().unwrap();
        storage.create_session("s1", "/tmp/test").unwrap();

        let session = storage.get_session("s1").unwrap().unwrap();
        assert_eq!(session.directory, "/tmp/test");

        let msg = Message::user_text("s1", "Hello!");
        storage.create_message(&msg).unwrap();

        let messages = storage.get_messages("s1").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text_content(), "Hello!");
    }

    #[test]
    fn test_provider_auth() {
        let storage = Storage::open_in_memory().unwrap();
        storage.set_provider_auth("anthropic", "sk-test-123").unwrap();
        let key = storage.get_provider_auth("anthropic").unwrap();
        assert_eq!(key, Some("sk-test-123".to_string()));
    }

    #[test]
    fn test_archive_session() {
        let storage = Storage::open_in_memory().unwrap();
        storage.create_session("s1", "/tmp").unwrap();
        storage.archive_session("s1").unwrap();
        let sessions = storage.list_sessions().unwrap();
        assert!(sessions.is_empty());
    }
}
