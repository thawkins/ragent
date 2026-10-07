//! Cron-event execution logging for the agent cron system (spec `agentchron`).
//!
//! When a scheduled cron event fires, the scheduler writes a single JSON line
//! to a log file in `<working_dir>/log/cron-<timestamp>.jsonl`. Each line records
//! the timestamp, event id, agent type, prompt, schedule, outcome, error, and
//! run id.
//!
//! This mirrors the existing edit-log JSONL convention in
//! [`crate::edit_log`] (`log/edits-<timestamp>.jsonl`): best-effort append, a
//! "pick most recent file" helper, and graceful failure via `tracing::warn`.
//!
//! Unlike the edit log, cron logging is always enabled - it is the primary
//! audit trail for scheduled agent runs (FR-003, FR-006). There is no runtime
//! toggle.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

/// The outcome of a single cron event execution.
///
/// Recorded in the JSONL log entry as the `outcome` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronOutcome {
    /// The agent run completed successfully.
    Success,
    /// The agent run failed (e.g. unknown agent type, spawn error).
    Error,
    /// The event was skipped (disabled, or a previous run is still active).
    Skipped,
}

impl CronOutcome {
    /// Returns the string label used in log entries.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }
}

/// A single cron execution log entry, serialised as one JSON line.
///
/// This struct is the deserialised form of a line in
/// `log/cron-<timestamp>.jsonl`. It is used by [`read_cron_log`] to parse the
/// log back into structured records for the `/cron log` sub-command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronLogEntry {
    /// When the execution happened (RFC 3339).
    pub timestamp: String,
    /// The event id that triggered this execution.
    pub event_id: String,
    /// The agent type that was run.
    pub agent_type: String,
    /// The initial prompt passed to the agent.
    pub prompt: String,
    /// The raw schedule expression (e.g. `every 30m`).
    pub schedule: String,
    /// The outcome: `success`, `error`, or `skipped`.
    pub outcome: String,
    /// Error message if the outcome was `error`, otherwise `null`.
    pub error: Option<String>,
    /// The session/run id of the spawned agent run, if any.
    pub run_id: Option<String>,
}

/// Build the path to the log directory (`<working_dir>/log`).
fn log_dir(working_dir: &Path) -> PathBuf {
    working_dir.join("log")
}

/// Build a unique cron log file path based on the current UTC timestamp.
fn log_file_path(log_dir: &Path) -> PathBuf {
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%6f").to_string();
    log_dir.join(format!("cron-{timestamp}.jsonl"))
}

/// Write a single JSON log entry for a cron event execution.
///
/// This is a best-effort operation: failures are logged with `tracing::warn`
/// but are never propagated to the caller, so a scheduling problem cannot fail
/// because of a logging problem.
///
/// # Arguments
///
/// * `working_dir` - project working directory; the `log/` subdirectory is
///   created here if needed.
/// * `event_id` - the id of the cron event that fired.
/// * `agent_type` - the agent type that was run.
/// * `prompt` - the initial prompt passed to the agent.
/// * `schedule` - the raw schedule expression string.
/// * `outcome` - the [`CronOutcome`] of the execution.
/// * `error` - error message if the outcome was `Error`, otherwise `None`.
/// * `run_id` - the session/run id of the spawned agent run, if any.
#[allow(clippy::too_many_arguments)]
pub fn log_cron_execution(
    working_dir: &Path,
    event_id: &str,
    agent_type: &str,
    prompt: &str,
    schedule: &str,
    outcome: CronOutcome,
    error: Option<&str>,
    run_id: Option<&str>,
) {
    let log_dir = log_dir(working_dir);
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        tracing::warn!(
            "cron_log: failed to create log directory {}: {e}",
            log_dir.display()
        );
        return;
    }

    let path = crate::jsonl_log::pick_most_recent(&log_dir, "cron-")
        .unwrap_or_else(|| log_file_path(&log_dir));

    let entry = json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "event_id": event_id,
        "agent_type": agent_type,
        "prompt": prompt,
        "schedule": schedule,
        "outcome": outcome.as_str(),
        "error": error,
        "run_id": run_id,
    });

    if let Err(e) = crate::jsonl_log::append_json_line(&path, &entry) {
        tracing::warn!("cron_log: failed to append to {}: {e}", path.display());
    }
}

/// Read all cron execution log entries from the `log/` directory.
///
/// Parses every `cron-*.jsonl` file in `<working_dir>/log`, returning entries
/// in chronological order (oldest first). Entries that fail to parse are
/// skipped with a `tracing::warn`.
///
/// Optionally filter by `event_id` - when `Some(id)` is passed, only entries
/// matching that event id are returned (FR-013).
///
/// # Arguments
///
/// * `working_dir` - project working directory containing the `log/` folder.
/// * `event_id` - optional event id filter.
#[must_use]
pub fn read_cron_log(working_dir: &Path, event_id: Option<&str>) -> Vec<CronLogEntry> {
    let log_dir = log_dir(working_dir);
    let mut entries: Vec<CronLogEntry> = Vec::new();

    let Ok(files) = std::fs::read_dir(&log_dir) else {
        return entries;
    };

    // Collect and sort files by name (timestamp prefix) for chronological order.
    let mut paths: Vec<PathBuf> = files
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("cron-")
                && Path::new(&name)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
        })
        .map(|e| e.path())
        .collect();
    paths.sort();

    for path in paths {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<CronLogEntry>(line) {
                Ok(entry) => {
                    if event_id.is_none_or(|id| entry.event_id == id) {
                        entries.push(entry);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "cron_log: failed to parse {}:{}: {e}",
                        path.display(),
                        lineno + 1
                    );
                }
            }
        }
    }

    entries
}

/// Delete all `cron-*.jsonl` files in the log directory.
///
/// Returns the number of files removed. Used by tests for cleanup.
#[allow(dead_code)]
pub fn clear_cron_logs(working_dir: &Path) -> usize {
    let log_dir = log_dir(working_dir);
    if !log_dir.exists() {
        return 0;
    }

    let mut removed = 0usize;
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("cron-")
                && name_str.ends_with(".jsonl")
                && std::fs::remove_file(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
    }
    removed
}

#[cfg(test)]
#[path = "../tests/inline/cron_log_tests.rs"]
mod tests;
