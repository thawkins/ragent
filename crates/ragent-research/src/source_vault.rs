//! Persistent source vault for Hyperresearch-style deep research (FR-002, FR-003).
//!
//! A [`SourceVault`] is a per-run store at `.ragent/research_vault/<run_tag>/`.
//! It keeps an embedded SQLite index plus raw content files so that future
//! research runs can reuse gathered sources before issuing new web searches
//! (FR-009) and so that every citation can be traced back to the captured
//! source text (FR-014).
//!
//! The SQLite schema records provenance metadata required by FR-003:
//! URL, title, fetch timestamp, search engine, media type, and the on-disk
//! path of the raw content. Searches run case-insensitively across URL,
//! title, and body text.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

/// Errors emitted by the vault layer.
#[derive(Debug, Error)]
pub enum SourceVaultError {
    /// A filesystem operation failed.
    #[error("vault I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The SQLite index returned an error.
    #[error("vault database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The supplied run tag is not safe to use as a directory name.
    #[error("invalid run tag: {0}")]
    InvalidRunTag(String),
    /// A requested source id was not found in the index.
    #[error("vault source '{0}' not found")]
    SourceNotFound(String),
}

/// Result alias for vault operations.
pub type Result<T> = std::result::Result<T, SourceVaultError>;

/// Metadata stored in the vault for one captured web source (FR-003).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultSource {
    /// SQLite row id.
    pub id: i64,
    /// Stable uuid used as the raw-content filename.
    pub source_id: String,
    /// Run tag that owns this source.
    pub run_tag: String,
    /// Full source URL.
    pub url: String,
    /// Page or article title (may be empty).
    pub title: String,
    /// UTC timestamp at which the source was captured.
    pub fetch_timestamp: DateTime<Utc>,
    /// Name of the search tool that discovered the source (e.g. `mf_search`).
    pub search_tool: String,
    /// Comma-separated list of search engines that returned the URL.
    pub search_engine: String,
    /// Media classifier (`page`, `pdf`, `youtube`, etc.).
    pub media_type: String,
    /// Absolute path to the raw content file.
    pub content_path: PathBuf,
    /// Hex blake3 hash of the stored body text.
    pub content_hash: String,
    /// Full text body stored for FTS search.
    pub body_text: String,
}

/// Input used to store a new source in the vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewVaultSource {
    /// Full source URL.
    pub url: String,
    /// Page or article title (may be empty).
    pub title: String,
    /// Optional explicit fetch timestamp; defaults to now.
    pub fetch_timestamp: Option<DateTime<Utc>>,
    /// Name of the search tool that discovered the source.
    pub search_tool: String,
    /// Comma-separated list of search engines that returned the URL.
    pub search_engine: String,
    /// Media classifier.
    pub media_type: String,
    /// Optional HTTP `Content-Type` reported by the fetcher.
    pub content_type: Option<String>,
    /// Full text body to store and index.
    pub body_text: String,
}

impl NewVaultSource {
    /// Resolve the effective fetch timestamp.
    #[must_use]
    pub fn fetch_timestamp(&self) -> DateTime<Utc> {
        self.fetch_timestamp.unwrap_or_else(Utc::now)
    }
}

/// Persistent, searchable source vault for a single research run.
pub struct SourceVault {
    vault_root: PathBuf,
    run_tag: String,
    conn: Mutex<Connection>,
}

impl fmt::Debug for SourceVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceVault")
            .field("vault_root", &self.vault_root)
            .field("run_tag", &self.run_tag)
            .field("db_path", &self.db_path())
            .finish_non_exhaustive()
    }
}

impl SourceVault {
    /// Open (or create) a vault rooted at `project_root/.ragent/research_vault/<run_tag>/`.
    ///
    /// `run_tag` must be a single path component with no traversal sequences.
    pub fn open(project_root: &Path, run_tag: &str) -> Result<Self> {
        let vault_root = project_root.join(".ragent").join("research_vault");
        Self::open_with_root(&vault_root, run_tag)
    }

    /// Open (or create) a vault with an explicit root directory.
    ///
    /// Mostly useful for tests that want to place the vault inside a temp dir.
    pub fn open_with_root(vault_root: &Path, run_tag: &str) -> Result<Self> {
        validate_run_tag(run_tag)?;
        fs::create_dir_all(vault_root.join(run_tag).join("raw"))?;

        let db_path = vault_root.join(run_tag).join("vault.db");
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;

        let vault = Self {
            vault_root: vault_root.to_path_buf(),
            run_tag: run_tag.to_string(),
            conn: Mutex::new(conn),
        };
        vault.migrate()?;
        Ok(vault)
    }

