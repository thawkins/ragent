//! Append-only log of member-status transitions, per team (spec `teamssetup`
//! FR-036).
//!
//! FR-035 makes every status write publish one `Event::TeammateStatusChanged`
//! carrying `from`, `to`, and `cause`, which is enough to reconstruct a member's
//! history *while someone is listening*. A tool call is not a listener: by the
//! time a lead runs `team_status` to ask "why did this teammate stop?", the
//! events that answer it have already gone past. FR-036 therefore requires the
//! tool surface to report the last five transitions per member with timestamps,
//! which means the transitions must be on disk.
//!
//! The log is `status-history.jsonl`, one JSON object per line:
//!
//! ```json
//! {"timestamp":"2026-08-01T09:15:00.123456789Z","agent_id":"tm-001",
//!  "name":"alpha","from":"working","to":"idle","cause":"task_completed"}
//! ```
//!
//! It is written by [`TeamManager::set_member_status`] - the single status
//! writer FR-008 establishes - so a transition cannot be persisted without
//! being logged and cannot be logged without being persisted. Nothing else
//! appends to it.
//!
//! [`TeamManager::set_member_status`]: crate::team::manager::TeamManager::set_member_status
//!
//! ## Bounds (NFR-004)
//!
//! Append-only and bounded: before an append that would take the file past
//! [`MAX_HISTORY_BYTES`], it is renamed to `status-history.jsonl.1` (replacing
//! any previous one) and the new record starts a fresh file. That makes the
//! history a rolling window rather than an unbounded growth vector, which is
//! the failure mode a per-transition log invites.
//!
//! ## Failure policy
//!
//! Recording is best-effort: the history is observability, not a gate, so a
//! filesystem error is logged and swallowed rather than turned into a status
//! write failure. A status write never fails because its history line could
//! not be appended.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::team::config::{MemberStatus, TransitionCause};

/// Name of the live history file inside a team directory.
pub const STATUS_HISTORY_FILE: &str = "status-history.jsonl";

/// Name the live history is rotated to when it reaches the cap.
pub const STATUS_HISTORY_ROTATED_FILE: &str = "status-history.jsonl.1";

/// Maximum size of the live history before it is rotated (NFR-004: 1 MiB).
pub const MAX_HISTORY_BYTES: u64 = 1024 * 1024;

/// How many transitions [`render_status_history`] reports per member (FR-036).
pub const STATUS_HISTORY_LIMIT: usize = 5;

/// One recorded member-status transition.
///
/// `from` and `to` are the canonical [`MemberStatus::as_str`] tokens and
/// `cause` the canonical [`TransitionCause::as_str`] token, so the file is
/// readable by an operator who has never seen the enum and by a future release
/// that has parsed a status with [`MemberStatus::from_wire`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusHistoryEntry {
    /// When the transition was committed (UTC).
    pub timestamp: DateTime<Utc>,
    /// The member's agent ID (`tm-NNN`).
    pub agent_id: String,
    /// The member's human-facing name, recorded so a member removed from the
    /// roster is still identifiable in its own history.
    pub name: String,
    /// The status the member held before the write (`snake_case`).
    pub from: String,
    /// The status the member holds now (`snake_case`).
    pub to: String,
    /// Why the member moved (`snake_case`).
    pub cause: String,
}

impl StatusHistoryEntry {
    /// Build an entry, stamping it with the current time.
    #[must_use]
    pub fn new(
        agent_id: &str,
        name: &str,
        from: MemberStatus,
        to: MemberStatus,
        cause: TransitionCause,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            agent_id: agent_id.to_string(),
            name: name.to_string(),
            from: from.as_str().to_string(),
            to: to.as_str().to_string(),
            cause: cause.as_str().to_string(),
        }
    }

    /// The status word the member now holds, decoded from `to`.
    ///
    /// Falls back to the raw token when it is not one of the canonical ones, so
    /// a line written by a future release still renders something truthful
    /// instead of being dropped.
    #[must_use]
    pub fn to_status(&self) -> Option<MemberStatus> {
        MemberStatus::from_wire(&self.to)
    }

    /// The one-line human rendering used by `team_status` (FR-036).
    ///
    /// Both status words and the cause go through the canonical mapping, so a
    /// history line spells a state exactly as the roster row above it does.
    #[must_use]
    pub fn report_line(&self) -> String {
        let from = MemberStatus::from_wire(&self.from).map_or_else(
            || self.from.clone(),
            |s| crate::team::status_label(s).to_string(),
        );
        let to = MemberStatus::from_wire(&self.to).map_or_else(
            || self.to.clone(),
            |s| crate::team::status_label(s).to_string(),
        );
        format!(
            "    {} {} -> {} (cause: {})",
            self.timestamp.format("%Y-%m-%dT%H:%M:%SZ"),
            from,
            to,
            self.cause
        )
    }
}

