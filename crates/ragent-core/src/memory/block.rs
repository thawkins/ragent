//! Memory block types and YAML frontmatter serialisation.
//!
//! A [`MemoryBlock`] represents a named, scoped unit of persistent memory.
//! Blocks are serialised as Markdown files with YAML frontmatter, making
//! them human-readable and version-control-friendly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Scope determining where a memory block is stored.
///
/// - `Global`: stored under `~/.ragent/memory/` — shared across all projects.
/// - `Project`: stored under `<working_dir>/.ragent/memory/` — specific to
///   the current project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BlockScope {
    /// Global memory shared across all projects (`~/.ragent/memory/`).
    Global,
    /// Project-specific memory (`<working_dir>/.ragent/memory/`).
    #[default]
    Project,
}

impl BlockScope {
    /// Returns the string representation used in tool parameters.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }

    /// Parse from a tool parameter string.
    ///
    /// Accepts "global" or "user" for [`BlockScope::Global`] and
    /// "project" for [`BlockScope::Project`].
    pub fn from_param(s: &str) -> Option<Self> {
        match s {
            "global" | "user" => Some(Self::Global),
            "project" => Some(Self::Project),
            _ => None,
        }
    }
}

impl std::fmt::Display for BlockScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A named, scoped unit of persistent memory.
///
/// Blocks are stored as `.md` files with YAML frontmatter. The frontmatter
/// holds metadata (label, description, scope, size limit, etc.) while the
/// file body contains the actual memory content in Markdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBlock {
    /// Unique label identifying this block (also used as the filename).
    ///
    /// Must be a valid filename stem: lowercase alphanumeric with hyphens.
    pub label: String,
    /// Short human-readable description of the block's purpose.
    #[serde(default)]
    pub description: String,
    /// Storage scope (global vs project).
    pub scope: BlockScope,
    /// Maximum content size in bytes. `0` means no limit.
    #[serde(default)]
    pub limit: usize,
    /// If `true`, the block cannot be modified via `memory_write` or
    /// `memory_replace`.
    #[serde(default)]
    pub read_only: bool,
    /// ISO 8601 timestamp when the block was first created.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    /// ISO 8601 timestamp when the block was last updated.
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    /// The Markdown content of the block (the file body, not frontmatter).
    #[serde(skip)]
    pub content: String,
}

impl MemoryBlock {
    /// Create a new memory block with the given label and scope.
    ///
    /// Timestamps are set to the current time. All other metadata fields
    /// default to empty/zero/false.
    pub fn new(label: impl Into<String>, scope: BlockScope) -> Self {
        let now = Utc::now();
        Self {
            label: label.into(),
            description: String::new(),
            scope,
            limit: 0,
            read_only: false,
            created_at: now,
            updated_at: now,
            content: String::new(),
        }
    }

