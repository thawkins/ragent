//! Journal system for append-only recording of insights, decisions, and discoveries.
//!
//! The journal provides a structured, searchable log of observations made by
//! agents during sessions. Entries are stored in SQLite with FTS5 full-text
//! search and tag-based filtering.
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE journal_entries (
//!     id TEXT PRIMARY KEY,
//!     title TEXT NOT NULL,
//!     content TEXT NOT NULL,
//!     project TEXT NOT NULL DEFAULT '',
//!     session_id TEXT NOT NULL DEFAULT '',
//!     timestamp TEXT NOT NULL,
//!     created_at TEXT NOT NULL
//! );
//!
//! CREATE TABLE journal_tags (
//!     entry_id TEXT NOT NULL,
//!     tag TEXT NOT NULL,
//!     PRIMARY KEY (entry_id, tag)
//! );
//!
//! CREATE VIRTUAL TABLE journal_fts USING fts5(title, content, content=journal_entries);
//! ```
//!
//! See [`crate::storage::Storage`] for the CRUD methods that operate on these
//! tables.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// A single journal entry recording an insight, decision, or discovery.
///
/// Entries are append-only — once created they are never modified, only
/// deleted or left to decay. Each entry has a unique ID (UUID v4), a short
/// title, full content, optional tags, and metadata linking it to the
/// project and session that produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Short title describing the entry.
    pub title: String,
    /// Full content of the journal entry.
    pub content: String,
    /// Tags for categorisation and filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Project this entry belongs to (directory name or configured name).
    #[serde(default)]
    pub project: String,
    /// Session that created this entry.
    #[serde(default)]
    pub session_id: String,
    /// ISO 8601 timestamp of the observation/event.
    pub timestamp: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}

impl JournalEntry {
    /// Create a new journal entry with the given title and content.
    ///
    /// A UUID v4 is generated for the `id` field and the current UTC time
    /// is used for `timestamp` and `created_at`.
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            content: content.into(),
            tags: Vec::new(),
            project: String::new(),
            session_id: String::new(),
            timestamp: now.clone(),
            created_at: now,
        }
    }

    /// Set the tags for this entry.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set the project name.
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = project.into();
        self
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }

    /// Validate that tag strings are well-formed.
    ///
    /// Tags must be non-empty, lowercase, and contain only ASCII letters,
    /// digits, and hyphens. Returns `Ok(())` if all tags are valid.
    pub fn validate_tags(tags: &[String]) -> Result<(), String> {
        for tag in tags {
            if tag.is_empty() {
                return Err("Tag must not be empty".to_string());
            }
            if tag.len() > 64 {
                return Err(format!("Tag '{tag}' exceeds 64 characters"));
            }
            for ch in tag.chars() {
                if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && ch != '-' && ch != '_' {
                    return Err(format!(
                        "Tag '{tag}' contains invalid character '{ch}'. \
                         Use lowercase letters, digits, hyphens, or underscores."
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A lightweight summary of a journal entry, used in search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntrySummary {
    /// Unique identifier.
    pub id: String,
    /// Short title.
    pub title: String,
    /// Snippet of the content (first 200 characters).
    pub snippet: String,
    /// Tags for this entry.
    #[serde(default)]
    pub tags: Vec<String>,
    /// ISO 8601 timestamp.
    pub timestamp: String,
}

impl JournalEntrySummary {
    /// Create a summary from a full entry.
    pub fn from_entry(entry: &JournalEntry) -> Self {
        let snippet = if entry.content.len() > 200 {
            format!("{}…", &entry.content[..200])
        } else {
            entry.content.clone()
        };
        Self {
            id: entry.id.clone(),
            title: entry.title.clone(),
            snippet,
            tags: entry.tags.clone(),
            timestamp: entry.timestamp.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_journal_entry_new() {
        let entry = JournalEntry::new("Bug fix", "Fixed off-by-one error in parser");
        assert!(!entry.id.is_empty());
        assert_eq!(entry.title, "Bug fix");
        assert_eq!(entry.content, "Fixed off-by-one error in parser");
        assert!(entry.tags.is_empty());
        assert!(entry.project.is_empty());
        assert!(entry.session_id.is_empty());
        assert!(!entry.timestamp.is_empty());
        assert!(!entry.created_at.is_empty());
    }

    #[test]
    fn test_journal_entry_builder() {
        let entry = JournalEntry::new("Pattern", "Use Result<T, E>")
            .with_tags(vec!["rust".to_string(), "error-handling".to_string()])
            .with_project("ragent")
            .with_session_id("sess-123");
        assert_eq!(entry.tags, vec!["rust", "error-handling"]);
        assert_eq!(entry.project, "ragent");
        assert_eq!(entry.session_id, "sess-123");
    }

    #[test]
    fn test_validate_tags() {
        assert!(
            JournalEntry::validate_tags(&["rust".to_string(), "error-handling".to_string()])
                .is_ok()
        );
        assert!(JournalEntry::validate_tags(&["".to_string()]).is_err());
        assert!(JournalEntry::validate_tags(&["Has Upper".to_string()]).is_err());
        assert!(JournalEntry::validate_tags(&["valid_tag".to_string()]).is_ok());
    }

    #[test]
    fn test_entry_summary() {
        let entry = JournalEntry::new("Test", "A".repeat(300)).with_tags(vec!["test".to_string()]);
        let summary = JournalEntrySummary::from_entry(&entry);
        assert_eq!(summary.id, entry.id);
        assert_eq!(summary.title, "Test");
        assert!(summary.snippet.len() <= 203); // 200 + "…"
        assert!(summary.snippet.ends_with('…'));
        assert_eq!(summary.tags, vec!["test"]);
    }

    #[test]
    fn test_entry_summary_short_content() {
        let entry = JournalEntry::new("Short", "Hello");
        let summary = JournalEntrySummary::from_entry(&entry);
        assert_eq!(summary.snippet, "Hello");
    }

    #[test]
    fn test_uuid_uniqueness() {
        let a = JournalEntry::new("A", "content");
        let b = JournalEntry::new("B", "content");
        assert_ne!(a.id, b.id);
    }
}
