//! `SQLite` WAL-mode content cache for the masterfetch toolset.
//!
//! Implements FR-018 and NFR-002.
//!
//! The cache is keyed by the tuple `(url, extraction_type, css_selector,
//! pages)` so that two calls with different `format`/`css_selector`/`pages`
//! parameters do not collide. Entries carry a per-insert TTL (default 3600 s);
//! `cache_ttl = 0` bypasses the cache entirely (the caller skips the
//! `get_cached` call). Entries inserted with `content_ok = false` are never
//! stored — bad content is never cached (FR-018).
//!
//! A configurable size cap (`max_bytes`, default 100 MiB) evicts the oldest
//! entries after every insert so a long-lived agent's cache cannot grow
//! unbounded.
//!
//! The cache is backed by a single `rusqlite::Connection` (reusing the
//! workspace `rusqlite` dependency per NFR-002) opened in WAL mode. All methods
//! are synchronous and acquire an internal `std::sync::Mutex`; async callers
//! should wrap calls in `tokio::task::spawn_blocking`.
//!
//! # Example
//!
//! ```no_run
//! use ragent_tools_extended::masterfetch::cache::{CacheKey, ContentCache};
//!
//! # fn main() -> anyhow::Result<()> {
//! let cache = ContentCache::open_in_memory()?;
//!
//! let key = CacheKey::new("https://example.com/page")
//!     .with_extraction_type("markdown");
//! let entry = cache.get_cached(&key)?;
//! assert!(entry.is_none()); // empty cache
//!
//! cache.set_cached(
//!     &key,
//!     "# Hello",          // content
//!     true,               // content_ok — bad content is never cached
//!     200,                // status_code
//!     "text/markdown",    // content_type
//!     3600,               // ttl_seconds
//! )?;
//!
//! let entry = cache.get_cached(&key)?.unwrap();
//! assert_eq!(entry.content, "# Hello");
//! # Ok(()) }
//! ```
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE fetch_cache (
//!     url             TEXT    NOT NULL,
//!     extraction_type TEXT    NOT NULL,
//!     css_selector    TEXT    NOT NULL,
//!     pages           TEXT    NOT NULL,
//!     content         TEXT    NOT NULL,
//!     content_ok      INTEGER NOT NULL,
//!     status_code     INTEGER NOT NULL,
//!     content_type    TEXT    NOT NULL,
//!     created_at      INTEGER NOT NULL,  -- unix seconds
//!     expires_at      INTEGER NOT NULL,  -- created_at + ttl
//!     size_bytes      INTEGER NOT NULL,
//!     extraction_method TEXT,             -- extraction chain stage (added later; NULL for legacy entries)
//!     metadata_json   TEXT,               -- serialized PageMetadata (added later; NULL for legacy entries)
//!     PRIMARY KEY (url, extraction_type, css_selector, pages)
//! );
//! ```

use std::sync::{Arc, Mutex};

use super::PageMetadata;
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json;

/// Default cache TTL: 1 hour (3600 seconds).
pub const DEFAULT_CACHE_TTL: u64 = 3600;

/// Default size cap: 100 MiB. When the total stored content exceeds this, the
/// oldest entries are evicted until the total is at or below the cap.
pub const DEFAULT_MAX_BYTES: usize = 100 * 1024 * 1024;

/// Sentinel value used for the `css_selector` and `pages` key components when
/// they are absent. Stored in the database so the composite primary key is
/// always well-formed.
const EMPTY_COMPONENT: &str = "";

/// A cache key — the composite of URL + extraction type + CSS selector + pages.
///
/// Construct with [`CacheKey::new`] and the `.with_*` builders. The components
/// are normalised into owned strings so the key is self-contained and can be
/// hashed/compared deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Normalised page URL.
    pub url: String,
    /// Extraction format / type (e.g. `"markdown"`, `"html"`, `"text"`,
    /// `"raw"`). Defaults to `""` which is treated as the default extraction.
    pub extraction_type: String,
    /// CSS selector used to narrow extraction, if any. `None` means no
    /// selector was applied.
    pub css_selector: Option<String>,
    /// Pages parameter (e.g. `"1"`, `"1-3"`), if any. `None` means a single
    /// page / no pagination.
    pub pages: Option<String>,
}