    /// Path to the vault database file.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.run_dir().join("vault.db")
    }

    /// Path to the run directory (`<vault_root>/<run_tag>/`).
    #[must_use]
    pub fn run_dir(&self) -> PathBuf {
        self.vault_root.join(&self.run_tag)
    }

    /// Path to the raw-content subdirectory.
    #[must_use]
    pub fn raw_dir(&self) -> PathBuf {
        self.run_dir().join("raw")
    }

    /// Absolute path of the raw content file for a stored source id.
    #[must_use]
    pub fn content_path(&self, source_id: &str, media_type: &str) -> PathBuf {
        let ext = media_extension(media_type);
        self.raw_dir().join(format!("{source_id}.{ext}"))
    }

    /// Read the raw content file for a stored source id back into a string.
    ///
    /// # Errors
    ///
    /// Returns [`SourceVaultError::SourceNotFound`] when the source id does not
    /// exist in the index, or [`SourceVaultError::Io`] when the file is missing.
    pub fn read_content(&self, source_id: &str) -> Result<String> {
        let content_path: Option<String> = self
            .conn
            .lock()
            .map_err(|e| SourceVaultError::InvalidRunTag(format!("lock poisoned: {e}")))?
            .query_row(
                "SELECT content_path FROM vault_sources
                 WHERE run_tag = ?1 AND source_id = ?2",
                params![self.run_tag, source_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(path) = content_path else {
            return Err(SourceVaultError::SourceNotFound(source_id.to_string()));
        };
        Ok(fs::read_to_string(PathBuf::from(path))?)
    }

    /// Store a new source in the vault, or return the existing record if the
    /// same URL has already been captured for this run.
    ///
    /// The body text is written to a raw-content file and also indexed for
    /// full-text search.
    pub fn store(&self, source: &NewVaultSource) -> Result<VaultSource> {
        // Deduplicate within the run by URL.
        if let Some(existing) = self.find_by_url(&source.url)? {
            return Ok(existing);
        }

        let source_id = Uuid::new_v4().to_string();
        let fetch_timestamp = source.fetch_timestamp();
        let content_hash = blake3::hash(source.body_text.as_bytes())
            .to_hex()
            .to_string();
        let content_path = self.content_path(&source_id, &source.media_type);

        atomic_write_file(&content_path, source.body_text.as_bytes())?;

        let conn = self
            .conn
            .lock()
            .map_err(|e| SourceVaultError::InvalidRunTag(format!("lock poisoned: {e}")))?;

        conn.execute(
            "INSERT INTO vault_sources
             (source_id, run_tag, url, title, fetch_timestamp, search_tool, search_engine,
              media_type, content_path, content_hash, body_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                source_id,
                self.run_tag,
                source.url,
                source.title,
                fetch_timestamp.to_rfc3339(),
                source.search_tool,
                source.search_engine,
                source.media_type,
                content_path.to_string_lossy().as_ref(),
                content_hash,
                source.body_text,
            ],
        )?;

        let id = conn.last_insert_rowid();
        drop(conn);

        Ok(VaultSource {
            id,
            source_id,
            run_tag: self.run_tag.clone(),
            url: source.url.clone(),
            title: source.title.clone(),
            fetch_timestamp,
            search_tool: source.search_tool.clone(),
            search_engine: source.search_engine.clone(),
            media_type: source.media_type.clone(),
            content_path,
            content_hash,
            body_text: source.body_text.clone(),
        })
    }

    /// Look up a source by exact URL within this run.
    pub fn find_by_url(&self, url: &str) -> Result<Option<VaultSource>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| SourceVaultError::InvalidRunTag(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT id, source_id, run_tag, url, title, fetch_timestamp, search_tool,
                    search_engine, media_type, content_path, content_hash, body_text
             FROM vault_sources
             WHERE run_tag = ?1 AND url = ?2",
        )?;
        let mut rows = stmt.query(params![self.run_tag, url])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_source(row)?));
        }
        Ok(None)
    }

    /// Search the vault by URL, title, or body text.
    ///
    /// The query is tokenised into plain alphanumeric words; each token must
    /// appear somewhere in the source (case-insensitive) for the source to
    /// match. An empty query lists every source.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<VaultSource>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.list(limit);
        }

        let tokens = sanitize_search_tokens(trimmed);
        if tokens.is_empty() {
            return self.list(limit);
        }

        let mut sql = String::from(
            "SELECT id, source_id, run_tag, url, title, fetch_timestamp, search_tool,
                    search_engine, media_type, content_path, content_hash, body_text
             FROM vault_sources
             WHERE run_tag = ?1",
        );
        for (i, _) in tokens.iter().enumerate() {
            sql.push_str(&format!(
                " AND (lower(url) LIKE ?{i} OR lower(title) LIKE ?{i} OR lower(body_text) LIKE ?{i})",
                i = i + 2
            ));
        }
        sql.push_str(" ORDER BY fetch_timestamp DESC LIMIT ?");
        sql.push_str(&format!("{}", tokens.len() + 2));

        let conn = self
            .conn
            .lock()
            .map_err(|e| SourceVaultError::InvalidRunTag(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(&sql)?;
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(tokens.len() + 2);
        params.push(&self.run_tag);
        let patterns: Vec<String> = tokens.iter().map(|t| format!("%{t}%")).collect();
        for p in &patterns {
            params.push(p);
        }
        let limit_i64 = limit as i64;
        params.push(&limit_i64);

        let mut rows = stmt.query(params.as_slice())?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_source(row)?);
        }
        Ok(out)
    }

    /// List every source in this run, newest first.
    pub fn list(&self, limit: usize) -> Result<Vec<VaultSource>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| SourceVaultError::InvalidRunTag(format!("lock poisoned: {e}")))?;
        let mut stmt = conn.prepare(
            "SELECT id, source_id, run_tag, url, title, fetch_timestamp, search_tool,
                    search_engine, media_type, content_path, content_hash, body_text
             FROM vault_sources
             WHERE run_tag = ?1
             ORDER BY fetch_timestamp DESC
             LIMIT ?2",
        )?;
        let mut rows = stmt.query(params![self.run_tag, limit as i64])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(row_to_source(row)?);
        }
        Ok(out)
    }

    /// Total number of sources stored for this run.
    pub fn count(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| SourceVaultError::InvalidRunTag(format!("lock poisoned: {e}")))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vault_sources WHERE run_tag = ?1",
            params![self.run_tag],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| SourceVaultError::InvalidRunTag(format!("lock poisoned: {e}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS vault_sources (
                id INTEGER PRIMARY KEY,
                source_id TEXT NOT NULL UNIQUE,
                run_tag TEXT NOT NULL,
                url TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                fetch_timestamp TEXT NOT NULL,
                search_tool TEXT NOT NULL DEFAULT '',
                search_engine TEXT NOT NULL DEFAULT '',
                media_type TEXT NOT NULL DEFAULT 'page',
                content_path TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                body_text TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_vault_sources_run_tag_url
                ON vault_sources(run_tag, url);
            CREATE INDEX IF NOT EXISTS idx_vault_sources_run_tag_fetch_ts
                ON vault_sources(run_tag, fetch_timestamp DESC);
            ",
        )?;
        Ok(())
    }
}

