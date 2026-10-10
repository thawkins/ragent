//! Automation run records (spec `openhands` T-016; FR-018).
//!
//! FR-018 requires a terminal automation run to append a run-history record
//! capturing the automation id, the trigger kind, start and end times, the
//! outcome, the selected execution backend, and a reference to the run output.
//! These are the shared types for that record; the durable store lives in
//! `ragent-storage` (`Storage::insert_automation_run` et al.).
//!
//! Timestamps are stored as RFC 3339 (UTC) strings in SQLite and carried here
//! as [`chrono::DateTime<Utc>`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// How an automation run was triggered (FR-013, FR-014, FR-018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationTrigger {
    /// An inbound webhook POST (FR-013).
    Webhook,
    /// The scheduler reached the due time (FR-014).
    Schedule,
    /// A manual run (the TUI `/automation run` or CLI `automation run`
    /// surface).
    Manual,
}

impl AutomationTrigger {
    /// The stable lowercase label persisted and rendered (`webhook` /
    /// `schedule` / `manual`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::Schedule => "schedule",
            Self::Manual => "manual",
        }
    }

    /// Parse a stored label back into a trigger, defaulting to [`Self::Manual`]
    /// for an unrecognised value so a corrupt row never fails a read.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label {
            "webhook" => Self::Webhook,
            "schedule" => Self::Schedule,
            _ => Self::Manual,
        }
    }
}

impl std::fmt::Display for AutomationTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The terminal outcome of an automation run (FR-018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    /// The run completed successfully.
    Success,
    /// The run failed (agent error, provisioning failure, or dispatch error).
    Error,
    /// The run was cancelled before completion.
    Cancelled,
}

impl RunOutcome {
    /// The stable lowercase label persisted and rendered.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse a stored label back into an outcome, defaulting to [`Self::Error`]
    /// for an unrecognised value so a corrupt row never reads as a success.
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label {
            "success" => Self::Success,
            "cancelled" => Self::Cancelled,
            _ => Self::Error,
        }
    }
}

impl std::fmt::Display for RunOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A durable automation run-history record (FR-018).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRun {
    /// Unique run identifier.
    pub id: String,
    /// The automation id this run belongs to.
    pub automation_id: String,
    /// How the run was triggered.
    pub trigger: AutomationTrigger,
    /// When the run started (UTC).
    pub started_at: DateTime<Utc>,
    /// When the run reached a terminal state (UTC), or `None` while running.
    pub ended_at: Option<DateTime<Utc>>,
    /// The terminal outcome, or `None` while running.
    pub outcome: Option<RunOutcome>,
    /// The execution backend the run was confined to (FR-033).
    pub backend: String,
    /// A reference to the run output: a child session id or an output-file path.
    pub output_ref: Option<String>,
}

impl AutomationRun {
    /// Create a run record started now with no terminal state yet.
    #[must_use]
    pub fn start(
        id: String,
        automation_id: String,
        trigger: AutomationTrigger,
        backend: String,
    ) -> Self {
        Self {
            id,
            automation_id,
            trigger,
            started_at: Utc::now(),
            ended_at: None,
            outcome: None,
            backend,
            output_ref: None,
        }
    }

    /// Whether the run has reached a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.ended_at.is_some()
    }
}