    /// Validate that a label is a valid filename stem.
    ///
    /// Valid labels contain only lowercase ASCII letters, digits, and hyphens,
    /// must start with a letter, and be 1–64 characters long.
    pub fn validate_label(label: &str) -> Result<(), String> {
        if label.is_empty() {
            return Err("Label must not be empty".to_string());
        }
        if label.len() > 64 {
            return Err("Label must be at most 64 characters".to_string());
        }
        let mut chars = label.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_lowercase() {
            return Err(format!(
                "Label must start with a lowercase letter, got '{first}'"
            ));
        }
        for ch in chars {
            if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' {
                return Err(format!(
                    "Label must contain only lowercase letters, digits, and hyphens, got '{ch}'"
                ));
            }
        }
        Ok(())
    }

    /// Check whether the content exceeds the size limit.
    ///
    /// Returns `Ok(())` if within limit or no limit is set, `Err` with a
    /// descriptive message otherwise.
    pub fn check_content_limit(&self) -> Result<(), String> {
        if self.limit > 0 && self.content.len() > self.limit {
            return Err(format!(
                "Content ({} bytes) exceeds block limit ({} bytes) for '{}'",
                self.content.len(),
                self.limit,
                self.label
            ));
        }
        Ok(())
    }

    /// Serialise this block to a Markdown string with YAML frontmatter.
    ///
    /// The output format is:
    /// ```text
    /// ---
    /// label: my-block
    /// description: ...
    /// ---
    ///
    /// Content here.
    /// ```
    pub fn to_markdown(&self) -> String {
        let frontmatter = serde_yaml::to_string(&FrontmatterData {
            label: self.label.clone(),
            description: if self.description.is_empty() {
                None
            } else {
                Some(self.description.clone())
            },
            scope: self.scope.clone(),
            limit: if self.limit > 0 {
                Some(self.limit)
            } else {
                None
            },
            read_only: if self.read_only { Some(true) } else { None },
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.updated_at.to_rfc3339(),
        })
        .unwrap_or_default();

        format!("---\n{frontmatter}---\n\n{}", self.content)
    }

    /// Deserialise a block from a Markdown string with YAML frontmatter.
    ///
    /// If the frontmatter is missing or malformed, returns a block with
    /// default metadata and the full text as content (backward compatible
    /// with existing plain MEMORY.md files).
    pub fn from_markdown(text: &str, default_scope: BlockScope) -> Self {
        if let Some((fm, body)) = split_frontmatter(text) {
            match serde_yaml::from_str::<FrontmatterData>(&fm) {
                Ok(data) => Self {
                    label: data.label,
                    description: data.description.unwrap_or_default(),
                    scope: data.scope,
                    limit: data.limit.unwrap_or(0),
                    read_only: data.read_only.unwrap_or(false),
                    created_at: parse_datetime(&data.created_at),
                    updated_at: parse_datetime(&data.updated_at),
                    content: body.trim_start().to_string(),
                },
                Err(_) => {
                    // Frontmatter present but unparseable — treat as legacy
                    let label = "MEMORY".to_string();
                    Self::new(label, default_scope).with_content(text.to_string())
                }
            }
        } else {
            // No frontmatter — plain legacy MEMORY.md
            Self::new("MEMORY", default_scope).with_content(text.to_string())
        }
    }

    /// Set the block content and update the `updated_at` timestamp.
    #[must_use]
    pub fn with_content(mut self, content: String) -> Self {
        self.updated_at = Utc::now();
        self.content = content;
        self
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set the size limit in bytes.
    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set the read-only flag.
    #[must_use]
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Return the filename for this block (label + `.md` extension).
    pub fn filename(&self) -> String {
        format!("{}.md", self.label)
    }
}

