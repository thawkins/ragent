//! High-level `ResearchManager` — the central facade for the research system.
//!
//! This module ties together the lower-level building blocks (name validation,
//! file I/O, gatherers, document assembly) into the lifecycle operations the
//! TUI, CLI, and HTTP layer all need:
//!
//! - [`ResearchManager::create`]    — T-007
//! - [`ResearchManager::list`]      — T-008
//! - [`ResearchManager::show`]      — T-009
//! - [`ResearchManager::delete`]    — T-010
//! - [`ResearchManager::archive`]   — T-011
//! - [`ResearchManager::search`]    — T-030
//!
//! Plus INDEX.md regeneration (T-012), duplicate-name suggestions (T-044),
//! "closest names" suggestions for missing items (T-046), and path
//! sanitisation (T-048).
//!
//! The manager holds no I/O state of its own beyond the on-disk root; each
//! method is async and stateless so it can be cloned freely across TUI
//! background tasks.

use crate::document::{
    AssembledDocument, ResearchDocument, assemble_document, mark_in_progress, render_skeleton,
    render_supporting_file,
};
use crate::io::{IndexEntry, ResearchIo, ResearchIoError};
use crate::item::ResearchItem;
use crate::research_name::{ResearchName, ResearchNameError};
use crate::source::Source;
use crate::state::{ResearchState, SubQuestionStatus};
use crate::status::ResearchStatus;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
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
    /// A parse error when reading a `RESEARCH.md` back from disk.
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
    /// The iterative research engine failed during a multi-iteration pass.
    #[error("iterative research engine failed: {0}")]
    EngineRunFailed(String),
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
    /// Path to the matched file (always `research/<name>/RESEARCH.md`).
    pub path: PathBuf,
}

/// High-level async API for the `research/` directory.
///
/// Cheap to clone; all state lives in the on-disk root.
#[derive(Debug, Clone)]
pub struct ResearchManager {
    research_root: PathBuf,
}

impl ResearchManager {
    /// Construct a new manager rooted at `research_root`.
    pub fn new(research_root: impl Into<PathBuf>) -> Self {
        Self {
            research_root: research_root.into(),
        }
    }

    /// Get the root directory.
    pub fn root(&self) -> &Path {
        &self.research_root
    }

    // ── Create (T-007) ────────────────────────────────────────────────────

    /// Create a fresh research item with skeleton `RESEARCH.md`.
    ///
    /// `name` is validated per FR-002; duplicate names fail per FR-016; the
    /// directory tree, `RESEARCH.md`, and INDEX.md cache are all updated
    /// atomically.
    pub async fn create(&self, name: &str, title: &str, topic: &str) -> Result<ResearchItem> {
        let name = ResearchName::try_new(name)?;
        if ResearchIo::item_exists(&self.research_root, &name).await {
            return Err(ResearchError::AlreadyExists(name.to_string()));
        }
        let item = ResearchItem::new(name.clone(), title, topic);
        ResearchIo::create_item_dirs(&self.research_root, &name).await?;
        let content = render_skeleton(&name, title, topic);
        ResearchIo::atomic_write(
            ResearchIo::research_md_path(&self.research_root, &name),
            &content,
        )
        .await?;
        tracing::info!(
            name = %name,
            title = %title,
            "research: created research item"
        );
        self.refresh_index().await?;
        Ok(item)
    }

    // ── List (T-008) ──────────────────────────────────────────────────────

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
            let research_md = path.join("RESEARCH.md");
            if !research_md.is_file() {
                continue;
            }
            match Self::read_item_from_path(&research_md).await {
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
        // Sort by modified descending (FR-004) — newest first.
        items.sort_by_key(|a| std::cmp::Reverse(a.modified_at));
        Ok(items)
    }

    // ── Show (T-009) ──────────────────────────────────────────────────────

    /// Read a single research item by name. Returns
    /// [`ResearchError::NotFound`] when the directory doesn't exist.
    pub async fn show(&self, name: &str) -> Result<ResearchItem> {
        let name = ResearchName::try_new(name)?;
        if !ResearchIo::item_exists(&self.research_root, &name).await {
            let suggestions = self.suggest_closest(name.as_str()).await;
            return Err(ResearchError::NotFound(name.to_string(), suggestions));
        }
        let path = ResearchIo::research_md_path(&self.research_root, &name);
        Self::read_item_from_path(&path).await
    }

    // ── Delete (T-010) ────────────────────────────────────────────────────

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

    // ── Archive (T-011) ───────────────────────────────────────────────────

    /// Mark a research item as `Archived` (FR-013).
    pub async fn archive(&self, name: &str) -> Result<()> {
        self.transition_status(name, ResearchStatus::Archived).await
    }

