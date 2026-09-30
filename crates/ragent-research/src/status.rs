//! Lifecycle status of a research item.
//!
//! Tracks the state of an individual `research/<name>/` directory from
//! creation through completion and archival. Required by FR-013.
//!
//! Status transitions follow a simple linear progression with one terminal
//! branch:
//!
//! ```text
//!   Draft ──> InProgress ──> Complete
//!                                │
//!                                v
//!                            Archived
//! ```
//!
//! - `Draft` - the research item has been created but no gathering has run.
//! - `InProgress` - a `ResearchSession` is mid-flight; `RESEARCH.md` is
//!   partially written.
//! - `Complete` - `RESEARCH.md` is fully written and references are indexed.
//! - `Archived` - terminal state; the item is excluded from default list
//!   output unless `--all` is supplied (FR-013).
//!
//! This is intentionally a smaller state machine than the spec lifecycle;
//! research items are simpler artifacts that don't go through formal review.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Lifecycle status of a research item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResearchStatus {
    /// Research item has been created but no gathering has run.
    #[default]
    Draft,
    /// A gathering session is in flight; `RESEARCH.md` is being written.
    InProgress,
    /// `RESEARCH.md` is fully written and sources are indexed.
    Complete,
    /// Terminal state - excluded from default list output.
    Archived,
}

impl ResearchStatus {
    /// All possible status values in canonical order.
    pub const ALL: &[Self] = &[
        Self::Draft,
        Self::InProgress,
        Self::Complete,
        Self::Archived,
    ];

    /// `true` if the status represents a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Archived)
    }

    /// `true` if the status represents work that is finished but not archived.
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Lowercase kebab-style identifier used in YAML frontmatter and the
    /// research index.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InProgress => "in-progress",
            Self::Complete => "complete",
            Self::Archived => "archived",
        }
    }

    /// Parse a status from its kebab-case string representation. Returns
    /// `None` if the input does not match any known status.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "in-progress" | "in_progress" | "inprogress" => Some(Self::InProgress),
            "complete" | "completed" | "done" => Some(Self::Complete),
            "archived" | "archive" => Some(Self::Archived),
            _ => None,
        }
    }
}

impl fmt::Display for ResearchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<ResearchStatus> for &'static str {
    fn from(value: ResearchStatus) -> Self {
        value.as_str()
    }
}

#[cfg(test)]
#[path = "../tests/inline/status_tests.rs"]
mod tests;
