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
//! ├── INDEX.md                       (derived cache, regenerated on change)
//! ├── <name>/
//! │   ├── RESEARCH.md                (frontmatter + body, 8 sections)
//! │   └── sources/
//! │       ├── web-NN.md
//! │       ├── local-NN.md
//! │       ├── spec-NN.md
//! │       └── other-NN.md
//! └── _templates/                    (optional research templates, FR-020)
//!     └── <name>.md
//! ```

use crate::research_name::ResearchName;
use crate::source::Source;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;

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

    /// Compute the path of the `RESEARCH.md` file for a research item.
    #[must_use]
    pub fn research_md_path(research_root: &Path, name: &ResearchName) -> PathBuf {
        Self::item_dir(research_root, name).join("RESEARCH.md")
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
    #[must_use]
    pub fn template_path(research_root: &Path, template_name: &str) -> PathBuf {
        research_root
            .join("_templates")
            .join(format!("{template_name}.md"))
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

    /// Write `content` to `path` atomically (write to `<path>.tmp`, then rename).
    pub async fn atomic_write(path: impl AsRef<Path>, content: &str) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, content).await?;
        match fs::rename(&tmp, path).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Best-effort cleanup of the leftover .tmp file so a later
                // retry doesn't trip over a stale inode.
                let _ = fs::remove_file(&tmp).await;
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
        //   ---\n…\n---\n
        //   …\n---\n…\n---\n…
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
    /// The table includes a **Published** column (between **Title** and
    /// **Relevance**) showing each web source's publication date when it
    /// could be parsed from the page's embedded metadata, and `—` otherwise.
    /// Non-web sources have no publication date and always show `—`.
    #[must_use]
    pub fn render_references_index(sources: &[Source], captured_at: DateTime<Utc>) -> String {
        if sources.is_empty() {
            return format!(
                "## References Index\n\n| # | Type | Path/URL | Title | Published | Relevance | Captured |\n\
                 |---|------|----------|-------|-----------|-----------|----------|\n\
                 | 1 | other | — | No sources captured | — | (no gathering run) | {} |\n",
                captured_at.to_rfc3339()
            );
        }
        let mut out = String::from(
            "## References Index\n\n\
             | # | Type | Path/URL | Title | Published | Relevance | Captured |\n\
             |---|------|----------|-------|-----------|-----------|----------|\n",
        );
        for (idx, source) in sources.iter().enumerate() {
            let n = idx + 1;
            let kind = source.type_str();
            let path = source.path_or_url();
            let title = sanitize_inline(source.title());
            let published = source
                .published_at()
                .map_or_else(|| "—".to_string(), |dt| dt.format("%Y-%m-%d").to_string());
            let relevance = match source {
                Source::Local { relevance, .. } => sanitize_inline(relevance),
                Source::Spec { relevance, .. } if !relevance.is_empty() => {
                    sanitize_inline(relevance)
                }
                Source::Web { relevance, .. } if !relevance.is_empty() => {
                    sanitize_inline(relevance)
                }
                _ => "—".to_string(),
            };
            let captured = source.captured_at().to_rfc3339();
            out.push_str(&format!(
                "| {n} | {kind} | {path} | {title} | {published} | {relevance} | {captured} |\n"
            ));
        }
        out
    }

    /// Render the global `research/INDEX.md` derived cache (FR-012).
    ///
    /// `items` is the list of items to include; pass an empty slice to render
    /// the "no research yet" placeholder. Items are emitted in the order
    /// they are passed — callers are responsible for sorting.
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
             This file is regenerated on every change — do not edit by hand.\n\n\
             | Name | Title | Status | Created (UTC) | Modified (UTC) |\n\
             |------|-------|--------|---------------|----------------|\n",
        );
        for item in items {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                item.name,
                sanitize_inline(&item.title),
                item.status.as_str(),
                item.created_at.to_rfc3339(),
                item.modified_at.to_rfc3339(),
            ));
        }
        out.push_str(&format!(
            "\n_Generated {} · {} items._\n",
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
    if let Some(rest) = content.strip_prefix("---") {
        // Fence must be on its own line.
        if rest.starts_with('\n') {
            return Some(0);
        }
    }
    None
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
mod tests {
    use super::*;
    use crate::status::ResearchStatus;
    use tempfile::TempDir;

    #[tokio::test]
    async fn atomic_write_then_read_round_trips() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("note.md");
        ResearchIo::atomic_write(&path, "hello").await.unwrap();
        let read = ResearchIo::read_file(&path).await.unwrap();
        assert_eq!(read, "hello");
    }

    #[tokio::test]
    async fn atomic_write_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested/deep/file.md");
        ResearchIo::atomic_write(&path, "x").await.unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn item_dir_uses_name_as_dir_name() {
        let name = ResearchName::new("rust-async").unwrap();
        let path = ResearchIo::item_dir(Path::new("/data"), &name);
        assert_eq!(path, PathBuf::from("/data/rust-async"));
    }

    #[test]
    fn research_md_path_appends_filename() {
        let name = ResearchName::new("rust-async").unwrap();
        let path = ResearchIo::research_md_path(Path::new("/data"), &name);
        assert_eq!(path, PathBuf::from("/data/rust-async/RESEARCH.md"));
    }

    #[test]
    fn sources_dir_sits_inside_item_dir() {
        let name = ResearchName::new("rust-async").unwrap();
        let path = ResearchIo::sources_dir(Path::new("/data"), &name);
        assert_eq!(path, PathBuf::from("/data/rust-async/sources"));
    }

    #[test]
    fn source_body_path_uses_two_digit_index() {
        let name = ResearchName::new("rust-async").unwrap();
        assert_eq!(
            ResearchIo::source_body_path(Path::new("/data"), &name, "web", 1),
            PathBuf::from("/data/rust-async/sources/web-01.md"),
        );
        assert_eq!(
            ResearchIo::source_body_path(Path::new("/data"), &name, "local", 12),
            PathBuf::from("/data/rust-async/sources/local-12.md"),
        );
    }

    #[test]
    fn template_path_sits_under_templates_dir() {
        assert_eq!(
            ResearchIo::template_path(Path::new("/data"), "deepdive"),
            PathBuf::from("/data/_templates/deepdive.md"),
        );
    }

    #[test]
    fn split_frontmatter_extracts_yaml_block() {
        let content = "---\nname: foo\n---\n\n# Title\nbody\n";
        let (fm, body) = ResearchIo::split_frontmatter(content);
        assert_eq!(fm, "name: foo");
        assert_eq!(body, "# Title\nbody\n");
    }

    #[test]
    fn split_frontmatter_handles_missing_block() {
        let content = "# No frontmatter\nbody\n";
        let (fm, body) = ResearchIo::split_frontmatter(content);
        assert_eq!(fm, "");
        assert_eq!(body, content);
    }

    #[test]
    fn references_index_includes_placeholder_when_empty() {
        let idx = ResearchIo::render_references_index(&[], Utc::now());
        assert!(idx.contains("No sources captured"));
    }

    #[test]
    fn references_index_numbers_sources_sequentially() {
        let sources = vec![
            Source::Other {
                label: "first".into(),
                captured_at: Utc::now(),
                body_path: PathBuf::from("sources/other-01.md"),

                body: String::new(),
            },
            Source::Other {
                label: "second".into(),
                captured_at: Utc::now(),
                body_path: PathBuf::from("sources/other-02.md"),

                body: String::new(),
            },
        ];
        let idx = ResearchIo::render_references_index(&sources, Utc::now());
        assert!(idx.contains("| 1 | other"));
        assert!(idx.contains("| 2 | other"));
    }

    #[test]
    fn references_index_escapes_pipes_in_titles() {
        let sources = vec![Source::Other {
            label: "a|b".into(),
            captured_at: Utc::now(),
            body_path: PathBuf::from("sources/other-01.md"),
            body: String::new(),
        }];
        let idx = ResearchIo::render_references_index(&sources, Utc::now());
        assert!(idx.contains(r"a\|b"), "pipe must be escaped: {idx}");
    }

    #[test]
    fn references_index_includes_published_column_for_web_sources() {
        use chrono::TimeZone;
        let published = Utc.with_ymd_and_hms(2024, 3, 22, 0, 0, 0).unwrap();
        let sources = vec![
            Source::Web {
                url: "https://dated.example".into(),
                title: "Dated".into(),
                captured_at: Utc::now(),
                published_at: Some(published),
                body_path: PathBuf::from("sources/web-01.md"),
                relevance: String::new(),

                body: String::new(),
            },
            Source::Web {
                url: "https://undated.example".into(),
                title: "Undated".into(),
                captured_at: Utc::now(),
                published_at: None,
                body_path: PathBuf::from("sources/web-02.md"),
                relevance: String::new(),

                body: String::new(),
            },
        ];
        let idx = ResearchIo::render_references_index(&sources, Utc::now());
        assert!(
            idx.contains("Published"),
            "header row must include Published column: {idx}"
        );
        assert!(
            idx.contains("2024-03-22"),
            "dated web source should show its publication date: {idx}"
        );
        // The undated row should render an em-dash placeholder for Published.
        let undated_row = idx
            .lines()
            .find(|l| l.contains("https://undated.example"))
            .unwrap_or_default();
        assert!(
            undated_row.contains("| — |"),
            "undated web source should show '—' for Published: {idx}"
        );
    }

    #[test]
    fn index_renders_empty_placeholder_when_no_items() {
        let out = ResearchIo::render_index(&[]);
        assert!(out.contains("No research items yet"));
    }

    #[test]
    fn index_includes_one_row_per_item() {
        let now = Utc::now();
        let items = vec![
            IndexEntry {
                name: "alpha".into(),
                title: "Alpha research".into(),
                status: ResearchStatus::Complete,
                created_at: now,
                modified_at: now,
            },
            IndexEntry {
                name: "beta".into(),
                title: "Beta research".into(),
                status: ResearchStatus::Draft,
                created_at: now,
                modified_at: now,
            },
        ];
        let out = ResearchIo::render_index(&items);
        assert!(out.contains("| alpha | Alpha research | complete |"));
        assert!(out.contains("| beta | Beta research | draft |"));
        assert!(out.contains("2 items"));
    }

    #[tokio::test]
    async fn remove_item_returns_not_found_for_missing_dir() {
        let tmp = TempDir::new().unwrap();
        let name = ResearchName::new("rust-async").unwrap();
        let err = ResearchIo::remove_item(tmp.path(), &name)
            .await
            .unwrap_err();
        assert!(matches!(err, ResearchIoError::NotFound(_)));
    }

    #[tokio::test]
    async fn remove_item_deletes_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let name = ResearchName::new("rust-async").unwrap();
        let dir = ResearchIo::item_dir(tmp.path(), &name);
        tokio::fs::create_dir_all(&dir).await.unwrap();
        ResearchIo::remove_item(tmp.path(), &name).await.unwrap();
        assert!(!dir.exists());
    }

    #[tokio::test]
    async fn create_item_dirs_makes_sources_subdir() {
        let tmp = TempDir::new().unwrap();
        let name = ResearchName::new("rust-async").unwrap();
        ResearchIo::create_item_dirs(tmp.path(), &name)
            .await
            .unwrap();
        assert!(ResearchIo::sources_dir(tmp.path(), &name).is_dir());
    }
}
