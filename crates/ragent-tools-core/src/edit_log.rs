//! Edit-operation instrumentation for `edit` and `multi_edit`.
//!
//! When enabled, every `edit` and `multi_edit` invocation writes a single JSON
//! line to a log file in `<working_dir>/log/edits-<timestamp>.jsonl`. Each line
//! records the timestamp, the target file path, the search/replacement text,
//! and the outcome.
//!
//! Logging is controlled by a process-wide atomic flag that is toggled via the
//! `/editlog on|off` slash command in the TUI.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use serde_json::{Value, json};

/// Process-wide switch that enables or disables edit logging.
///
/// The default is `false`. The TUI's `/editlog on|off` command toggles this.
static EDIT_LOG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Returns the current edit-log enabled state.
#[must_use]
pub fn is_edit_log_enabled() -> bool {
    EDIT_LOG_ENABLED.load(Ordering::Relaxed)
}

/// Enables or disables edit logging.
pub fn set_edit_log_enabled(enabled: bool) {
    EDIT_LOG_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Build the path to the log directory (`<working_dir>/log`).
fn log_dir(working_dir: &Path) -> PathBuf {
    working_dir.join("log")
}

/// Build a unique log file path based on the current UTC timestamp.
fn log_file_path(log_dir: &Path) -> PathBuf {
    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S-%6f").to_string();
    log_dir.join(format!("edits-{timestamp}.jsonl"))
}

/// Write a single JSON log entry for an edit operation.
///
/// This is a best-effort operation: failures are logged with `tracing::warn`
/// but are never propagated to the caller, so an edit cannot fail because of a
/// logging problem.
///
/// # Arguments
///
/// * `working_dir` — project working directory; the `log/` subdirectory is
///   created here if needed.
/// * `tool` — name of the tool that performed the edit (`"edit"` or
///   `"multi_edit"`).
/// * `file_path` — absolute or project-relative path of the edited file.
/// * `old_str` — search string that was replaced.
/// * `new_str` — replacement string that was inserted.
/// * `outcome` — `"success"` or an error message.
/// * `dry_run` — whether the edit was a dry-run preview.
pub fn log_edit_operation(
    working_dir: &Path,
    tool: &str,
    file_path: &Path,
    old_str: &str,
    new_str: &str,
    outcome: &str,
    dry_run: bool,
) {
    if !is_edit_log_enabled() {
        return;
    }

    let log_dir = log_dir(working_dir);
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        tracing::warn!(
            "edit_log: failed to create log directory {}: {e}",
            log_dir.display()
        );
        return;
    }

    // Pick an existing log file from the current session, or start a new one.
    let path = pick_log_file(&log_dir);

    let entry = json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "tool": tool,
        "file_path": file_path.display().to_string(),
        "old_str": old_str,
        "new_str": new_str,
        "outcome": outcome,
        "dry_run": dry_run,
    });

    if let Err(e) = append_json_line(&path, &entry) {
        tracing::warn!("edit_log: failed to append to {}: {e}", path.display());
    }
}

/// Pick the most recent `edits-*.jsonl` file in the log directory, or create
/// a new one if none exists. This keeps a single session's edits in one file.
fn pick_log_file(log_dir: &Path) -> PathBuf {
    let mut latest: Option<PathBuf> = None;
    let mut latest_mtime: Option<SystemTime> = None;

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("edits-") && name_str.ends_with(".jsonl") {
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

/// Append a JSON value as a single line to the given file, creating it if needed.
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

/// Summarise the edit log for the given working directory.
///
/// Returns `(count, success_count, fail_count, percentage_success)`. If no
/// log files exist or cannot be parsed, the counts are zero.
#[must_use]
pub fn edit_log_summary(working_dir: &Path) -> (usize, usize, usize, f64) {
    let log_dir = log_dir(working_dir);
    if !log_dir.exists() {
        return (0, 0, 0, 0.0);
    }

    let mut total = 0usize;
    let mut success = 0usize;

    let entries = match std::fs::read_dir(&log_dir) {
        Ok(e) => e,
        Err(_) => return (0, 0, 0, 0.0),
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !(name_str.starts_with("edits-") && name_str.ends_with(".jsonl")) {
            continue;
        }
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                total += 1;
                if value.get("outcome").and_then(|v| v.as_str()) == Some("success") {
                    success += 1;
                }
            }
        }
    }

    let fail = total.saturating_sub(success);
    let pct = if total == 0 {
        0.0
    } else {
        (success as f64 / total as f64) * 100.0
    };

    (total, success, fail, pct)
}

/// Delete all edit-log files in the working directory's `log/` subdirectory.
///
/// Returns the number of files removed. The `log/` directory itself is left
/// intact; only matching `edits-*.jsonl` files are deleted.
pub fn clear_edit_logs(working_dir: &Path) -> usize {
    let log_dir = log_dir(working_dir);
    if !log_dir.exists() {
        return 0;
    }

    let mut removed = 0usize;
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("edits-")
                && name_str.ends_with(".jsonl")
                && std::fs::remove_file(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
    }
    removed
}

/// Empty the contents of every edit-log file without deleting the files.
///
/// Truncates each `edits-*.jsonl` file in the log directory to zero bytes and
/// returns the number of files cleared. The `log/` directory itself is
/// preserved (created if missing) so that future edit operations can keep
/// logging to the same location.
pub fn clear_edit_log_contents(working_dir: &Path) -> usize {
    let log_dir = log_dir(working_dir);
    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
        return 0;
    }

    let mut cleared = 0usize;
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("edits-") && name_str.ends_with(".jsonl") {
                if let Ok(file) = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(entry.path())
                {
                    drop(file);
                    cleared += 1;
                }
            }
        }
    }
    cleared
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn log_dir_resolves_under_working_dir() {
        assert_eq!(
            log_dir(Path::new("/project")),
            PathBuf::from("/project/log")
        );
    }

    #[test]
    fn summary_counts_success_and_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();
        set_edit_log_enabled(true);

        log_edit_operation(
            wd,
            "edit",
            Path::new("a.rs"),
            "old",
            "new",
            "success",
            false,
        );
        log_edit_operation(wd, "edit", Path::new("b.rs"), "x", "y", "not found", false);
        log_edit_operation(
            wd,
            "multi_edit",
            Path::new("c.rs"),
            "x",
            "y",
            "success",
            true,
        );

        let (total, success, fail, pct) = edit_log_summary(wd);
        assert_eq!(total, 3);
        assert_eq!(success, 2);
        assert_eq!(fail, 1);
        assert!((pct - 66.67).abs() < 0.01, "expected ~66.67%, got {pct}");

        set_edit_log_enabled(false);
        clear_edit_logs(wd);
    }

    #[test]
    fn disabled_logging_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();
        set_edit_log_enabled(false);
        log_edit_operation(
            wd,
            "edit",
            Path::new("a.rs"),
            "old",
            "new",
            "success",
            false,
        );
        assert_eq!(edit_log_summary(wd).0, 0);
    }
}