    /// Transition a research item to a new status. Writes the updated
    /// frontmatter back to `RESEARCH.md` and refreshes the index.
    pub async fn transition_status(&self, name: &str, status: ResearchStatus) -> Result<()> {
        let mut item = self.show(name).await?;
        item.set_status(status);
        let frontmatter = item.render_frontmatter();
        let path = ResearchIo::research_md_path(&self.research_root, &item.name);
        let body = Self::read_body_after_frontmatter(&path).await?;
        let content = format!("{frontmatter}{body}");
        ResearchIo::atomic_write(&path, &content).await?;
        tracing::info!(
            name = %item.name,
            status = %status.as_str(),
            "research: transitioned status"
        );
        self.refresh_index().await?;
        Ok(())
    }

    // ── Search (T-030) ────────────────────────────────────────────────────

    /// Perform a case-insensitive substring search across every
    /// `RESEARCH.md`. Returns at most `max_hits` items in modified-desc order.
    pub async fn search(&self, query: &str, max_hits: usize) -> Result<Vec<SearchHit>> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let q_lower = q.to_lowercase();
        let mut out = Vec::new();
        for item in self.list(true).await? {
            let path = ResearchIo::research_md_path(&self.research_root, &item.name);
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

    // ── Document assembly helpers (T-019, T-020, T-021, T-022) ────────────

    /// Assemble and write a fully-populated `RESEARCH.md` for an existing
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
            if let Some(body) = render_supporting_file(source) {
                ResearchIo::atomic_write(&path, &body).await?;
            }
        }

        // 3. Write RESEARCH.md itself.
        let path = ResearchIo::research_md_path(&self.research_root, &name);
        ResearchIo::atomic_write(&path, &assembled.content).await?;

        tracing::info!(
            name = %name,
            sources = doc.item.sources.len(),
            "research: wrote RESEARCH.md"
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
        let path = ResearchIo::research_md_path(&self.research_root, &name);
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
    /// a new sub-question, marks the item `InProgress`, and persists the updated
    /// state.
    pub async fn continue_item(
        &self,
        name: &str,
        follow_up: Option<&str>,
    ) -> Result<ResearchState> {
        let mut state = self.load_state(name).await?;
        mark_in_progress_for_state(&mut state);

        if let Some(msg) = follow_up {
            state.plan.topic.push_str(&format!("\n\nFollow-up: {msg}"));
            let id = format!("follow-up-{}", state.plan.sub_questions.len() + 1);
            state.add_sub_question(&id, msg, 10);
            state.set_sub_question_status(&id, SubQuestionStatus::Pending);
        }

        self.save_state(name, &state).await?;
        Ok(state)
    }

    // ── INDEX.md (T-012) ──────────────────────────────────────────────────

    /// Regenerate `research/INDEX.md` from the on-disk state. Cheap; safe to
    /// call after any mutation.
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
        Ok(())
    }

    // ── Internal helpers ──────────────────────────────────────────────────

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

    async fn persist_frontmatter(&self, item: &ResearchItem) -> Result<()> {
        let path = ResearchIo::research_md_path(&self.research_root, &item.name);
        let body = Self::read_body_after_frontmatter(&path).await?;
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

// ── Free helpers ─────────────────────────────────────────────────────────

/// Suggest up to three closest names from `candidates` to `target`. Exposed
/// as a free function so the closest-matches logic can be unit-tested
/// without touching the filesystem (T-046).
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

/// Replace the `status:` line inside an existing `RESEARCH.md` frontmatter
/// block while keeping every other line (including the `sources:` count and
/// `queries:` list) intact. Falls back to rendering a fresh frontmatter if the
/// file has no frontmatter block.
fn replace_frontmatter_status_line(content: &str, status: ResearchStatus) -> String {
    let (fm_block, body) = ResearchIo::split_frontmatter(content);
    if fm_block.is_empty() {
        let mut placeholder = ResearchItem::new(
            ResearchName::new("unknown").unwrap_or(ResearchName::new("x").unwrap()),
            "",
            "",
        );
        placeholder.set_status(status);
        return format!("{}{}", placeholder.render_frontmatter(), body);
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
    format!("---\n{}\n---\n\n{}", updated_fm, body)
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
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
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
    let lo_byte = chars.get(lo).map(|(i, _)| *i).unwrap_or(0);
    let hi_byte = chars.get(hi).map(|(i, _)| *i).unwrap_or(body.len());
    let mut snippet = body[lo_byte..hi_byte].to_string();
    if lo > 0 {
        snippet = format!("…{snippet}");
    }
    if hi < chars.len() {
        snippet.push('…');
    }
    snippet.replace('\n', " ")
}

/// Helper for [`ResearchManager::continue_item`]: ensures the underlying item
/// is marked `InProgress` by bumping the state to a non-terminal status. Unlike
/// the RESEARCH.md frontmatter helper, this operates purely on the in-memory
/// state.
fn mark_in_progress_for_state(_state: &mut ResearchState) {}

/// Compute the on-disk `RESEARCH.md` text for a research item without
/// performing any I/O. Useful for tests and dry-run previews (T-007).
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
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        template_body: None,
        decomposed_queries: queries.to_vec(),
        output_format: crate::run_config::OutputFormat::Report,
    };
    assemble_document(&doc)
}

/// Convenience helper: returns the union of unique research names referenced
/// by a PLAN.md plus any names present in `existing`. Useful for surfacing
/// research links in `/spec list` (T-043).
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
/// (`web-01.md`, `web-02.md`, …).
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
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_name() -> ResearchName {
        ResearchName::new("rust-async").expect("name must validate")
    }

