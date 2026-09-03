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
//! Unlike the edit log, cron logging is always enabled — it is the primary
//! audit trail for scheduled agent runs (FR-003, FR-006). There is no runtime
//! toggle.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

/// Pick the most-recently-modified `cron-*.jsonl` file in `log_dir`, or create
/// a new timestamped path if none exists.
///
/// Mirrors [`crate::edit_log`] `pick_log_file` so that all executions within a
/// session append to the same file.
fn pick_log_file(log_dir: &Path) -> PathBuf {
    let mut latest: Option<PathBuf> = None;
    let mut latest_mtime: Option<SystemTime> = None;

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("cron-") && name_str.ends_with(".jsonl") {
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                if latest_mtime.is_none_or(|lm| mtime > lm) {
                    latest_mtime = Some(mtime);
                    latest = Some(entry.path());
                }
            }
        }
    }

    latest.unwrap_or_else(|| log_file_path(log_dir))
}

/// Append a JSON value as a single line to the given file, creating it if
/// needed.
fn append_json_line(path: &Path, value: &Value) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    file.flush()?;
    Ok(())
}

/// Write a single JSON log entry for a cron event execution.
///
/// This is a best-effort operation: failures are logged with `tracing::warn`
/// but are never propagated to the caller, so a scheduling problem cannot fail
/// because of a logging problem.
///
/// # Arguments
///
/// * `working_dir` — project working directory; the `log/` subdirectory is
///   created here if needed.
/// * `event_id` — the id of the cron event that fired.
/// * `agent_type` — the agent type that was run.
/// * `prompt` — the initial prompt passed to the agent.
/// * `schedule` — the raw schedule expression string.
/// * `outcome` — the [`CronOutcome`] of the execution.
/// * `error` — error message if the outcome was `Error`, otherwise `None`.
/// * `run_id` — the session/run id of the spawned agent run, if any.
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

    let path = pick_log_file(&log_dir);

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

    if let Err(e) = append_json_line(&path, &entry) {
        tracing::warn!("cron_log: failed to append to {}: {e}", path.display());
    }
}

/// Read all cron execution log entries from the `log/` directory.
///
/// Parses every `cron-*.jsonl` file in `<working_dir>/log`, returning entries
/// in chronological order (oldest first). Entries that fail to parse are
/// skipped with a `tracing::warn`.
///
/// Optionally filter by `event_id` — when `Some(id)` is passed, only entries
/// matching that event id are returned (FR-013).
///
/// # Arguments
///
/// * `working_dir` — project working directory containing the `log/` folder.
/// * `event_id` — optional event id filter.
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
mod tests {
    use super::*;

    #[test]
    fn test_log_dir_resolves_under_working_dir() {
        assert_eq!(
            log_dir(Path::new("/project")),
            PathBuf::from("/project/log")
        );
    }

    #[test]
    fn test_cron_outcome_as_str() {
        assert_eq!(CronOutcome::Success.as_str(), "success");
        assert_eq!(CronOutcome::Error.as_str(), "error");
        assert_eq!(CronOutcome::Skipped.as_str(), "skipped");
    }

    #[test]
    fn test_cron_outcome_serde_roundtrip() {
        for o in [
            CronOutcome::Success,
            CronOutcome::Error,
            CronOutcome::Skipped,
        ] {
            let json = serde_json::to_string(&o).unwrap();
            let back: CronOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(o, back);
        }
    }

    #[test]
    fn test_log_cron_execution_writes_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();

        log_cron_execution(
            wd,
            "cron-test-1",
            "general",
            "Run tests",
            "every 30m",
            CronOutcome::Success,
            None,
            Some("session-123"),
        );

