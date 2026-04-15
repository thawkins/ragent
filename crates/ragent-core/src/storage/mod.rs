//! Persistent storage layer backed by `SQLite`.
//!
//! [`Storage`] manages the database lifecycle (open, migrate) and exposes
//! CRUD operations for sessions, messages, provider credentials, and MCP
//! server configuration. All access is thread-safe via an internal `Mutex`.
//!
//! # Async writes
//!
//! Because `rusqlite` is synchronous, write operations block the calling
//! thread. Use [`Storage::write_async`] to off-load any write closure onto a
//! `tokio` blocking thread-pool thread, keeping the async executor free:
//!
//! ```no_run
//! use std::sync::Arc;
//! use ragent_core::storage::Storage;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let storage = Arc::new(Storage::open_in_memory()?);
//! let id = "sess-1".to_string();
//! Storage::write_async(Arc::clone(&storage), move |s| {
//!     s.create_session(&id, "/tmp")
//! }).await?;
//! # Ok(()) }
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::message::{Message, MessagePart, Role};

/// Fixed key used for legacy XOR-based obfuscation (v1 format).
const OBFUSCATION_KEY: &[u8] = b"ragent-obfuscation-key-v1";

/// Version prefix for the new encryption format.
const ENCRYPT_V2_PREFIX: &str = "v2:";

/// Nonce length in bytes for v2 encryption.
const NONCE_LEN: usize = 16;

/// Machine-local encryption key derived from system identity.
///
/// Uses blake3 key derivation with username + home directory as input material.
/// This ties the encrypted data to the current machine/user, preventing
/// credential theft by simply copying the database file.
static MACHINE_KEY: LazyLock<[u8; 32]> = LazyLock::new(|| {
    let username = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "ragent-default-user".to_string());

    let home = dirs::home_dir().map_or_else(
        || "ragent-default-home".to_string(),
        |p| p.to_string_lossy().into_owned(),
    );

    let input = format!("{username}:{home}");
    blake3::derive_key("ragent credential encryption v2", input.as_bytes())
});

/// Encrypts an API key using blake3-derived keystream with a random nonce.
///
/// Returns a `v2:` prefixed base64 string containing `nonce || ciphertext`.
/// The encryption key is derived from the current machine identity, so the
/// ciphertext can only be decrypted on the same machine by the same user.
///
/// # Examples
///
/// ```
/// use ragent_core::storage::{encrypt_key, decrypt_key};
///
/// let encrypted = encrypt_key("sk-secret-key");
/// assert!(encrypted.starts_with("v2:"));
/// assert_ne!(encrypted, "sk-secret-key");
/// let recovered = decrypt_key(&encrypted);
/// assert_eq!(recovered, "sk-secret-key");
/// ```
#[must_use]
pub fn encrypt_key(key: &str) -> String {
    use rand::Rng;
    let mut nonce = [0u8; NONCE_LEN];
    rand::rng().fill(&mut nonce);

    let keystream = generate_keystream(&nonce, key.len());
    let ciphertext: Vec<u8> = key
        .as_bytes()
        .iter()
        .zip(keystream.iter())
        .map(|(p, k)| p ^ k)
        .collect();

    let mut payload = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);

    format!("{ENCRYPT_V2_PREFIX}{}", STANDARD.encode(&payload))
}

/// Decrypts an API key encrypted with [`encrypt_key`].
///
/// Also handles legacy v1 (XOR-obfuscated) format for backward compatibility.
/// Returns the original key, or an empty string if decoding fails.
///
/// # Examples
///
/// ```
/// use ragent_core::storage::{encrypt_key, decrypt_key};
///
/// let encrypted = encrypt_key("my-api-key");
/// let recovered = decrypt_key(&encrypted);
/// assert_eq!(recovered, "my-api-key");
/// ```
#[must_use]
pub fn decrypt_key(encoded: &str) -> String {
    if let Some(v2_data) = encoded.strip_prefix(ENCRYPT_V2_PREFIX) {
        // v2 format: blake3-derived keystream
        let Ok(payload) = STANDARD.decode(v2_data) else {
            return String::new();
        };
        if payload.len() < NONCE_LEN {
            return String::new();
        }
        let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
        let keystream = generate_keystream(
            nonce.try_into().unwrap_or(&[0u8; NONCE_LEN]),
            ciphertext.len(),
        );
        let plaintext: Vec<u8> = ciphertext
            .iter()
            .zip(keystream.iter())
            .map(|(c, k)| c ^ k)
            .collect();
        String::from_utf8(plaintext).unwrap_or_default()
    } else {
        // Legacy v1 format: repeating-key XOR
        deobfuscate_key_v1(encoded)
    }
}

/// Generates a keystream of the given length using blake3 in XOF mode.
fn generate_keystream(nonce: &[u8; NONCE_LEN], len: usize) -> Vec<u8> {
    let mut hasher = blake3::Hasher::new_keyed(&MACHINE_KEY);
    hasher.update(nonce);
    let mut output = vec![0u8; len];
    let mut reader = hasher.finalize_xof();
    reader.fill(&mut output);
    output
}

