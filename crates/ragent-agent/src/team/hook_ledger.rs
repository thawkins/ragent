//! Append-only ledger of hook executions that never reached a verdict.
//!
//! Spec `teamssetup` FR-019 / NFR-004. Before this module a quality-gate hook
//! that could not run - a command refused by
//! [`validate_hook_command`](crate::team::manager::validate_hook_command), a
//! command that failed to spawn, or a command killed at the 30-second
//! [`HOOK_TIMEOUT`](crate::team::manager::HOOK_TIMEOUT) - returned
//! `HookOutcome::Allow` and left nothing behind. An inert hook was therefore
//! indistinguishable from a hook that had run and approved, which is finding
//! F-14.
//!
//! The ledger records each such failure as one JSON object per line in the
//! team's `hook-failures.jsonl`:
//!
//! ```json
//! {"timestamp":"2026-08-01T09:15:00.123456789Z","event":"TaskCompleted",
//!  "hook":"echo > out.txt","reason":"invalid","detail":"hook command contains
//!  a shell metacharacter"}
//! ```
//!
//! ## Bounds (NFR-004)
//!
//! The file is append-only and bounded: before an append that would take it
//! past [`MAX_LEDGER_BYTES`], the whole file is renamed to
//! `hook-failures.jsonl.1` (replacing any previous one) and the new record
//! starts a fresh `hook-failures.jsonl`. A reader therefore sees one live file
//! of at most one record more than the cap - the cap is checked before the
//! write, and a single record is far smaller than 1 MiB - plus at most one
//! previous generation.
//!
//! ## Failure policy
//!
//! Recording is best-effort by design: the ledger is observability, not a gate,
//! so a filesystem error is logged and swallowed rather than turned into a hook
//! verdict the runtime would then have to invent. `run_hook` never gains a new
//! failure mode from this module.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::team::config::HookEvent;

/// Name of the live ledger file inside a team directory.
pub const HOOK_FAILURES_FILE: &str = "hook-failures.jsonl";

/// Name the live ledger is rotated to when it reaches the cap.
pub const HOOK_FAILURES_ROTATED_FILE: &str = "hook-failures.jsonl.1";

/// Maximum size of the live ledger before it is rotated (NFR-004: 1 MiB).
pub const MAX_LEDGER_BYTES: u64 = 1024 * 1024;

/// Why a hook produced no verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookFailureReason {
    /// The command was refused by `validate_hook_command` before it was spawned.
    Invalid,
    /// The command could not be spawned (missing binary, permission denied).
    SpawnFailed,
    /// The command exceeded `HOOK_TIMEOUT` and was killed.
    Timeout,
}

impl HookFailureReason {
    /// The wire token, matching the on-disk `snake_case` form.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::SpawnFailed => "spawn_failed",
            Self::Timeout => "timeout",
        }
    }
}

/// One ledger line: a hook that was configured but never reached a verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookFailure {
    /// When the hook was attempted (UTC).
    pub timestamp: DateTime<Utc>,
    /// The lifecycle event the hook was registered for.
    pub event: HookEvent,
    /// The configured command, so an operator can identify the hook without
    /// re-reading `config.json` (which may have changed since).
    pub hook: String,
    /// Which of the three failure kinds occurred.
    pub reason: HookFailureReason,
    /// The specific cause, e.g. `hook command contains a shell metacharacter`.
    pub detail: String,
}

impl HookFailure {
    /// Build a record, stamping it with the current time.
    #[must_use]
    pub fn new(event: HookEvent, hook: &str, reason: HookFailureReason, detail: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            event,
            hook: hook.to_string(),
            reason,
            detail: detail.to_string(),
        }
    }

    /// The one-line human rendering used by `/team show` and `team_status`.
    #[must_use]
    pub fn report_line(&self) -> String {
        format!(
            "  - {} {} hook `{}`: {}\n",
            self.timestamp.format("%Y-%m-%dT%H:%M:%SZ"),
            self.event.as_str(),
            self.hook,
            self.detail
        )
    }
}