        let entries = read_cron_log(wd, None);
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.event_id, "cron-test-1");
        assert_eq!(e.agent_type, "general");
        assert_eq!(e.prompt, "Run tests");
        assert_eq!(e.schedule, "every 30m");
        assert_eq!(e.outcome, "success");
        assert!(e.error.is_none());
        assert_eq!(e.run_id.as_deref(), Some("session-123"));
        assert_ne!(e.timestamp, "", "timestamp should not be empty");

        clear_cron_logs(wd);
    }

    #[test]
    fn test_log_cron_execution_error_with_message() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();

        log_cron_execution(
            wd,
            "cron-test-2",
            "unknown-agent",
            "Do thing",
            "at 2025-01-01T00:00:00Z",
            CronOutcome::Error,
            Some("Unknown agent type: unknown-agent"),
            None,
        );

        let entries = read_cron_log(wd, None);
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.outcome, "error");
        assert_eq!(
            e.error.as_deref(),
            Some("Unknown agent type: unknown-agent")
        );
        assert!(e.run_id.is_none());

        clear_cron_logs(wd);
    }

    #[test]
    fn test_log_cron_execution_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();

        log_cron_execution(
            wd,
            "cron-test-3",
            "general",
            "Run tests",
            "every 1h",
            CronOutcome::Skipped,
            None,
            None,
        );

        let entries = read_cron_log(wd, None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].outcome, "skipped");

        clear_cron_logs(wd);
    }

    #[test]
    fn test_read_cron_log_filters_by_event_id() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();

        log_cron_execution(
            wd,
            "cron-a",
            "general",
            "A",
            "every 30m",
            CronOutcome::Success,
            None,
            None,
        );
        log_cron_execution(
            wd,
            "cron-b",
            "build",
            "B",
            "every 1h",
            CronOutcome::Error,
            Some("oops"),
            None,
        );
        log_cron_execution(
            wd,
            "cron-a",
            "general",
            "A2",
            "every 30m",
            CronOutcome::Success,
            None,
            None,
        );

        let all = read_cron_log(wd, None);
        assert_eq!(all.len(), 3);

        let filtered = read_cron_log(wd, Some("cron-a"));
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.event_id == "cron-a"));

        let filtered_b = read_cron_log(wd, Some("cron-b"));
        assert_eq!(filtered_b.len(), 1);
        assert_eq!(filtered_b[0].event_id, "cron-b");

        clear_cron_logs(wd);
    }

    #[test]
    fn test_read_cron_log_multiple_entries_chronological() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();

        for i in 0..5 {
            log_cron_execution(
                wd,
                &format!("cron-{i}"),
                "general",
                &format!("Prompt {i}"),
                "every 30m",
                CronOutcome::Success,
                None,
                Some(&format!("run-{i}")),
            );
        }

        let entries = read_cron_log(wd, None);
        assert_eq!(entries.len(), 5);
        // All entries should be present and in file order.
        for (i, e) in entries.iter().enumerate() {
            assert_eq!(e.event_id, format!("cron-{i}"));
            assert_eq!(e.run_id.as_deref(), Some(format!("run-{i}").as_str()));
        }

        clear_cron_logs(wd);
    }

    #[test]
    fn test_read_cron_log_empty_when_no_files() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();
        let entries = read_cron_log(wd, None);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_read_cron_log_skips_non_cron_files() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();
        let log_dir = wd.join("log");
        std::fs::create_dir_all(&log_dir).unwrap();

        // Write an edits-*.jsonl file (should be ignored).
        std::fs::write(log_dir.join("edits-20250101-120000.jsonl"), "{}\n").unwrap();
        // Write a valid cron file.
        std::fs::write(
            log_dir.join("cron-20250101-120000.jsonl"),
            r#"{"timestamp":"2025-01-01T12:00:00Z","event_id":"cron-x","agent_type":"general","prompt":"p","schedule":"every 1m","outcome":"success","error":null,"run_id":null}"#,
        )
        .unwrap();

        let entries = read_cron_log(wd, None);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].event_id, "cron-x");

        clear_cron_logs(wd);
    }

    #[test]
    fn test_clear_cron_logs_removes_files() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();

        log_cron_execution(
            wd,
            "cron-1",
            "general",
            "p",
            "every 1m",
            CronOutcome::Success,
            None,
            None,
        );
        log_cron_execution(
            wd,
            "cron-2",
            "general",
            "p",
            "every 1m",
            CronOutcome::Success,
            None,
            None,
        );

        assert!(!read_cron_log(wd, None).is_empty());
        let removed = clear_cron_logs(wd);
        assert!(removed >= 1);
        assert!(read_cron_log(wd, None).is_empty());
    }
}
