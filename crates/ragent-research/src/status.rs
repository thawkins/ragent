//! Lifecycle status of a research item.
//!
//! Tracks the state of an individual `research/<name>/` directory from
//! creation through completion and archival. Required by FR-013.
//!
//! Status transitions follow a simple linear progression with one terminal
//! branch:
//!
//! ```text
//!   Draft ──► InProgress ──► Complete
//!                                │
//!                                ▼
//!                            Archived
//! ```
//!
//! - `Draft` — the research item has been created but no gathering has run.
//! - `InProgress` — a `ResearchSession` is mid-flight; `RESEARCH.md` is
//!   partially written.
//! - `Complete` — `RESEARCH.md` is fully written and references are indexed.
//! - `Archived` — terminal state; the item is excluded from default list
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
    /// Terminal state — excluded from default list output.
    Archived,
}

impl ResearchStatus {
    /// All possible status values in canonical order.
    pub const ALL: &[ResearchStatus] = &[
        Self::Draft,
        Self::InProgress,
        Self::Complete,
        Self::Archived,
    ];

    /// `true` if the status represents a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Archived)
    }

    /// `true` if the status represents work that is finished but not archived.
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Lowercase kebab-style identifier used in YAML frontmatter and the
    /// research index.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InProgress => "in-progress",
            Self::Complete => "complete",
            Self::Archived => "archived",
        }
    }

    /// Parse a status from its kebab-case string representation. Returns
    /// `None` if the input does not match any known status.
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
mod tests {
    use super::*;

    #[test]
    fn default_is_draft() {
        assert_eq!(ResearchStatus::default(), ResearchStatus::Draft);
    }

    #[test]
    fn as_str_returns_kebab_case() {
        assert_eq!(ResearchStatus::Draft.as_str(), "draft");
        assert_eq!(ResearchStatus::InProgress.as_str(), "in-progress");
        assert_eq!(ResearchStatus::Complete.as_str(), "complete");
        assert_eq!(ResearchStatus::Archived.as_str(), "archived");
    }

    #[test]
    fn parse_round_trips_through_as_str() {
        for &status in ResearchStatus::ALL {
            assert_eq!(ResearchStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn parse_accepts_common_aliases() {
        assert_eq!(ResearchStatus::parse("in_progress"), Some(ResearchStatus::InProgress));
        assert_eq!(ResearchStatus::parse("in-progress"), Some(ResearchStatus::InProgress));
        assert_eq!(ResearchStatus::parse("inprogress"), Some(ResearchStatus::InProgress));
        assert_eq!(ResearchStatus::parse("completed"), Some(ResearchStatus::Complete));
        assert_eq!(ResearchStatus::parse("done"), Some(ResearchStatus::Complete));
        assert_eq!(ResearchStatus::parse("archive"), Some(ResearchStatus::Archived));
    }

    #[test]
    fn parse_returns_none_for_unknown() {
        assert_eq!(ResearchStatus::parse(""), None);
        assert_eq!(ResearchStatus::parse("unknown"), None);
        assert_eq!(ResearchStatus::parse("DRAFT"), None);
    }

    #[test]
    fn terminal_predicate() {
        assert!(ResearchStatus::Archived.is_terminal());
        assert!(!ResearchStatus::Draft.is_terminal());
        assert!(!ResearchStatus::InProgress.is_terminal());
        assert!(!ResearchStatus::Complete.is_terminal());
    }

    #[test]
    fn finished_predicate() {
        assert!(ResearchStatus::Complete.is_finished());
        assert!(!ResearchStatus::Draft.is_finished());
        assert!(!ResearchStatus::InProgress.is_finished());
        assert!(!ResearchStatus::Archived.is_finished());
    }

    #[test]
    fn serde_round_trip() {
        for &status in ResearchStatus::ALL {
            let json = serde_json::to_string(&status).unwrap();
            let back: ResearchStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn display_matches_as_str() {
        for &status in ResearchStatus::ALL {
            assert_eq!(status.to_string(), status.as_str());
        }
    }
}