/// The live ledger path for a team directory.
#[must_use]
pub fn ledger_path(team_dir: &Path) -> PathBuf {
    team_dir.join(HOOK_FAILURES_FILE)
}

/// The rotated (previous generation) ledger path for a team directory.
#[must_use]
pub fn rotated_ledger_path(team_dir: &Path) -> PathBuf {
    team_dir.join(HOOK_FAILURES_ROTATED_FILE)
}

/// Append one failure record to the team's ledger, rotating when it is full.
///
/// Best-effort: an I/O error is logged and swallowed, because the ledger must
/// never change a hook's verdict.
pub fn record_hook_failure(team_dir: &Path, failure: &HookFailure) {
    if let Err(e) = try_record_hook_failure(team_dir, failure) {
        tracing::warn!(
            team_dir = %team_dir.display(),
            hook = %failure.hook,
            error = %e,
            "Cannot append to the team hook-failure ledger"
        );
    }
}

/// The fallible body of [`record_hook_failure`], exposed so tests can assert on
/// the write rather than on a swallowed error.
pub fn try_record_hook_failure(team_dir: &Path, failure: &HookFailure) -> std::io::Result<()> {
    fs::create_dir_all(team_dir)?;
    let path = ledger_path(team_dir);

    // NFR-004: check the size *before* the append so the live file is bounded
    // without ever rewriting it (append-only outside rotation).
    let current = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if current >= MAX_LEDGER_BYTES {
        rotate(&path)?;
    }

    let mut line = serde_json::to_string(failure)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    line.push('\n');

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    file.flush()
}

/// Move the live ledger to its single previous generation, replacing it.
fn rotate(path: &Path) -> std::io::Result<()> {
    let rotated = path.with_file_name(HOOK_FAILURES_ROTATED_FILE);
    fs::rename(path, &rotated)
}

/// Read the live ledger's records, oldest first.
///
/// A line that is not valid JSON is skipped rather than failing the read: the
/// ledger is diagnostic state, and one truncated final line (a crash mid-write)
/// must not hide the rest. A missing file is an empty ledger.
#[must_use]
pub fn read_hook_failures(team_dir: &Path) -> Vec<HookFailure> {
    let Ok(text) = fs::read_to_string(ledger_path(team_dir)) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<HookFailure>(line).ok())
        .collect()
}

/// The number of failure records in the live ledger (FR-019 surface count).
#[must_use]
pub fn hook_failure_count(team_dir: &Path) -> usize {
    read_hook_failures(team_dir).len()
}

/// Render the ledger as a report block for `/team show` and `team_status`.
///
/// Returns an empty string when nothing has failed, so a healthy team adds no
/// noise. When records exist the block names the count, then each failure, then
/// the file it came from - so an inert hook is visible as an inert hook.
#[must_use]
pub fn render_hook_failures(
    team_name: &str,
    failures: &[HookFailure],
    heading_level: usize,
) -> String {
    if failures.is_empty() {
        return String::new();
    }
    let hashes = "#".repeat(heading_level);
    let mut out = format!(
        "{hashes} Hook failures ({})\n\n\
         {} configured hook(s) could not run and therefore never produced a \
         verdict - they did not approve the action, they were inert. Each line \
         below is one entry of `{HOOK_FAILURES_FILE}`.\n\n",
        failures.len(),
        failures.len()
    );
    for failure in failures {
        out.push_str(&failure.report_line());
    }
    out.push_str(&format!(
        "\nFix the hook command in team '{team_name}' (`{team_name}` settings → \
         hooks) and re-run the action it gates; the ledger never clears itself \
         and rotates to `{HOOK_FAILURES_ROTATED_FILE}` at 1 MiB.\n"
    ));
    out
}

impl HookEvent {
    /// The `snake_case` wire token of the event, as the ledger records it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TeammateIdle => "teammate_idle",
            Self::TaskCreated => "task_created",
            Self::TaskClaimed => "task_claimed",
            Self::TaskCompleted => "task_completed",
        }
    }
}