/// Legacy v1 obfuscation — kept for reading old database entries.
fn deobfuscate_key_v1(encoded: &str) -> String {
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

/// Obfuscates an API key using repeating-key XOR and base64 encoding.
///
/// **Deprecated:** Use [`encrypt_key`] instead. This function is retained
/// for backward compatibility with tests and migration scenarios.
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
#[must_use]
pub fn obfuscate_key(key: &str) -> String {
    encrypt_key(key)
}

/// Reverses [`obfuscate_key`], recovering the original API key.
///
/// **Deprecated:** Use [`decrypt_key`] instead. This function handles both
/// v1 (legacy XOR) and v2 (blake3 encrypted) formats.
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
#[must_use]
pub fn deobfuscate_key(encoded: &str) -> String {
    decrypt_key(encoded)
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
    /// Opens (or creates) a `SQLite` database at the given filesystem path and
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
            
                          -- Journal system tables (Milestone 2)
                          CREATE TABLE IF NOT EXISTS journal_entries (
                              id TEXT PRIMARY KEY,
                              title TEXT NOT NULL,
                              content TEXT NOT NULL,
                              project TEXT NOT NULL DEFAULT '',
                              session_id TEXT NOT NULL DEFAULT '',
                              timestamp TEXT NOT NULL,
                              created_at TEXT NOT NULL
                          );
            
                          CREATE TABLE IF NOT EXISTS journal_tags (
                              entry_id TEXT NOT NULL,
                              tag TEXT NOT NULL,
                              PRIMARY KEY (entry_id, tag),
                              FOREIGN KEY (entry_id) REFERENCES journal_entries(id) ON DELETE CASCADE
                          );
            
                          CREATE INDEX IF NOT EXISTS idx_journal_tags_tag
                              ON journal_tags(tag);
            
                          CREATE INDEX IF NOT EXISTS idx_journal_entries_project
                              ON journal_entries(project, timestamp DESC);
            
                          CREATE INDEX IF NOT EXISTS idx_journal_entries_session
                              ON journal_entries(session_id, timestamp DESC);
            
                                                      CREATE VIRTUAL TABLE IF NOT EXISTS journal_fts
                                                          USING fts5(title, content, content=journal_entries, content_rowid=rowid);
                          
                                                      -- Structured memory store tables (Milestone 3)
                                                      CREATE TABLE IF NOT EXISTS memories (
                                                          id INTEGER PRIMARY KEY,
                                                          content TEXT NOT NULL,
                                                          category TEXT NOT NULL CHECK(category IN ('fact','pattern','preference','insight','error','workflow')),
                                                          source TEXT NOT NULL DEFAULT '',
                                                          confidence REAL NOT NULL DEFAULT 0.5,
                                                          project TEXT NOT NULL DEFAULT '',
                                                          session_id TEXT NOT NULL DEFAULT '',
                                                          created_at TEXT NOT NULL,
                                                          updated_at TEXT NOT NULL,
                                                          access_count INTEGER NOT NULL DEFAULT 0,
                                                          last_accessed TEXT
                                                      );
                          
                                                      CREATE TABLE IF NOT EXISTS memory_tags (
                                                          memory_id INTEGER NOT NULL,
                                                          tag TEXT NOT NULL,
                                                          PRIMARY KEY (memory_id, tag),
                                                          FOREIGN KEY (memory_id) REFERENCES memories(id) ON DELETE CASCADE
                                                      );
                          
                                                      CREATE INDEX IF NOT EXISTS idx_memory_tags_tag
                                                          ON memory_tags(tag);
                          
                                                      CREATE INDEX IF NOT EXISTS idx_memories_category
                                                          ON memories(category, confidence DESC);
                          
                                                      CREATE INDEX IF NOT EXISTS idx_memories_project
                                                          ON memories(project, updated_at DESC);
                          
                                                      CREATE INDEX IF NOT EXISTS idx_memories_confidence
                                                          ON memories(confidence DESC, updated_at DESC);
                          
                                                                                                                                                                                                                              CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts
                                                                                                                                                                                                                                  USING fts5(content, content=memories, content_rowid=rowid);
                                                                                                                                                                      
                                                                                                                                                                      -- Knowledge graph tables (Milestone 9)
                                                                                                                                                                      CREATE TABLE IF NOT EXISTS kg_entities (
                                                                                                                                                                          id INTEGER PRIMARY KEY,
                                                                                                                                                                          name TEXT NOT NULL,
                                                                                                                                                                          entity_type TEXT NOT NULL CHECK(entity_type IN ('project','tool','language','pattern','person','concept')),
                                                                                                                                                                          mention_count INTEGER NOT NULL DEFAULT 1,
                                                                                                                                                                          first_memory_id INTEGER,
                                                                                                                                                                          created_at TEXT NOT NULL,
                                                                                                                                                                          updated_at TEXT NOT NULL,
                                                                                                                                                                          UNIQUE(name, entity_type)
                                                                                                                                                                      );
                                                                                                              
                                                                                                                                                                      CREATE TABLE IF NOT EXISTS kg_relationships (
                                                                                                                                                                          id INTEGER PRIMARY KEY,
                                                                                                                                                                          source_id INTEGER NOT NULL,
                                                                                                                                                                          target_id INTEGER NOT NULL,
                                                                                                                                                                          relation_type TEXT NOT NULL CHECK(relation_type IN ('uses','prefers','depends_on','avoids','related_to')),
                                                                                                                                                                          confidence REAL NOT NULL DEFAULT 0.7,
                                                                                                                                                                          source_memory_id INTEGER,
                                                                                                                                                                          created_at TEXT NOT NULL,
                                                                                                                                                                          FOREIGN KEY (source_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
                                                                                                                                                                          FOREIGN KEY (target_id) REFERENCES kg_entities(id) ON DELETE CASCADE,
                                                                                                                                                                          UNIQUE(source_id, target_id, relation_type)
                                                                                                                                                                      );
                                                                                                              
                                                                                                                                                                      CREATE INDEX IF NOT EXISTS idx_kg_entities_name
                                                                                                                                                                          ON kg_entities(name);
                                                                                                              
                                                                                                                                                                      CREATE INDEX IF NOT EXISTS idx_kg_entities_type
                                                                                                                                                                          ON kg_entities(entity_type);
                                                                                                              
                                                                                                                                                                      CREATE INDEX IF NOT EXISTS idx_kg_relationships_source
                                                                                                                                                                          ON kg_relationships(source_id);
                                                                                                              
                                                                                                                                                                      CREATE INDEX IF NOT EXISTS idx_kg_relationships_target
                                                                                                                                                                          ON kg_relationships(target_id);
                                                                                                              
                                                                                                                                                                    ",        )?;
        // Idempotent column additions (SQLite has no ALTER TABLE ADD COLUMN IF NOT EXISTS)
        for (table, col) in &[("memories", "embedding"), ("journal_entries", "embedding")] {
            let has_col: bool = conn
                .prepare(&format!(
                    "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name='{}'",
                    table, col
                ))?
                .query_row([], |r| r.get::<_, i64>(0))
                .unwrap_or(0)
                > 0;
            if !has_col {
                conn.execute_batch(&format!("ALTER TABLE {} ADD COLUMN {} BLOB;", table, col))?;
            }
        }

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
                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
            let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
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

    /// Deletes all messages for a session.  Used by compaction to clear the
    /// message history before inserting a summary message.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::storage::Storage;
    /// use ragent_core::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// storage.create_message(&Message::user_text("sess-1", "hello")).unwrap();
    /// let deleted = storage.delete_messages("sess-1").unwrap();
    /// assert_eq!(deleted, 1);
    /// assert!(storage.get_messages("sess-1").unwrap().is_empty());
    /// ```
    pub fn delete_messages(&self, session_id: &str) -> Result<usize> {
        let conn = lock_conn!(self)?;
        let n = conn.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(n)
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
        // Register in the in-memory secret registry for exact-match redaction.
        crate::sanitize::register_secret(api_key);
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
        // Unregister the secret before deleting from DB.
        if let Ok(Some(key)) = self.get_provider_auth(provider_id) {
            crate::sanitize::unregister_secret(&key);
        }
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
        let encoded = stmt
            .query_row(params![provider_id], |row| row.get::<_, String>(0))
            .optional()?;

        match encoded {
            Some(ref enc) if !enc.starts_with(ENCRYPT_V2_PREFIX) => {
                // Auto-migrate legacy v1 to v2 encryption.
                let plaintext = deobfuscate_key_v1(enc);
                if !plaintext.is_empty() {
                    let v2 = encrypt_key(&plaintext);
                    let now = Utc::now().to_rfc3339();
                    let _ = conn.execute(
                        "UPDATE provider_auth SET api_key = ?1, updated_at = ?2 \
                         WHERE provider_id = ?3",
                        params![v2, now, provider_id],
                    );
                }
                Ok(Some(plaintext))
            }
            Some(enc) => Ok(Some(decrypt_key(&enc))),
            None => Ok(None),
        }
    }

    /// Seeds the global secret registry with all stored provider credentials.
    ///
    /// Call this once at startup so that [`crate::sanitize::redact_secrets`]
    /// can perform exact-match redaction on known secrets.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub fn seed_secret_registry(&self) -> Result<()> {
        let keys: Vec<String> = {
            let conn = lock_conn!(self)?;
            let mut stmt = conn.prepare("SELECT api_key FROM provider_auth")?;
            stmt.query_map([], |row| row.get::<_, String>(0))?
                .filter_map(std::result::Result::ok)
                .map(|encoded| deobfuscate_key(&encoded))
                .filter(|k| !k.is_empty())
                .collect()
        };
        crate::sanitize::seed_secrets(keys);
        Ok(())
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
            "INSERT OR IGNORE INTO todos (id, session_id, title, status, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, session_id, title, status, description, &now, &now],
        )?;
        Ok(())
    }

    /// Lists TODO items for a session, optionally filtered by status.
    ///
    /// Pass `Some("pending")` etc. to filter, or `None` / `Some("all")` for all.
    pub fn get_todos(&self, session_id: &str, status_filter: Option<&str>) -> Result<Vec<TodoRow>> {
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
        let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

        if let Some(t) = title {
            sets.push(if idx == 2 {
                "title = ?2"
            } else {
                unreachable!()
            });
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
            vals.iter().map(std::convert::AsRef::as_ref).collect();
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

    // ── Journal CRUD ────────────────────────────────────────────────

    /// Inserts a new journal entry and its tags, and updates the FTS index.
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
    /// storage.create_journal_entry(
    ///     "entry-1", "Bug fix", "Fixed off-by-one in parser",
    ///     "my-project", "sess-1", &["bug".to_string()]
    /// ).unwrap();
    /// let entry = storage.get_journal_entry("entry-1").unwrap().unwrap();
    /// assert_eq!(entry.title, "Bug fix");
    /// ```
    pub fn create_journal_entry(
        &self,
        id: &str,
        title: &str,
        content: &str,
        project: &str,
        session_id: &str,
        tags: &[String],
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        let timestamp = now.clone();

        conn.execute(
                    "INSERT INTO journal_entries (id, title, content, project, session_id, timestamp, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![id, title, content, project, session_id, timestamp, now],
                )?;

        for tag in tags {
            conn.execute(
                "INSERT OR IGNORE INTO journal_tags (entry_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )?;
        }

        // Update FTS index.
        conn.execute(
            "INSERT INTO journal_fts(rowid, title, content)
                             SELECT rowid, title, content FROM journal_entries WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Retrieves a single journal entry by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_journal_entry(&self, id: &str) -> Result<Option<JournalEntryRow>> {
        let conn = lock_conn!(self)?;
        let row = conn
            .query_row(
                "SELECT id, title, content, project, session_id, timestamp, created_at
                         FROM journal_entries WHERE id = ?1",
                params![id],
                |row| {
                    Ok(JournalEntryRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        project: row.get(3)?,
                        session_id: row.get(4)?,
                        timestamp: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Retrieves tags for a journal entry.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_journal_tags(&self, entry_id: &str) -> Result<Vec<String>> {
        let conn = lock_conn!(self)?;
        let mut stmt =
            conn.prepare("SELECT tag FROM journal_tags WHERE entry_id = ?1 ORDER BY tag")?;
        let tags: Vec<String> = stmt
            .query_map(params![entry_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    /// Searches journal entries using FTS5 full-text search, optionally
    /// filtered by tags.
    ///
    /// Returns entries ordered by FTS rank (most relevant first).
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn search_journal_entries(
        &self,
        query: &str,
        tags: Option<&[String]>,
        limit: usize,
    ) -> Result<Vec<JournalEntryRow>> {
        let conn = lock_conn!(self)?;

        // Sanitise the FTS query: wrap each term in quotes to prevent injection.
        let safe_query: String = query
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|term| format!("\"{}\"", term.replace('"', "")))
            .collect::<Vec<_>>()
            .join(" ");

        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        let entries = if let Some(tags) = tags {
            if tags.is_empty() {
                self.list_journal_entries(limit)?
            } else {
                // Search with tag filter: join entries that have ALL specified tags.
                let tag_placeholders: Vec<String> =
                    (1..=tags.len()).map(|i| format!("?{i}")).collect();
                let tag_filter = tag_placeholders.join(", ");
                let sql = format!(
                            "SELECT e.id, e.title, e.content, e.project, e.session_id, e.timestamp, e.created_at
                             FROM journal_entries e
                             INNER JOIN journal_fts f ON f.rowid = e.rowid
                             WHERE journal_fts MATCH ?{}
                             AND e.id IN (
                                 SELECT entry_id FROM journal_tags WHERE tag IN ({})
                                 GROUP BY entry_id HAVING COUNT(DISTINCT tag) = {}
                             )
                             ORDER BY f.rank
                             LIMIT ?{}",
                            tags.len() + 1,
                            tag_filter,
                            tags.len(),
                            tags.len() + 2,
                        );

                let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
                for tag in tags {
                    params_vec.push(Box::new(tag.clone()));
                }
                params_vec.push(Box::new(safe_query));
                params_vec.push(Box::new(limit as i64));

                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params_vec.iter().map(|p| p.as_ref()).collect();

                let mut stmt = conn.prepare(&sql)?;
                let rows: Vec<JournalEntryRow> = stmt
                    .query_map(param_refs.as_slice(), |row| {
                        Ok(JournalEntryRow {
                            id: row.get(0)?,
                            title: row.get(1)?,
                            content: row.get(2)?,
                            project: row.get(3)?,
                            session_id: row.get(4)?,
                            timestamp: row.get(5)?,
                            created_at: row.get(6)?,
                        })
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            }
        } else {
            // Search without tag filter.
            let sql =
                        "SELECT e.id, e.title, e.content, e.project, e.session_id, e.timestamp, e.created_at
                         FROM journal_entries e
                         INNER JOIN journal_fts f ON f.rowid = e.rowid
                         WHERE journal_fts MATCH ?1
                         ORDER BY f.rank
                         LIMIT ?2";
            let mut stmt = conn.prepare(sql)?;
            let rows: Vec<JournalEntryRow> = stmt
                .query_map(params![safe_query, limit as i64], |row| {
                    Ok(JournalEntryRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        project: row.get(3)?,
                        session_id: row.get(4)?,
                        timestamp: row.get(5)?,
                        created_at: row.get(6)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };

        Ok(entries)
    }

    /// Lists recent journal entries, ordered by timestamp descending.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_journal_entries(&self, limit: usize) -> Result<Vec<JournalEntryRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, title, content, project, session_id, timestamp, created_at
                     FROM journal_entries ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(JournalEntryRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    project: row.get(3)?,
                    session_id: row.get(4)?,
                    timestamp: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Lists journal entries filtered by tag, ordered by timestamp descending.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_journal_entries_by_tag(
        &self,
        tag: &str,
        limit: usize,
    ) -> Result<Vec<JournalEntryRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT e.id, e.title, e.content, e.project, e.session_id, e.timestamp, e.created_at
                     FROM journal_entries e
                     INNER JOIN journal_tags t ON t.entry_id = e.id
                     WHERE t.tag = ?1
                     ORDER BY e.timestamp DESC
                     LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![tag, limit as i64], |row| {
                Ok(JournalEntryRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    project: row.get(3)?,
                    session_id: row.get(4)?,
                    timestamp: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Deletes a journal entry by ID (cascades to tags and FTS).
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    pub fn delete_journal_entry(&self, id: &str) -> Result<bool> {
        let conn = lock_conn!(self)?;

        // Remove from FTS first.
        conn.execute(
                    "DELETE FROM journal_fts WHERE rowid = (SELECT rowid FROM journal_entries WHERE id = ?1)",
                    params![id],
                )?;

        // Tags are removed by ON DELETE CASCADE.
        let affected = conn.execute("DELETE FROM journal_entries WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    /// Counts the total number of journal entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn count_journal_entries(&self) -> Result<u64> {
        let conn = lock_conn!(self)?;
        let count: u64 =
            conn.query_row("SELECT COUNT(*) FROM journal_entries", [], |row| row.get(0))?;
        Ok(count)
    }

    // ── Structured Memory CRUD ──────────────────────────────────────

    /// Inserts a new structured memory with category, tags, and confidence.
    ///
    /// Returns the auto-generated row ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails (e.g., invalid category).
    pub fn create_memory(
        &self,
        content: &str,
        category: &str,
        source: &str,
        confidence: f64,
        project: &str,
        session_id: &str,
        tags: &[String],
    ) -> Result<i64> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO memories (content, category, source, confidence, project, session_id, created_at, updated_at, access_count, last_accessed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0, ?7)",
            params![content, category, source, confidence, project, session_id, now],
        )?;

        let id = conn.last_insert_rowid();

        for tag in tags {
            conn.execute(
                "INSERT OR IGNORE INTO memory_tags (memory_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )?;
        }

        // Update FTS index.
        conn.execute(
            "INSERT INTO memories_fts(rowid, content)
             SELECT rowid, content FROM memories WHERE id = ?1",
            params![id],
        )?;

        Ok(id)
    }

    /// Retrieves a single structured memory by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_memory(&self, id: i64) -> Result<Option<MemoryRow>> {
        let conn = lock_conn!(self)?;
        let row = conn
            .query_row(
                "SELECT id, content, category, source, confidence, project, session_id,
                        created_at, updated_at, access_count, last_accessed
                 FROM memories WHERE id = ?1",
                params![id],
                |row| {
                    Ok(MemoryRow {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        category: row.get(2)?,
                        source: row.get(3)?,
                        confidence: row.get(4)?,
                        project: row.get(5)?,
                        session_id: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        access_count: row.get(9)?,
                        last_accessed: row.get(10)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Retrieves tags for a structured memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_memory_tags(&self, memory_id: i64) -> Result<Vec<String>> {
        let conn = lock_conn!(self)?;
        let mut stmt =
            conn.prepare("SELECT tag FROM memory_tags WHERE memory_id = ?1 ORDER BY tag")?;
        let tags: Vec<String> = stmt
            .query_map(params![memory_id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }

    /// Searches structured memories using FTS5 full-text search, optionally
    /// filtered by categories, tags, and minimum confidence.
    ///
    /// Returns entries ordered by FTS rank (most relevant first).
    /// Increments `access_count` and updates `last_accessed` for returned results.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn search_memories(
        &self,
        query: &str,
        categories: Option<&[String]>,
        tags: Option<&[String]>,
        limit: usize,
        min_confidence: f64,
    ) -> Result<Vec<MemoryRow>> {
        let conn = lock_conn!(self)?;

        // Sanitise the FTS query.
        let safe_query: String = query
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|term| format!("\"{}\"", term.replace('"', "")))
            .collect::<Vec<_>>()
            .join(" ");

        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        // Build category filter clause.
        let category_clause = if let Some(cats) = categories {
            if cats.is_empty() {
                String::new()
            } else {
                let placeholders: Vec<String> = (1..=cats.len()).map(|i| format!("?{i}")).collect();
                format!(" AND e.category IN ({})", placeholders.join(", "))
            }
        } else {
            String::new()
        };

        // Compute parameter offset for FTS query param.
        let fts_param_idx = categories.map_or(1, |c| c.len() + 1);
        let limit_param_idx = fts_param_idx + 1;

        // Build tag filter clause (entries that have ALL specified tags).
        let tag_clause = if let Some(tags) = tags {
            if tags.is_empty() {
                String::new()
            } else {
                let tag_placeholders: Vec<String> = (1..=tags.len())
                    .map(|i| format!("?{}", limit_param_idx + i))
                    .collect();
                let tag_count = tags.len();
                format!(
                    " AND e.id IN (\
                     SELECT memory_id FROM memory_tags WHERE tag IN ({}) \
                     GROUP BY memory_id HAVING COUNT(DISTINCT tag) = {})",
                    tag_placeholders.join(", "),
                    tag_count
                )
            }
        } else {
            String::new()
        };

        let sql = format!(
            "SELECT e.id, e.content, e.category, e.source, e.confidence,
                    e.project, e.session_id, e.created_at, e.updated_at,
                    e.access_count, e.last_accessed
             FROM memories e
             INNER JOIN memories_fts f ON f.rowid = e.rowid
             WHERE memories_fts MATCH ?{fts_param_idx}
               AND e.confidence >= ?{limit_param_idx}
               {category_clause}
               {tag_clause}
             ORDER BY f.rank
             LIMIT ?{}",
            limit_param_idx + tags.map_or(0, |t| t.len()) + 1
        );

        // Build parameter list.
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(cats) = categories {
            if !cats.is_empty() {
                for cat in cats {
                    params_vec.push(Box::new(cat.clone()));
                }
            }
        }
        params_vec.push(Box::new(safe_query));
        params_vec.push(Box::new(min_confidence));
        if let Some(tags) = tags {
            if !tags.is_empty() {
                for tag in tags {
                    params_vec.push(Box::new(tag.clone()));
                }
            }
        }
        params_vec.push(Box::new(limit as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows: Vec<MemoryRow> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(MemoryRow {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    category: row.get(2)?,
                    source: row.get(3)?,
                    confidence: row.get(4)?,
                    project: row.get(5)?,
                    session_id: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: row.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Increment access count for returned results.
        for row in &rows {
            let now = Utc::now().to_rfc3339();
            let _ = conn.execute(
                "UPDATE memories SET access_count = access_count + 1, last_accessed = ?1 WHERE id = ?2",
                params![now, row.id],
            );
        }

        Ok(rows)
    }

    /// Lists recent structured memories for a project, ordered by recency
    /// and confidence.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_memories(&self, project: &str, limit: usize) -> Result<Vec<MemoryRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, content, category, source, confidence, project, session_id,
                    created_at, updated_at, access_count, last_accessed
             FROM memories
             WHERE project = ?1
             ORDER BY updated_at DESC, confidence DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![project, limit as i64], |row| {
                Ok(MemoryRow {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    category: row.get(2)?,
                    source: row.get(3)?,
                    confidence: row.get(4)?,
                    project: row.get(5)?,
                    session_id: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: row.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Deletes a structured memory by ID (cascades to tags and FTS).
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    pub fn delete_memory(&self, id: i64) -> Result<bool> {
        let conn = lock_conn!(self)?;

        conn.execute(
            "DELETE FROM memories_fts WHERE rowid = (SELECT rowid FROM memories WHERE id = ?1)",
            params![id],
        )?;

        let affected = conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    /// Deletes structured memories matching filter criteria.
    ///
    /// At least one filter criterion must be provided (safety).
    ///
    /// # Errors
    ///
    /// Returns an error if no criteria are provided or the delete fails.
    pub fn delete_memories_by_filter(
        &self,
        older_than_days: Option<u32>,
        max_confidence: Option<f64>,
        category: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<usize> {
        if older_than_days.is_none()
            && max_confidence.is_none()
            && category.is_none()
            && tags.is_none_or(|t| t.is_empty())
        {
            anyhow::bail!("At least one filter criterion is required to delete memories");
        }

        let conn = lock_conn!(self)?;
        let cutoff = older_than_days.map(|days| {
            let dt = Utc::now() - chrono::Duration::days(days as i64);
            dt.to_rfc3339()
        });

        // Build a subquery to find IDs to delete.
        let mut conditions = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref cutoff) = cutoff {
            conditions.push(format!("updated_at < ?{param_idx}"));
            params_vec.push(Box::new(cutoff.clone()));
            param_idx += 1;
        }
        if let Some(max_conf) = max_confidence {
            conditions.push(format!("confidence <= ?{param_idx}"));
            params_vec.push(Box::new(max_conf));
            param_idx += 1;
        }
        if let Some(cat) = category {
            conditions.push(format!("category = ?{param_idx}"));
            params_vec.push(Box::new(cat.to_string()));
            param_idx += 1;
        }
        if let Some(tags) = tags {
            if !tags.is_empty() {
                let placeholders: Vec<String> = (0..tags.len())
                    .map(|i| format!("?{}", param_idx + i))
                    .collect();
                conditions.push(format!(
                    "id IN (SELECT memory_id FROM memory_tags WHERE tag IN ({}) GROUP BY memory_id)",
                    placeholders.join(", ")
                ));
                for tag in tags {
                    params_vec.push(Box::new(tag.clone()));
                }
            }
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!("SELECT id FROM memories WHERE {where_clause}");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let ids: Vec<i64> = stmt
            .query_map(param_refs.as_slice(), |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let count = ids.len();
        for id in &ids {
            let _ = conn.execute(
                "DELETE FROM memories_fts WHERE rowid = (SELECT rowid FROM memories WHERE id = ?1)",
                params![id],
            );
            let _ = conn.execute("DELETE FROM memories WHERE id = ?1", params![id]);
        }

        Ok(count)
    }

    /// Updates the confidence score of a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn update_memory_confidence(&self, id: i64, confidence: f64) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE memories SET confidence = ?1, updated_at = ?2 WHERE id = ?3",
            params![confidence, now, id],
        )?;
        Ok(affected > 0)
    }

    /// Increments the access count and updates last_accessed for a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn increment_memory_access(&self, id: i64) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE memories SET access_count = access_count + 1, last_accessed = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(affected > 0)
    }

    /// Counts the total number of structured memories.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn count_memories(&self) -> Result<u64> {
        let conn = lock_conn!(self)?;
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Updates the content of a memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn update_memory_content(&self, id: i64, content: &str) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE memories SET content = ?1, updated_at = ?2 WHERE id = ?3",
            params![content, now, id],
        )?;
        // For content-synced FTS5 tables, the index is automatically updated
        // when the underlying content table is modified. No manual FTS update needed.
        Ok(affected > 0)
    }
    /// Sets the tags for a memory, replacing any existing tags.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails.
    pub fn set_memory_tags(&self, memory_id: i64, tags: &[String]) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "DELETE FROM memory_tags WHERE memory_id = ?1",
            params![memory_id],
        )?;
        for tag in tags {
            conn.execute(
                "INSERT INTO memory_tags (memory_id, tag) VALUES (?1, ?2)",
                params![memory_id, tag],
            )?;
        }
        Ok(())
    }

    /// Delete memories using a [`ForgetFilter`](crate::memory::store::ForgetFilter).
    ///
    /// Handles both the `Id` variant (single delete) and `Filter` variant
    /// (criteria-based delete).
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter specifying which memories to delete.
    /// * `session_id` - Session ID for auditing (unused in core delete, but required for API consistency).
    ///
    /// # Returns
    ///
    /// Number of deleted memories.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    pub fn delete_memories(
        &self,
        filter: crate::memory::store::ForgetFilter,
        _session_id: &str,
    ) -> Result<usize> {
        match filter {
            crate::memory::store::ForgetFilter::Id(id) => {
                let deleted = self.delete_memory(id)?;
                Ok(if deleted { 1 } else { 0 })
            }
            crate::memory::store::ForgetFilter::Filter {
                older_than_days,
                max_confidence,
                category,
                tags,
            } => self.delete_memories_by_filter(
                older_than_days,
                max_confidence,
                category.as_deref(),
                tags.as_deref(),
            ),
        }
    }
    // ── Embedding storage and search ─────────────────────────────────

    /// Stores an embedding vector for a structured memory.
    ///
    /// The embedding is serialised as a little-endian f32 blob and stored in
    /// the `embedding` BLOB column of the `memories` table.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails (e.g., memory not found).
    pub fn store_memory_embedding(&self, id: i64, embedding_blob: &[u8]) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let affected = conn.execute(
            "UPDATE memories SET embedding = ?1 WHERE id = ?2",
            params![embedding_blob, id],
        )?;
        Ok(affected > 0)
    }

    /// Stores an embedding vector for a journal entry.
    ///
    /// The embedding is serialised as a little-endian f32 blob and stored in
    /// the `embedding` BLOB column of the `journal_entries` table.
    ///
    /// # Errors
    ///
    /// Returns an error if the update fails (e.g., entry not found).
    pub fn store_journal_embedding(&self, id: &str, embedding_blob: &[u8]) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let affected = conn.execute(
            "UPDATE journal_entries SET embedding = ?1 WHERE id = ?2",
            params![embedding_blob, id],
        )?;
        Ok(affected > 0)
    }

    /// Returns all memory embeddings that are not NULL.
    ///
    /// Each result contains the row ID and the raw embedding blob.
    /// Used for brute-force cosine similarity search.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_memory_embeddings(&self) -> Result<Vec<(i64, Vec<u8>)>> {
        let conn = lock_conn!(self)?;
        let mut stmt =
            conn.prepare("SELECT id, embedding FROM memories WHERE embedding IS NOT NULL")?;
        let rows: Vec<(i64, Vec<u8>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Returns all journal entry embeddings that are not NULL.
    ///
    /// Each result contains the entry ID and the raw embedding blob.
    /// Used for brute-force cosine similarity search.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_journal_embeddings(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let conn = lock_conn!(self)?;
        let mut stmt =
            conn.prepare("SELECT id, embedding FROM journal_entries WHERE embedding IS NOT NULL")?;
        let rows: Vec<(String, Vec<u8>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Search structured memories by cosine similarity against a query embedding.
    ///
    /// Loads all stored memory embeddings and computes brute-force cosine
    /// similarity. Returns results ranked by similarity (highest first).
    ///
    /// This approach is acceptable for up to ~10K memories. For larger datasets,
    /// consider using `sqlite-vec` for ANN search.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn search_memories_by_embedding(
        &self,
        query_embedding: &[f32],
        dimensions: usize,
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<crate::memory::embedding::SimilarityResult>> {
        let embeddings = self.list_memory_embeddings()?;
        let mut results: Vec<crate::memory::embedding::SimilarityResult> = Vec::new();

        for (row_id, blob) in &embeddings {
            if let Ok(stored) = crate::memory::embedding::deserialise_embedding(blob, dimensions) {
                let score = crate::memory::embedding::cosine_similarity(query_embedding, &stored);
                if score >= min_similarity {
                    results.push(crate::memory::embedding::SimilarityResult {
                        row_id: *row_id,
                        score,
                    });
                }
            }
        }

        // Sort by similarity descending.
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// Search journal entries by cosine similarity against a query embedding.
    ///
    /// Loads all stored journal embeddings and computes brute-force cosine
    /// similarity. Returns results ranked by similarity (highest first).
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn search_journal_by_embedding(
        &self,
        query_embedding: &[f32],
        dimensions: usize,
        limit: usize,
        min_similarity: f32,
    ) -> Result<Vec<crate::memory::embedding::SimilarityResult>> {
        let embeddings = self.list_journal_embeddings()?;
        let mut results: Vec<crate::memory::embedding::SimilarityResult> = Vec::new();

        for (_entry_id, blob) in &embeddings {
            if let Ok(stored) = crate::memory::embedding::deserialise_embedding(blob, dimensions) {
                let score = crate::memory::embedding::cosine_similarity(query_embedding, &stored);
                if score >= min_similarity {
                    results.push(crate::memory::embedding::SimilarityResult {
                        row_id: 0, // Journal uses string IDs, store rowid separately
                        score,
                    });
                }
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    // ── Knowledge Graph CRUD ────────────────────────────────────────────

    /// Insert or update a knowledge graph entity.
    ///
    /// If an entity with the same `name` and `entity_type` already exists,
    /// its `mention_count` is incremented and `updated_at` is refreshed.
    /// Otherwise, a new entity is created.
    ///
    /// Returns the entity's row ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert/upsert fails.
    pub fn upsert_entity(
        &self,
        name: &str,
        entity_type: &str,
        first_memory_id: i64,
    ) -> Result<i64> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();

        // Try to find existing entity.
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM kg_entities WHERE name = ?1 AND entity_type = ?2",
                params![name, entity_type],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            // Increment mention count and update timestamp.
            conn.execute(
                        "UPDATE kg_entities SET mention_count = mention_count + 1, updated_at = ?1 WHERE id = ?2",
                        params![now, id],
                    )?;
            Ok(id)
        } else {
            // Insert new entity.
            conn.execute(
                        "INSERT INTO kg_entities (name, entity_type, mention_count, first_memory_id, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?4, ?5)",
                        params![name, entity_type, first_memory_id, now, now],
                    )?;
            Ok(conn.last_insert_rowid())
        }
    }

    /// Create a relationship between two entities.
    ///
    /// If a relationship with the same source, target, and type already exists,
    /// the confidence is updated to the maximum of the existing and new values.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails.
    pub fn create_relationship(
        &self,
        source_id: i64,
        target_id: i64,
        relation_type: &str,
        confidence: f64,
        source_memory_id: Option<i64>,
    ) -> Result<i64> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();

        // Use INSERT OR REPLACE to handle uniqueness constraint.
        conn.execute(
                    "INSERT INTO kg_relationships (source_id, target_id, relation_type, confidence, source_memory_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(source_id, target_id, relation_type) DO UPDATE SET confidence = MAX(confidence, ?4)",
                    params![source_id, target_id, relation_type, confidence, source_memory_id, now],
                )?;
        Ok(conn.last_insert_rowid())
    }

    /// List all knowledge graph entities.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_entities(&self) -> Result<Vec<crate::memory::knowledge_graph::Entity>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
                    "SELECT id, name, entity_type, mention_count, created_at, updated_at FROM kg_entities ORDER BY mention_count DESC",
                )?;
        let entities = stmt
            .query_map([], |row| {
                Ok(crate::memory::knowledge_graph::Entity {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    entity_type: row.get(2)?,
                    mention_count: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entities)
    }

    /// List all knowledge graph relationships.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_relationships(&self) -> Result<Vec<crate::memory::knowledge_graph::Relationship>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
                    "SELECT id, source_id, target_id, relation_type, confidence, source_memory_id, created_at FROM kg_relationships ORDER BY confidence DESC",
                )?;
        let relationships = stmt
            .query_map([], |row| {
                Ok(crate::memory::knowledge_graph::Relationship {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    target_id: row.get(2)?,
                    relation_type: row.get(3)?,
                    confidence: row.get(4)?,
                    source_memory_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(relationships)
    }

    /// Execute a knowledge graph query: find all entities and relationships
    /// connected to a given entity (1-hop neighbours).
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn query_entity_neighbours(
        &self,
        entity_id: i64,
    ) -> Result<(
        Vec<crate::memory::knowledge_graph::Entity>,
        Vec<crate::memory::knowledge_graph::Relationship>,
    )> {
        let conn = lock_conn!(self)?;

        // Find all relationships where this entity is source or target.
        let mut rel_stmt = conn.prepare(
                    "SELECT id, source_id, target_id, relation_type, confidence, source_memory_id, created_at
                     FROM kg_relationships WHERE source_id = ?1 OR target_id = ?1",
                )?;
        let relationships: Vec<crate::memory::knowledge_graph::Relationship> = rel_stmt
            .query_map(params![entity_id], |row| {
                Ok(crate::memory::knowledge_graph::Relationship {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    target_id: row.get(2)?,
                    relation_type: row.get(3)?,
                    confidence: row.get(4)?,
                    source_memory_id: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Collect all unique entity IDs from relationships.
        let mut entity_ids = std::collections::HashSet::new();
        entity_ids.insert(entity_id);
        for rel in &relationships {
            entity_ids.insert(rel.source_id);
            entity_ids.insert(rel.target_id);
        }

        // Fetch all neighbour entities.
        let ids: Vec<i64> = entity_ids.into_iter().collect();
        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>();
        let sql = format!(
            "SELECT id, name, entity_type, mention_count, created_at, updated_at FROM kg_entities WHERE id IN ({})",
            placeholders.join(",")
        );
        let mut entity_stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let entities: Vec<crate::memory::knowledge_graph::Entity> = entity_stmt
            .query_map(params.as_slice(), |row| {
                Ok(crate::memory::knowledge_graph::Entity {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    entity_type: row.get(2)?,
                    mention_count: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok((entities, relationships))
    }

    /// Executes a blocking write closure on a Tokio blocking-thread-pool thread.    ///
    /// All `rusqlite` operations are synchronous. Call this from async code to    /// avoid stalling the async executor during writes. The closure receives a
    /// reference to the storage and returns any `Result<T>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the blocking task panics or if the closure itself
    /// returns an error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use ragent_core::storage::Storage;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let storage = Arc::new(Storage::open_in_memory()?);
    /// let id = "sess-1".to_string();
    /// Storage::write_async(Arc::clone(&storage), move |s| {
    ///     s.create_session(&id, "/tmp")
    /// }).await?;
    /// # Ok(()) }
    /// ```
    pub async fn write_async<F, T>(storage: Arc<Self>, f: F) -> Result<T>
    where
        F: FnOnce(&Self) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(move || f(&storage))
            .await
            .context("storage write task panicked")?
    }
}

/// Raw row representation of a session as stored in `SQLite`.
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

/// Row representation of a journal entry.
#[derive(Debug, Clone)]
pub struct JournalEntryRow {
    /// Unique entry identifier (UUID v4).
    pub id: String,
    /// Short title describing the entry.
    pub title: String,
    /// Full content of the journal entry.
    pub content: String,
    /// Project this entry belongs to.
    pub project: String,
    /// Session that created this entry.
    pub session_id: String,
    /// ISO-8601 timestamp of the observation/event.
    pub timestamp: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// Row representation of a structured memory.
#[derive(Debug, Clone)]
pub struct MemoryRow {
    /// Auto-generated row ID.
    pub id: i64,
    /// The memory content.
    pub content: String,
    /// Category: fact, pattern, preference, insight, error, workflow.
    pub category: String,
    /// Source of the memory (e.g., tool name, auto-extract).
    pub source: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Project this memory belongs to.
    pub project: String,
    /// Session that created this memory.
    pub session_id: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
    /// Number of times this memory has been accessed in search results.
    pub access_count: i64,
    /// ISO-8601 timestamp of last access.
    pub last_accessed: Option<String>,
}