fn row_to_source(row: &rusqlite::Row<'_>) -> Result<VaultSource> {
    let fetch_timestamp: String = row.get(5)?;
    let content_path: String = row.get(9)?;
    Ok(VaultSource {
        id: row.get(0)?,
        source_id: row.get(1)?,
        run_tag: row.get(2)?,
        url: row.get(3)?,
        title: row.get(4)?,
        fetch_timestamp: DateTime::parse_from_rfc3339(&fetch_timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        search_tool: row.get(6)?,
        search_engine: row.get(7)?,
        media_type: row.get(8)?,
        content_path: PathBuf::from(content_path),
        content_hash: row.get(10)?,
        body_text: row.get(11)?,
    })
}

/// Reject run tags that are empty or would escape the intended directory.
fn validate_run_tag(run_tag: &str) -> Result<()> {
    if run_tag.is_empty() {
        return Err(SourceVaultError::InvalidRunTag("empty".into()));
    }
    for component in Path::new(run_tag).components() {
        match component {
            std::path::Component::Normal(_) => {}
            other => {
                return Err(SourceVaultError::InvalidRunTag(format!(
                    "traversal or separator component: {}",
                    other.as_os_str().display()
                )));
            }
        }
    }
    Ok(())
}

/// Map a media-type classifier to a file extension for raw-content files.
fn media_extension(media_type: &str) -> String {
    match media_type.to_lowercase().as_str() {
        "pdf" => "pdf",
        _ => "md",
    }
    .to_string()
}

/// Write `content` to `path` atomically (temp file + rename).
fn atomic_write_file(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}

/// Sanitise a raw user query into lowercase alphanumeric tokens suitable for
/// SQL `LIKE` clauses. Punctuation, quotes, and FTS5-special characters are
/// dropped, so arbitrary input cannot break the query.
pub(crate) fn sanitize_search_tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}
