//! Atomic file I/O for the research system (T-006, T-013).
//!
//! The research system writes many small files under `research/<name>/` and
//! `research/INDEX.md`. Every write goes through [`ResearchIo::atomic_write`]
//! which performs a write-then-rename so readers never see a partially
//! written file. This is the same guarantee `SpecIo` provides for specs and
//! is required by NFR-002.
//!
//! ## Layout
//!
//! ```text
//! research/
//! |-- INDEX.md                       (derived cache, regenerated on change)
//! |-- <name>/
//! |   |-- RESEARCH-<ISO date>.md     (frontmatter + body, 8 sections)
//! |   |-- CORPA-<ISO date>.md        (QA companion)
//! |   |-- RESEARCH.md                (only when the dated name is not unique)
//! |   \-- sources/
//! |       |-- web-NN.md
//! |       |-- local-NN.md
//! |       |-- spec-NN.md
//! |       \-- other-NN.md
//! \-- _templates/                    (optional research templates, FR-020)
//!     \-- <name>.md
//! ```
//!
//! ## Dated output files
//!
//! Every run writes its report to `RESEARCH-YYYY-MM-DD.md` and its companion to
//! `CORPA-YYYY-MM-DD.md`, so a re-run of an existing item (`/research update`)
//! keeps the earlier report instead of overwriting it (see
//! [`ResearchIo::dated_research_file_name`]).

use crate::research_name::ResearchName;
use crate::source::Source;
use chrono::{DateTime, Utc};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;

/// The `YYYY-MM-DD` stamp used in the dated output file names.
///
/// `created_at` is the item's creation instant carried in its `RESEARCH.md`
/// frontmatter, so the primary report of an item is named for the day the
/// research was created and every later run (same day or not) still points at
/// one stable, existing file.
#[must_use]
pub fn dated_stamp(created_at: DateTime<Utc>) -> String {
    created_at.format("%Y-%m-%d").to_string()
}

/// `RESEARCH-<stamp>.md` - the dated report file name for `stamp`.
#[must_use]
pub fn dated_research_file_name(stamp: &str) -> String {
    format!("RESEARCH-{stamp}.md")
}

/// `CORPA-<stamp>.md` - the dated companion file name for `stamp`.
#[must_use]
pub fn dated_corpa_file_name(stamp: &str) -> String {
    format!("CORPA-{stamp}.md")
}

/// Pick the report file name from the entries in an item directory.
///
/// Dated reports (`RESEARCH-YYYY-MM-DD.md`) win over the legacy undated
/// `RESEARCH.md`, and among several dated reports the lexicographically
/// greatest name wins - the ISO date prefix sorts chronologically, so that is
/// the most recent run. Returns `None` when the directory holds no report.
#[must_use]
pub fn pick_research_file_name(names: &[String]) -> Option<String> {
    names
        .iter()
        .filter(|n| is_dated_research_file_name(n))
        .max()
        .or_else(|| {
            names
                .iter()
                .find(|n| n.as_str() == LEGACY_RESEARCH_FILE_NAME)
        })
        .cloned()
}

/// Whether `name` is a dated report file name (`RESEARCH-YYYY-MM-DD.md`).
#[must_use]
pub fn is_dated_research_file_name(name: &str) -> bool {
    name.strip_prefix("RESEARCH-")
        .and_then(|rest| rest.strip_suffix(".md"))
        .is_some_and(is_iso_date_component)
}

/// Whether `name` is a dated companion file name (`CORPA-YYYY-MM-DD.md`).
#[must_use]
pub fn is_dated_corpa_file_name(name: &str) -> bool {
    name.strip_prefix("CORPA-")
        .and_then(|rest| rest.strip_suffix(".md"))
        .is_some_and(is_iso_date_component)
}

