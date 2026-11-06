//! High-level `ResearchManager` - the central facade for the research system.
//!
//! This module ties together the lower-level building blocks (name validation,
//! file I/O, gatherers, document assembly) into the lifecycle operations the
//! TUI, CLI, and HTTP layer all need:
//!
//! - [`ResearchManager::create`]    - T-007
//! - [`ResearchManager::list`]      - T-008
//! - [`ResearchManager::show`]      - T-009
//! - [`ResearchManager::delete`]    - T-010
//! - [`ResearchManager::archive`]   - T-011
//! - [`ResearchManager::search`]    - T-030
//!
//! Plus INDEX.md regeneration (T-012), duplicate-name suggestions (T-044),
//! "closest names" suggestions for missing items (T-046), and path
//! sanitisation (T-048).
//!
//! The manager holds no I/O state of its own beyond the on-disk root; each
//! method is async and stateless so it can be cloned freely across TUI
//! background tasks.

use crate::deck::{DeckElement, ReportSeries};
use crate::document::{
    AssembledDocument, ResearchDocument, assemble_document, mark_in_progress,
    render_corpa_skeleton, render_skeleton, render_supporting_file,
};
use crate::io::{IndexEntry, ResearchIo, ResearchIoError};
use crate::item::ResearchItem;
use crate::research_name::{ResearchName, ResearchNameError};
use crate::source::Source;
use crate::state::{ResearchState, SubQuestionStatus};
use crate::status::ResearchStatus;
use chrono::{DateTime, Utc};
use ragent_types::strutil::truncate_bytes;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
// PERF-080: FxHash for the short non-adversarial item-path/key sets.
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use thiserror::Error;

/// Errors surfaced by [`ResearchManager`].
#[derive(Debug, Error)]
pub enum ResearchError {
    /// A name failed FR-002 validation.
    #[error("{0}")]
    InvalidName(#[from] ResearchNameError),
    /// An on-disk I/O error (see [`ResearchIoError`]).
    #[error("{0}")]
    Io(#[from] ResearchIoError),
    /// A parse error when reading a research report back from disk.
    #[error("failed to parse research item '{name}': {message}")]
    Parse {
        /// The research name (always a valid [`ResearchName`]).
        name: String,
        /// Human-readable parse error message.
        message: String,
    },
    /// An item was requested that does not exist (FR-018).
    #[error("research item '{0}' not found. Closest matches: {1}")]
    NotFound(String, String),
    /// A duplicate create attempt (FR-016).
    #[error("research item '{0}' already exists; use `/research open {0}` to view it")]
    AlreadyExists(String),
    /// The primary `--from-url` page could not be fetched.
    #[error("failed to fetch --from-url '{url}': {message}")]
    FromUrlFetchFailed {
        /// URL that could not be fetched.
        url: String,
        /// Underlying error message.
        message: String,
    },
    /// The `--from-url` page was fetched, but its body did not contain enough
    /// usable text to derive a research topic.
    #[error(
        "--from-url '{url}' returned a page with no usable article body; cannot derive a topic"
    )]
    FromUrlNoUsableBody {
        /// URL that produced an empty or chrome-only body.
        url: String,
    },
    /// The `--from-file` document could not be read or parsed.
    #[error("failed to extract --from-file '{path}': {message}")]
    FromFileExtractFailed {
        /// File path that could not be extracted.
        path: String,
        /// Underlying error message.
        message: String,
    },
    /// The `--from-file` document was extracted, but its body did not contain
    /// enough usable text to derive a research topic.
    #[error("--from-file '{path}' contained no usable text to derive a topic")]
    FromFileNoUsableBody {
        /// File path that produced an empty body.
        path: String,
    },
    /// The iterative research engine failed during a multi-iteration pass.
    #[error("iterative research engine failed: {0}")]
    EngineRunFailed(String),
    /// The cite-check gate closed because a cited source was missing or its
    /// captured body did not support the claim (FR-006, FR-014).
    #[error("cite-check failed: {claims:?}")]
    CiteCheckFailed {
        /// Failed claims, each prefixed with `CITATION_VERIFICATION_FAILED`.
        claims: Vec<String>,
    },
    /// The configured analysis provider is not available.
    #[error("research provider not available: {0}")]
    ProviderNotAvailable(String),
    /// The topic is ambiguous and the caller must answer a clarifying question
    /// before web searches are performed (FR-005, FR-017).
    #[error("clarification needed: {question}")]
    NeedsClarification {
        /// The single clarifying question to present to the user.
        question: String,
    },
    /// A deck run could not select its two reports (FR-011, FR-012, FR-020).
    #[error("{0}")]
    DeckSelect(#[from] crate::deck::DeckSelectError),
    /// A deck run read a report that is not UTF-8 or not parseable (FR-019).
    #[error("cannot read report '{path}' for deck: {reason}")]
    DeckReport {
        /// The offending report file.
        path: String,
        /// Why it could not be used.
        reason: String,
    },
    /// A deck run could not write its output (FR-016, FR-017).
    #[error("{0}")]
    DeckWrite(#[from] crate::deck::DeckWriteError),
}

/// Result alias for [`ResearchManager`].
pub type Result<T> = std::result::Result<T, ResearchError>;

/// One row of a full-text search hit (T-030).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Research name.
    pub name: String,
    /// Title.
    pub title: String,
    /// Snippet showing the matched context.
    pub snippet: String,
    /// Path to the matched file (always a report inside `research/<name>/`).
    pub path: PathBuf,
}

// -- Search index cache (Milestone G) -----------------------------------------

/// One entry in the derived `research/.index.json` search cache.
///
/// Contains enough metadata + searchable text for [`ResearchManager::search`]
/// to serve results without reading every research report on every query. The
/// cache is regenerated by [`ResearchManager::refresh_index`] alongside
/// `INDEX.md` on every mutation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SearchIndexEntry {
    /// Research name (also the directory name).
    pub name: String,
    /// Human-readable title.
    pub title: String,
    /// Free-form topic description.
    pub topic: String,
    /// Lifecycle status as a kebab-case string (e.g. `"draft"`, `"in-progress"`).
    pub status: String,
    /// Tags associated with the item (currently always empty - reserved for
    /// future use).
    #[serde(default)]
    pub tags: Vec<String>,
    /// One-line summary extracted from the `## Executive Summary` section of
    /// the research report. Empty when no summary has been recorded yet.
    #[serde(default)]
    pub summary: String,
    /// UTC timestamp at which the item was created.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp of the most recent write.
    pub modified_at: DateTime<Utc>,
    /// Full body text (after frontmatter) of the research report. Used by
    /// [`ResearchManager::search`] to match queries against the complete
    /// document content without re-reading each file from disk.
    #[serde(default)]
    pub search_text: String,
}

/// The top-level structure serialized to `research/.index.json`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SearchIndex {
    /// When the cache was last generated (UTC).
    pub generated_at: DateTime<Utc>,
    /// One entry per research item, sorted alphabetically by name.
    pub entries: Vec<SearchIndexEntry>,
}

