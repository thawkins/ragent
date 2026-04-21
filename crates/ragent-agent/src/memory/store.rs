//! Structured memory store with categories, tags, and confidence scoring.
//!
//! Unlike the file-based [`MemoryBlock`](super::block::MemoryBlock) system
//! which stores freeform Markdown, structured memories are individual facts,
//! patterns, preferences, insights, errors, or workflows stored in SQLite
//! with metadata (category, confidence, tags, source).
//!
//! # Categories
//!
//! | Category     | Description                                    |
//! |-------------|------------------------------------------------|
//! | `fact`      | Objective facts about the project or tools      |
//! | `pattern`   | Recurring code or process patterns              |
//! | `preference`| User or project style preferences               |
//! | `insight`   | Aha-moments and learned knowledge               |
//! | `error`     | Bugs encountered and how they were resolved     |
//! | `workflow`  | Step-by-step procedures for common tasks         |
//!
//! # Storage
//!
//! Structured memories are persisted in the `memories` and `memory_tags`
//! SQLite tables, with a `memories_fts` FTS5 virtual table for full-text
//! search. The CRUD methods live on [`crate::storage::Storage`].

use serde::{Deserialize, Serialize};

/// Valid categories for structured memories.
pub const MEMORY_CATEGORIES: &[&str] = &[
    "fact",
    "pattern",
    "preference",
    "insight",
    "error",
    "workflow",
];

/// A structured memory with metadata.
///
/// This is the domain type used by the memory tools. It is converted to/from
/// [`crate::storage::MemoryRow`] when persisting to SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredMemory {
    /// Auto-generated row ID (0 for unsaved memories).
    #[serde(default)]
    pub id: i64,
    /// The memory content.
    pub content: String,
    /// Category: one of [`MEMORY_CATEGORIES`].
    pub category: String,
    /// Source of the memory (e.g., "manual", "auto-extract", tool name).
    #[serde(default)]
    pub source: String,
    /// Confidence score (0.0–1.0). Higher = more certain.
    pub confidence: f64,
    /// Project this memory belongs to.
    #[serde(default)]
    pub project: String,
    /// Session that created this memory.
    #[serde(default)]
    pub session_id: String,
    /// Tags for filtering and categorisation.
    #[serde(default)]
    pub tags: Vec<String>,
    /// ISO 8601 creation timestamp.
    #[serde(default)]
    pub created_at: String,
    /// ISO 8601 last-updated timestamp.
    #[serde(default)]
    pub updated_at: String,
    /// Number of times accessed in search results.
    #[serde(default)]
    pub access_count: i64,
    /// ISO 8601 timestamp of last access.
    #[serde(default)]
    pub last_accessed: Option<String>,
}

impl StructuredMemory {
    /// Create a new structured memory with the given content and category.
    ///
    /// Confidence defaults to 0.7. Call `with_confidence()` to override.
    pub fn new(content: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            id: 0,
            content: content.into(),
            category: category.into(),
            source: String::new(),
            confidence: 0.7,
            project: String::new(),
            session_id: String::new(),
            tags: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
            access_count: 0,
            last_accessed: None,
        }
    }

    /// Set the confidence score.
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set the source.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Set the project name.
    #[must_use]
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = project.into();
        self
    }

    /// Set the session ID.
    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }

    /// Set the tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Validate that the category is one of the allowed values.
    pub fn validate_category(category: &str) -> Result<(), String> {
        if MEMORY_CATEGORIES.contains(&category) {
            Ok(())
        } else {
            Err(format!(
                "Invalid category '{}'. Must be one of: {}",
                category,
                MEMORY_CATEGORIES.join(", ")
            ))
        }
    }

    /// Validate that the confidence is in [0.0, 1.0].
    pub fn validate_confidence(confidence: f64) -> Result<(), String> {
        if (0.0..=1.0).contains(&confidence) {
            Ok(())
        } else {
            Err(format!(
                "Confidence must be between 0.0 and 1.0, got {confidence}"
            ))
        }
    }
}

