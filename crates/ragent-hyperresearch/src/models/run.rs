//! Research run record (FR-021, FR-031).

use serde::{Deserialize, Serialize};

/// Terminal status of a research run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Still executing.
    Running,
    /// Finished normally.
    Completed,
    /// Stopped on an error.
    Failed,
    /// Stopped by the user.
    Cancelled,
}

/// One persisted research run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Run id.
    pub id: String,
    /// The question the run answered.
    pub query: String,
    /// Research profile used (FR-031).
    pub profile: String,
    /// Providers the run used, in order.
    #[serde(default)]
    pub providers: Vec<String>,
    /// Number of sources fetched.
    #[serde(default)]
    pub sources_fetched: u64,
    /// Number of claims extracted.
    #[serde(default)]
    pub claims_extracted: u64,
    /// Search width applied by the effort controls (FR-031).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_width: Option<u32>,
    /// Terminal status.
    pub status: RunStatus,
    /// Start timestamp, RFC 3339 UTC.
    pub started: String,
    /// End timestamp, RFC 3339 UTC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished: Option<String>,
}