/// Whether `s` is exactly an ISO `YYYY-MM-DD` date component.
fn is_iso_date_component(s: &str) -> bool {
    s.len() == 10
        && s.as_bytes().iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// The undated report file name used before the dated scheme, still accepted
/// for items created by an older ragent.
pub const LEGACY_RESEARCH_FILE_NAME: &str = "RESEARCH.md";

/// The undated companion file name used before the dated scheme.
pub const LEGACY_CORPA_FILE_NAME: &str = "CORPA.md";

/// Whether `name` is safe to interpolate into a template file path.
///
/// SEC-ragent-research-001 (SECTASKS T-014): a template name must be a single
/// plain component drawn from `[A-Za-z0-9_-]`. Empty names, separators,
/// `..`, drive prefixes, and absolute components are all rejected. This is
/// the same shape as `source_vault::validate_run_tag`, applied to a value
/// that is never used as a *path* but is always *joined into* one.
#[must_use]
pub fn is_valid_template_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Errors emitted by the research I/O layer.
#[derive(Debug, Error)]
pub enum ResearchIoError {
    /// The on-disk research item does not exist where it was expected.
    #[error("research item not found: {0}")]
    NotFound(String),
    /// The on-disk research item already exists (FR-016 duplicate-create).
    #[error("research item already exists: {0}")]
    AlreadyExists(String),
    /// A wrapping I/O error from the underlying filesystem call.
    #[error("research I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A JSON (de)serialization error from frontmatter round-trips.
    #[error("research I/O serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Result alias for research I/O operations.
pub type Result<T> = std::result::Result<T, ResearchIoError>;

/// Helpers for the on-disk layout under `research/`.
#[derive(Debug, Clone)]
pub struct ResearchIo;

impl ResearchIo {
    /// Compute the absolute path of a research item directory.
    #[must_use]
    pub fn item_dir(research_root: &Path, name: &ResearchName) -> PathBuf {
        research_root.join(name.dir_name())
    }

    /// Compute the path of the dated `RESEARCH-<stamp>.md` file for a research
    /// item, where `stamp` is the ISO date of the run being written.
    ///
    /// The default output file name is the one for the item's own creation
    /// date ([`Self::primary_research_md_path`]); a re-run passes its own
    /// run date so every run lands in a file of its own.
    #[must_use]
    pub fn dated_research_md_path(
        research_root: &Path,
        name: &ResearchName,
        stamp: &str,
    ) -> PathBuf {
        Self::item_dir(research_root, name).join(dated_research_file_name(stamp))
    }

    /// Compute the path of the dated `CORPA-<stamp>.md` companion file for a
    /// research item, alongside [`Self::dated_research_md_path`].
    #[must_use]
    pub fn dated_corpa_md_path(research_root: &Path, name: &ResearchName, stamp: &str) -> PathBuf {
        Self::item_dir(research_root, name).join(dated_corpa_file_name(stamp))
    }

    /// Compute the path of an item's canonical report file for the given
    /// creation instant: `RESEARCH-<YYYY-MM-DD>.md`.
    ///
    /// This is the name [`crate::manager::ResearchManager::create`] writes, so
    /// an item created before any run can still be opened by the same rule
    /// that opens it after one.
    #[must_use]
    pub fn primary_research_md_path(
        research_root: &Path,
        name: &ResearchName,
        created_at: DateTime<Utc>,
    ) -> PathBuf {
        Self::dated_research_md_path(research_root, name, &dated_stamp(created_at))
    }

    /// Compute the path of an item's canonical companion file for the given
    /// creation instant: `CORPA-<YYYY-MM-DD>.md`.
    #[must_use]
    pub fn primary_corpa_md_path(
        research_root: &Path,
        name: &ResearchName,
        created_at: DateTime<Utc>,
    ) -> PathBuf {
        Self::dated_corpa_md_path(research_root, name, &dated_stamp(created_at))
    }

    /// Locate an item's report file, whether it uses the dated naming scheme
    /// (`RESEARCH-YYYY-MM-DD.md`, most recent first) or the legacy undated
    /// `RESEARCH.md`.
    ///
    /// # Errors
    ///
    /// Returns [`ResearchIoError::NotFound`] when the item directory holds no
    /// report file, and [`ResearchIoError::Io`] when the directory itself
    /// cannot be read.
    pub async fn find_research_md(research_root: &Path, name: &ResearchName) -> Result<PathBuf> {
        let dir = Self::item_dir(research_root, name);
        let mut read_dir = fs::read_dir(&dir).await.map_err(ResearchIoError::Io)?;
        let mut names = Vec::new();
        while let Some(entry) = read_dir.next_entry().await.map_err(ResearchIoError::Io)? {
            if let Some(file_name) = entry.file_name().to_str() {
                names.push(file_name.to_string());
            }
        }
        match pick_research_file_name(&names) {
            Some(file_name) => Ok(dir.join(file_name)),
            None => Err(ResearchIoError::NotFound(name.to_string())),
        }
    }

    /// Locate an item's report file from the creation instant recorded in its
    /// frontmatter, falling back to a directory scan.
    ///
    /// The canonical name is checked first: it is what
    /// [`crate::manager::ResearchManager::create`] publishes and what a re-run
    /// leaves alone, so the common case costs one `stat`. The scan covers an
    /// item whose oldest surviving report is dated differently (a re-run never
    /// removes a dated file) and one created before the dated scheme.
    ///
    /// # Errors
    ///
    /// As [`Self::find_research_md`].
    pub async fn resolve_research_md(
        research_root: &Path,
        name: &ResearchName,
        created_at: DateTime<Utc>,
    ) -> Result<PathBuf> {
        let canonical = Self::primary_research_md_path(research_root, name, created_at);
        if fs::try_exists(&canonical).await.unwrap_or(false) {
            return Ok(canonical);
        }
        Self::find_research_md(research_root, name).await
    }

    /// Locate an item's companion file, whether it uses the dated naming
    /// scheme (`CORPA-YYYY-MM-DD.md`, most recent first) or the legacy undated
    /// `CORPA.md`.
    ///
    /// # Errors
    ///
    /// Returns [`ResearchIoError::NotFound`] when the item directory holds no
    /// companion file, and [`ResearchIoError::Io`] when the directory itself
    /// cannot be read.
    pub async fn find_corpa_md(research_root: &Path, name: &ResearchName) -> Result<PathBuf> {
        let dir = Self::item_dir(research_root, name);
        let mut read_dir = fs::read_dir(&dir).await.map_err(ResearchIoError::Io)?;
        let mut names = Vec::new();
        while let Some(entry) = read_dir.next_entry().await.map_err(ResearchIoError::Io)? {
            if let Some(file_name) = entry.file_name().to_str() {
                names.push(file_name.to_string());
            }
        }
        let picked = names
            .iter()
            .filter(|n| is_dated_corpa_file_name(n))
            .max()
            .or_else(|| names.iter().find(|n| n.as_str() == LEGACY_CORPA_FILE_NAME))
            .cloned();
        match picked {
            Some(file_name) => Ok(dir.join(file_name)),
            None => Err(ResearchIoError::NotFound(name.to_string())),
        }
    }

    /// Locate an item's companion file from the creation instant recorded in
    /// its frontmatter, falling back to a directory scan.
    ///
    /// # Errors
    ///
    /// As [`Self::find_corpa_md`].
    pub async fn resolve_corpa_md(
        research_root: &Path,
        name: &ResearchName,
        created_at: DateTime<Utc>,
    ) -> Result<PathBuf> {
        let canonical = Self::primary_corpa_md_path(research_root, name, created_at);
        if fs::try_exists(&canonical).await.unwrap_or(false) {
            return Ok(canonical);
        }
        Self::find_corpa_md(research_root, name).await
    }

    /// Compute the path of a research item's companion file.
    ///
    /// Since the dated scheme this is the *legacy* undated name
    /// ([`LEGACY_CORPA_FILE_NAME`]); use [`Self::resolve_corpa_md`] or
    /// [`Self::find_corpa_md`] to locate the companion that actually exists.
    #[must_use]
    pub fn corpa_md_path(research_root: &Path, name: &ResearchName) -> PathBuf {
        Self::item_dir(research_root, name).join(LEGACY_CORPA_FILE_NAME)
    }

    /// Compute the path of the `CONCEPTS.md` file produced by the `cluster`
    /// sub-command for a research item (FR-007).
    #[must_use]
    pub fn concepts_md_path(research_root: &Path, name: &ResearchName) -> PathBuf {
        Self::item_dir(research_root, name).join("CONCEPTS.md")
    }

    /// Compute the path of the `sources/` subdirectory for a research item.
    #[must_use]
    pub fn sources_dir(research_root: &Path, name: &ResearchName) -> PathBuf {
        Self::item_dir(research_root, name).join("sources")
    }

    /// Compute the path of a numbered supporting file under `sources/`.
    ///
    /// `prefix` is one of `"web"`, `"local"`, `"spec"`, or `"other"`. Index
    /// is 1-based for human-friendly filenames (`web-01.md`).
    #[must_use]
    pub fn source_body_path(
        research_root: &Path,
        name: &ResearchName,
        prefix: &str,
        index: usize,
    ) -> PathBuf {
        Self::sources_dir(research_root, name).join(format!("{prefix}-{index:02}.md"))
    }

    /// Compute the path of the `research/_templates/<name>.md` template file.
    ///
    /// Returns `None` when `template_name` is not a single plain
    /// `[A-Za-z0-9_-]` path component (SEC-ragent-research-001 / SECTASKS
    /// T-014). The template name is caller/LLM-controlled (CLI `--template`,
    /// TUI flag, `POST /research`), and `Path::join` discards the base for an
    /// absolute component, so an unvalidated name would read any `.md` file
    /// the process can reach (`../../../../home/u/.config/notes`).
    #[must_use]
    pub fn template_path(research_root: &Path, template_name: &str) -> Option<PathBuf> {
        if !is_valid_template_name(template_name) {
            tracing::warn!(
                template = %template_name,
                "research: rejecting template name that is not a single \
                 [A-Za-z0-9_-] component"
            );
            return None;
        }
        Some(
            research_root
                .join("_templates")
                .join(format!("{template_name}.md")),
        )
    }

    /// Compute the path of the per-item serialized state file (`state.json`).
    #[must_use]
    pub fn state_json_path(research_root: &Path, name: &ResearchName) -> PathBuf {
        Self::item_dir(research_root, name).join("state.json")
    }

    /// Write a `ResearchState` to the per-item `state.json` file atomically.
    pub async fn write_state(
        research_root: &Path,
        name: &ResearchName,
        state: &crate::state::ResearchState,
    ) -> Result<()> {
        let path = Self::state_json_path(research_root, name);
        let json = serde_json::to_string_pretty(state).map_err(ResearchIoError::Serde)?;
        Self::atomic_write(&path, &json).await
    }

    /// Read a `ResearchState` from the per-item `state.json` file.
    pub async fn read_state(
        research_root: &Path,
        name: &ResearchName,
    ) -> Result<crate::state::ResearchState> {
        let path = Self::state_json_path(research_root, name);
        let json = fs::read_to_string(&path)
            .await
            .map_err(ResearchIoError::Io)?;
        serde_json::from_str(&json).map_err(ResearchIoError::Serde)
    }

    /// Compute the path of the global index file.
    #[must_use]
    pub fn index_path(research_root: &Path) -> PathBuf {
        research_root.join("INDEX.md")
    }

    /// Compute the path of the derived `research/.index.json` search cache
    /// (Milestone G). This is a machine-readable cache regenerated alongside
    /// `INDEX.md` on every mutation; it is **not** meant for human editing.
    #[must_use]
    pub fn cache_path(research_root: &Path) -> PathBuf {
        research_root.join(".index.json")
    }

    /// Write `content` to `path` atomically (write to a sibling temp file, then
    /// rename it onto `path`).
    ///
    /// The temp name carries the process id and a counter, not just the
    /// extension: two writers of the same path in one process (a `create` and a
    /// concurrent `refresh_index`, or the same item written from two tasks)
    /// would otherwise share one `<path>.tmp` and their renames would race -
    /// the loser's `rename` fails because the winner already moved the file,
    /// and the losing write is lost with an I/O error. `path.with_extension
    /// ("tmp")` also collapses a dotted file name (`item.v1.md` ->
    /// `item.v1.tmp`), so the name is built from the file name instead.
    pub async fn atomic_write(path: impl AsRef<Path>, content: &str) -> Result<()> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let file_name = path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("tmp");
        let tmp = path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&tmp, content).await?;
        match fs::rename(&tmp, path).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Best-effort cleanup of the leftover .tmp file so a later
                // retry doesn't trip over a stale inode.
                let _ = fs::remove_file(&tmp).await; // INTENTIONAL: best-effort temp cleanup
                Err(ResearchIoError::Io(e))
            }
        }
    }
    /// Read a file to a `String`.
    pub async fn read_file(path: impl AsRef<Path>) -> Result<String> {
        Ok(fs::read_to_string(path).await?)
    }

    /// `true` if a research item directory exists under `research_root`.
    pub async fn item_exists(research_root: &Path, name: &ResearchName) -> bool {
        Self::item_dir(research_root, name).is_dir()
    }

    /// Create the per-item directory tree (item dir + `sources/`).
    pub async fn create_item_dirs(research_root: &Path, name: &ResearchName) -> Result<()> {
        fs::create_dir_all(Self::sources_dir(research_root, name)).await?;
        Ok(())
    }

    /// Recursively remove a research item directory.
    pub async fn remove_item(research_root: &Path, name: &ResearchName) -> Result<()> {
        let dir = Self::item_dir(research_root, name);
        if !dir.exists() {
            return Err(ResearchIoError::NotFound(name.to_string()));
        }
        fs::remove_dir_all(&dir).await?;
        Ok(())
    }

    /// Read a `RESEARCH.md` and split it into the frontmatter block + body.
    ///
    /// The frontmatter is the leading YAML between `---` fences; the body is
    /// everything after the closing `---`. If the file has no frontmatter,
    /// the body is the whole file and `frontmatter` is empty.
    #[must_use]
    pub fn split_frontmatter(content: &str) -> (String, String) {
        // Accept either:
        //   ---             (leading, immediately followed by newline)
        //   ---\n...\n---\n
        //   ...\n---\n...\n---\n...
        //
        // The first `---` opens the block; the next `---` on its own line
        // closes it. We anchor on a newline so `---` inside a YAML scalar
        // (e.g. `sources: 0 # see sources/ --- subdirectory`) doesn't break
        // parsing.
        if let Some(start_rel) = find_yaml_open(content) {
            let after_open = start_rel + 4; // length of "---\n"
            if let Some(close_rel) = find_yaml_close(&content[after_open..]) {
                let fm = content[after_open..after_open + close_rel]
                    .trim()
                    .to_string();
                let body = content[after_open + close_rel + 4..]
                    .trim_start_matches('\n')
                    .to_string();
                return (fm, body);
            }
        }
        (String::new(), content.to_string())
    }

    /// Render the References Index markdown table that appears at the
    /// bottom of every `RESEARCH.md` (FR-011).
    ///
    /// `captured_at` is the timestamp shown for the "No sources captured"
    /// placeholder; pass `Utc::now()` when generating fresh output.
    ///
    /// When `cloak_urls` is `true` the `Path/URL` cell of every web source is
    /// defanged with [`cloak_url`] so the emitted URL is plain text rather
    /// than a clickable link (`/research create --url-cloak`).
    #[must_use]
    pub fn render_references_index(
        sources: &[Source],
        captured_at: DateTime<Utc>,
        cloak_urls: bool,
    ) -> String {
        let mut out = String::from("## References Index\n\n");
        out.push_str(&Self::render_references_index_table(
            sources,
            captured_at,
            cloak_urls,
        ));
        out
    }

    /// Render the references table body without the `## References Index`
    /// heading, for reuse by companion documents (e.g. the `Sources Reference`
    /// copy in `CORPA.md`).
    ///
    /// An empty source list still renders the table with the "No sources
    /// captured" placeholder row so the document structure is stable for
    /// downstream tooling.
    ///
    /// Column semantics:
    ///
    /// - **Published** shows each web source's publication date when it could
    ///   be parsed from the page's embedded metadata, and `-` otherwise.
    ///   Non-web sources have no publication date and always show `-`.
    /// - **Media** shows the classified media type for web sources (`page`,
    ///   `pdf`, or `youtube`), and `-` for non-web sources.
    /// - **Language** shows the human language detected from each web source's
    ///   extracted text (e.g. `English`, `French`), and `-` for non-web
    ///   sources or when detection was inconclusive.
    /// - **Author** shows the author extracted from the page's embedded
    ///   metadata, and `-` for non-web sources or when no author was detected.
    ///
    /// When `cloak_urls` is `true`, web-source `Path/URL` values are defanged
    /// via [`cloak_url`] so the table emits them as non-clickable text.
    #[must_use]
    pub fn render_references_index_table(
        sources: &[Source],
        captured_at: DateTime<Utc>,
        cloak_urls: bool,
    ) -> String {
        if sources.is_empty() {
            return format!(
                "| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |\n\
                            |---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|\n\
                            | 1 | other | - | - | - | No sources captured | - | - | (no gathering run) | - | - | {} |\n",
                captured_at.to_rfc3339()
            );
        }
        let mut out = String::from(
            "| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |\n\
             |---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|\n",
        );
        for (idx, source) in sources.iter().enumerate() {
            let n = idx + 1;
            let kind = source.type_str();
            let media = sanitize_inline(source.media_type());
            let language = match source.language() {
                Some(l) if !l.is_empty() => sanitize_inline(l),
                _ => "-".to_string(),
            };
            let path = if cloak_urls && matches!(source, Source::Web { .. }) {
                cloak_url(source.path_or_url())
            } else {
                sanitize_inline(source.path_or_url())
            };
            let title = sanitize_inline(source.title());
            let author = match source.author() {
                Some(a) if !a.is_empty() => format!("[{}]", sanitize_inline(a)),
                _ => "-".to_string(),
            };
            let published = source
                .published_at()
                .map_or_else(|| "-".to_string(), |dt| dt.format("%Y-%m-%d").to_string());
            let relevance = match source {
                Source::Local { relevance, .. } => sanitize_inline(relevance),
                Source::Spec { relevance, .. } if !relevance.is_empty() => {
                    sanitize_inline(relevance)
                }
                Source::Web { relevance, .. } if !relevance.is_empty() => {
                    sanitize_inline(relevance)
                }
                _ => "-".to_string(),
            };
            let (search_tool, search_engine) = match source {
                Source::Web {
                    search_tool,
                    search_engine,
                    ..
                } => (sanitize_inline(search_tool), sanitize_inline(search_engine)),
                _ => ("-".to_string(), "-".to_string()),
            };
            let captured = source.captured_at().to_rfc3339();
            let _ = writeln!(
                // INTENTIONAL: write to a String buffer is infallible
                out,
                "| {n} | {kind} | {media} | {language} | {path} | {title} | {author} | {published} | {relevance} | {search_tool} | {search_engine} | {captured} |"
            );
        }
        out
    }

    /// Render the global `research/INDEX.md` derived cache (FR-012).
    ///
    /// `items` is the list of items to include; pass an empty slice to render
    /// the "no research yet" placeholder. Items are emitted in the order
    /// they are passed - callers are responsible for sorting.
    #[must_use]
    pub fn render_index(items: &[IndexEntry]) -> String {
        if items.is_empty() {
            return String::from(
                "# Research Index\n\n\
                 No research items yet. Use `/research <name> <topic>` to start one.\n",
            );
        }
        let mut out = String::from(
            "# Research Index\n\n\
             Derived cache of every research item on disk. \
             This file is regenerated on every change - do not edit by hand.\n\n\
             | Name | Title | Status | Created (UTC) | Modified (UTC) |\n\
             |------|-------|--------|---------------|----------------|\n",
        );
        for item in items {
            let _ = writeln!(
                // INTENTIONAL: write to a String buffer is infallible
                out,
                "| {} | {} | {} | {} | {} |",
                item.name,
                sanitize_inline(&item.title),
                item.status.as_str(),
                item.created_at.to_rfc3339(),
                item.modified_at.to_rfc3339(),
            );
        }
        out.push_str(&format!(
            "\n_Generated {} * {} items._\n",
            Utc::now().to_rfc3339(),
            items.len()
        ));
        out
    }
}