/// Extract a one-line summary from the body of a research report.
///
/// Looks for the `## Executive Summary` section and returns the first non-empty,
/// non-placeholder line. Returns an empty string when the section is absent
/// or contains only the default placeholder text.
fn extract_one_line_summary(body: &str) -> String {
    // Find the `## Executive Summary` heading.
    let summary_marker = "## Executive Summary";
    let summary_start = match body.find(summary_marker) {
        Some(pos) => pos,
        None => return String::new(),
    };
    // Skip past the heading line to the end of its line.
    let after_heading = &body[summary_start..];
    let line_end = after_heading.find('\n').unwrap_or(after_heading.len());
    let mut rest = &after_heading[line_end..];

    // Stop at the next `## ` heading (next section).
    let next_section = rest.find("\n## ");
    if let Some(pos) = next_section {
        rest = &rest[..pos];
    }

    for line in rest.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip the default placeholder.
        if trimmed.starts_with("(no executive summary recorded yet") {
            continue;
        }
        // Strip leading markdown list markers / formatting.
        let cleaned = trimmed
            .trim_start_matches("- ")
            .trim_start_matches("* ")
            .trim();
        if !cleaned.is_empty() {
            // Collapse to a single line and cap at 200 chars.
            let single = cleaned.replace('\n', " ");
            if single.len() > 200 {
                return truncate_bytes(&single, 200);
            }
            return single;
        }
    }
    String::new()
}

/// High-level async API for the `research/` directory.
///
/// Cheap to clone; all state lives in the on-disk root.
#[derive(Debug, Clone)]
pub struct ResearchManager {
    research_root: PathBuf,
    /// Mtime-keyed cache of parsed `ResearchItem` metadata to avoid redundant
    /// disk reads and YAML re-parsing on repeated `show()`/`list()` calls
    /// (PERF-MGR-02). Entries are invalidated when the file's mtime changes.
    /// Wrapped in `Arc` so `ResearchManager` remains `Clone`.
    item_cache: Arc<Mutex<FxHashMap<PathBuf, (SystemTime, ResearchItem)>>>,
}

impl ResearchManager {
    /// Construct a new manager rooted at `research_root`.
    pub fn new(research_root: impl Into<PathBuf>) -> Self {
        Self {
            research_root: research_root.into(),
            item_cache: Arc::new(Mutex::new(FxHashMap::default())),
        }
    }

