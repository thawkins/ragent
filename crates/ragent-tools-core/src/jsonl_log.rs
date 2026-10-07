//! Shared JSONL log helpers (T-305).
//!
//! Both the edit log ([`crate::edit_log`]) and the cron execution log
//! ([`crate::cron_log`]) append one JSON object per line to a timestamped
//! `*.jsonl` file and pick the most-recently-modified file so a session's
//! records stay in one file. Those two routines had drifted into byte-identical
//! copies; they now delegate here.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

/// Pick the most-recently-modified file in `log_dir` whose name starts with
/// `prefix` and ends with `.jsonl`.
///
/// Returns `None` when `log_dir` does not exist or contains no matching file,
/// letting the caller fall back to a freshly timestamped path.
// reason: the containing module is `pub(crate)`, so `pub` here never escapes
// the crate (mirrors `edit_common`).
#[allow(unreachable_pub)]
pub fn pick_most_recent(log_dir: &Path, prefix: &str) -> Option<PathBuf> {
    let mut latest: Option<PathBuf> = None;
    let mut latest_mtime: Option<SystemTime> = None;

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(prefix) && name_str.ends_with(".jsonl") {
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

    latest
}

/// Append a JSON value as a single line to the given file, creating it if
/// needed.
// reason: the containing module is `pub(crate)`, so `pub` here never escapes
// the crate (mirrors `edit_common`).
#[allow(unreachable_pub)]
pub fn append_json_line(path: &Path, value: &Value) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    file.flush()?;
    Ok(())
}