/// The live history path for a team directory.
#[must_use]
pub fn history_path(team_dir: &Path) -> PathBuf {
    team_dir.join(STATUS_HISTORY_FILE)
}

/// The rotated (previous generation) history path for a team directory.
#[must_use]
pub fn rotated_history_path(team_dir: &Path) -> PathBuf {
    team_dir.join(STATUS_HISTORY_ROTATED_FILE)
}

/// Append one transition to the team's history, rotating when it is full.
///
/// Best-effort: an I/O error is logged and swallowed, because the history must
/// never fail the status write it is describing.
pub fn record_status_transition(team_dir: &Path, entry: &StatusHistoryEntry) {
    if let Err(e) = try_record_status_transition(team_dir, entry) {
        tracing::warn!(
            team_dir = %team_dir.display(),
            agent_id = %entry.agent_id,
            error = %e,
            "Cannot append to the team status history"
        );
    }
}

/// The fallible body of [`record_status_transition`], exposed so tests can
/// assert on the write rather than on a swallowed error.
pub fn try_record_status_transition(
    team_dir: &Path,
    entry: &StatusHistoryEntry,
) -> std::io::Result<()> {
    fs::create_dir_all(team_dir)?;
    let path = history_path(team_dir);

    // NFR-004: check the size *before* the append so the live file is bounded
    // without ever rewriting it (append-only outside rotation).
    let current = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if current >= MAX_HISTORY_BYTES {
        let rotated = path.with_file_name(STATUS_HISTORY_ROTATED_FILE);
        fs::rename(&path, &rotated)?;
    }

    let mut line = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    line.push('\n');

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    file.flush()
}

/// Read the live history, oldest first.
///
/// A line that is not valid JSON is skipped rather than failing the read: the
/// history is diagnostic state, and one truncated final line (a crash mid-write)
/// must not hide the rest. A missing file is an empty history.
#[must_use]
pub fn read_status_history(team_dir: &Path) -> Vec<StatusHistoryEntry> {
    let Ok(text) = fs::read_to_string(history_path(team_dir)) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<StatusHistoryEntry>(line).ok())
        .collect()
}

/// The last [`STATUS_HISTORY_LIMIT`] transitions recorded for `agent_id`,
/// oldest first, so a reader walks the member forwards through time.
#[must_use]
pub fn recent_status_history(
    team_dir: &Path,
    agent_id: &str,
    limit: usize,
) -> Vec<StatusHistoryEntry> {
    let all = read_status_history(team_dir);
    let member: Vec<StatusHistoryEntry> = all
        .into_iter()
        .filter(|entry| entry.agent_id == agent_id)
        .collect();
    // Keep the newest `limit` but preserve chronological order, so the last
    // line is the member's current status.
    let start = member.len().saturating_sub(limit);
    member[start..].to_vec()
}

/// Render a member's history as a report block for `team_status` (FR-036).
///
/// Returns an empty string when the member has no recorded transitions, so a
/// freshly spawned teammate does not print an empty heading. When entries
/// exist the block names the member, the limit, and the file the rows came
/// from - a five-row window is only honest if the reader can tell it is a
/// window.
#[must_use]
pub fn render_status_history(entries: &[StatusHistoryEntry], heading_level: usize) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let hashes = "#".repeat(heading_level);
    let name = entries
        .last()
        .map(|entry| entry.name.clone())
        .unwrap_or_default();
    let agent_id = entries
        .last()
        .map(|entry| entry.agent_id.clone())
        .unwrap_or_default();
    let mut out = format!(
        "{hashes} Status history (last {} transition(s) of {name} {agent_id})\n\n",
        entries.len()
    );
    for entry in entries {
        out.push_str(&entry.report_line());
        out.push('\n');
    }
    out.push_str(&format!(
        "\nNewest last. Rows come from `{STATUS_HISTORY_FILE}`; it rotates to \
         `{STATUS_HISTORY_ROTATED_FILE}` at 1 MiB, so an older transition may \
         have rolled off.\n"
    ));
    out
}