    /// Get the root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.research_root
    }

    // -- Create (T-007) ----------------------------------------------------

    /// Create a fresh research item with a skeleton dated report.
    ///
    /// `name` is validated per FR-002; duplicate names fail per FR-016; the
    /// directory tree, the dated report, and INDEX.md cache are all updated
    /// atomically. The skeleton uses the legacy report layout; use
    /// [`Self::create_with_format`] to request a different output artifact.
    pub async fn create(&self, name: &str, title: &str, topic: &str) -> Result<ResearchItem> {
        self.create_with_format(name, title, topic, crate::run_config::OutputFormat::Report)
            .await
    }

    /// Create a fresh research item, selecting the skeleton layout via
    /// `output_format` (FR-011 / FR-012).
    ///
    /// Non-default formats are persisted in the frontmatter so the requested
    /// artifact is recorded from the moment the file lands on disk.
    pub async fn create_with_format(
        &self,
        name: &str,
        title: &str,
        topic: &str,
        output_format: crate::run_config::OutputFormat,
    ) -> Result<ResearchItem> {
        let name = ResearchName::try_new(name)?;
        if ResearchIo::item_exists(&self.research_root, &name).await {
            return Err(ResearchError::AlreadyExists(name.to_string()));
        }
        let mut item = ResearchItem::new(name.clone(), title, topic);
        if output_format != crate::run_config::OutputFormat::Report {
            item.output_format = Some(output_format.as_str().to_string());
        }
        ResearchIo::create_item_dirs(&self.research_root, &name).await?;
        // Hold the item's write lock for the whole publication: a concurrent
        // `persist_frontmatter` must not slip between the two renames below, or
        // a reader sees one file of the pair without the other.
        let write_lock = self.item_write_lock(&name);
        let write_guard = write_lock.lock().await;
        let content = render_skeleton(&name, title, topic, output_format);
        // Dated output names so a re-run of this item keeps the report it
        // writes today (`RESEARCH-<YYYY-MM-DD>.md`, `CORPA-<YYYY-MM-DD>.md`).
        ResearchIo::atomic_write(
            ResearchIo::primary_research_md_path(&self.research_root, &name, item.created_at),
            &content,
        )
        .await?;
        // CORPA companion skeleton: QA sections are empty at creation but
        // the file is present so downstream tooling sees a stable structure.
        let corpa_content = render_corpa_skeleton(&name, title, topic, output_format);
        ResearchIo::atomic_write(
            ResearchIo::primary_corpa_md_path(&self.research_root, &name, item.created_at),
            &corpa_content,
        )
        .await?;
        // The pair is on disk. The `refresh_index` rescan below runs as a
        // separate step a caller may observe half-done, and a caller that
        // immediately rewrites the item (a run marking it in progress) must not
        // wait behind a whole-vault rescan, so the lock is released here.
        drop(write_guard);
        tracing::info!(
            name = %name,
            title = %title,
            format = %output_format.as_str(),
            "research: created research item"
        );
        self.refresh_index().await?;
        Ok(item)
    }

    // -- List (T-008) ------------------------------------------------------

    /// Discover every research item under the root.
    pub async fn list(&self, include_archived: bool) -> Result<Vec<ResearchItem>> {
        let mut items = Vec::new();
        let read_dir = match tokio::fs::read_dir(&self.research_root).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(ResearchError::Io(ResearchIoError::Io(e))),
        };
        let mut entries = read_dir;
        while let Some(entry) = entries.next_entry().await.map_err(ResearchIoError::Io)? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // Skip non-research dirs such as `_templates` and INDEX.md.
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if dir_name.starts_with('_') || dir_name.starts_with('.') {
                continue;
            }
            // The report may be dated (`RESEARCH-YYYY-MM-DD.md`, the current
            // scheme) or undated (`RESEARCH.md`, an item created by an older
            // ragent); either makes the directory an item.
            let item_name = match ResearchName::try_new(&dir_name) {
                Ok(n) => n,
                Err(_) => continue,
            };
            let research_md =
                match ResearchIo::find_research_md(&self.research_root, &item_name).await {
                    Ok(path) => path,
                    Err(_) => continue,
                };
            match self.read_item_cached(&research_md).await {
                Ok(item) => {
                    if !include_archived && item.status == ResearchStatus::Archived {
                        continue;
                    }
                    items.push(item);
                }
                Err(e) => {
                    tracing::warn!(
                        path = %research_md.display(),
                        error = %e,
                        "research: skipping unparseable item"
                    );
                }
            }
        }
        // Sort by modified descending (FR-004) - newest first.
        items.sort_by_key(|a| std::cmp::Reverse(a.modified_at));
        Ok(items)
    }

    // -- Show (T-009) ------------------------------------------------------

    /// Read a single research item by name. Returns
    /// [`ResearchError::NotFound`] when the directory doesn't exist.
    pub async fn show(&self, name: &str) -> Result<ResearchItem> {
        let name = ResearchName::try_new(name)?;
        Ok(self.show_validated(&name).await?.0)
    }

    /// [`Self::show`] for a caller that already holds a validated name.
    ///
    /// Returns the item and its validated name, so a route that has to ask
    /// "does this exist" before using it does not validate the same string
    /// twice.
    ///
    /// # Errors
    ///
    /// Returns [`ResearchError::NotFound`] when the item directory exists but
    /// its report does not: `create` publishes the directory and the file
    /// in separate steps, so a directory alone is not an item.
    pub async fn show_validated(
        &self,
        name: &ResearchName,
    ) -> Result<(ResearchItem, ResearchName)> {
        if !ResearchIo::item_exists(&self.research_root, name).await {
            let suggestions = self.suggest_closest(name.as_str()).await;
            return Err(ResearchError::NotFound(name.to_string(), suggestions));
        }
        let path = match ResearchIo::find_research_md(&self.research_root, name).await {
            Ok(path) => path,
            Err(ResearchIoError::NotFound(_)) => {
                let suggestions = self.suggest_closest(name.as_str()).await;
                return Err(ResearchError::NotFound(name.to_string(), suggestions));
            }
            Err(e) => return Err(ResearchError::Io(e)),
        };
        match self.read_item_cached(&path).await {
            Ok(item) => Ok((item, name.clone())),
            // `create` makes the item directory before it writes the file, so a
            // reader that arrives in between sees a directory with no item. That
            // is "not an item yet", not an I/O failure: report it as not found
            // rather than as a 500 the caller cannot act on.
            Err(ResearchError::Io(ResearchIoError::Io(e)))
                if e.kind() == std::io::ErrorKind::NotFound =>
            {
                let suggestions = self.suggest_closest(name.as_str()).await;
                Err(ResearchError::NotFound(name.to_string(), suggestions))
            }
            Err(e) => Err(e),
        }
    }

    // -- Delete (T-010) ----------------------------------------------------

    /// Recursively delete a research item.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let name = ResearchName::try_new(name)?;
        if !ResearchIo::item_exists(&self.research_root, &name).await {
            let suggestions = self.suggest_closest(name.as_str()).await;
            return Err(ResearchError::NotFound(name.to_string(), suggestions));
        }
        ResearchIo::remove_item(&self.research_root, &name).await?;
        tracing::info!(name = %name, "research: deleted research item");
        self.refresh_index().await?;
        Ok(())
    }

    // -- Archive (T-011) ---------------------------------------------------

    /// Mark a research item as `Archived` (FR-013).
    pub async fn archive(&self, name: &str) -> Result<()> {
        self.transition_status(name, ResearchStatus::Archived).await
    }

    /// Transition a research item to a new status. Writes the updated
    /// frontmatter back to the report and refreshes the index.
    ///
    /// Reads the file once, splits frontmatter in-memory, and avoids the
    /// previous double-read pattern (PERF-MGR-01).
    pub async fn transition_status(&self, name: &str, status: ResearchStatus) -> Result<()> {
        let name = ResearchName::try_new(name)?;
        if !ResearchIo::item_exists(&self.research_root, &name).await {
            let suggestions = self.suggest_closest(name.as_str()).await;
            return Err(ResearchError::NotFound(name.to_string(), suggestions));
        }
        let path = match ResearchIo::find_research_md(&self.research_root, &name).await {
            Ok(path) => path,
            Err(ResearchIoError::NotFound(_)) => {
                let suggestions = self.suggest_closest(name.as_str()).await;
                return Err(ResearchError::NotFound(name.to_string(), suggestions));
            }
            Err(e) => return Err(ResearchError::Io(e)),
        };
        // Single read: get the full content, split in-memory.
        let content = ResearchIo::read_file(&path).await?;
        let (frontmatter, body) = ResearchIo::split_frontmatter(&content);
        let mut item =
            ResearchItem::from_frontmatter(&frontmatter).map_err(|e| ResearchError::Parse {
                name: name.to_string(),
                message: e.to_string(),
            })?;
        item.set_status(status);
        let new_frontmatter = item.render_frontmatter();
        let new_content = format!("{new_frontmatter}{body}");
        ResearchIo::atomic_write(&path, &new_content).await?;
        // Invalidate the cache entry for this item.
        self.invalidate_cache(&path);
        tracing::info!(
            name = %item.name,
            status = %status.as_str(),
            "research: transitioned status"
        );
        self.refresh_index().await?;
        Ok(())
    }

    // -- Search (T-030) ----------------------------------------------------

    /// Perform a case-insensitive substring search across every
    /// research report. Returns at most `max_hits` items in modified-desc order.
    ///
    /// Reads the derived `research/.index.json` cache first (G-002); falls
    /// back to a full scan only when the cache is stale or missing.
    pub async fn search(&self, query: &str, max_hits: usize) -> Result<Vec<SearchHit>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let q_lower = q.to_lowercase();

        // Try the cache first (G-002).
        if let Some(hits) = self.search_via_cache(&q_lower, q.len(), max_hits).await? {
            return Ok(hits);
        }

        // Fall back to full scan (cache stale or missing).
        let mut out = Vec::new();
        for item in self.list(true).await? {
            let path =
                ResearchIo::resolve_research_md(&self.research_root, &item.name, item.created_at)
                    .await
                    .unwrap_or_else(|_| {
                        ResearchIo::primary_research_md_path(
                            &self.research_root,
                            &item.name,
                            item.created_at,
                        )
                    });
            let body = match tokio::fs::read_to_string(&path).await {
                Ok(b) => b,
                Err(_) => continue,
            };
            let lower = body.to_lowercase();
            if let Some(byte_idx) = lower.find(&q_lower) {
                let snippet = extract_snippet(&body, byte_idx, q.len(), 80);
                out.push(SearchHit {
                    name: item.name.to_string(),
                    title: item.title,
                    snippet,
                    path,
                });
            }
            if out.len() >= max_hits {
                break;
            }
        }
        Ok(out)
    }

    /// Search using the `.index.json` cache. Returns `None` when the cache
    /// is stale, missing, or unparseable - the caller should fall back to a
    /// full scan in that case.
    async fn search_via_cache(
        &self,
        q_lower: &str,
        q_len: usize,
        max_hits: usize,
    ) -> Result<Option<Vec<SearchHit>>> {
        let cache_path = ResearchIo::cache_path(&self.research_root);
        let cache_json = match tokio::fs::read_to_string(&cache_path).await {
            Ok(j) => j,
            Err(_) => return Ok(None), // cache missing -> fall back
        };
        let cache: SearchIndex = match serde_json::from_str(&cache_json) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "research: .index.json unparseable, falling back");
                return Ok(None);
            }
        };

        // Staleness check (G-003): compare cache entry names against the
        // actual set of research item directories on disk. If the sets differ
        // (item added/deleted/renamed), the cache is stale.
        let mut disk_names: FxHashSet<String> = FxHashSet::default();
        let read_dir = match tokio::fs::read_dir(&self.research_root).await {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };
        let mut entries = read_dir;
        while let Some(entry) = entries.next_entry().await.map_err(ResearchIoError::Io)? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if dir_name.starts_with('_') || dir_name.starts_with('.') {
                continue;
            }
            let item_name = match ResearchName::try_new(&dir_name) {
                Ok(n) => n,
                Err(_) => continue,
            };
            let research_md =
                match ResearchIo::find_research_md(&self.research_root, &item_name).await {
                    Ok(path) => path,
                    Err(_) => continue,
                };
            disk_names.insert(dir_name.clone());
            // Also check mtime: if the report was modified after the
            // cache was generated, the cache is stale (manual edit).
            if let Ok(meta) = tokio::fs::metadata(&research_md).await
                && let Ok(mtime) = meta.modified()
                && let Ok(mtime_dt) = mtime.duration_since(std::time::UNIX_EPOCH)
            {
                let mtime_utc = DateTime::<Utc>::from_timestamp(
                    mtime_dt.as_secs() as i64,
                    mtime_dt.subsec_nanos(),
                )
                .unwrap_or(Utc::now());
                if mtime_utc > cache.generated_at {
                    tracing::debug!(
                        "research: .index.json stale (report newer than cache for {})",
                        dir_name
                    );
                    return Ok(None);
                }
            }
        }
        let cache_names: FxHashSet<String> = cache.entries.iter().map(|e| e.name.clone()).collect();
        if cache_names != disk_names {
            tracing::debug!("research: .index.json stale (item set mismatch), falling back");
            return Ok(None);
        }

        // Cache is fresh - search through cached search_text.
        // Sort entries by modified_at descending to match list() ordering.
        let mut sorted = cache.entries.clone();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.modified_at));

        let mut out = Vec::new();
        for entry in &sorted {
            let lower = entry.search_text.to_lowercase();
            if let Some(byte_idx) = lower.find(q_lower) {
                let snippet = extract_snippet(&entry.search_text, byte_idx, q_len, 80);
                let name = match ResearchName::try_new(&entry.name) {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                let path =
                    ResearchIo::resolve_research_md(&self.research_root, &name, entry.created_at)
                        .await
                        .unwrap_or_else(|_| {
                            ResearchIo::primary_research_md_path(
                                &self.research_root,
                                &name,
                                entry.created_at,
                            )
                        });
                out.push(SearchHit {
                    name: entry.name.clone(),
                    title: entry.title.clone(),
                    snippet,
                    path,
                });
            }
            if out.len() >= max_hits {
                break;
            }
        }
        Ok(Some(out))
    }

    // -- Document assembly helpers (T-019, T-020, T-021, T-022) ------------

    /// Assemble and write a fully-populated dated report for an existing
    /// research item. The item's `sources` are persisted as numbered
    /// supporting files (T-015, T-017) and the frontmatter is rewritten to
    /// include the new source list. Returns the assembled [`AssembledDocument`]
    /// for caller logging.
    pub async fn write_document(&self, doc: &ResearchDocument) -> Result<AssembledDocument> {
        let name = doc.item.name.clone();
        if !ResearchIo::item_exists(&self.research_root, &name).await {
            return Err(ResearchError::NotFound(
                name.to_string(),
                self.suggest_closest(name.as_str()).await,
            ));
        }
        // 1. Render frontmatter + body for the new content.
        let assembled = assemble_document(doc);

        // 2. Rewrite supporting files for every source that has a body.
        for (idx, source) in doc.item.sources.iter().enumerate() {
            let prefix = match source {
                Source::Web { .. } => "web",
                Source::Local { .. } => "local",
                Source::Other { .. } => "other",
                Source::Spec { .. } => continue,
            };
            // Supporting files use 1-based zero-padded numbering (`web-01.md`),
            // not the position within the sources list.
            let index = supporting_index(prefix, &doc.item.sources[..=idx]);
            let path = ResearchIo::source_body_path(&self.research_root, &name, prefix, index);
            if let Some(body) = render_supporting_file(source, doc.item.url_cloak) {
                ResearchIo::atomic_write(&path, &body).await?;
            }
        }

        // 3. Write the report under this run's dated name, so an earlier
        //    run's `RESEARCH-<date>.md` survives.
        let stamp = crate::io::dated_stamp(Utc::now());
        let path = ResearchIo::dated_research_md_path(&self.research_root, &name, &stamp);
        ResearchIo::atomic_write(&path, &assembled.content).await?;

        // 4. Write the CORPA companion with the QA render sections.
        let corpa_path = ResearchIo::dated_corpa_md_path(&self.research_root, &name, &stamp);
        ResearchIo::atomic_write(&corpa_path, &assembled.corpa).await?;

        tracing::info!(
            name = %name,
            sources = doc.item.sources.len(),
            path = %path.display(),
            "research: wrote research report"
        );
        self.refresh_index().await?;
        Ok(assembled)
    }

    /// Mark the item `InProgress` and persist.
    pub async fn start_gathering(&self, name: &str) -> Result<()> {
        let mut item = self.show(name).await?;
        mark_in_progress(&mut item);
        self.persist_frontmatter(&item).await?;
        self.refresh_index().await?;
        Ok(())
    }

    /// Mark the item `Complete` and persist.
    pub async fn complete_gathering(&self, name: &str) -> Result<()> {
        let name = ResearchName::try_new(name)?;
        let path = ResearchIo::find_research_md(&self.research_root, &name).await?;
        let content = ResearchIo::read_file(&path).await?;
        // Preserve the full frontmatter block that `write_document` produced;
        // only replace the status line. Re-rendering from a re-read item would
        // drop the `sources` count because `from_frontmatter` treats it as a
        // count-only hint and initialises `sources` to an empty vec.
        let new_content = replace_frontmatter_status_line(&content, ResearchStatus::Complete);
        ResearchIo::atomic_write(&path, &new_content).await?;
        tracing::info!(
            name = %name,
            status = %ResearchStatus::Complete.as_str(),
            "research: marked item complete"
        );
        self.refresh_index().await?;
        Ok(())
    }

    /// Persist the current [`ResearchState`] for a research item (T-013).
    pub async fn save_state(&self, name: &str, state: &ResearchState) -> Result<()> {
        let name = ResearchName::try_new(name)?;
        ResearchIo::write_state(&self.research_root, &name, state).await?;
        self.refresh_index().await?;
        Ok(())
    }

    /// Load a previously saved [`ResearchState`] for a research item.
    /// Returns [`ResearchError::NotFound`] when the item directory or
    /// `state.json` does not exist.
    pub async fn load_state(&self, name: &str) -> Result<ResearchState> {
        let name = ResearchName::try_new(name)?;
        if !ResearchIo::item_exists(&self.research_root, &name).await {
            let suggestions = self.suggest_closest(name.as_str()).await;
            return Err(ResearchError::NotFound(name.to_string(), suggestions));
        }
        ResearchIo::read_state(&self.research_root, &name)
            .await
            .map_err(ResearchError::Io)
    }

    /// Resume an in-progress research item (T-012, T-014).
    ///
    /// Loads the saved state, applies an optional follow-up message by adding
    /// a new sub-question, and persists the updated state. The saved state is
    /// written back unchanged when there is no follow-up (the item's status
    /// lives in the report frontmatter, not the state file).
    pub async fn continue_item(
        &self,
        name: &str,
        follow_up: Option<&str>,
    ) -> Result<ResearchState> {
        let mut state = self.load_state(name).await?;

        if let Some(msg) = follow_up {
            state.plan.topic.push_str(&format!("\n\nFollow-up: {msg}"));
            let id = format!("follow-up-{}", state.plan.sub_questions.len() + 1);
            state.add_sub_question(&id, msg, 10);
            state.set_sub_question_status(&id, SubQuestionStatus::Pending);
        }

        self.save_state(name, &state).await?;
        Ok(state)
    }

    // -- Deck (spec `diffdeck`, T-008) --------------------------------------

    /// Write a diff deck for `name` from two of its dated reports
    /// (FR-002, FR-011, FR-014, FR-015, FR-016, FR-017, FR-018, FR-019, FR-021).
    ///
    /// The run is a sequence of steps whose refusal points all come **before**
    /// the single [`ResearchIo::atomic_write`] that publishes the deck, so a
    /// refusal never leaves a half-written or modified deck behind:
    ///
    /// 1. the item must exist, or the run is refused naming it and creates
    ///    nothing (FR-018);
    /// 2. the item directory is enumerated and [`ReportSeries::select_pair`]
    ///    picks the two reports - the two most recent by default, or the pair
    ///    `--from` / `--to` names - refusing when the directory cannot supply
    ///    two (FR-011, FR-012, FR-020);
    /// 3. both reports are read as UTF-8 text and parsed into
    ///    [`ParsedReport`](crate::deck::ParsedReport)s; an unreadable file, a
    ///    non-UTF-8 file, or one with no recognised top-level section is
    ///    refused naming the path and the reason (FR-019, `--force` never
    ///    bypasses this);
    /// 4. the parsed pair is diffed, then restricted to the elements
    ///    `--elements` names (FR-014) - the filter runs before rendering so a
    ///    filtered run's cover and summary counts describe only what it wrote;
    /// 5. the output path is resolved: the `--out` value when given (with its
    ///    missing parent directories created, FR-016), otherwise
    ///    `DECK-<today>.md` inside the item directory (FR-002);
    /// 6. an existing output file is refused unless `--force` was given
    ///    (FR-017);
    /// 7. the deck is rendered with `--include-old` applied (FR-015) and
    ///    written atomically.
    ///
    /// No file outside the research root or the caller's `--out` path is
    /// touched (FR-021). This entry point makes no network request and no LLM
    /// call; [`Self::deck_with_model`] is the same run followed by the optional
    /// enrichment stage (FR-023, FR-024).
    ///
    /// # Errors
    ///
    /// - [`ResearchError::NotFound`] when the item does not exist (FR-018);
    /// - [`ResearchError::DeckSelect`] when the pair cannot be selected
    ///   (FR-011, FR-012, FR-020);
    /// - [`ResearchError::DeckReport`] when a report cannot be read or parsed
    ///   (FR-019);
    /// - [`ResearchError::DeckWrite`] when the destination is unusable or
    ///   already exists without `--force` (FR-016, FR-017);
    /// - [`ResearchError::Io`] when the write itself fails.
    pub async fn deck(
        &self,
        name: &str,
        options: &crate::deck::DeckRunOptions,
    ) -> Result<crate::deck::DeckOutcome> {
        self.deck_with_model(name, options, None).await
    }

    /// Write a diff deck for `name` with an optional LLM enrichment stage
    /// (spec `diffdeck` FR-023, FR-024).
    ///
    /// The stages, the refusals and the atomic publication are exactly
    /// [`Self::deck`]'s; the only difference is that the parsed reports are
    /// handed to [`crate::deck_enrich::enrich_deck`] before rendering, so each
    /// finding slide carries a one-paragraph summary of its observation and
    /// analysis and each Top Implication lists the findings ranked most relevant
    /// to it.
    ///
    /// `model: None` is a supported call, not a degraded one: the enrichment
    /// stage then computes the deterministic baseline - the first sentence of
    /// each finding's observation and analysis, and a term-overlap ranking of
    /// findings behind each implication - so a run with no provider configured
    /// still produces the improved layout and spends no tokens. A model that
    /// fails mid-stage falls back to the same values per finding/implication
    /// rather than failing the run.
    pub async fn deck_with_model(
        &self,
        name: &str,
        options: &crate::deck::DeckRunOptions,
        model: Option<&dyn crate::deck_enrich::DeckModel>,
    ) -> Result<crate::deck::DeckOutcome> {
        self.deck_with_sink(name, options, model, None).await
    }

    /// Write a diff deck and, when a sink is supplied, its PowerPoint twin
    /// (spec `diffdeck` FR-030).
    ///
    /// The markdown deck is produced exactly as [`Self::deck_with_model`] does -
    /// same stages, same refusals, same atomic publication - and the `.pptx`
    /// export runs *after* it has been written. That ordering is the contract:
    ///
    /// - the markdown deck is the canonical artifact, so a failed or absent
    ///   presentation export can never cost the user their deck;
    /// - the export is best-effort, so a sink that fails is recorded in
    ///   [`crate::deck::DeckOutcome::pptx`] as
    ///   [`crate::deck_pptx::DeckPptxOutcome::Failed`] and reported to the user,
    ///   while the run still returns `Ok`;
    /// - `sink: None` is a supported call meaning "this surface offers no
    ///   presentation capability", recorded as
    ///   [`crate::deck_pptx::DeckPptxOutcome::Unavailable`].
    ///
    /// The sink is handed the markdown deck's own rendered text
    /// ([`crate::deck::DeckDocument::markdown`]) rather than the parsed reports,
    /// so the presentation describes the same data as the file beside it and the
    /// two can never diverge.
    ///
    /// # Errors
    ///
    /// Exactly [`Self::deck_with_model`]'s errors: the export stage cannot fail a
    /// run that reached it.
    pub async fn deck_with_sink(
        &self,
        name: &str,
        options: &crate::deck::DeckRunOptions,
        model: Option<&dyn crate::deck_enrich::DeckModel>,
        sink: Option<&dyn crate::deck_pptx::DeckSlideSink>,
    ) -> Result<crate::deck::DeckOutcome> {
        self.deck_with_progress(name, options, model, sink, None)
            .await
    }

    /// Write a diff deck, reporting each stage's progress (FR-031).
    ///
    /// The stages, the refusals, the atomic publication and the optional
    /// presentation twin are exactly [`Self::deck_with_sink`]'s. `progress`
    /// receives one report as each stage begins and one as it ends, plus a
    /// per-unit report where a stage has units (one per finding paragraph, one
    /// per exported slide), so a surface can show a run that is working rather
    /// than one that has gone quiet.
    ///
    /// Reporting is additive: `progress: None` is [`Self::deck_with_sink`], and a
    /// sink that fails to deliver a report changes nothing about the artifact.
    ///
    /// # Errors
    ///
    /// Exactly [`Self::deck_with_sink`]'s errors. A refusal reports the failing
    /// stage before returning, so the surface sees where the run stopped.
    pub async fn deck_with_progress(
        &self,
        name: &str,
        options: &crate::deck::DeckRunOptions,
        model: Option<&dyn crate::deck_enrich::DeckModel>,
        sink: Option<&dyn crate::deck_pptx::DeckSlideSink>,
        progress: Option<&dyn crate::deck_progress::DeckProgress>,
    ) -> Result<crate::deck::DeckOutcome> {
        use crate::deck_progress::{DeckEvent, DeckStage, report};

        let name = ResearchName::try_new(name)?;
        report(
            progress,
            DeckEvent::started(
                DeckStage::Select,
                format!("looking up item '{name}' and its report series"),
            ),
        );
        // Step 1 (FR-018): refuse before touching the directory.
        if !ResearchIo::item_exists(&self.research_root, &name).await {
            let suggestions = self.suggest_closest(name.as_str()).await;
            return Err(ResearchError::NotFound(name.to_string(), suggestions));
        }
        let item_dir = ResearchIo::item_dir(&self.research_root, &name);

        // Step 2 (FR-006, FR-007, FR-011, FR-012, FR-020).
        let series = ReportSeries::classify(&self.deck_dir_entry_names(&item_dir).await?);
        let pair = series.select_pair(
            name.as_str(),
            options.from.as_deref(),
            options.to.as_deref(),
        )?;
        report(
            progress,
            DeckEvent::done(
                DeckStage::Select,
                format!("comparing {} to {}", pair.older_label(), pair.newer_label()),
            ),
        );

        // Step 3 (FR-019): read both reports before deciding anything else.
        report(
            progress,
            DeckEvent::started(DeckStage::Read, "reading both reports of the pair"),
        );
        let older = self.read_deck_report(&item_dir, &pair.older).await?;
        let newer = self.read_deck_report(&item_dir, &pair.newer).await?;
        report(
            progress,
            DeckEvent::done(
                DeckStage::Read,
                format!(
                    "read {} item(s) from the older and {} from the newer report",
                    older.items(DeckElement::Findings).len()
                        + older.items(DeckElement::Concepts).len(),
                    newer.items(DeckElement::Findings).len()
                        + newer.items(DeckElement::Concepts).len()
                ),
            ),
        );

        // Step 4 (FR-004, FR-008, FR-013, FR-014).
        report(
            progress,
            DeckEvent::started(
                DeckStage::Diff,
                "diffing the two reports element by element",
            ),
        );
        let diff = crate::deck::filter_elements(
            &crate::deck::diff_reports(&older, &newer),
            &options.elements,
        );
        report(
            progress,
            DeckEvent::done(
                DeckStage::Diff,
                format!("{} element(s) in the diff", diff.elements.len()),
            ),
        );

        // Step 5 (FR-002, FR-016): resolve the destination.
        report(
            progress,
            DeckEvent::started(DeckStage::Destination, "resolving the output path"),
        );
        let generated_on = DateTime::<Utc>::from(SystemTime::now()).date_naive();
        let stamp = generated_on.format("%Y-%m-%d").to_string();
        let path = match options.out.as_deref() {
            Some(value) => self.deck_out_path(value)?,
            None => crate::deck::default_deck_path(&item_dir, &stamp),
        };

        // Step 6 (FR-017): an existing deck is only replaced under `--force`.
        if !options.force && tokio::fs::metadata(&path).await.is_ok() {
            return Err(crate::deck::DeckWriteError::Exists {
                path: path.display().to_string(),
            }
            .into());
        }
        report(
            progress,
            DeckEvent::done(
                DeckStage::Destination,
                format!("writing to {}", path.display()),
            ),
        );

        // Step 7 (FR-023, FR-024): enrich the findings and implications. This
        // is the only step that can consult a model; it is best-effort, so a
        // failure here leaves the deterministic values in place rather than
        // refusing a deck that is otherwise complete. The enrichment reports its
        // own per-finding progress (FR-031), which is why it is handed the sink.
        let enrichment =
            crate::deck_enrich::enrich_deck_with_progress(&newer, older.clone(), model, progress)
                .await;

        // Step 8 (FR-015, FR-002): render and publish atomically.
        report(
            progress,
            DeckEvent::started(DeckStage::Render, "rendering the deck document"),
        );
        let render_options =
            crate::deck::DeckRenderOptions::new(name.as_str(), generated_on, &pair, &older, &newer)
                .with_include_old(options.include_old)
                .with_enrichment(enrichment.clone());
        let document = crate::deck::render_deck(&diff, &render_options);
        ResearchIo::atomic_write(&path, &document.markdown).await?;
        report(
            progress,
            DeckEvent::done(
                DeckStage::Render,
                format!(
                    "wrote {} ({} element slide(s), {} byte(s))",
                    path.display(),
                    document.elements.len(),
                    document.markdown.len()
                ),
            ),
        );
        tracing::info!(
            name = %name,
            path = %path.display(),
            from = %pair.older_label(),
            to = %pair.newer_label(),
            elements = document.elements.len(),
            model = enrichment.used_model,
            summaries = enrichment.summaries.len(),
            ranked = enrichment.concept_findings.len(),
            "research: wrote diff deck"
        );

        // Step 9 (FR-030): the optional PowerPoint twin. This runs strictly
        // after the markdown deck has been published and is best-effort: the
        // markdown deck is the canonical artifact, so an absent or failing
        // presentation capability is reported on the outcome and never turned
        // into an error that would make the run look as if it wrote nothing.
        let mut outcome = crate::deck::DeckOutcome::new(path.clone(), pair, document, &diff);
        outcome.pptx = match sink {
            None => {
                report(
                    progress,
                    DeckEvent::skipped(
                        DeckStage::Export,
                        "no presentation capability connected; the markdown deck is complete",
                    ),
                );
                crate::deck_pptx::DeckPptxOutcome::Unavailable
            }
            Some(sink) => {
                report(
                    progress,
                    DeckEvent::started(
                        DeckStage::Export,
                        format!("writing the presentation beside {}", path.display()),
                    ),
                );
                let generated_on = generated_on.format("%Y-%m-%d").to_string();
                let plan = crate::deck_pptx::DeckSlidePlan::from_document(
                    name.as_str(),
                    // `render_options.title` is already the newer report's
                    // `# Title:` line, resolved once for the markdown deck; reusing
                    // it keeps the two artifacts describing the same run.
                    render_options.title.clone(),
                    &outcome.pair.older_label(),
                    &outcome.pair.newer_label(),
                    &generated_on,
                    &outcome.document,
                    // The plan is rendered from the same diff the markdown deck
                    // was: the presentation carries the run's data, not a
                    // transcription of the markdown file.
                    &diff,
                    render_options.include_old,
                    crate::deck_pptx::deck_pptx_path(&path),
                    // FR-017's `--force` governs the markdown deck's own
                    // destination; the presentation beside it is an addition, so
                    // it is replaced whenever its markdown twin is, and never
                    // becomes the reason a rerun is refused.
                    true,
                );
                let result = sink.write_deck_with_progress(&plan, progress).await;
                match &result {
                    crate::deck_pptx::DeckPptxOutcome::Written { path, slides, .. } => {
                        report(
                            progress,
                            DeckEvent::done(
                                DeckStage::Export,
                                format!("wrote {} ({slides} slide(s))", path.display()),
                            ),
                        );
                        tracing::info!(
                            name = %name,
                            path = %path.display(),
                            slides,
                            "research: wrote deck presentation"
                        );
                    }
                    crate::deck_pptx::DeckPptxOutcome::Failed { reason } => {
                        // Warn, not error: the deck run itself succeeded and the
                        // markdown file is on disk and complete.
                        report(
                            progress,
                            DeckEvent::skipped(
                                DeckStage::Export,
                                format!("presentation export skipped: {reason}"),
                            ),
                        );
                        tracing::warn!(
                            name = %name,
                            reason = %reason,
                            "research: deck presentation export failed; markdown deck is unaffected"
                        );
                    }
                    crate::deck_pptx::DeckPptxOutcome::Unavailable => {
                        report(
                            progress,
                            DeckEvent::skipped(
                                DeckStage::Export,
                                "no presentation capability connected; the markdown deck is complete",
                            ),
                        );
                        tracing::debug!(
                            name = %name,
                            "research: no PowerPoint capability available for the deck"
                        );
                    }
                }
                result
            }
        };
        Ok(outcome)
    }

    /// The entry names of a deck run's item directory, sorted for a stable
    /// series.
    ///
    /// A directory that vanished between the existence check and the read is
    /// reported as an empty series rather than an I/O error, so the run reaches
    /// the FR-011 refusal that names the item and the count found instead of
    /// surfacing a bare `NotFound` from `read_dir`.
    async fn deck_dir_entry_names(&self, item_dir: &Path) -> Result<Vec<String>> {
        let mut read_dir = match tokio::fs::read_dir(item_dir).await {
            Ok(read_dir) => read_dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(ResearchIoError::Io(e).into()),
        };
        let mut names: Vec<String> = Vec::new();
        while let Some(entry) = read_dir.next_entry().await.map_err(ResearchIoError::Io)? {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Read and parse one report of a deck run's pair (FR-019).
    ///
    /// The bytes are read once and decoded explicitly so a non-UTF-8 report is
    /// named as such rather than reported as a generic I/O failure, and a report
    /// with no recognised top-level section names the reason the parse gave.
    async fn read_deck_report(
        &self,
        item_dir: &Path,
        report: &crate::deck::ReportFile,
    ) -> Result<crate::deck::ParsedReport> {
        let path = item_dir.join(&report.name);
        let report_error = |reason: String| ResearchError::DeckReport {
            path: path.display().to_string(),
            reason,
        };
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| report_error(e.to_string()))?;
        let text = String::from_utf8(bytes)
            .map_err(|e| report_error(format!("file is not valid UTF-8: {e}")))?;
        crate::deck::parse_report(&text).map_err(|e| report_error(e.to_string()))
    }

    /// Resolve an FR-016 `--out` value into a writable deck path.
    ///
    /// The value is taken relative to the process working directory, and its
    /// missing parent directories are created so a run can write into a fresh
    /// directory in one step. A value that names no file (an empty string, or a
    /// path ending in a directory separator) is refused rather than written
    /// somewhere unintended.
    fn deck_out_path(&self, value: &str) -> Result<PathBuf> {
        let invalid = |reason: &'static str| {
            ResearchError::DeckWrite(crate::deck::DeckWriteError::InvalidOut {
                value: value.to_string(),
                reason,
            })
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(invalid("expected a file path"));
        }
        let path = PathBuf::from(trimmed);
        if path.file_name().is_none() || path.is_dir() {
            return Err(invalid("expected a file path, not a directory"));
        }
        // `atomic_write` creates missing parents; this keeps the refusal path
        // (FR-017) able to report a real destination.
        Ok(path)
    }

    // -- INDEX.md (T-012) + .index.json cache (Milestone G) ---------------

    /// Regenerate `research/INDEX.md` and `research/.index.json` from the
    /// on-disk state. Cheap; safe to call after any mutation.
    pub async fn refresh_index(&self) -> Result<()> {
        let items = self.list(true).await?;
        let entries: Vec<IndexEntry> = items
            .iter()
            .map(|i| IndexEntry {
                name: i.name.to_string(),
                title: i.title.clone(),
                status: i.status,
                created_at: i.created_at,
                modified_at: i.modified_at,
            })
            .collect();
        // Sort INDEX.md rows alphabetically by name for stability.
        let mut entries = entries;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let body = ResearchIo::render_index(&entries);
        let path = ResearchIo::index_path(&self.research_root);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(ResearchIoError::Io)?;
        }
        ResearchIo::atomic_write(&path, &body).await?;

        // Build and write the derived `.index.json` search cache (G-001).
        let mut cache_entries = Vec::with_capacity(items.len());
        for i in &items {
            let research_md =
                ResearchIo::resolve_research_md(&self.research_root, &i.name, i.created_at)
                    .await
                    .unwrap_or_else(|_| {
                        ResearchIo::primary_research_md_path(
                            &self.research_root,
                            &i.name,
                            i.created_at,
                        )
                    });
            // FUNC-028: an unreadable report must not become a blank index
            // entry that is silently unsearchable. Log the failure and still
            // emit the entry, but with a clear summary note rather than empty
            // content, so the omission is traceable.
            let (body, readable) = match tokio::fs::read_to_string(&research_md).await {
                Ok(body) => (body, true),
                Err(e) => {
                    tracing::warn!(
                        name = %i.name,
                        path = %research_md.display(),
                        error = %e,
                        "failed to read the research report while building search index; entry will lack full text"
                    );
                    (String::new(), false)
                }
            };
            let (_, body_after_fm) = ResearchIo::split_frontmatter(&body);
            let summary = if readable {
                extract_one_line_summary(&body_after_fm)
            } else {
                format!("[unreadable: {}]", research_md.display())
            };
            cache_entries.push(SearchIndexEntry {
                name: i.name.to_string(),
                title: i.title.clone(),
                topic: i.topic.clone(),
                status: i.status.as_str().to_string(),
                tags: Vec::new(),
                summary,
                created_at: i.created_at,
                modified_at: i.modified_at,
                search_text: body_after_fm,
            });
        }
        let cache = SearchIndex {
            generated_at: Utc::now(),
            entries: cache_entries,
        };
        let cache_json = serde_json::to_string(&cache).map_err(ResearchIoError::Serde)?;
        let cache_path = ResearchIo::cache_path(&self.research_root);
        ResearchIo::atomic_write(&cache_path, &cache_json).await?;
        Ok(())
    }

    // -- Internal helpers --------------------------------------------------

    /// Read a research item from disk, using the mtime-keyed cache to skip
    /// redundant file reads and YAML parsing when the file hasn't changed
    /// (PERF-MGR-02). Falls back to a direct read on any cache miss or
    /// mtime mismatch.
    async fn read_item_cached(&self, path: &Path) -> Result<ResearchItem> {
        // Check cache: if we have a fresh entry with matching mtime, return it.
        let mtime = tokio::fs::metadata(path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        if let Some(mtime) = mtime
            && let Ok(cache) = self.item_cache.lock()
            && let Some((cached_mtime, item)) = cache.get(path)
            && *cached_mtime == mtime
        {
            return Ok(item.clone());
        }
        // Cache miss: read and parse from disk.
        let item = Self::read_item_from_path(path).await?;
        // Store in cache if we have the mtime.
        if let Some(mtime) = mtime
            && let Ok(mut cache) = self.item_cache.lock()
        {
            cache.insert(path.to_path_buf(), (mtime, item.clone()));
        }
        Ok(item)
    }

    /// Invalidate the cache entry for the given path (e.g. after a write).
    fn invalidate_cache(&self, path: &Path) {
        if let Ok(mut cache) = self.item_cache.lock() {
            cache.remove(path);
        }
    }

    async fn read_item_from_path(path: &Path) -> Result<ResearchItem> {
        let content = ResearchIo::read_file(path).await?;
        let (frontmatter, _) = ResearchIo::split_frontmatter(&content);
        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        ResearchItem::from_frontmatter(&frontmatter).map_err(|e| ResearchError::Parse {
            name,
            message: e.to_string(),
        })
    }

    async fn read_body_after_frontmatter(path: &Path) -> Result<String> {
        let content = ResearchIo::read_file(path).await?;
        let (_, body) = ResearchIo::split_frontmatter(&content);
        Ok(body)
    }

    /// The in-process write lock for one research item's files.
    ///
    /// `create` publishes the dated report and its companion in two renames and
    /// `persist_frontmatter` rewrites the report in a third; a reader that
    /// arrives mid-publication, or a second writer interleaved with the first,
    /// sees a half-written item. Writers of one item take this lock, so the pair
    /// is always published as one step. Keyed by the item directory, so two
    /// managers over the same root serialise too.
    fn item_write_lock(&self, name: &ResearchName) -> Arc<tokio::sync::Mutex<()>> {
        use std::collections::HashMap;
        use std::sync::{Mutex, OnceLock};

        static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
            OnceLock::new();

        let key = ResearchIo::item_dir(&self.research_root, name);
        let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(guard.entry(key).or_default())
    }

    async fn persist_frontmatter(&self, item: &ResearchItem) -> Result<()> {
        // A full rewrite has no body to preserve yet: a caller that marks an
        // item in progress while the item is still being created (the HTTP
        // `POST /research` route runs the session in the background, and the
        // session marks the item in progress immediately after `create`)
        // would otherwise fail here with `NotFound` and leave a half-created
        // item - the directory exists but no report does.
        // One writer at a time per item: a `create` publishing the pair must not
        // interleave with this rewrite.
        let write_lock = self.item_write_lock(&item.name);
        let _write_guard = write_lock.lock().await;
        let path =
            match ResearchIo::resolve_research_md(&self.research_root, &item.name, item.created_at)
                .await
            {
                Ok(path) => path,
                Err(_) => ResearchIo::primary_research_md_path(
                    &self.research_root,
                    &item.name,
                    item.created_at,
                ),
            };
        let body = match Self::read_body_after_frontmatter(&path).await {
            Ok(body) => body,
            Err(ResearchError::Io(ResearchIoError::NotFound(_))) => String::new(),
            Err(ResearchError::Io(ResearchIoError::Io(e)))
                if e.kind() == std::io::ErrorKind::NotFound =>
            {
                String::new()
            }
            Err(e) => return Err(e),
        };
        let content = format!("{}{}", item.render_frontmatter(), body);
        ResearchIo::atomic_write(&path, &content).await?;
        Ok(())
    }

    /// Suggest up to three closest existing research names by Levenshtein
    /// distance (FR-018). Used by [`ResearchError::NotFound`] when the
    /// requested name doesn't exist.
    pub async fn suggest_closest(&self, target: &str) -> String {
        let names: Vec<String> = match self.list(true).await {
            Ok(items) => items.into_iter().map(|i| i.name.to_string()).collect(),
            Err(_) => Vec::new(),
        };
        suggest_closest_from(&names, target)
    }
}

