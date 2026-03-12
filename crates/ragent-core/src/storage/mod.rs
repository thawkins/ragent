//! Persistent storage layer backed by SQLite.
//!
//! [`Storage`] manages the database lifecycle (open, migrate) and exposes
//! CRUD operations for sessions, messages, provider credentials, and MCP
//! server configuration. All access is thread-safe via an internal `Mutex`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::message::{Message, MessagePart, Role};

/// Fixed key used for XOR-based obfuscation of API keys.
///
/// **Note:** This is simple obfuscation, *not* encryption. It prevents
/// casual inspection of keys stored on disk but will not withstand a
/// determined attacker. For production use, consider a keyring-based
/// solution (e.g., `keyring` crate or OS-level credential storage).
const OBFUSCATION_KEY: &[u8] = b"ragent-obfuscation-key-v1";

/// Obfuscates an API key using repeating-key XOR and base64 encoding.
///
/// This is *not* encryption — it only prevents plaintext keys from
/// appearing in the database. A keyring-based solution is recommended
/// for production use.
///
/// # Examples
///
/// ```
/// use ragent_core::storage::obfuscate_key;
///
/// let obfuscated = obfuscate_key("sk-secret-key");
/// assert!(!obfuscated.is_empty());
/// assert_ne!(obfuscated, "sk-secret-key");
/// ```
pub fn obfuscate_key(key: &str) -> String {
    let xored: Vec<u8> = key
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ OBFUSCATION_KEY[i % OBFUSCATION_KEY.len()])
        .collect();
    STANDARD.encode(&xored)
}

/// Reverses [`obfuscate_key`], recovering the original API key.
///
/// Returns the original key, or an empty string if decoding fails.
///
/// # Examples
///
/// ```
/// use ragent_core::storage::{obfuscate_key, deobfuscate_key};
///
/// let obfuscated = obfuscate_key("my-api-key");
/// let recovered = deobfuscate_key(&obfuscated);
/// assert_eq!(recovered, "my-api-key");
/// ```
pub fn deobfuscate_key(encoded: &str) -> String {
    let Ok(xored) = STANDARD.decode(encoded) else {
        return String::new();
    };
    let bytes: Vec<u8> = xored
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ OBFUSCATION_KEY[i % OBFUSCATION_KEY.len()])
        .collect();
    String::from_utf8(bytes).unwrap_or_default()
}

/// SQLite-backed storage for sessions, messages, and provider credentials.
pub struct Storage {
    conn: Mutex<Connection>,
}

/// Acquires the database connection lock, mapping a poisoned mutex to an anyhow error.
macro_rules! lock_conn {
    ($self:expr) => {
        $self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))
    };
}

impl Storage {
    /// Opens (or creates) a SQLite database at the given filesystem path and
    /// runs migrations to ensure the schema is up to date.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directory cannot be created, the database
    /// file cannot be opened, or migrations fail.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_core::storage::Storage;
    /// use std::path::Path;
    ///
    /// let storage = Storage::open(Path::new("/tmp/ragent-test.db")).unwrap();
    /// ```
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

