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
//! use ragent_storage::storage::Storage;
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

use ragent_types::message::{Message, MessagePart, Role};

/// Extract searchable text content from a message's parts.
///
/// Concatenates all [`MessagePart::Text`] blocks, tool-call names, and
/// reasoning text.  This is the text that gets indexed in `messages_fts`
/// and returned in [`MessageSearchResult::content`].
fn extract_message_text(parts: &[MessagePart]) -> String {
    let mut buf = Vec::new();
    for part in parts {
        match part {
            MessagePart::Text { text } => buf.push(text.clone()),
            MessagePart::ToolCall { tool, .. } => {
                buf.push(format!("[tool: {tool}]"));
            }
            MessagePart::Reasoning { text } => buf.push(text.clone()),
            MessagePart::Image(_) => {}
        }
    }
    buf.join(" ")
}

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
/// use ragent_storage::storage::{encrypt_key, decrypt_key};
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
    let mut rng = rand::thread_rng();
    rng.fill(&mut nonce);

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
/// use ragent_storage::storage::{encrypt_key, decrypt_key};
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
/// use ragent_storage::storage::obfuscate_key;
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
/// use ragent_storage::storage::{obfuscate_key, deobfuscate_key};
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
    /// PERF-004: cached result of the `format_version` column-existence
    /// pragma query.  Populated once during [`Storage::migrate`] (or lazily
    /// on the first session read if the storage was constructed without
    /// running migrations) and reused by [`Storage::get_session`] and
    /// [`Storage::list_sessions`] to skip one `SQLite` round-trip per call.
    /// An `AtomicBool` is sufficient because the schema never loses the
    /// column once it has been added.
    has_format_version: std::sync::atomic::AtomicBool,
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
    /// Acquire the internal connection lock for raw SQL access.
    ///
    /// Returns a `MutexGuard` that derefs to the `rusqlite::Connection`.
    /// Intended for migration verification tests that need to inspect
    /// table schemas directly.
    #[doc(hidden)]
    pub fn conn_lock_for_test(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))
    }

    /// PERF-004: return whether the `sessions.format_version` column
    /// exists, using the cached `AtomicBool` when it has already been
    /// populated (by [`migrate`](Self::migrate) or a prior call).
    ///
    /// On the first call after construction — when the flag is still
    /// `false` — this runs the `pragma_table_info` query once and records
    /// the result so every subsequent `get_session` / `list_sessions`
    /// call skips the `SQLite` round-trip.  The schema never loses the
    /// column after it has been added, so caching is safe.
    fn has_format_version_cached(&self, conn: &rusqlite::Connection) -> Result<bool> {
        if self
            .has_format_version
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(true);
        }
        let has: bool = conn
            .prepare(
                "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name='format_version'",
            )?
            .query_row([], |r| r.get::<_, i64>(0))
            .unwrap_or(0)
            > 0;
        self.has_format_version
            .store(has, std::sync::atomic::Ordering::Relaxed);
        Ok(has)
    }

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
    /// use ragent_storage::storage::Storage;
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
        // PERF: Use WAL so background writers (e.g. the FTS warm-up) do not
        // block concurrent readers on the main thread, and set a busy_timeout
        // so a transient lock never surfaces as an immediate `DatabaseBusy`
        // error. Without these, `journal_mode=delete` serialises every reader
        // behind a writer and startup `get_setting` calls stall for the whole
        // rebuild.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        let storage = Self {
            conn: Mutex::new(conn),
            has_format_version: std::sync::atomic::AtomicBool::new(false),
        };
        storage.migrate()?;
        Ok(storage)
    }

    /// Returns the current `journal_mode` (e.g. `"wal"`, `"delete"`) for the
    /// underlying connection.  Used by tests to assert that a file-backed
    /// [`open`](Self::open) enables WAL so a background writer never
    /// serialises concurrent readers.
    pub fn journal_mode(&self) -> Result<String> {
        let conn = lock_conn!(self)?;
        let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        Ok(mode)
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
    /// use ragent_storage::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// ```
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Mutex::new(conn),
            has_format_version: std::sync::atomic::AtomicBool::new(false),
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
            format_version INTEGER NOT NULL DEFAULT 1,
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

            CREATE TABLE IF NOT EXISTS run_cost_summaries (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            total_cost_usd REAL NOT NULL,
            duration_ms INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE INDEX IF NOT EXISTS idx_run_cost_session
            ON run_cost_summaries(session_id, created_at);

            CREATE TABLE IF NOT EXISTS background_tasks (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            command TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'running',
            exit_code INTEGER,
            stdout TEXT NOT NULL DEFAULT '',
            stderr TEXT NOT NULL DEFAULT '',
            progress_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT,
            FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE INDEX IF NOT EXISTS idx_bg_tasks_session
            ON background_tasks(session_id, status, updated_at);

            CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS discovered_models (
            provider_id TEXT PRIMARY KEY,
            models_json TEXT NOT NULL,
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

            -- Durable initiatives (JCODEPLAN M8): long-lived goals that
            -- survive across sessions, with JSON milestone tracking.
            CREATE TABLE IF NOT EXISTS initiatives (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'active',
            milestones_json TEXT NOT NULL DEFAULT '[]',
            progress INTEGER NOT NULL DEFAULT 0,
            project TEXT NOT NULL DEFAULT '',
            session_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            closed_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_initiatives_project
            ON initiatives(project, status);

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

            -- M5: Full-text search index over session messages.
            -- Standalone FTS5 table (not external-content) because the
            -- messages table uses TEXT primary keys rather than integer
            -- rowids.  Kept in sync manually by create_message /
            -- update_message / delete_messages and by
            -- warm_message_search_index.
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            message_id UNINDEXED,
            session_id UNINDEXED,
            role UNINDEXED,
            content,
            tokenize = 'porter unicode61'
            );

            -- M5: Optional embedding cache for session messages.
            -- When an embedding provider is configured, message vectors are
            -- stored here and used for semantic search.  If the table is empty,
            -- tools fall back to the FTS5 keyword index.
            CREATE TABLE IF NOT EXISTS messages_embedding (
            message_id TEXT PRIMARY KEY,
            embedding BLOB NOT NULL,
            dimensions INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_messages_embedding_dims
            ON messages_embedding(dimensions);

            -- Agent cron system (spec agentchron): scheduled agent runs.
            -- Stores one-shot and repeating events with their schedule
            -- definition, enabled flag, and computed next-due timestamp.
            -- The scheduler reads enabled events whose next_due has passed
            -- and spawns agent runs via the existing new_task path.
            CREATE TABLE IF NOT EXISTS cron_events (
            id TEXT PRIMARY KEY,
            agent_type TEXT NOT NULL,
            prompt TEXT NOT NULL,
            schedule_form TEXT NOT NULL,
            start_at TEXT,
            duration_secs INTEGER,
            schedule_raw TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            next_due TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_fired TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_cron_events_next_due
            ON cron_events(enabled, next_due);

            ",
        )?;
        // Idempotent column additions (SQLite has no ALTER TABLE ADD COLUMN IF NOT EXISTS)
        for (table, col) in &[
            ("memories", "embedding"),
            ("sessions", "format_version"),
            ("cron_events", "stateful"),
        ] {
            let has_col: bool = conn
                .prepare(&format!(
                    "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name='{col}'"
                ))?
                .query_row([], |r| r.get::<_, i64>(0))
                .unwrap_or(0)
                > 0;
            if !has_col {
                let sql = if *table == "sessions" && *col == "format_version" {
                    "ALTER TABLE sessions ADD COLUMN format_version INTEGER NOT NULL DEFAULT 1;"
                } else if *table == "cron_events" && *col == "stateful" {
                    "ALTER TABLE cron_events ADD COLUMN stateful INTEGER NOT NULL DEFAULT 0;"
                } else {
                    &format!("ALTER TABLE {table} ADD COLUMN {col} BLOB;")
                };
                conn.execute_batch(sql)?;
            } else if *table == "sessions" && *col == "format_version" {
                // PERF-004: cache the column existence so get_session /
                // list_sessions can skip the pragma round-trip on every
                // call. `migrate` runs exactly once per Storage handle, so
                // recording the result here is safe.
                self.has_format_version
                    .store(true, std::sync::atomic::Ordering::Relaxed);
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/home/user/project").unwrap();
    /// let session = storage.get_session("sess-1").unwrap();
    /// assert!(session.is_some());
    /// assert_eq!(session.unwrap().directory, "/home/user/project");
    /// ```
    pub fn get_session(&self, id: &str) -> Result<Option<SessionRow>> {
        let conn = lock_conn!(self)?;
        // PERF-004: skip the pragma_table_info round-trip when we already
        // know from `migrate()` whether the `format_version` column exists.
        // On the rare miss (storage constructed without migrate, or the
        // flag was never set), fall back to the pragma query and cache the
        // result so subsequent calls stay on the fast path.
        let has_format_version = self.has_format_version_cached(&conn)?;
        let sql = if has_format_version {
            "SELECT id, title, project_id, directory, parent_id, version, format_version, \
             created_at, updated_at, archived_at, summary FROM sessions WHERE id = ?1"
                .to_string()
        } else {
            "SELECT id, title, project_id, directory, parent_id, version, \
             created_at, updated_at, archived_at, summary FROM sessions WHERE id = ?1"
                .to_string()
        };

        let mut stmt = conn.prepare(&sql)?;
        let row = stmt
            .query_row(params![id], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    directory: row.get(3)?,
                    parent_id: row.get(4)?,
                    version: row.get(5)?,
                    format_version: if has_format_version { row.get(6)? } else { 1 },
                    created_at: if has_format_version {
                        row.get(7)?
                    } else {
                        row.get(6)?
                    },
                    updated_at: if has_format_version {
                        row.get(8)?
                    } else {
                        row.get(7)?
                    },
                    archived_at: if has_format_version {
                        row.get(9)?
                    } else {
                        row.get(8)?
                    },
                    summary: if has_format_version {
                        row.get(10)?
                    } else {
                        row.get(9)?
                    },
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
    /// use ragent_storage::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project-a").unwrap();
    /// storage.create_session("sess-2", "/tmp/project-b").unwrap();
    /// let sessions = storage.list_sessions().unwrap();
    /// assert_eq!(sessions.len(), 2);
    /// ```
    pub fn list_sessions(&self) -> Result<Vec<SessionRow>> {
        let conn = lock_conn!(self)?;
        // PERF-004: use the cached `format_version` existence flag so we
        // skip the pragma round-trip on every `list_sessions` call.
        let has_format_version = self.has_format_version_cached(&conn)?;
        let sql = if has_format_version {
            "SELECT id, title, project_id, directory, parent_id, version, format_version, \
             created_at, updated_at, archived_at, summary \
             FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC"
                .to_string()
        } else {
            "SELECT id, title, project_id, directory, parent_id, version, \
             created_at, updated_at, archived_at, summary \
             FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC"
                .to_string()
        };

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    directory: row.get(3)?,
                    parent_id: row.get(4)?,
                    version: row.get(5)?,
                    format_version: if has_format_version { row.get(6)? } else { 1 },
                    created_at: if has_format_version {
                        row.get(7)?
                    } else {
                        row.get(6)?
                    },
                    updated_at: if has_format_version {
                        row.get(8)?
                    } else {
                        row.get(7)?
                    },
                    archived_at: if has_format_version {
                        row.get(9)?
                    } else {
                        row.get(8)?
                    },
                    summary: if has_format_version {
                        row.get(10)?
                    } else {
                        row.get(9)?
                    },
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
    /// use ragent_types::message::Message;
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
        // M5: Sync the message FTS index.
        let content = extract_message_text(&msg.parts);
        conn.execute(
            "INSERT INTO messages_fts (message_id, session_id, role, content) \
             VALUES (?1, ?2, ?3, ?4)",
            params![msg.id, msg.session_id, role_str, content],
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
    /// use ragent_storage::storage::Storage;
    /// use ragent_types::message::Message;
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
                "compaction" => Role::Compaction,
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
    /// use ragent_storage::storage::Storage;
    /// use ragent_types::message::{Message, MessagePart};
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// let mut msg = Message::user_text("sess-1", "draft");
    /// storage.create_message(&msg).unwrap();
    /// msg.parts = vec![MessagePart::Text { text: "revised".into() }];
    /// storage.update_message(&msg).unwrap();
    /// ```
    pub fn update_message(&self, msg: &Message) -> Result<()> {
        self.update_message_parts(msg, true)
    }

    /// Updates the parts and `updated_at` timestamp of a message, optionally
    /// syncing the FTS index.
    ///
    /// When `sync_fts` is `false` the `messages_fts` entry is left untouched.
    /// This is used for cheap interim saves whose text content is unchanged
    /// (e.g. a tool-call status transition mid-step) so the FTS index is not
    /// rewritten on every stream event.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the update fails.
    fn update_message_parts(&self, msg: &Message, sync_fts: bool) -> Result<()> {
        let conn = lock_conn!(self)?;
        let parts_json = serde_json::to_string(&msg.parts)?;
        let updated = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE messages SET parts = ?1, updated_at = ?2 WHERE id = ?3",
            params![parts_json, updated, msg.id],
        )?;
        if sync_fts {
            // M5: Sync the message FTS index — delete old entry and re-insert.
            conn.execute(
                "DELETE FROM messages_fts WHERE message_id = ?1",
                params![msg.id],
            )?;
            let content = extract_message_text(&msg.parts);
            let role_str = msg.role.to_string();
            conn.execute(
                "INSERT INTO messages_fts (message_id, session_id, role, content) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![msg.id, msg.session_id, role_str, content],
            )?;
        }
        Ok(())
    }

    /// Updates the parts of a message without rewriting the FTS index.
    ///
    /// This is a cheap variant of [`Storage::update_message`] intended for
    /// interim (in-progress) assistant saves whose searchable text content is
    /// unchanged — typically a tool-call status transition mid-step. The FTS
    /// entry is only re-synced once the message is finalised via
    /// [`Storage::update_message`].
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the update fails.
    pub fn update_message_parts_skip_fts(&self, msg: &Message) -> Result<()> {
        self.update_message_parts(msg, false)
    }

    /// Deletes all messages for a session.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::Storage;
    /// use ragent_types::message::Message;
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
        // M5: Remove FTS entries before deleting messages.
        conn.execute(
            "DELETE FROM messages_fts WHERE session_id = ?1",
            params![session_id],
        )?;
        let n = conn.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(n)
    }

    // ── Run-cost summaries (FR-018) ────────────────────────────────────

    /// Inserts a persisted run-cost summary row (FR-018).
    ///
    /// Run-cost summaries are stored separately from the session transcript so
    /// that the default session export never exposes per-run dollar costs.
    /// They are only included in an export when the caller explicitly opts in
    /// via the `include_cost` flag.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails (e.g., foreign-key violation when
    /// the referenced session does not exist).
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::{RunCostSummaryRow, Storage};
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// let row = RunCostSummaryRow {
    ///     id: "rc-1".to_string(),
    ///     session_id: "sess-1".to_string(),
    ///     model_id: "gpt-4o".to_string(),
    ///     input_tokens: 100,
    ///     output_tokens: 50,
    ///     total_cost_usd: 0.001,
    ///     duration_ms: 1_200,
    ///     created_at: chrono::Utc::now().to_rfc3339(),
    /// };
    /// storage.create_run_cost_summary(&row).unwrap();
    /// let summaries = storage.list_run_cost_summaries("sess-1").unwrap();
    /// assert_eq!(summaries.len(), 1);
    /// assert_eq!(summaries[0].model_id, "gpt-4o");
    /// ```
    pub fn create_run_cost_summary(&self, row: &RunCostSummaryRow) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "INSERT INTO run_cost_summaries \
             (id, session_id, model_id, input_tokens, output_tokens, total_cost_usd, duration_ms, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.id,
                row.session_id,
                row.model_id,
                row.input_tokens as i64,
                row.output_tokens as i64,
                row.total_cost_usd,
                row.duration_ms as i64,
                row.created_at,
            ],
        )?;
        Ok(())
    }

    /// Retrieves all run-cost summaries for a session, ordered chronologically
    /// (oldest first) by `created_at`.
    ///
    /// Used by the `--include-cost` session export path (FR-018) to attach
    /// per-run cost data to an export that explicitly requested it.
    ///
    /// # Errors
    ///
    /// Returns an error if the query or row mapping fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::Storage;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// // No summaries yet for a fresh session.
    /// assert!(storage.list_run_cost_summaries("sess-1").unwrap().is_empty());
    /// ```
    pub fn list_run_cost_summaries(&self, session_id: &str) -> Result<Vec<RunCostSummaryRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, model_id, input_tokens, output_tokens, total_cost_usd, \
             duration_ms, created_at \
             FROM run_cost_summaries WHERE session_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(RunCostSummaryRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    model_id: row.get(2)?,
                    input_tokens: row.get::<_, i64>(3)? as u64,
                    output_tokens: row.get::<_, i64>(4)? as u64,
                    total_cost_usd: row.get(5)?,
                    duration_ms: row.get::<_, i64>(6)? as u64,
                    created_at: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
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
    /// use ragent_storage::storage::Storage;
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

    /// Stores or replaces cached discovered model metadata for a provider.
    ///
    /// The payload should be a serialized JSON array of model metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the upsert fails.
    pub fn set_discovered_models(&self, provider_id: &str, models_json: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO discovered_models (provider_id, models_json, updated_at) VALUES (?1, ?2, ?3)",
            params![provider_id, models_json, now],
        )?;
        Ok(())
    }

    /// Retrieves cached discovered model metadata for a provider, or `None` if absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_discovered_models(&self, provider_id: &str) -> Result<Option<String>> {
        let conn = lock_conn!(self)?;
        let mut stmt =
            conn.prepare("SELECT models_json FROM discovered_models WHERE provider_id = ?1")?;
        let val = stmt
            .query_row(params![provider_id], |row| row.get::<_, String>(0))
            .optional()?;
        Ok(val)
    }

    /// Deletes cached discovered model metadata for a provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete fails.
    pub fn delete_discovered_models(&self, provider_id: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "DELETE FROM discovered_models WHERE provider_id = ?1",
            params![provider_id],
        )?;
        Ok(())
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

    // ── Durable Initiatives (JCODEPLAN M8) ──────────────────────────

    /// Inserts a new durable initiative.
    ///
    /// `milestones` is serialised to JSON into `milestones_json`.
    pub fn create_initiative(
        &self,
        id: &str,
        title: &str,
        description: &str,
        milestones: &[InitiativeMilestone],
        project: &str,
        session_id: &str,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        let milestones_json = serde_json::to_string(milestones)?;
        conn.execute(
            "INSERT INTO initiatives (id, title, description, status, milestones_json, progress, project, session_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'active', ?4, 0, ?5, ?6, ?7, ?7)",
            params![id, title, description, milestones_json, project, session_id, now],
        )?;
        Ok(())
    }

    /// Fetches a single initiative by ID, scoped to `project`.
    pub fn get_initiative(&self, id: &str, project: &str) -> Result<Option<InitiativeRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, title, description, status, milestones_json, progress, project, session_id, created_at, updated_at, closed_at
             FROM initiatives WHERE id = ?1 AND project = ?2",
        )?;
        let row = stmt
            .query_row(params![id, project], initiative_from_row)
            .optional()?;
        Ok(row)
    }

    /// Lists initiatives for a project, optionally filtered by status.
    ///
    /// Pass `Some("active")` etc. to filter, or `None` / `Some("all")` for all.
    pub fn list_initiatives(
        &self,
        project: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<InitiativeRow>> {
        let conn = lock_conn!(self)?;
        let rows = match status_filter {
            Some(s) if s != "all" => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, description, status, milestones_json, progress, project, session_id, created_at, updated_at, closed_at
                     FROM initiatives WHERE project = ?1 AND status = ?2
                     ORDER BY created_at",
                )?;
                stmt.query_map(params![project, s], initiative_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
            _ => {
                let mut stmt = conn.prepare(
                    "SELECT id, title, description, status, milestones_json, progress, project, session_id, created_at, updated_at, closed_at
                     FROM initiatives WHERE project = ?1
                     ORDER BY created_at",
                )?;
                stmt.query_map(params![project], initiative_from_row)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            }
        };
        Ok(rows)
    }

    /// Updates mutable initiative fields and/or status/closed_at.
    ///
    /// Returns `true` when a row was updated. `closed_at` is set automatically
    /// when `status` transitions to `completed` or `abandoned` and is cleared
    /// when the initiative re-opens (`active`/`paused`).
    #[allow(clippy::too_many_arguments)]
    pub fn update_initiative(
        &self,
        id: &str,
        project: &str,
        title: Option<&str>,
        description: Option<&str>,
        milestones: Option<&[InitiativeMilestone]>,
        progress: Option<u8>,
        status: Option<&str>,
        note: Option<&str>,
    ) -> Result<bool> {
        let conn = lock_conn!(self)?;
        // Checkpoint notes are recorded in the `initiative_notes` JSON array
        // embedded in the description is *not* used — notes are appended to a
        // dedicated `settings` key instead so they never collide with the
        // human-readable description. Kept simple: ignore when None.
        let _ = note;

        let now = Utc::now().to_rfc3339();
        let mut sets: Vec<String> = vec!["updated_at = ?NOW".to_string()];
        let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now.clone())];

        if let Some(t) = title {
            vals.push(Box::new(t.to_string()));
            sets.push(format!("title = ?{}", vals.len()));
        }
        if let Some(d) = description {
            vals.push(Box::new(d.to_string()));
            sets.push(format!("description = ?{}", vals.len()));
        }
        if let Some(m) = milestones {
            let mj = serde_json::to_string(m)?;
            vals.push(Box::new(mj));
            sets.push(format!("milestones_json = ?{}", vals.len()));
        }
        if let Some(p) = progress {
            vals.push(Box::new(i64::from(p)));
            sets.push(format!("progress = ?{}", vals.len()));
        }
        if let Some(s) = status {
            vals.push(Box::new(s.to_string()));
            sets.push(format!("status = ?{}", vals.len()));
            if s == "completed" || s == "abandoned" {
                vals.push(Box::new(now.clone()));
                sets.push(format!("closed_at = ?{}", vals.len()));
            } else {
                sets.push("closed_at = NULL".to_string());
            }
        }

        vals.push(Box::new(id.to_string()));
        let id_ph = format!("?{}", vals.len());
        vals.push(Box::new(project.to_string()));
        let proj_ph = format!("?{}", vals.len());

        let sql = format!(
            "UPDATE initiatives SET {} WHERE id = {} AND project = {}",
            sets.join(", ").replace("?NOW", "?1"),
            id_ph,
            proj_ph
        );
        let params: Vec<&dyn rusqlite::types::ToSql> =
            vals.iter().map(std::convert::AsRef::as_ref).collect();
        let changed = conn.execute(&sql, params.as_slice())?;
        Ok(changed > 0)
    }

    /// Deletes an initiative. Returns `true` when a row was removed.
    pub fn delete_initiative(&self, id: &str, project: &str) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let changed = conn.execute(
            "DELETE FROM initiatives WHERE id = ?1 AND project = ?2",
            params![id, project],
        )?;
        Ok(changed > 0)
    }

    // ── Cron Events CRUD (spec agentchron T-006) ─────────────────────

    /// Insert a new cron event into the `cron_events` table (FR-001, FR-002).
    ///
    /// The event's `id` must be unique; inserting a duplicate id fails with a
    /// constraint error. All schedule fields are flattened into columns:
    /// `schedule_form` stores the serde string (`one_shot`, `repeat_from`,
    /// `repeat_now`), `start_at` and `duration_secs` are nullable.
    ///
    /// # Errors
    ///
    /// Returns an error if the SQLite insert fails (e.g. duplicate id).
    pub fn insert_cron_event(&self, event: &ragent_types::CronEvent) -> Result<()> {
        let conn = lock_conn!(self)?;
        let form_str = serde_json::to_string(&event.schedule.form)
            .map_err(|e| anyhow::anyhow!("failed to serialise CronForm: {e}"))?;
        // serde_json::to_string produces a quoted string; strip the quotes.
        let form_str = form_str.trim_matches('"');
        let start_at = event.schedule.start_at.map(|t| t.to_rfc3339());
        let next_due = event.next_due.to_rfc3339();
        let created_at = event.created_at.to_rfc3339();
        let last_fired = event.last_fired.map(|t| t.to_rfc3339());
        let enabled_i: i64 = i64::from(event.enabled);
        conn.execute(
            "INSERT INTO cron_events \
           (id, agent_type, prompt, schedule_form, start_at, duration_secs, \
           schedule_raw, enabled, next_due, created_at, last_fired, stateful) \
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                event.id,
                event.agent_type,
                event.prompt,
                form_str,
                start_at,
                event.schedule.duration_secs,
                event.schedule_raw,
                enabled_i,
                next_due,
                created_at,
                last_fired,
                i64::from(event.stateful),
            ],
        )?;
        Ok(())
    }

    /// Retrieve a single cron event by its id (FR-001).
    ///
    /// Returns `None` if no event with the given id exists.
    pub fn get_cron_event(&self, id: &str) -> Result<Option<CronEventRow>> {
        let conn = lock_conn!(self)?;
        let row = conn
            .query_row(
                "SELECT id, agent_type, prompt, schedule_form, start_at, \
               duration_secs, schedule_raw, enabled, next_due, created_at, \
               last_fired, stateful FROM cron_events WHERE id = ?1",
                params![id],
                cron_event_from_row,
            )
            .optional()?;
        Ok(row)
    }

    /// List all cron events, ordered by `next_due` ascending (FR-001).
    ///
    /// Used by the `/cron list` slash command.
    pub fn list_cron_events(&self) -> Result<Vec<CronEventRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, agent_type, prompt, schedule_form, start_at, \
           duration_secs, schedule_raw, enabled, next_due, created_at, \
           last_fired, stateful FROM cron_events ORDER BY next_due",
        )?;
        let rows = stmt
            .query_map([], cron_event_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// List enabled cron events whose `next_due` is at or before `now`
    /// (FR-010). Used by the scheduler tick to find events that need firing.
    ///
    /// Returns events ordered by `next_due` ascending.
    pub fn list_due_cron_events(&self, now: &DateTime<Utc>) -> Result<Vec<CronEventRow>> {
        let conn = lock_conn!(self)?;
        let now_str = now.to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, agent_type, prompt, schedule_form, start_at, \
           duration_secs, schedule_raw, enabled, next_due, created_at, \
           last_fired, stateful FROM cron_events WHERE enabled = 1 AND next_due <= ?1 \
           ORDER BY next_due",
        )?;
        let rows = stmt
            .query_map(params![now_str], cron_event_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// List **disabled** cron events whose `next_due` is at or before `now`
    /// (FR-007, FR-011). Used by the scheduler tick to log `"skipped"`
    /// outcomes for due events that are not enabled.
    ///
    /// Returns events ordered by `next_due` ascending.
    pub fn list_disabled_due_cron_events(&self, now: &DateTime<Utc>) -> Result<Vec<CronEventRow>> {
        let conn = lock_conn!(self)?;
        let now_str = now.to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, agent_type, prompt, schedule_form, start_at, \
           duration_secs, schedule_raw, enabled, next_due, created_at, \
           last_fired, stateful FROM cron_events WHERE enabled = 0 AND next_due <= ?1 \
           ORDER BY next_due",
        )?;
        let rows = stmt
            .query_map(params![now_str], cron_event_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Delete a cron event by id (FR-001). Returns `true` if a row was removed.
    pub fn delete_cron_event(&self, id: &str) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let changed = conn.execute("DELETE FROM cron_events WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    /// Update a cron event's `next_due` and optionally `last_fired` after a
    /// fire (FR-004, FR-005). This is the "touch-next-due" operation the
    /// scheduler calls after spawning an agent run.
    ///
    /// - For repeating events, pass the advanced `next_due` and `Some(now)` as
    ///   `last_fired`.
    /// - For one-shot events, pass `Some(now)` as `last_fired` and set
    ///   `next_due` to the same value (or disable the event separately via
    ///   [`Self::set_cron_event_enabled`]).
    ///
    /// Returns `true` if a row was updated.
    pub fn update_cron_event_next_due(
        &self,
        id: &str,
        next_due: &DateTime<Utc>,
        last_fired: Option<&DateTime<Utc>>,
    ) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let next_due_str = next_due.to_rfc3339();
        let last_fired_str = last_fired.map(|t| t.to_rfc3339());
        let changed = conn.execute(
            "UPDATE cron_events SET next_due = ?1, last_fired = ?2 WHERE id = ?3",
            params![next_due_str, last_fired_str, id],
        )?;
        Ok(changed > 0)
    }

    /// Enable or disable a cron event (FR-007, FR-011).
    ///
    /// When `enabled` is `false`, the scheduler skips the event and logs
    /// `"skipped"` instead of firing it.
    ///
    /// Returns `true` if a row was updated.
    pub fn set_cron_event_enabled(&self, id: &str, enabled: bool) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let enabled_i: i64 = i64::from(enabled);
        let changed = conn.execute(
            "UPDATE cron_events SET enabled = ?1 WHERE id = ?2",
            params![enabled_i, id],
        )?;
        Ok(changed > 0)
    }

    // ── Structured Memory CRUD ──────────────────────────────────────

    /// Inserts a new structured memory with category, tags, and confidence.
    ///
    /// Returns the auto-generated row ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert fails (e.g., invalid category).
    #[allow(clippy::too_many_arguments)]
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
            limit_param_idx + tags.map_or(0, <[std::string::String]>::len) + 1
        );

        // Build parameter list.
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(cats) = categories
            && !cats.is_empty()
        {
            for cat in cats {
                params_vec.push(Box::new(cat.clone()));
            }
        }
        params_vec.push(Box::new(safe_query));
        params_vec.push(Box::new(min_confidence));
        if let Some(tags) = tags
            && !tags.is_empty()
        {
            for tag in tags {
                params_vec.push(Box::new(tag.clone()));
            }
        }
        params_vec.push(Box::new(limit as i64));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

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
            && tags.is_none_or(<[std::string::String]>::is_empty)
        {
            anyhow::bail!("At least one filter criterion is required to delete memories");
        }

        let conn = lock_conn!(self)?;
        let cutoff = older_than_days.map(|days| {
            let dt = Utc::now() - chrono::Duration::days(i64::from(days));
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
        if let Some(tags) = tags
            && !tags.is_empty()
        {
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

        let where_clause = conditions.join(" AND ");
        let sql = format!("SELECT id FROM memories WHERE {where_clause}");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();

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

    /// Increments the access count and updates `last_accessed` for a memory.
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

    /// Counts structured memories scoped to the current working directory.
    ///
    /// Memories are matched by either the full directory path (as used by the
    /// current `memory_store` tool) or the directory basename (used by older
    /// versions), so that legacy entries remain visible after the project key
    /// format changed from a basename to a full path.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn count_memories_for_project(&self, project_dir: &Path) -> Result<u64> {
        let conn = lock_conn!(self)?;
        let full = project_dir.to_string_lossy();
        let name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let count: u64 = if name.is_empty() {
            conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE project = ?1",
                params![full.as_ref()],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE project IN (?1, ?2)",
                params![full.as_ref(), name],
                |row| row.get(0),
            )?
        };
        Ok(count)
    }

    /// List structured memories scoped to the current working directory.
    ///
    /// Matches memories whose `project` column equals either the full directory
    /// path or the directory basename. This keeps memories stored by older
    /// ragent versions (which used the basename as the project key) visible
    /// alongside memories stored by the current full-path format.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_memories_for_project(
        &self,
        project_dir: &Path,
        limit: usize,
    ) -> Result<Vec<MemoryRow>> {
        let conn = lock_conn!(self)?;
        let full = project_dir.to_string_lossy();
        let name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let mut stmt;
        let rows = if name.is_empty() {
            stmt = conn.prepare(
                "SELECT id, content, category, source, confidence, project, session_id,
                        created_at, updated_at, access_count, last_accessed
                 FROM memories
                 WHERE project = ?1
                 ORDER BY updated_at DESC, confidence DESC
                 LIMIT ?2",
            )?;
            stmt.query_map(params![full.as_ref(), limit as i64], memory_row_from_sql)?
        } else {
            stmt = conn.prepare(
                "SELECT id, content, category, source, confidence, project, session_id,
                        created_at, updated_at, access_count, last_accessed
                 FROM memories
                 WHERE project IN (?1, ?2)
                 ORDER BY updated_at DESC, confidence DESC
                 LIMIT ?3",
            )?;
            stmt.query_map(
                params![full.as_ref(), name, limit as i64],
                memory_row_from_sql,
            )?
        };
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// List all structured memories across every project.
    ///
    /// Returns entries ordered by most recently updated first, limited to the
    /// requested number. This is used by UI panels that need to surface every
    /// stored memory regardless of project scope.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_all_memories(&self, limit: usize) -> Result<Vec<MemoryRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, content, category, source, confidence, project, session_id,
                    created_at, updated_at, access_count, last_accessed
             FROM memories
             ORDER BY updated_at DESC, confidence DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
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

    /// Decode an embedding blob into a `Vec<f32>` of the expected dimensionality.
    ///
    /// Embeddings are stored as little-endian `f32` arrays (4 bytes per
    /// dimension).  Returns an error if the blob length does not match
    /// `dimensions * 4`.  This is the storage-local equivalent of
    /// `ragent_tools_extended::memory::embedding::deserialise_embedding`;
    /// it lives here so `search_memories_by_embedding` can decode blobs
    /// without depending on the tools-extended embedding helpers.
    fn deserialise_embedding_owned(blob: &[u8], dimensions: usize) -> Result<Vec<f32>> {
        if blob.len() != dimensions * 4 {
            anyhow::bail!(
                "Embedding blob length {} does not match expected {} bytes ({} dims × 4)",
                blob.len(),
                dimensions * 4,
                dimensions
            );
        }
        let mut vec = Vec::with_capacity(dimensions);
        for chunk in blob.chunks_exact(4) {
            let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            vec.push(val);
        }
        Ok(vec)
    }

    /// Search structured memories by cosine similarity against a query embedding.
    ///
    /// Loads all stored memory embeddings and computes brute-force cosine
    /// similarity against `query_embedding`. Returns results ranked by
    /// similarity (highest first), filtered to those with `score >=
    /// min_similarity`, and truncated to `limit` rows.
    ///
    /// This approach is acceptable for up to ~10K memories. For larger
    /// datasets, consider using `sqlite-vec` for ANN search.
    ///
    /// The cosine-similarity computation is delegated to the caller-supplied
    /// `similarity` closure so that `ragent-storage` does not need to depend
    /// on the embedding helpers that live in `ragent-agent`/`ragent-tools-extended`.
    /// Callers typically pass `ragent_tools_extended::memory::embedding::cosine_similarity`
    /// (or the agent's local equivalent).
    ///
    /// # Errors
    ///
    /// Returns an error if the embedding-list query fails.
    pub fn search_memories_by_embedding<F>(
        &self,
        query_embedding: &[f32],
        dimensions: usize,
        limit: usize,
        min_similarity: f32,
        similarity: F,
    ) -> Result<Vec<EmbeddingMatch>>
    where
        F: Fn(&[f32], &[f32]) -> f32,
    {
        let embeddings = self.list_memory_embeddings()?;
        let mut results: Vec<EmbeddingMatch> = Vec::new();

        for (row_id, blob) in &embeddings {
            // Attempt to deserialise the blob into a `dimensions`-length
            // `f32` slice.  Blobs that fail to deserialise or have the wrong
            // dimensionality are silently skipped — they correspond to
            // memories embedded with a different model/dimensionality and
            // cannot be compared against this query.
            if let Ok(stored) = Self::deserialise_embedding_owned(blob, dimensions) {
                let score = similarity(query_embedding, &stored);
                if score >= min_similarity {
                    results.push(EmbeddingMatch {
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

    // ── Session message embeddings (M5) ────────────────────────���────────

    /// Stores or updates an embedding vector for a session message.
    ///
    /// The `embedding_blob` is a little-endian `f32` byte blob produced by an
    /// embedding provider.  The message's `created_at` timestamp is recorded
    /// alongside the vector.  When no provider is available, tools fall back
    /// to the FTS5 keyword index instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the insert/upsert fails.
    pub fn store_message_embedding(
        &self,
        message_id: &str,
        embedding_blob: &[u8],
        dimensions: usize,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO messages_embedding (message_id, embedding, dimensions, created_at) \
                   VALUES (?1, ?2, ?3, ?4) \
                   ON CONFLICT(message_id) DO UPDATE SET \
                       embedding = excluded.embedding, \
                       dimensions = excluded.dimensions, \
                       created_at = excluded.created_at",
            params![message_id, embedding_blob, dimensions as i64, now],
        )?;
        Ok(())
    }

    /// Returns the stored embedding for a single message, if present.
    ///
    /// Decodes the blob against the requested dimensionality.  Returns `None`
    /// when no embedding has been stored for the message or the stored vector
    /// has a different `dimensions`.
    pub fn get_message_embedding(
        &self,
        message_id: &str,
        dimensions: usize,
    ) -> Result<Option<Vec<f32>>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT embedding FROM messages_embedding \
                   WHERE message_id = ?1 AND dimensions = ?2",
        )?;
        let blob: Option<Vec<u8>> = stmt
            .query_row(params![message_id, dimensions as i64], |row| row.get(0))
            .optional()?;
        match blob {
            Some(b) => Self::deserialise_embedding_owned(&b, dimensions).map(Some),
            None => Ok(None),
        }
    }

    /// Lists all stored session-message embeddings.
    pub fn list_message_embeddings(&self) -> Result<Vec<(String, Vec<u8>, usize)>> {
        let conn = lock_conn!(self)?;
        let mut stmt =
            conn.prepare("SELECT message_id, embedding, dimensions FROM messages_embedding")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, blob, dims)| (id, blob, dims as usize))
            .collect())
    }

    /// Searches session messages by cosine similarity against a query embedding.
    ///
    /// Loads all stored message embeddings of the requested dimensionality
    /// and computes brute-force cosine similarity.  Results are ranked high
    /// to low and truncated to `limit`.  Scores below `min_similarity` are
    /// dropped.  The returned `EmbeddingMatch` contains the message ID as the
    /// `row_id`.
    pub fn search_messages_by_embedding<F>(
        &self,
        query_embedding: &[f32],
        dimensions: usize,
        limit: usize,
        min_similarity: f32,
        similarity: F,
    ) -> Result<Vec<MessageEmbeddingMatch>>
    where
        F: Fn(&[f32], &[f32]) -> f32,
    {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT message_id, embedding FROM messages_embedding WHERE dimensions = ?1",
        )?;
        let rows: Vec<(String, Vec<u8>)> = stmt
            .query_map(params![dimensions as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut results = Vec::new();
        for (message_id, blob) in &rows {
            if let Ok(stored) = Self::deserialise_embedding_owned(blob, dimensions) {
                let score = similarity(query_embedding, &stored);
                if score >= min_similarity {
                    results.push(MessageEmbeddingMatch {
                        message_id: message_id.clone(),
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

    /// List all knowledge graph entities, ordered by descending mention count.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_entities(&self) -> Result<Vec<KgEntityRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, name, entity_type, mention_count, created_at, updated_at FROM kg_entities ORDER BY mention_count DESC",
        )?;
        let entities = stmt
            .query_map([], |row| {
                Ok(KgEntityRow {
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

    /// List all knowledge graph relationships, ordered by descending confidence.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn list_relationships(&self) -> Result<Vec<KgRelationshipRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, relation_type, confidence, source_memory_id, created_at FROM kg_relationships ORDER BY confidence DESC",
        )?;
        let relationships = stmt
            .query_map([], |row| {
                Ok(KgRelationshipRow {
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
    ) -> Result<(Vec<KgEntityRow>, Vec<KgRelationshipRow>)> {
        let conn = lock_conn!(self)?;

        // Find all relationships where this entity is source or target.
        let mut rel_stmt = conn.prepare(
            "SELECT id, source_id, target_id, relation_type, confidence, source_memory_id, created_at
             FROM kg_relationships WHERE source_id = ?1 OR target_id = ?1",
        )?;
        let relationships: Vec<KgRelationshipRow> = rel_stmt
            .query_map(params![entity_id], |row| {
                Ok(KgRelationshipRow {
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
        let entities: Vec<KgEntityRow> = entity_stmt
            .query_map(params.as_slice(), |row| {
                Ok(KgEntityRow {
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

    /// Returns `true` if the session has at least one assistant message.
    ///
    /// Used to skip re-running init exchanges on sessions that already have
    /// an assistant turn.  This is a storage-level query (no `Message`
    /// hydration) so it is cheap.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::Storage;
    /// use ragent_types::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// assert!(!storage.has_assistant_messages("sess-1").unwrap());
    /// let msg = Message::assistant_text("sess-1", "Hi");
    /// storage.create_message(&msg).unwrap();
    /// assert!(storage.has_assistant_messages("sess-1").unwrap());
    /// ```
    pub fn has_assistant_messages(&self, session_id: &str) -> Result<bool> {
        let conn = lock_conn!(self)?;
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM messages WHERE session_id = ?1 AND role IN ('assistant', 'compaction') LIMIT 1",
                params![session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        Ok(exists.is_some())
    }

    /// Executes a blocking write closure on a Tokio blocking-thread-pool thread.
    ///
    /// All `rusqlite` operations are synchronous. Call this from async code to
    /// avoid stalling the async executor during writes. The closure receives a
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
    /// use ragent_storage::storage::Storage;
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

    /// Inserts a new background task row.
    pub fn create_background_task(&self, row: &BackgroundTaskRow) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute(
            "INSERT INTO background_tasks (
                id, session_id, command, status, exit_code,
                stdout, stderr, progress_json, created_at, updated_at, completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.id,
                row.session_id,
                row.command,
                row.status,
                row.exit_code,
                row.stdout,
                row.stderr,
                row.progress_json,
                row.created_at,
                row.updated_at,
                row.completed_at
            ],
        )?;
        Ok(())
    }

    /// Fetches a single background task by id.
    pub fn get_background_task(&self, id: &str) -> Result<Option<BackgroundTaskRow>> {
        let conn = lock_conn!(self)?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, command, status, exit_code, stdout, stderr,
                    progress_json, created_at, updated_at, completed_at
             FROM background_tasks WHERE id = ?1",
        )?;
        Ok(stmt
            .query_row(params![id], |row| {
                Ok(BackgroundTaskRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    command: row.get(2)?,
                    status: row.get(3)?,
                    exit_code: row.get(4)?,
                    stdout: row.get(5)?,
                    stderr: row.get(6)?,
                    progress_json: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    completed_at: row.get(10)?,
                })
            })
            .optional()?)
    }

    /// Lists background tasks, optionally filtered by session and/or status.
    pub fn list_background_tasks(
        &self,
        session_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<BackgroundTaskRow>> {
        let conn = lock_conn!(self)?;
        let mut sql = String::from(
            "SELECT id, session_id, command, status, exit_code, stdout, stderr,
                    progress_json, created_at, updated_at, completed_at
             FROM background_tasks WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(sid) = session_id {
            sql.push_str(" AND session_id = ?");
            params_vec.push(Box::new(sid.to_string()));
        }
        if let Some(st) = status {
            sql.push_str(" AND status = ?");
            params_vec.push(Box::new(st.to_string()));
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
        params_vec.push(Box::new(limit as i64));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(BackgroundTaskRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    command: row.get(2)?,
                    status: row.get(3)?,
                    exit_code: row.get(4)?,
                    stdout: row.get(5)?,
                    stderr: row.get(6)?,
                    progress_json: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                    completed_at: row.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Updates status, exit code, and optional completion timestamp for a task.
    pub fn update_background_task_status(
        &self,
        id: &str,
        status: &str,
        exit_code: Option<i64>,
        completed_at: Option<&str>,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE background_tasks SET status = ?1, exit_code = ?2, completed_at = ?3, updated_at = ?4 WHERE id = ?5",
            params![status, exit_code, completed_at, now, id],
        )?;
        Ok(())
    }

    /// Appends captured stdout/stderr to a task and updates `progress_json`.
    pub fn append_background_task_output(
        &self,
        id: &str,
        stdout: &str,
        stderr: &str,
        progress_json: &str,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE background_tasks
             SET stdout = stdout || ?1,
                 stderr = stderr || ?2,
                 progress_json = ?3,
                 updated_at = ?4
             WHERE id = ?5",
            params![stdout, stderr, progress_json, now, id],
        )?;
        Ok(())
    }

    /// Overwrites the full stdout/stderr/progress for a task.
    pub fn set_background_task_output(
        &self,
        id: &str,
        stdout: &str,
        stderr: &str,
        progress_json: &str,
    ) -> Result<()> {
        let conn = lock_conn!(self)?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE background_tasks
             SET stdout = ?1, stderr = ?2, progress_json = ?3, updated_at = ?4
             WHERE id = ?5",
            params![stdout, stderr, progress_json, now, id],
        )?;
        Ok(())
    }

    /// Deletes a single background task row.
    pub fn delete_background_task(&self, id: &str) -> Result<()> {
        let conn = lock_conn!(self)?;
        conn.execute("DELETE FROM background_tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Removes background tasks older than the given number of minutes.
    pub fn cleanup_background_tasks(
        &self,
        session_id: Option<&str>,
        older_than_minutes: i64,
        completed_only: bool,
    ) -> Result<usize> {
        let conn = lock_conn!(self)?;
        let cutoff = (Utc::now() - chrono::Duration::minutes(older_than_minutes)).to_rfc3339();
        let mut sql = String::from("DELETE FROM background_tasks WHERE updated_at < ?1");
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params_vec.push(Box::new(cutoff));
        if completed_only {
            sql.push_str(" AND status IN ('completed', 'failed', 'cancelled')");
        }
        if let Some(sid) = session_id {
            sql.push_str(" AND session_id = ?2");
            params_vec.push(Box::new(sid.to_string()));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(std::convert::AsRef::as_ref).collect();
        Ok(conn.execute(&sql, param_refs.as_slice())?)
    }

    // ── M5: Session message search ──────────────────────────────────────

    /// Searches the current session's messages using the FTS5 full-text index.
    ///
    /// Returns matching messages ranked by FTS5 relevance, with extracted text
    /// content.  Compaction messages are included so the search covers the full
    /// stored history, not just the active context window.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or the FTS match syntax is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::Storage;
    /// use ragent_types::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// storage.create_message(&Message::user_text("sess-1", "database migration plan")).unwrap();
    /// storage.create_message(&Message::assistant_text("sess-1", "I will help with the migration")).unwrap();
    /// let results = storage.search_conversation("sess-1", "migration", 10).unwrap();
    /// assert!(!results.is_empty());
    /// ```
    pub fn search_conversation(
        &self,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MessageSearchResult>> {
        let conn = lock_conn!(self)?;

        // Sanitise the FTS query.
        let safe_query = sanitise_fts_query(query);
        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = conn.prepare(
            "SELECT f.message_id, f.session_id, f.role, f.content,
                    m.created_at, s.title, s.directory, f.rank
             FROM messages_fts f
             INNER JOIN messages m ON m.id = f.message_id
             LEFT JOIN sessions s ON s.id = f.session_id
             WHERE messages_fts MATCH ?1 AND f.session_id = ?2
             ORDER BY f.rank
             LIMIT ?3",
        )?;

        let rows = stmt
            .query_map(params![safe_query, session_id, limit as i64], |row| {
                Ok(MessageSearchResult {
                    message_id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                    session_title: row.get(5)?,
                    session_directory: row.get(6)?,
                    rank: row.get::<_, f64>(7).unwrap_or(0.0),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(rows)
    }

    /// Searches across all sessions' messages using the FTS5 full-text index
    /// with optional filters.
    ///
    /// Supports filtering by date range, working directory, roles, and
    /// per-session result limits.  Returns ranked results from all matching
    /// sessions.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or the FTS match syntax is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::{Storage, SessionSearchParams};
    /// use ragent_types::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project-a").unwrap();
    /// storage.create_message(&Message::user_text("sess-1", "database migration")).unwrap();
    /// storage.create_session("sess-2", "/tmp/project-b").unwrap();
    /// storage.create_message(&Message::assistant_text("sess-2", "the migration is done")).unwrap();
    ///
    /// let params = SessionSearchParams {
    ///     query: "migration".to_string(),
    ///     limit: 10,
    ///     ..Default::default()
    /// };
    /// let results = storage.search_session_messages(&params).unwrap();
    /// assert!(!results.is_empty());
    /// ```
    pub fn search_session_messages(
        &self,
        params: &SessionSearchParams,
    ) -> Result<Vec<MessageSearchResult>> {
        let conn = lock_conn!(self)?;

        // Sanitise the FTS query.
        let safe_query = sanitise_fts_query(&params.query);
        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        // Build the SQL query with optional filters.
        // We use a CTE to apply max_per_session if requested.
        let has_max_per_session = params.max_per_session.is_some();
        let max_per_session = params.max_per_session.unwrap_or(0);

        let mut where_clauses = vec!["messages_fts MATCH ?1".to_string()];
        let mut sql_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        sql_params.push(Box::new(safe_query));

        let mut param_idx = 2; // ?1 is the FTS query

        if let Some(ref since) = params.since {
            where_clauses.push(format!("m.created_at >= ?{param_idx}"));
            sql_params.push(Box::new(since.clone()));
            param_idx += 1;
        }

        if let Some(ref until) = params.until {
            where_clauses.push(format!("m.created_at <= ?{param_idx}"));
            sql_params.push(Box::new(until.clone()));
            param_idx += 1;
        }

        if let Some(ref working_dir) = params.working_dir {
            where_clauses.push(format!("s.directory = ?{param_idx}"));
            sql_params.push(Box::new(working_dir.clone()));
            param_idx += 1;
        }

        if let Some(ref roles) = params.roles
            && !roles.is_empty()
        {
            let placeholders: Vec<String> = (0..roles.len())
                .map(|i| format!("?{}", param_idx + i))
                .collect();
            where_clauses.push(format!("f.role IN ({})", placeholders.join(", ")));
            for role in roles {
                sql_params.push(Box::new(role.clone()));
            }
            param_idx += roles.len();
        }
        if let Some(ref session_id) = params.session_id {
            where_clauses.push(format!("f.session_id = ?{param_idx}"));
            sql_params.push(Box::new(session_id.clone()));
            param_idx += 1;
        }

        let where_sql = where_clauses.join(" AND ");
        let limit = params.limit;

        let sql = if has_max_per_session {
            // Use ROW_NUMBER() to limit per session.
            format!(
                "WITH ranked AS (
                    SELECT f.message_id, f.session_id, f.role, f.content,
                           m.created_at, s.title, s.directory, f.rank,
                           ROW_NUMBER() OVER (PARTITION BY f.session_id ORDER BY f.rank) AS rn
                    FROM messages_fts f
                    INNER JOIN messages m ON m.id = f.message_id
                    LEFT JOIN sessions s ON s.id = f.session_id
                    WHERE {where_sql}
                )
                SELECT message_id, session_id, role, content,
                       created_at, title, directory, rank
                FROM ranked
                WHERE rn <= ?{param_idx}
                ORDER BY rank
                LIMIT ?{}",
                param_idx + 1
            )
        } else {
            format!(
                "SELECT f.message_id, f.session_id, f.role, f.content,
                        m.created_at, s.title, s.directory, f.rank
                 FROM messages_fts f
                 INNER JOIN messages m ON m.id = f.message_id
                 LEFT JOIN sessions s ON s.id = f.session_id
                 WHERE {where_sql}
                 ORDER BY f.rank
                 LIMIT ?{param_idx}"
            )
        };

        if has_max_per_session {
            sql_params.push(Box::new(max_per_session as i64));
        }
        sql_params.push(Box::new(limit as i64));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            sql_params.iter().map(std::convert::AsRef::as_ref).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(MessageSearchResult {
                    message_id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                    session_title: row.get(5)?,
                    session_directory: row.get(6)?,
                    rank: row.get::<_, f64>(7).unwrap_or(0.0),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(rows)
    }

    /// Returns the total message count and per-role breakdown for a session.
    ///
    /// Used by the `conversation_search` stats mode to give the agent a quick
    /// overview of the current session size.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::Storage;
    /// use ragent_types::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// storage.create_message(&Message::user_text("sess-1", "hello")).unwrap();
    /// storage.create_message(&Message::assistant_text("sess-1", "hi there")).unwrap();
    /// let stats = storage.conversation_stats("sess-1").unwrap();
    /// assert_eq!(stats.total, 2);
    /// ```
    pub fn conversation_stats(&self, session_id: &str) -> Result<ConversationStats> {
        let conn = lock_conn!(self)?;
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
                params![session_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let user_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1 AND role = 'user'",
                params![session_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let assistant_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1 AND role = 'assistant'",
                params![session_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let compaction_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id = ?1 AND role = 'compaction'",
                params![session_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let has_compaction = compaction_count > 0;

        Ok(ConversationStats {
            total,
            user_count,
            assistant_count,
            compaction_count,
            has_compaction,
        })
    }

    /// Rebuilds the `messages_fts` index from the existing `messages` table.
    ///
    /// This should be called on startup (T-043) to ensure the FTS index is
    /// up to date after an upgrade or after messages were inserted by a
    /// version of ragent that predated the FTS index.
    ///
    /// Returns the number of messages indexed.
    ///
    /// # Errors
    ///
    /// Returns an error if the rebuild fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_storage::storage::Storage;
    /// use ragent_types::message::Message;
    ///
    /// let storage = Storage::open_in_memory().unwrap();
    /// storage.create_session("sess-1", "/tmp/project").unwrap();
    /// storage.create_message(&Message::user_text("sess-1", "hello")).unwrap();
    /// let count = storage.warm_message_search_index().unwrap();
    /// assert!(count >= 1);
    /// ```
    pub fn warm_message_search_index(&self) -> Result<usize> {
        let mut conn = lock_conn!(self)?;

        // Batch the rebuild in a single transaction.  Without an explicit
        // transaction every INSERT commits independently, and with the
        // default `journal_mode=delete` + `synchronous=FULL` each commit
        // fsyncs the journal and the database file — ~4ms per row, so a
        // 2,000+ message history takes ~9s at startup.  One transaction =
        // one fsync, making the warm-up effectively free.
        let tx = conn.transaction()?;

        // Clear the existing index.
        tx.execute("DELETE FROM messages_fts", [])?;

        // Rebuild from the messages table.
        // We extract text content from the JSON parts column.
        let mut stmt = tx.prepare("SELECT id, session_id, role, parts FROM messages")?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let session_id: String = row.get(1)?;
                let role: String = row.get(2)?;
                let parts_json: String = row.get(3)?;
                Ok((id, session_id, role, parts_json))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        let mut count = 0usize;
        for (id, session_id, role, parts_json) in rows {
            let parts: Vec<MessagePart> = serde_json::from_str(&parts_json).unwrap_or_default();
            let content = extract_message_text(&parts);
            tx.execute(
                "INSERT INTO messages_fts (message_id, session_id, role, content) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, session_id, role, content],
            )?;
            count += 1;
        }

        tx.commit()?;

        Ok(count)
    }
}

/// Statistics about a session's message history.
///
/// Returned by [`Storage::conversation_stats`].  Used by the
/// `conversation_search` tool's stats mode.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversationStats {
    /// Total number of messages in the session.
    pub total: i64,
    /// Number of user messages.
    pub user_count: i64,
    /// Number of assistant messages.
    pub assistant_count: i64,
    /// Number of compaction messages.
    pub compaction_count: i64,
    /// `true` if the session has been compacted at least once.
    pub has_compaction: bool,
}

/// Sanitise a user-supplied query string for safe use as an FTS5 MATCH expression.
///
/// Splits on whitespace, wraps each term in double quotes (removing any
/// embedded double quotes), and joins with spaces so FTS5 treats each term
/// as a phrase query connected by implicit AND.
fn sanitise_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|term| format!("\"{}\"", term.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
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
    /// Storage format version for backward compatibility.
    pub format_version: i64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
    /// ISO-8601 archive timestamp, if archived.
    pub archived_at: Option<String>,
    /// JSON-encoded session summary, if available.
    pub summary: Option<String>,
}

// ── Initiatives (JCODEPLAN M8) ──────────────────────────────────────

/// A serialisable milestone within a durable initiative.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct InitiativeMilestone {
    /// Stable milestone identifier (unique within the initiative).
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// `true` when the milestone is complete.
    pub done: bool,
    /// ISO-8601 completion timestamp (set when `done` flips to true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// Row representation of a durable initiative.
#[derive(Debug, Clone)]
pub struct InitiativeRow {
    /// Unique initiative identifier.
    pub id: String,
    /// Human-readable goal title.
    pub title: String,
    /// Longer description / success criteria.
    pub description: String,
    /// Lifecycle status: active, paused, completed, abandoned.
    pub status: String,
    /// JSON-encoded `Vec<InitiativeMilestone>`.
    pub milestones_json: String,
    /// Overall progress 0–100.
    pub progress: u32,
    /// Project the initiative belongs to (typically working-dir string).
    pub project: String,
    /// Session that created the initiative (informational).
    pub session_id: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
    /// ISO-8601 close timestamp (when status became completed/abandoned).
    pub closed_at: Option<String>,
}

impl InitiativeRow {
    /// Decode `milestones_json` into a structured milestone list.
    ///
    /// Falls back to an empty vector on malformed JSON (should never happen
    /// for rows written through [`Storage::create_initiative`]).
    #[must_use]
    pub fn milestones(&self) -> Vec<InitiativeMilestone> {
        serde_json::from_str(&self.milestones_json).unwrap_or_default()
    }
}

/// Row-mapping helper for `initiatives` queries.
fn initiative_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<InitiativeRow> {
    let progress_i: i64 = row.get(5)?;
    Ok(InitiativeRow {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: row.get(3)?,
        milestones_json: row.get(4)?,
        progress: u32::try_from(progress_i).unwrap_or(0),
        project: row.get(6)?,
        session_id: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        closed_at: row.get(10)?,
    })
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

/// Row representation of a persisted run-cost summary (FR-018).
///
/// Mirrors the `run_cost_summaries` table. Run-cost summaries are stored
/// separately from the session transcript so the default session export
/// never exposes per-run dollar costs; they are only attached to an export
/// when the caller explicitly opts in via the `include_cost` flag.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunCostSummaryRow {
    /// Unique identifier for this summary record (UUID v4).
    pub id: String,
    /// Session this summary belongs to.
    pub session_id: String,
    /// Model identifier that produced the usage.
    pub model_id: String,
    /// Total input (prompt) tokens across all LLM requests in the run.
    pub input_tokens: u64,
    /// Total output (completion) tokens across all LLM requests in the run.
    pub output_tokens: u64,
    /// Estimated total cost in USD.
    pub total_cost_usd: f64,
    /// Wall-clock duration of the run in milliseconds.
    pub duration_ms: u64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// Maps a SQL row to a [`MemoryRow`].
fn memory_row_from_sql(row: &rusqlite::Row) -> rusqlite::Result<MemoryRow> {
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

/// Row representation of a knowledge-graph entity.
///
/// Mirrors the `kg_entities` table.  The agent crate maps this into its
/// `ragent_agent::memory::knowledge_graph::Entity` type (which carries the
/// same fields) so that `ragent-storage` does not need to depend on the
/// agent's memory module.
#[derive(Debug, Clone)]
pub struct KgEntityRow {
    /// Unique row ID.
    pub id: i64,
    /// Entity name (e.g. `"Rust"`, `"Docker"`).
    pub name: String,
    /// Entity type (`project`/`tool`/`language`/`pattern`/`person`/`concept`).
    pub entity_type: String,
    /// Number of memories mentioning this entity.
    pub mention_count: i64,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
}

/// Row representation of a knowledge-graph relationship.
///
/// Mirrors the `kg_relationships` table.  The agent crate maps this into its
/// `ragent_agent::memory::knowledge_graph::Relationship` type.
#[derive(Debug, Clone)]
pub struct KgRelationshipRow {
    /// Unique row ID.
    pub id: i64,
    /// Source entity ID.
    pub source_id: i64,
    /// Target entity ID.
    pub target_id: i64,
    /// Relationship type (`uses`/`prefers`/`depends_on`/`avoids`/`related_to`).
    pub relation_type: String,
    /// Confidence in this relationship (0.0–1.0).
    pub confidence: f64,
    /// The memory ID that established this relationship, if any.
    pub source_memory_id: Option<i64>,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// A scored search result from embedding-based memory search.
///
/// Pairs a memory row ID with its cosine-similarity score against the query
/// embedding.  The agent and tools-extended crates map this into their own
/// `SimilarityResult` / `EmbeddingMatch` types respectively, so
/// `ragent-storage` does not need to depend on either.
#[derive(Debug, Clone)]
pub struct EmbeddingMatch {
    /// Row ID of the matching memory.
    pub row_id: i64,
    /// Cosine similarity score in `[-1.0, 1.0]`.  Higher = more similar.
    pub score: f32,
}

/// Pairs a session-message identifier with its cosine-similarity score.
#[derive(Debug, Clone)]
pub struct MessageEmbeddingMatch {
    /// Message identifier (UUID v4).
    pub message_id: String,
    /// Cosine similarity score in `[-1.0, 1.0]`.  Higher = more similar.
    pub score: f32,
}

/// Row representation of a background shell task.
///
/// Mirrors the `background_tasks` table used by the M3 background task manager.
#[derive(Debug, Clone)]
pub struct BackgroundTaskRow {
    /// Unique task identifier (UUID v4).
    pub id: String,
    /// Session that owns this task.
    pub session_id: String,
    /// Shell command being executed.
    pub command: String,
    /// Current status: `running`, `completed`, `failed`, or `cancelled`.
    pub status: String,
    /// Exit code when the process finished.
    pub exit_code: Option<i64>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Parsed `JCODE_PROGRESS` payload as JSON text.
    pub progress_json: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
    /// ISO-8601 completion timestamp, if finished.
    pub completed_at: Option<String>,
}

// ── M5: Session message search types ───────────────────────────────────────

/// A single message search result from the `messages_fts` full-text index.
///
/// Returned by [`Storage::search_conversation`] and
/// [`Storage::search_session_messages`].  The `content` field is the
/// extracted text from the message parts (text blocks, tool names,
/// reasoning) — not the raw JSON parts blob.
#[derive(Debug, Clone)]
pub struct MessageSearchResult {
    /// Unique message identifier (UUID v4).
    pub message_id: String,
    /// Session this message belongs to.
    pub session_id: String,
    /// Message role: `user`, `assistant`, or `compaction`.
    pub role: String,
    /// Extracted text content from the message parts.
    pub content: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// Session title, if available (joined from `sessions` table).
    pub session_title: Option<String>,
    /// Session working directory, if available.
    pub session_directory: Option<String>,
    /// FTS5 rank score (lower = more relevant).
    pub rank: f64,
}

/// Parameters for cross-session message search ([`Storage::search_session_messages`]).
///
/// All filter fields are optional; when `None`, no filter is applied for that
/// dimension.
#[derive(Debug, Clone, Default)]
pub struct SessionSearchParams {
    /// Full-text search query (FTS5 syntax).
    pub query: String,
    /// Maximum total results to return.
    pub limit: usize,
    /// Maximum results per session (applied after ranking).
    pub max_per_session: Option<usize>,
    /// Only include messages created on or after this ISO-8601 timestamp.
    pub since: Option<String>,
    /// Only include messages created on or before this ISO-8601 timestamp.
    pub until: Option<String>,
    /// Filter to sessions whose working directory matches this path.
    pub working_dir: Option<String>,
    /// Filter to specific roles (e.g. `["user", "assistant"]`).
    pub roles: Option<Vec<String>>,
    /// When `true`, include tool-call content in the extracted text.
    pub include_tools: bool,
    /// When `true`, include reasoning/system content in the extracted text.
    pub include_system: bool,
    /// Restrict search to a specific session id (optional).
    pub session_id: Option<String>,
}

// ── Cron Events (spec agentchron T-006) ──────────────────────────────

/// Row representation of a cron event as stored in `SQLite`.
///
/// Returned by [`Storage::get_cron_event`], [`Storage::list_cron_events`],
/// and [`Storage::list_due_cron_events`]. All timestamp fields are ISO-8601
/// strings as stored in the database.
#[derive(Debug, Clone)]
pub struct CronEventRow {
    /// Unique event identifier.
    pub id: String,
    /// Built-in or custom agent name to run.
    pub agent_type: String,
    /// Initial prompt passed to the agent.
    pub prompt: String,
    /// Schedule form: `one_shot`, `repeat_from`, or `repeat_now`.
    pub schedule_form: String,
    /// Explicit start timestamp (ISO-8601), or `None` for `repeat_now`.
    pub start_at: Option<String>,
    /// Repeat interval in seconds, or `None` for one-shot events.
    pub duration_secs: Option<i64>,
    /// Raw schedule expression string (e.g. `every 30m`).
    pub schedule_raw: String,
    /// Whether the event is enabled for scheduling.
    pub enabled: bool,
    /// Next-due timestamp (ISO-8601).
    pub next_due: String,
    /// Creation timestamp (ISO-8601).
    pub created_at: String,
    /// Last-fired timestamp (ISO-8601), or `None` if never fired.
    pub last_fired: Option<String>,
    /// Whether this event runs in stateful loop mode (FR-004).
    pub stateful: bool,
}

/// Row-mapping helper for `cron_events` queries.
fn cron_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CronEventRow> {
    let enabled_i: i64 = row.get(7)?;
    // The `stateful` column was added via migration; handle the case where
    // it doesn't exist yet by defaulting to `false`.
    let stateful_i: i64 = row.get(11).unwrap_or(0);
    Ok(CronEventRow {
        id: row.get("id")?,
        agent_type: row.get("agent_type")?,
        prompt: row.get("prompt")?,
        schedule_form: row.get("schedule_form")?,
        start_at: row.get("start_at")?,
        duration_secs: row.get("duration_secs")?,
        schedule_raw: row.get("schedule_raw")?,
        enabled: enabled_i != 0,
        next_due: row.get("next_due")?,
        created_at: row.get("created_at")?,
        last_fired: row.get("last_fired")?,
        stateful: stateful_i != 0,
    })
}