// -- Free helpers ---------------------------------------------------------

/// Suggest up to three closest names from `candidates` to `target`. Exposed
/// as a free function so the closest-matches logic can be unit-tested
/// without touching the filesystem (T-046).
#[must_use]
pub fn suggest_closest_from(candidates: &[String], target: &str) -> String {
    if candidates.is_empty() {
        return "(no research items exist yet)".to_string();
    }
    let mut scored: Vec<(usize, &str)> = candidates
        .iter()
        .map(|c| (lev(c, target), c.as_str()))
        .collect();
    scored.sort_by_key(|(d, _)| *d);
    // Take up to three.
    let picks: Vec<&str> = scored.into_iter().take(3).map(|(_, s)| s).collect();
    picks.join(", ")
}

/// Replace the `status:` line inside an existing report frontmatter
/// block while keeping every other line (including the `sources:` count and
/// `queries:` list) intact. Falls back to rendering a fresh frontmatter if the
/// file has no frontmatter block.
fn replace_frontmatter_status_line(content: &str, status: ResearchStatus) -> String {
    let (fm_block, body) = ResearchIo::split_frontmatter(content);
    if fm_block.is_empty() {
        // "unknown" is always a valid `ResearchName`; handle the (unreachable)
        // failure explicitly instead of panicking via `unwrap` (no production
        // `unwrap`).
        return match ResearchName::new("unknown") {
            Some(name) => {
                let mut placeholder = ResearchItem::new(name, "", "");
                placeholder.set_status(status);
                format!("{}{}", placeholder.render_frontmatter(), body)
            }
            None => format!("status: {status}\n{body}"),
        };
    }

    let status_line = format!("status: {}", status.as_str());
    let mut replaced = false;
    let updated_fm: Vec<String> = fm_block
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("status:") {
                replaced = true;
                // Preserve any inline comment that might follow the status value.
                let rest = trimmed.trim_start_matches("status:").trim_start();
                let comment_start = rest.find('#').unwrap_or(rest.len());
                let comment = &rest[comment_start..];
                if comment.is_empty() {
                    status_line.clone()
                } else {
                    format!("{} {}", status_line, comment.trim())
                }
            } else {
                line.to_string()
            }
        })
        .collect();

    let updated_fm = updated_fm.join("\n");
    format!("---\n{updated_fm}\n---\n\n{body}")
}