/// One row of the global `research/INDEX.md` cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// Research name (also the directory name).
    pub name: String,
    /// Human-readable title.
    pub title: String,
    /// Current lifecycle status.
    pub status: crate::status::ResearchStatus,
    /// UTC timestamp at which the item was created.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp of the most recent write.
    pub modified_at: DateTime<Utc>,
}

/// Defang a URL so automated URL scanners that read `RESEARCH.md` do not
/// treat it as a live, clickable link, while keeping it human-readable.
///
/// The transformation is the classic "hxxp" obfuscation: the scheme is
/// rewritten (`https://` -> `hxxps://`, `http://` -> `hxxp://`) and every dot
/// in the authority/path is bracketed (`example.com` -> `example[.]com`).
/// The result is wrapped in a Markdown code span so renderers show it as
/// literal text rather than an auto-link.
///
/// Values that are not URLs (local file paths, spec ids, labels) are returned
/// after `sanitize_inline` escaping so non-web rows in the References Index
/// render exactly as before and a scheme-less value still cannot break a table
/// row.
///
/// Used by the `/research create --url-cloak` output path for the `Sources`
/// bullets and the `References Index` / `Sources Reference` tables.
#[must_use]
pub fn cloak_url(value: &str) -> String {
    let body = match strip_scheme(value, "https://") {
        Some(rest) => format!("hxxps://{rest}"),
        None => match strip_scheme(value, "http://") {
            Some(rest) => format!("hxxp://{rest}"),
            None => return sanitize_inline(value),
        },
    };
    let mut cloaked = String::with_capacity(body.len() + 16);
    for ch in body.chars() {
        match ch {
            '.' => cloaked.push_str("[.]"),
            // Escape table-cell separators so a cloaked URL embedded in the
            // References Index table cannot break the row.
            '|' => cloaked.push_str(r"\|"),
            '\n' | '\r' => cloaked.push(' '),
            _ => cloaked.push(ch),
        }
    }
    format!("`{cloaked}`")
}