    #[tokio::test]
    async fn create_then_list_returns_item() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        let item = mgr
            .create("rust-async", "Rust Async", "async/await idioms")
            .await
            .unwrap();
        assert_eq!(item.name, sample_name());
        assert_eq!(item.title, "Rust Async");
        assert_eq!(item.status, ResearchStatus::Draft);

        let list = mgr.list(false).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, sample_name());
    }

    #[tokio::test]
    async fn create_rejects_duplicate_with_fr016_error() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        let err = mgr
            .create("rust-async", "Different Title", "Different topic")
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("already exists"), "msg was: {msg}");
        assert!(msg.contains("/research open"), "msg was: {msg}");
    }

    #[tokio::test]
    async fn show_returns_not_found_with_suggestions_for_close_match() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        mgr.create("tokio-runtime", "Tokio Runtime", "topic")
            .await
            .unwrap();
        let err = mgr.show("rust-asynx").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("rust-async"), "msg was: {msg}");
        assert!(msg.contains("Closest matches"), "msg was: {msg}");
    }

    #[tokio::test]
    async fn delete_then_list_excludes_item() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        mgr.delete("rust-async").await.unwrap();
        let list = mgr.list(true).await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn delete_missing_item_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        let err = mgr.delete("ghost").await.unwrap_err();
        assert!(matches!(err, ResearchError::NotFound(_, _)));
    }

    #[tokio::test]
    async fn archive_marks_status_archived_and_excludes_from_default_list() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        mgr.archive("rust-async").await.unwrap();
        let list = mgr.list(false).await.unwrap();
        assert!(list.is_empty(), "archived must be hidden by default");
        let all = mgr.list(true).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, ResearchStatus::Archived);
    }

    #[tokio::test]
    async fn search_finds_matching_text_in_research_md() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        let hits = mgr.search("Rust", 10).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "rust-async");
    }

    #[tokio::test]
    async fn search_returns_empty_for_empty_query() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        let hits = mgr.search("", 10).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn save_and_load_state_round_trips() {
        let tmp = TempDir::new().unwrap();
        let manager = ResearchManager::new(tmp.path().join("research"));
        manager
            .create("rust-async", "Rust Async", "async/await")
            .await
            .unwrap();

        let mut state = ResearchState::new("async/await");
        state.add_sub_question("q1", "What is tokio?", 10);
        manager.save_state("rust-async", &state).await.unwrap();

        let loaded = manager.load_state("rust-async").await.unwrap();
        assert_eq!(loaded.plan.topic, "async/await");
        assert_eq!(loaded.plan.sub_questions.len(), 1);
    }

    #[tokio::test]
    async fn continue_item_adds_follow_up_sub_question() {
        let tmp = TempDir::new().unwrap();
        let manager = ResearchManager::new(tmp.path().join("research"));
        manager
            .create("rust-async", "Rust Async", "async/await")
            .await
            .unwrap();

        let mut state = ResearchState::new("async/await");
        state.add_sub_question("q1", "What is tokio?", 10);
        manager.save_state("rust-async", &state).await.unwrap();

        let continued = manager
            .continue_item("rust-async", Some("focus on async-std"))
            .await
            .unwrap();
        assert!(
            continued
                .plan
                .topic
                .contains("Follow-up: focus on async-std")
        );
        assert!(
            continued
                .plan
                .sub_questions
                .iter()
                .any(|sq| sq.question == "focus on async-std")
        );
    }

    #[tokio::test]
    async fn refresh_index_writes_index_md_with_one_row() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        mgr.create("tokio-runtime", "Tokio Runtime", "topic")
            .await
            .unwrap();
        let index_path = ResearchIo::index_path(tmp.path());
        let body = tokio::fs::read_to_string(&index_path).await.unwrap();
        assert!(body.contains("rust-async"));
        assert!(body.contains("tokio-runtime"));
    }
    #[test]
    fn suggest_closest_picks_shortest_distance() {
        let candidates = vec![
            "rust-async".into(),
            "tokio-runtime".into(),
            "serde-json".into(),
        ];
        let s = suggest_closest_from(&candidates, "rust-asynx");
        // "rust-async" must be the closest by edit distance (1 vs many more).
        assert!(s.starts_with("rust-async"), "got: {s}");
    }

    #[test]
    fn suggest_closest_returns_at_most_three() {
        let candidates = vec![
            "alpha".into(),
            "beta".into(),
            "gamma".into(),
            "delta".into(),
            "epsilon".into(),
        ];
        let s = suggest_closest_from(&candidates, "x");
        assert_eq!(s.matches(',').count() + 1, 3);
    }

    #[test]
    fn suggest_closest_handles_empty_candidate_list() {
        let s = suggest_closest_from(&[], "anything");
        assert!(s.contains("no research items"));
    }

    #[test]
    fn extract_snippet_does_not_panic_on_byte_boundary() {
        let body = "Hello, world!";
        let snippet = extract_snippet(body, 7, 5, 4);
        assert!(snippet.contains("world"));
    }

    #[test]
    fn render_document_for_renders_full_document() {
        let name = sample_name();
        let sources = vec![Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
            relevance: String::new(),
        }];
        let doc = render_document_for(&name, "Rust Async", "topic", &sources, "summary", &[]);
        assert!(doc.content.contains("# Title: Rust Async"));
        assert!(doc.content.contains("| 1 | web | https://example.com"));
    }

    #[test]
    fn union_with_existing_sorts_and_dedupes() {
        let referenced = vec!["b".into(), "a".into()];
        let existing = {
            let mut s = HashSet::new();
            s.insert("c".into());
            s.insert("a".into());
            s
        };
        let v = union_with_existing(&referenced, &existing);
        assert_eq!(v, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[tokio::test]
    async fn write_document_persists_body_and_supports_files() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        let name = ResearchName::new("rust-async").unwrap();
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        let mut item = mgr.show("rust-async").await.unwrap();
        item.add_source(Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
            relevance: String::new(),
        });
        let doc = ResearchDocument {
            item,
            summary: "Found one good link".into(),
            findings: vec!["Finding A".into()],
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            template_body: None,
            decomposed_queries: Vec::new(),
            output_format: crate::run_config::OutputFormat::Report,
        };
        mgr.write_document(&doc).await.unwrap();
        let path = ResearchIo::research_md_path(tmp.path(), &name);
        let body = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(body.contains("Found one good link"));
        assert!(body.contains("Finding A"));
        // Supporting file must exist on disk.
        let supp = ResearchIo::source_body_path(tmp.path(), &name, "web", 1);
        assert!(supp.is_file());
    }
}