/// Levenshtein distance implemented locally so we don't pull in extra deps
/// just for one helper.
fn lev(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = std::cmp::min(
                std::cmp::min(curr[j - 1] + 1, prev[j] + 1),
                prev[j - 1] + cost,
            );
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Extract a snippet around `byte_idx` (length `q_len`) bounded to
/// `window_chars` characters on either side.
fn extract_snippet(body: &str, byte_idx: usize, q_len: usize, window_chars: usize) -> String {
    // Convert byte indices to char indices so we don't slice mid-codepoint.
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    let start_byte = byte_idx;
    let end_byte = byte_idx + q_len;
    let start_char = chars
        .iter()
        .position(|(i, _)| *i >= start_byte)
        .unwrap_or(0);
    let end_char = chars
        .iter()
        .position(|(i, _)| *i >= end_byte)
        .unwrap_or(chars.len());
    let lo = start_char.saturating_sub(window_chars);
    let hi = (end_char + window_chars).min(chars.len());
    let lo_byte = chars.get(lo).map_or(0, |(i, _)| *i);
    let hi_byte = chars.get(hi).map_or(body.len(), |(i, _)| *i);
    let mut snippet = body[lo_byte..hi_byte].to_string();
    if lo > 0 {
        snippet = format!("...{snippet}");
    }
    if hi < chars.len() {
        snippet.push_str("...");
    }
    snippet.replace('\n', " ")
}

/// Compute the on-disk report text for a research item without
/// performing any I/O. Useful for tests and dry-run previews (T-007).
#[must_use]
pub fn render_document_for(
    name: &ResearchName,
    title: &str,
    topic: &str,
    sources: &[Source],
    summary: &str,
    queries: &[String],
) -> AssembledDocument {
    let mut item = ResearchItem::new(name.clone(), title, topic);
    item.set_queries(queries.to_vec());
    for s in sources {
        item.add_source(s.clone());
    }
    let doc = ResearchDocument {
        item,
        summary: summary.to_string(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        concepts: None,
        contradiction_graph: None,
        loci: None,
        depth_investigation: None,
        evidence_digest: None,
        triple_draft: None,
        cross_locus_reconcile: None,
        source_tensions: None,
        synthesis_audit: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        template_body: None,
        brief: None,
        decomposed_queries: queries.to_vec(),
        output_format: crate::run_config::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    assemble_document(&doc)
}

/// Convenience helper: returns the union of unique research names referenced
/// by a PLAN.md plus any names present in `existing`. Useful for surfacing
/// research links in `/spec list` (T-043).
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn union_with_existing(referenced: &[String], existing: &HashSet<String>) -> Vec<String> {
    let mut set: HashSet<String> = existing.clone();
    for n in referenced {
        set.insert(n.clone());
    }
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v
}

/// Re-export of the frontmatter timestamp type for downstream callers that
/// need to display INDEX rows.
pub type IndexTimestamp = DateTime<Utc>;

/// Compute the 1-based supporting-file index for `prefix` based on the
/// position of the current source among sources of the same type seen so
/// far. Used by `write_document` to ensure sequential numbering per type
/// (`web-01.md`, `web-02.md`, ...).
fn supporting_index(prefix: &str, prefix_sources: &[Source]) -> usize {
    prefix_sources
        .iter()
        .filter(|s| {
            matches!(
                (prefix, s),
                ("web", Source::Web { .. })
                    | ("local", Source::Local { .. })
                    | ("other", Source::Other { .. })
            )
        })
        .count()
}

#[cfg(test)]
#[path = "../tests/inline/manager_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/inline/manager_frontmatter_tests.rs"]
mod frontmatter_tests;