/// Case-insensitively strip `scheme` from the front of `url`, returning the
/// remainder. Returns `None` when `url` does not start with `scheme` or the
/// prefix is not a valid UTF-8 boundary.
fn strip_scheme<'a>(url: &'a str, scheme: &str) -> Option<&'a str> {
    let head = url.get(..scheme.len())?;
    if head.eq_ignore_ascii_case(scheme) {
        url.get(scheme.len()..)
    } else {
        None
    }
}

/// Strip backticks and pipe characters from user-controlled strings before
/// embedding them in a markdown table cell. Prevents accidental table
/// breakage from titles or relevance notes that contain `|` or newlines
/// (NFR-005 / NFR-006).
fn sanitize_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '|' => out.push_str(r"\|"),
            '\n' | '\r' => out.push(' '),
            '`' => out.push('\''),
            _ => out.push(ch),
        }
    }
    out
}

/// Find the byte offset of the opening `---` fence. The fence must occupy
/// the start of `content` (with optional leading whitespace). Returns the
/// offset of the leading `\n` that ends the fence line, or `None` if no
/// valid opening fence is present.
fn find_yaml_open(content: &str) -> Option<usize> {
    let trimmed = content.trim_start();
    trimmed.strip_prefix("---").and_then(|rest| {
        if rest.starts_with('\n') {
            Some(content.len() - trimmed.len())
        } else {
            None
        }
    })
}

/// Find the byte offset of the closing `---` fence within `content`, where
/// `content` is the slice **after** the opening fence's newline. The offset
/// is the byte index of the leading `\n` that begins the closing-fence line.
fn find_yaml_close(content: &str) -> Option<usize> {
    let mut idx = 0usize;
    while let Some(rel) = content[idx..].find("\n---") {
        let after = idx + rel + 4;
        // Must be followed by end-of-line, end-of-file, or whitespace.
        if after == content.len() {
            return Some(rel + idx);
        }
        let next = content[after..].chars().next().unwrap_or('\n');
        if next == '\n' || next == '\r' {
            return Some(rel + idx);
        }
        idx = after;
    }
    None
}

#[cfg(test)]
#[path = "../tests/inline/io_tests.rs"]
mod tests;