impl CacheKey {
    /// Create a new cache key for the given URL with default components.
    ///
    /// `extraction_type` defaults to an empty string (the default extraction),
    /// and `css_selector` / `pages` default to `None`.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            extraction_type: String::new(),
            css_selector: None,
            pages: None,
        }
    }

    /// Set the extraction type component (e.g. `"markdown"`, `"html"`).
    #[must_use]
    pub fn with_extraction_type(mut self, extraction_type: impl Into<String>) -> Self {
        self.extraction_type = extraction_type.into();
        self
    }

    /// Set the CSS selector component. `None` clears it.
    #[must_use]
    pub fn with_css_selector(mut self, css_selector: Option<impl Into<String>>) -> Self {
        self.css_selector = css_selector.map(Into::into);
        self
    }

    /// Set the pages component. `None` clears it.
    #[must_use]
    pub fn with_pages(mut self, pages: Option<impl Into<String>>) -> Self {
        self.pages = pages.map(Into::into);
        self
    }

    /// Return the `css_selector` component, or the empty sentinel if absent.
    fn css_selector_component(&self) -> &str {
        self.css_selector.as_deref().unwrap_or(EMPTY_COMPONENT)
    }

    /// Return the `pages` component, or the empty sentinel if absent.
    fn pages_component(&self) -> &str {
        self.pages.as_deref().unwrap_or(EMPTY_COMPONENT)
    }
}

/// A cached content entry returned by [`ContentCache::get_cached`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedEntry {
    /// The extracted content stored at insert time.
    pub content: String,
    /// Whether the content was marked usable (`content_ok`) at insert time.
    /// Always `true` for stored entries — bad content is never cached.
    pub content_ok: bool,
    /// HTTP status code recorded at insert time.
    pub status_code: u16,
    /// HTTP `Content-Type` recorded at insert time.
    pub content_type: String,
    /// Unix-second timestamp at which the entry was inserted.
    pub created_at: u64,
    /// Unix-second timestamp at which the entry expires.
    pub expires_at: u64,
    /// Stored content size in bytes.
    pub size_bytes: usize,
    /// Extraction-chain stage that produced the content (e.g. `"readability"`,
    /// `"html2text"`, `"raw_text"`). `None` for entries stored before this
    /// signal was recorded.
    pub extraction_method: Option<String>,
    /// Structured page metadata serialized at insert time (e.g. author, title,
    /// publication date). `None` for entries stored before this field existed.
    pub metadata: Option<PageMetadata>,
}

/// Configuration for a [`ContentCache`].
#[derive(Debug, Clone)]
pub struct ContentCacheConfig {
    /// Maximum total stored content in bytes before oldest entries are
    /// evicted. Default: 100 MiB ([`DEFAULT_MAX_BYTES`]).
    pub max_bytes: usize,
}

impl Default for ContentCacheConfig {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// SQLite-backed content cache (WAL mode).
///
/// See the [module docs](self) for the schema, key semantics, and usage.
#[derive(Clone)]
pub struct ContentCache {
    conn: Arc<Mutex<Connection>>,
    config: ContentCacheConfig,
}

impl ContentCache {
    /// Open or create a cache at the given filesystem path.
    ///
    /// Enables WAL journal mode and `synchronous = NORMAL` for throughput.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` database cannot be opened or the schema
    /// cannot be initialised.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("opening cache at {:?}", path.as_ref()))?;
        Self::init(conn, ContentCacheConfig::default())
    }