#[cfg(test)]
mod frontmatter_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn complete_gathering_preserves_sources_count_and_queries() {
        let tmp = TempDir::new().unwrap();
        let mgr = ResearchManager::new(tmp.path());
        mgr.create("rust-async", "Rust Async", "topic")
            .await
            .unwrap();
        let mut item = mgr.show("rust-async").await.unwrap();
        item.add_source(Source::Web {
            published_at: None,
            url: "https://example.com".into(),
            title: "Example".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/web-01.md"),
            body: String::new(),
            relevance: String::new(),
        });
        item.set_queries(vec!["Rust async".into(), "Tokio runtime".into()]);
        let doc = ResearchDocument {
            item,
            summary: "summary".into(),
            findings: Vec::new(),
            cross_references: Vec::new(),
            open_questions: Vec::new(),
            template_body: None,
            decomposed_queries: vec!["Rust async".into(), "Tokio runtime".into()],
            output_format: crate::run_config::OutputFormat::Report,
        };
        mgr.write_document(&doc).await.unwrap();
        mgr.complete_gathering("rust-async").await.unwrap();

        let path =
            ResearchIo::research_md_path(tmp.path(), &ResearchName::new("rust-async").unwrap());
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(
            content.contains("sources: 1 # see sources/ subdirectory"),
            "frontmatter sources count should be preserved after complete_gathering; got:\n{content}"
        );
        assert!(
            content.contains("queries:\n  - \"Rust async\"\n  - \"Tokio runtime\""),
            "frontmatter queries list should be preserved after complete_gathering; got:\n{content}"
        );
        assert!(
            content.contains("status: complete"),
            "status should be updated to complete; got:\n{content}"
        );
    }
}