/// Filter criteria for the `memory_forget` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForgetFilter {
    /// Delete a specific memory by its row ID.
    Id(i64),
    /// Delete memories matching filter criteria.
    Filter {
        /// Delete memories older than this many days.
        older_than_days: Option<u32>,
        /// Delete memories with confidence at or below this value.
        max_confidence: Option<f64>,
        /// Delete memories in this category.
        category: Option<String>,
        /// Delete memories that have ALL of these tags.
        tags: Option<Vec<String>>,
    },
}

impl ForgetFilter {
    /// Check that at least one filter criterion is set (for Filter variant only).
    pub fn has_any_criterion(&self) -> bool {
        match self {
            Self::Id(_) => true,
            Self::Filter {
                older_than_days,
                max_confidence,
                category,
                tags,
            } => {
                older_than_days.is_some()
                    || max_confidence.is_some()
                    || category.is_some()
                    || tags.as_ref().is_some_and(|t| !t.is_empty())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_category_valid() {
        for cat in MEMORY_CATEGORIES {
            assert!(StructuredMemory::validate_category(cat).is_ok());
        }
    }

    #[test]
    fn test_validate_category_invalid() {
        assert!(StructuredMemory::validate_category("invalid").is_err());
        assert!(StructuredMemory::validate_category("").is_err());
    }

    #[test]
    fn test_validate_confidence() {
        assert!(StructuredMemory::validate_confidence(0.0).is_ok());
        assert!(StructuredMemory::validate_confidence(1.0).is_ok());
        assert!(StructuredMemory::validate_confidence(0.5).is_ok());
        assert!(StructuredMemory::validate_confidence(-0.1).is_err());
        assert!(StructuredMemory::validate_confidence(1.1).is_err());
    }

    #[test]
    fn test_structured_memory_new() {
        let mem = StructuredMemory::new("Use Result<T, E>", "pattern");
        assert_eq!(mem.content, "Use Result<T, E>");
        assert_eq!(mem.category, "pattern");
        assert_eq!(mem.confidence, 0.7);
        assert!(mem.tags.is_empty());
    }

    #[test]
    fn test_structured_memory_builder() {
        let mem = StructuredMemory::new("Test content", "fact")
            .with_confidence(0.9)
            .with_source("auto-extract")
            .with_project("ragent")
            .with_session_id("sess-1")
            .with_tags(vec!["rust".to_string()]);
        assert_eq!(mem.confidence, 0.9);
        assert_eq!(mem.source, "auto-extract");
        assert_eq!(mem.project, "ragent");
        assert_eq!(mem.session_id, "sess-1");
        assert_eq!(mem.tags, vec!["rust"]);
    }

    #[test]
    fn test_confidence_clamped() {
        let mem = StructuredMemory::new("test", "fact").with_confidence(2.0);
        assert_eq!(mem.confidence, 1.0);
        let mem = StructuredMemory::new("test", "fact").with_confidence(-1.0);
        assert_eq!(mem.confidence, 0.0);
    }

    #[test]
    fn test_forget_filter_has_criterion() {
        let filter = ForgetFilter::Filter {
            older_than_days: Some(30),
            max_confidence: None,
            category: None,
            tags: None,
        };
        assert!(filter.has_any_criterion());

        let empty = ForgetFilter::Filter {
            older_than_days: None,
            max_confidence: None,
            category: None,
            tags: None,
        };
        assert!(!empty.has_any_criterion());

        let empty_tags = ForgetFilter::Filter {
            older_than_days: None,
            max_confidence: None,
            category: None,
            tags: Some(vec![]),
        };
        assert!(!empty_tags.has_any_criterion());

        let id_filter = ForgetFilter::Id(42);
        assert!(id_filter.has_any_criterion());
    }
}