    /// Opens an ephemeral in-memory database, useful for testing.
    ///
    /// # Errors
    ///
    /// Returns an error if the in-memory database cannot be created or
    /// migrations fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// ```
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

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS todos (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                description TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE INDEX IF NOT EXISTS idx_todos_session
                ON todos(session_id, status);
            ",
        )?;
        Ok(())
    }

    // ── Session CRUD ──────────────────────────────────────────────

    /// Inserts a new session row with the given `id` and `directory`.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails (e.g., duplicate id).
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/home/user/project").unwrap();
    /// ```
    pub fn create_session(&self, id: &str, directory: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO sessions (id, directory, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, directory, now, now],
        )?;
        Ok(())
    }

    /// Fetches a single session by `id`, returning `None` if it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/home/user/project").unwrap();
    /// let session = storage.get_session("sess-1").unwrap();
    /// assert!(session.is_some());
    /// assert_eq!(session.unwrap().directory, "/home/user/project");
    /// ```
    pub fn get_session(&self, id: &str) -> Result<Option<SessionRow>> {
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

    /// Lists all non-archived sessions ordered by most recently updated.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project-a").unwrap();
    /// storage.create_session("sess-2", "/tmp/project-b").unwrap();
    /// let sessions = storage.list_sessions().unwrap();
    /// assert_eq!(sessions.len(), 2);
    /// ```
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

    /// Updates the title of an existing session and touches `updated_at`.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// storage.update_session("sess-1", "My New Title").unwrap();
    /// let session = storage.get_session("sess-1").unwrap().unwrap();
    /// assert_eq!(session.title, "My New Title");
    /// ```
    pub fn update_session(&self, id: &str, title: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )?;
        Ok(())
    }

    /// Marks a session as archived by setting `archived_at` to the current time.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// storage.archive_session("sess-1").unwrap();
    /// let sessions = storage.list_sessions().unwrap();
    /// assert!(sessions.is_empty(), "archived sessions are excluded from list");
    /// ```
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

    /// Persists a new message and bumps the parent session's `updated_at`.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the insert fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    /// use ragent_core::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// let msg = Message::user_text("sess-1", "Hello, agent!");
    /// storage.create_message(&msg).unwrap();
    /// ```
    pub fn create_message(&self, msg: &Message) -> Result<()> {
        let conn = lock_conn!(self)?;
        let parts_json = serde_json::to_string(&msg.parts)?;
        let role_str = msg.role.to_string();
        let created = msg.created_at.to_rfc3339();
        let updated = msg.updated_at.to_rfc3339();
        conn.execute(
            "INSERT INTO messages (id, session_id, role, parts, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                msg.id,
                msg.session_id,
                role_str,
                parts_json,
                created,
                updated
            ],
        )?;
        // Touch session updated_at
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
            params![now, msg.session_id],
        )?;
        Ok(())
    }

    /// Retrieves all messages for a session, ordered chronologically.
    ///
    /// # Errors
    ///
    /// Returns an error if the query or deserialization fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    /// use ragent_core::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// storage.create_message(&Message::user_text("sess-1", "Hi")).unwrap();
    /// let messages = storage.get_messages("sess-1").unwrap();
    /// assert_eq!(messages.len(), 1);
    /// assert_eq!(messages[0].text_content(), "Hi");
    /// ```
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
            let parts: Vec<MessagePart> = serde_json::from_str(&parts_json).unwrap_or_default();
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

    /// Updates the parts and `updated_at` timestamp of an existing message.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the update fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    /// use ragent_core::message::{Message, MessagePart};
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// let mut msg = Message::user_text("sess-1", "draft");
    /// storage.create_message(&msg).unwrap();
    /// msg.parts = vec![MessagePart::Text { text: "revised".into() }];
    /// storage.update_message(&msg).unwrap();
    /// ```
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

    /// Stores or replaces the API key for the given provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the upsert fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.set_provider_auth("anthropic", "sk-ant-my-key").unwrap();
    /// ```
    pub fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        let obfuscated = obfuscate_key(api_key);
        conn.execute(
            "INSERT OR REPLACE INTO provider_auth (provider_id, api_key, updated_at) \
             VALUES (?1, ?2, ?3)",
            params![provider_id, obfuscated, now],
        )?;
        Ok(())
    }

    /// Removes the stored API key for the given provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.set_provider_auth("anthropic", "sk-ant-my-key").unwrap();
    /// storage.delete_provider_auth("anthropic").unwrap();
    /// assert!(storage.get_provider_auth("anthropic").unwrap().is_none());
    /// ```
    pub fn delete_provider_auth(&self, provider_id: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "DELETE FROM provider_auth WHERE provider_id = ?1",
            params![provider_id],
        )?;
        Ok(())
    }

    /// Retrieves the stored API key for a provider, or `None` if not set.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.set_provider_auth("anthropic", "sk-ant-my-key").unwrap();
    /// let key = storage.get_provider_auth("anthropic").unwrap();
    /// assert_eq!(key.unwrap(), "sk-ant-my-key");
    /// ```
    pub fn get_provider_auth(&self, provider_id: &str) -> Result<Option<String>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare("SELECT api_key FROM provider_auth WHERE provider_id = ?1")?;
        let key = stmt
            .query_row(params![provider_id], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(key.map(|k| deobfuscate_key(&k)))
    }

    // ── Settings (key-value) ──────────────────────────────────────

    /// Stores or replaces a setting value.
    ///
    /// # Errors
    ///
    /// Returns an error if the upsert fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.set_setting("theme", "dark").unwrap();
    /// ```
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, now],
        )?;
        Ok(())
    }

    /// Removes a setting value.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.set_setting("theme", "dark").unwrap();
    /// storage.delete_setting("theme").unwrap();
    /// assert!(storage.get_setting("theme").unwrap().is_none());
    /// ```
    pub fn delete_setting(&self, key: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Retrieves a setting value, or `None` if not set.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.set_setting("theme", "dark").unwrap();
    /// let val = storage.get_setting("theme").unwrap();
    /// assert_eq!(val.unwrap(), "dark");
    /// ```
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let val = stmt
            .query_row(params![key], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(val)
    }

    // ── Todo CRUD ───────────────────────────────────────────────────

    /// Creates a new TODO item in the given session.
    pub fn create_todo(
        &self,
        id: &str,
        session_id: &str,
        title: &str,
        status: &str,
        description: &str,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO todos (id, session_id, title, status, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, session_id, title, status, description, &now, &now],
        )?;
        Ok(())
    }

    /// Lists TODO items for a session, optionally filtered by status.
    ///
    /// Pass `Some("pending")` etc. to filter, or `None` / `Some("all")` for all.
    pub fn get_todos(
        &self,
        session_id: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<TodoRow>> {
        let conn = lock_conn!(self)?;
        let rows = match status_filter {
            Some(s) if s != "all" => {
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, title, status, description, created_at, updated_at
                     FROM todos WHERE session_id = ?1 AND status = ?2
                     ORDER BY created_at",
                )?;
                stmt.query_map(params![session_id, s], |row| {
                    Ok(TodoRow {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        title: row.get(2)?,
                        status: row.get(3)?,
                        description: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
            }
            _ => {
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, title, status, description, created_at, updated_at
                     FROM todos WHERE session_id = ?1
                     ORDER BY created_at",
                )?;
                stmt.query_map(params![session_id], |row| {
                    Ok(TodoRow {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        title: row.get(2)?,
                        status: row.get(3)?,
                        description: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
            }
        };
        Ok(rows)
    }

    /// Updates a TODO item's status and/or title/description.
    pub fn update_todo(
        &self,
        id: &str,
        session_id: &str,
        title: Option<&str>,
        status: Option<&str>,
        description: Option<&str>,
    ) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut sets = vec!["updated_at = ?1"];
        let mut idx = 2u32;
        let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(now.clone())];

        if let Some(t) = title {
            sets.push(if idx == 2 { "title = ?2" } else { unreachable!() });
            vals.push(Box::new(t.to_string()));
            idx += 1;
        }
        if let Some(s) = status {
            let placeholder = match idx {
                2 => "status = ?2",
                3 => "status = ?3",
                _ => unreachable!(),
            };
            sets.push(placeholder);
            vals.push(Box::new(s.to_string()));
            idx += 1;
        }
        if let Some(d) = description {
            let placeholder = match idx {
                2 => "description = ?2",
                3 => "description = ?3",
                4 => "description = ?4",
                _ => unreachable!(),
            };
            sets.push(placeholder);
            vals.push(Box::new(d.to_string()));
            idx += 1;
        }

        // id and session_id placeholders
        let id_ph = format!("?{idx}");
        let sid_ph = format!("?{}", idx + 1);
        vals.push(Box::new(id.to_string()));
        vals.push(Box::new(session_id.to_string()));

        let sql = format!(
            "UPDATE todos SET {} WHERE id = {} AND session_id = {}",
            sets.join(", "),
            id_ph,
            sid_ph
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            vals.iter().map(|b| b.as_ref()).collect();
        let changed = conn.execute(&sql, params.as_slice())?;
        Ok(changed > 0)
    }

    /// Deletes a TODO item.
    pub fn delete_todo(&self, id: &str, session_id: &str) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let changed = conn.execute(
            "DELETE FROM todos WHERE id = ?1 AND session_id = ?2",
            params![id, session_id],
        )?;
        Ok(changed > 0)
    }

    /// Deletes all TODO items for a session. Returns the number removed.
    pub fn clear_todos(&self, session_id: &str) -> Result<usize> {
        let conn = lock_conn!(self)?;
        let changed = conn.execute(
            "DELETE FROM todos WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(changed)
    }
}

/// Raw row representation of a session as stored in SQLite.
#[derive(Debug, Clone)]
pub struct SessionRow {
    /// Unique session identifier.
    pub id: String,
    /// Human-readable session title.
    pub title: String,
    /// Project this session belongs to.
    pub project_id: String,
    /// Working directory path stored as a string.
    pub directory: String,
    /// Optional parent session id for forked sessions.
    pub parent_id: Option<String>,
    /// Optimistic-concurrency version counter.
    pub version: i64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
    /// ISO-8601 archive timestamp, if archived.
    pub archived_at: Option<String>,
    /// JSON-encoded session summary, if available.
    pub summary: Option<String>,
}

/// Row representation of a TODO item.
#[derive(Debug, Clone)]
pub struct TodoRow {
    /// Unique todo identifier.
    pub id: String,
    /// Session this todo belongs to.
    pub session_id: String,
    /// Short title of the todo item.
    pub title: String,
    /// Current status (e.g. pending, done).
    pub status: String,
    /// Detailed description of the todo.
    pub description: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
}