/// Resolves the memory directory for a given scope.
///
/// - [`BlockScope::Global`] → `~/.ragent/memory/`
/// - [`BlockScope::Project`] → `<working_dir>/.ragent/memory/`
pub fn resolve_block_dir(scope: &BlockScope, working_dir: &PathBuf) -> anyhow::Result<PathBuf> {
    match scope {
        BlockScope::Global => {
            let home = dirs::home_dir().ok_or_else(|| {
                anyhow::anyhow!("Cannot determine home directory for global memory scope")
            })?;
            Ok(home.join(".ragent").join("memory"))
        }
        BlockScope::Project => Ok(working_dir.join(".ragent").join("memory")),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Intermediate struct for YAML frontmatter serialisation.
///
/// Only non-default fields are serialised to keep the frontmatter clean.
#[derive(Debug, Serialize, Deserialize)]
struct FrontmatterData {
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    scope: BlockScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    read_only: Option<bool>,
    created_at: String,
    updated_at: String,
}

/// Split a Markdown string into YAML frontmatter and body.
///
/// Returns `Some((frontmatter_text, body_text))` if the text starts with `---`,
/// or `None` if there is no frontmatter.
fn split_frontmatter(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    // Find the closing ---
    let after_open = &trimmed[3..];
    let rest = after_open.trim_start_matches(['\n', '\r']);
    if let Some(end_pos) = rest.find("\n---") {
        let fm = rest[..end_pos].to_string();
        let body = rest[end_pos + 4..].to_string();
        Some((fm, body))
    } else {
        None
    }
}

/// Parse an ISO 8601 / RFC 3339 datetime string, falling back to now.
fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_scope_from_param() {
        assert_eq!(BlockScope::from_param("global"), Some(BlockScope::Global));
        assert_eq!(BlockScope::from_param("user"), Some(BlockScope::Global));
        assert_eq!(BlockScope::from_param("project"), Some(BlockScope::Project));
        assert_eq!(BlockScope::from_param("invalid"), None);
    }

    #[test]
    fn test_block_scope_display() {
        assert_eq!(format!("{}", BlockScope::Global), "global");
        assert_eq!(format!("{}", BlockScope::Project), "project");
    }

    #[test]
    fn test_validate_label() {
        assert!(MemoryBlock::validate_label("patterns").is_ok());
        assert!(MemoryBlock::validate_label("my-block-2").is_ok());
        assert!(MemoryBlock::validate_label("").is_err());
        assert!(MemoryBlock::validate_label("1starts-with-digit").is_err());
        assert!(MemoryBlock::validate_label("Has_Upper").is_err());
        assert!(MemoryBlock::validate_label("has space").is_err());
        assert!(MemoryBlock::validate_label(&"a".repeat(65)).is_err());
    }

    #[test]
    fn test_block_new() {
        let block = MemoryBlock::new("test-block", BlockScope::Project);
        assert_eq!(block.label, "test-block");
        assert_eq!(block.scope, BlockScope::Project);
        assert!(block.description.is_empty());
        assert_eq!(block.limit, 0);
        assert!(!block.read_only);
        assert!(block.content.is_empty());
    }

    #[test]
    fn test_block_builder_pattern() {
        let block = MemoryBlock::new("persona", BlockScope::Global)
            .with_description("Agent personality traits".to_string())
            .with_limit(1024)
            .with_read_only(true)
            .with_content("I am a helpful coding assistant.".to_string());
        assert_eq!(block.description, "Agent personality traits");
        assert_eq!(block.limit, 1024);
        assert!(block.read_only);
        assert_eq!(block.content, "I am a helpful coding assistant.");
    }

    #[test]
    fn test_check_content_limit() {
        let block = MemoryBlock::new("test", BlockScope::Project)
            .with_limit(10)
            .with_content("1234567890extra".to_string());
        assert!(block.check_content_limit().is_err());
        let block_ok = MemoryBlock::new("test", BlockScope::Project)
            .with_limit(20)
            .with_content("short".to_string());
        assert!(block_ok.check_content_limit().is_ok());
    }

    #[test]
    fn test_filename() {
        let block = MemoryBlock::new("patterns", BlockScope::Project);
        assert_eq!(block.filename(), "patterns.md");
    }

    #[test]
    fn test_roundtrip_markdown() {
        let block = MemoryBlock::new("persona", BlockScope::Global)
            .with_description("Agent personality".to_string())
            .with_limit(2048)
            .with_content("I am a helpful coding assistant.\n\nI prefer Rust.".to_string());
        let md = block.to_markdown();
        let parsed = MemoryBlock::from_markdown(&md, BlockScope::Project);
        assert_eq!(parsed.label, "persona");
        assert_eq!(parsed.description, "Agent personality");
        assert_eq!(parsed.scope, BlockScope::Global);
        assert_eq!(parsed.limit, 2048);
        assert_eq!(
            parsed.content,
            "I am a helpful coding assistant.\n\nI prefer Rust."
        );
    }

    #[test]
    fn test_from_markdown_no_frontmatter() {
        let text = "This is a plain MEMORY.md file.\nNo frontmatter here.";
        let block = MemoryBlock::from_markdown(text, BlockScope::Project);
        assert_eq!(block.label, "MEMORY");
        assert_eq!(block.scope, BlockScope::Project);
        assert_eq!(
            block.content,
            "This is a plain MEMORY.md file.\nNo frontmatter here."
        );
    }

    #[test]
    fn test_split_frontmatter() {
        let text = "---\nlabel: test\nscope: global\n---\n\nContent here.";
        let (fm, body) = split_frontmatter(text).unwrap();
        assert!(fm.contains("label: test"));
        assert!(fm.contains("scope: global"));
        assert_eq!(body.trim(), "Content here.");
    }

    #[test]
    fn test_split_frontmatter_missing_close() {
        let text = "---\nlabel: test\nNo closing dashes";
        assert!(split_frontmatter(text).is_none());
    }

    #[test]
    fn test_resolve_block_dir() {
        let wd = PathBuf::from("/tmp/project");
        let project_dir = resolve_block_dir(&BlockScope::Project, &wd).unwrap();
        assert_eq!(project_dir, PathBuf::from("/tmp/project/.ragent/memory"));

        let global_dir = resolve_block_dir(&BlockScope::Global, &wd).unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(global_dir, home.join(".ragent/memory"));
    }
}