    /// Open or create a cache at the given path with a custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` database cannot be opened or the schema
    /// cannot be initialised.
    pub fn open_with_config(
        path: impl AsRef<std::path::Path>,
        config: ContentCacheConfig,
    ) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("opening cache at {:?}", path.as_ref()))?;
        Self::init(conn, config)
    }

    /// Create an in-memory cache (for tests and ephemeral use).
    ///
    /// # Errors
    ///
    /// Returns an error if the in-memory database cannot be created or the
    /// schema cannot be initialised.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory cache")?;
        Self::init(conn, ContentCacheConfig::default())
    }

    /// Create an in-memory cache with a custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the in-memory database cannot be created or the
    /// schema cannot be initialised.
    pub fn open_in_memory_with_config(config: ContentCacheConfig) -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory cache")?;
        Self::init(conn, config)
    }

    /// Initialise the connection: pragmas + schema, then wrap in the cache.
    fn init(conn: Connection, config: ContentCacheConfig) -> Result<Self> {
        // WAL mode for concurrent-read / single-write throughput. In-memory
        // databases report "memory" for journal_mode; ignore that.
        conn.pragma_update(None, "journal_mode", "WAL")
            .or_else(|_| conn.pragma_update(None, "journal_mode", "memory"))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS fetch_cache (
                url             TEXT    NOT NULL,
                extraction_type TEXT    NOT NULL,
                css_selector    TEXT    NOT NULL,
                pages           TEXT    NOT NULL,
                content         TEXT    NOT NULL,
                content_ok      INTEGER NOT NULL,
                status_code     INTEGER NOT NULL,
                content_type    TEXT    NOT NULL,
                created_at      INTEGER NOT NULL,
                expires_at      INTEGER NOT NULL,
                size_bytes      INTEGER NOT NULL,
                extraction_method TEXT,
                metadata_json   TEXT,
                PRIMARY KEY (url, extraction_type, css_selector, pages)
             );
             CREATE INDEX IF NOT EXISTS idx_fetch_cache_expires
                 ON fetch_cache(expires_at);
             CREATE INDEX IF NOT EXISTS idx_fetch_cache_created
                 ON fetch_cache(created_at);",
        )
        .context("initialising cache schema")?;

        // Older databases lack the extraction_method column; add it when
        // opening an existing cache. New databases created above already
        // have it, so probe first and only ALTER when missing.
        let has_method_column = conn
            .prepare("SELECT extraction_method FROM fetch_cache LIMIT 0")
            .is_ok();
        if !has_method_column {
            conn.execute_batch("ALTER TABLE fetch_cache ADD COLUMN extraction_method TEXT;")
                .context("migrating cache schema: adding extraction_method")?;
        }

        // Older databases also lack the metadata_json column.
        let has_metadata_column = conn
            .prepare("SELECT metadata_json FROM fetch_cache LIMIT 0")
            .is_ok();
        if !has_metadata_column {
            conn.execute_batch("ALTER TABLE fetch_cache ADD COLUMN metadata_json TEXT;")
                .context("migrating cache schema: adding metadata_json")?;
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            config,
        })
    }

    /// Look up a cached entry by key.
    ///
    /// Returns `Ok(None)` if the key is not present or the stored entry has
    /// expired (expired entries are lazily deleted on read).
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` query fails.
    pub fn get_cached(&self, key: &CacheKey) -> Result<Option<CachedEntry>> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let now = unix_now();

        // Lazily delete the entry if it exists but has expired.
        let expired_delete = conn.execute(
            "DELETE FROM fetch_cache
             WHERE url = ?1
               AND extraction_type = ?2
               AND css_selector = ?3
               AND pages = ?4
               AND expires_at <= ?5",
            params![
                key.url,
                key.extraction_type,
                key.css_selector_component(),
                key.pages_component(),
                now as i64,
            ],
        )?;

        if expired_delete > 0 {
            return Ok(None);
        }

        let row = conn
            .query_row(
                "SELECT content, content_ok, status_code, content_type, metadata_json,
                        created_at, expires_at, size_bytes, extraction_method
                 FROM fetch_cache
                 WHERE url = ?1
                   AND extraction_type = ?2
                   AND css_selector = ?3
                   AND pages = ?4",
                params![
                    key.url,
                    key.extraction_type,
                    key.css_selector_component(),
                    key.pages_component(),
                ],
                |row| {
                    let metadata_json: Option<String> = row.get(4)?;
                    let metadata = metadata_json
                        .and_then(|json| serde_json::from_str::<PageMetadata>(&json).ok());
                    Ok(CachedEntry {
                        content: row.get(0)?,
                        content_ok: row.get::<_, i64>(1)? != 0,
                        status_code: row.get::<_, i64>(2)? as u16,
                        content_type: row.get(3)?,
                        metadata,
                        created_at: row.get::<_, i64>(5)? as u64,
                        expires_at: row.get::<_, i64>(6)? as u64,
                        size_bytes: row.get::<_, i64>(7)? as usize,
                        extraction_method: row.get::<_, Option<String>>(8)?,
                    })
                },
            )
            .optional()?;

        Ok(row)
    }

    /// Store a content entry in the cache.
    ///
    /// If `content_ok` is `false`, the entry is **not** stored (bad content is
    /// never cached per FR-018) and `Ok(())` is returned immediately.
    ///
    /// After inserting, the size cap is enforced: if the total stored bytes
    /// exceed `max_bytes`, the oldest entries (by `created_at`) are evicted
    /// until the total is at or below the cap.
    ///
    /// `ttl_seconds` is the per-entry time-to-live. A `ttl_seconds` of `0`
    /// should bypass the cache entirely — the caller is expected to skip this
    /// call when `ttl == 0` (FR-018). If called with `ttl_seconds == 0` the
    /// entry is still stored with an expiry equal to `created_at` (i.e.
    /// immediately expired) so a subsequent `get_cached` will not return it.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` write fails.
    pub fn set_cached(
        &self,
        key: &CacheKey,
        content: &str,
        content_ok: bool,
        status_code: u16,
        content_type: &str,
        ttl_seconds: u64,
    ) -> Result<()> {
        self.set_cached_with_method(
            key,
            content,
            content_ok,
            status_code,
            content_type,
            ttl_seconds,
            None,
        )
    }

    /// Store a content entry in the cache, recording the extraction-chain
    /// stage (`extraction_method`) that produced the content.
    ///
    /// Behaves exactly like [`ContentCache::set_cached`]; the optional
    /// `extraction_method` is recorded so cache hits can report which stage
    /// of the chain (readability, html2text, raw text) produced the content.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` write fails.
    #[allow(clippy::too_many_arguments)]
    pub fn set_cached_with_method(
        &self,
        key: &CacheKey,
        content: &str,
        content_ok: bool,
        status_code: u16,
        content_type: &str,
        ttl_seconds: u64,
        extraction_method: Option<&str>,
    ) -> Result<()> {
        self.set_cached_with_metadata(
            key,
            content,
            content_ok,
            status_code,
            content_type,
            ttl_seconds,
            extraction_method,
            None,
        )
    }

    /// Store a content entry in the cache, recording both the extraction-chain
    /// stage and the structured page metadata.
    ///
    /// Behaves exactly like [`ContentCache::set_cached`]; the optional
    /// `metadata` is serialized and stored so cache hits can restore author,
    /// title, publication date, and other page metadata for downstream consumers
    /// such as the research References Index.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` write fails.
    #[allow(clippy::too_many_arguments)]
    pub fn set_cached_with_metadata(
        &self,
        key: &CacheKey,
        content: &str,
        content_ok: bool,
        status_code: u16,
        content_type: &str,
        ttl_seconds: u64,
        extraction_method: Option<&str>,
        metadata: Option<&PageMetadata>,
    ) -> Result<()> {
        // FR-018: bad content is never cached.
        if !content_ok {
            return Ok(());
        }

        let conn = self.conn.lock().expect("cache mutex poisoned");
        let now = unix_now();
        let expires_at = now.saturating_add(ttl_seconds);
        let size_bytes = content.len() as i64;
        let metadata_json = metadata.and_then(|m| serde_json::to_string(m).ok());

        conn.execute(
            "INSERT OR REPLACE INTO fetch_cache
                 (url, extraction_type, css_selector, pages,
                  content, content_ok, status_code, content_type,
                  created_at, expires_at, size_bytes, extraction_method, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                key.url,
                key.extraction_type,
                key.css_selector_component(),
                key.pages_component(),
                content,
                i64::from(content_ok),
                i64::from(status_code),
                content_type,
                now as i64,
                expires_at as i64,
                size_bytes,
                extraction_method,
                metadata_json,
            ],
        )
        .context("inserting cache entry")?;

        // Enforce the size cap by evicting oldest entries.
        self.evict_to_cap(&conn)?;

        Ok(())
    }

    /// Purge all expired entries.
    ///
    /// Returns the number of entries purged.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` delete fails.
    pub fn clear_expired(&self) -> Result<usize> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let now = unix_now();
        let purged = conn.execute(
            "DELETE FROM fetch_cache WHERE expires_at <= ?1",
            params![now as i64],
        )?;
        Ok(purged)
    }

    /// Purge every entry from the cache.
    ///
    /// Returns the number of entries purged.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` delete fails.
    pub fn clear_all(&self) -> Result<usize> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let purged = conn.execute("DELETE FROM fetch_cache", [])?;
        Ok(purged)
    }

    /// Return the number of entries currently in the cache (including any
    /// expired-but-not-yet-purged entries).
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` query fails.
    pub fn entry_count(&self) -> Result<usize> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM fetch_cache", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Return the total stored content size in bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the `SQLite` query fails.
    pub fn total_bytes(&self) -> Result<usize> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM fetch_cache",
            [],
            |r| r.get(0),
        )?;
        Ok(total as usize)
    }

    /// Return the current `SQLite` journal mode (e.g. `"wal"` or `"memory"`).
    ///
    /// Useful for diagnostics and for tests that need to verify WAL mode is
    /// active on a file-based cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the pragma query fails.
    pub fn journal_mode(&self) -> Result<String> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        Ok(mode)
    }

    /// Evict oldest entries until the total stored bytes is at or below
    /// `max_bytes`. Called after every insert.
    fn evict_to_cap(&self, conn: &Connection) -> Result<()> {
        let max = self.config.max_bytes as i64;
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM fetch_cache",
            [],
            |r| r.get(0),
        )?;

        if total <= max {
            return Ok(());
        }

        // Evict oldest entries (smallest created_at) until under the cap.
        // Use a prepared statement to iterate and delete row-by-row so we stop
        // as soon as the total drops below the cap.
        let mut stmt = conn.prepare(
            "SELECT url, extraction_type, css_selector, pages, size_bytes
             FROM fetch_cache
             ORDER BY created_at ASC",
        )?;
        let rows: Vec<(String, String, String, String, i64)> = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        let mut current = total;
        for (url, extraction_type, css_selector, pages, size_bytes) in rows {
            if current <= max {
                break;
            }
            conn.execute(
                "DELETE FROM fetch_cache
                 WHERE url = ?1
                   AND extraction_type = ?2
                   AND css_selector = ?3
                   AND pages = ?4",
                params![url, extraction_type, css_selector, pages],
            )?;
            current -= size_bytes;
        }

        Ok(())
    }
}

/// Return the current time as a Unix-second timestamp.
fn unix_now() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}
