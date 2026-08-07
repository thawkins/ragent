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

use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;

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

/// Characteristic of an `old_str` that is suspected to increase the chance of
/// an exact-match edit failure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OldStrRisk {
    /// The string contains one or more characters outside the ASCII range.
    ContainsUtf,
    /// The string contains a literal tab character (not just spaces).
    ContainsTabs,
    /// The string contains trailing or leading whitespace on any line.
    EscapedWhitespace,
    /// The string contains one or more blank lines.
    ContainsBlankLines,
    /// The string mixes `\r\n` and `\n` line endings.
    MixedLineEndings,
    /// The string starts or ends with whitespace (space or tab).
    LeadingTrailingWhitespace,
}

impl OldStrRisk {
    /// Human-readable label used in reports.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::ContainsUtf => "contains utf characters",
            Self::ContainsTabs => "contains tabs",
            Self::EscapedWhitespace => "contains escaped whitespace",
            Self::ContainsBlankLines => "contains blank lines",
            Self::MixedLineEndings => "contains mixed line endings",
            Self::LeadingTrailingWhitespace => "has leading or trailing whitespace",
        }
    }
}

/// Detect which risk characteristics are present in `old_str`.
#[must_use]
pub fn detect_old_str_risks(old_str: &str) -> Vec<OldStrRisk> {
    let mut risks = Vec::new();

    if !old_str.is_ascii() {
        risks.push(OldStrRisk::ContainsUtf);
    }

    if old_str.contains('\t') {
        risks.push(OldStrRisk::ContainsTabs);
    }

    if old_str
        .lines()
        .any(|line| line.starts_with(' ') || line.ends_with(' '))
        || old_str
            .lines()
            .any(|line| line.starts_with('\t') || line.ends_with('\t'))
    {
        risks.push(OldStrRisk::EscapedWhitespace);
    }

    // `lines()` skips the trailing empty string, so a trailing blank line is
    // detected separately below.
    let raw_has_blank_line = old_str.split(['\n', '\r']).any(|line| line.is_empty());
    let lines_have_blank = old_str.lines().any(|line| line.trim().is_empty());
    if raw_has_blank_line || lines_have_blank {
        risks.push(OldStrRisk::ContainsBlankLines);
    }

    let has_crlf = old_str.contains("\r\n");
    let has_lf = old_str.contains('\n');
    if has_crlf && has_lf && old_str.contains('\n') {
        // Confirm that there is at least one LF that is not part of a CRLF.
        let lf_not_crlf = old_str
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w[0] != '\r' && w[1] == '\n');
        if lf_not_crlf {
            risks.push(OldStrRisk::MixedLineEndings);
        }
    }

    if old_str
        .chars()
        .next()
        .is_some_and(|c| c == ' ' || c == '\t')
        || old_str
            .chars()
            .last()
            .is_some_and(|c| c == ' ' || c == '\t')
    {
        risks.push(OldStrRisk::LeadingTrailingWhitespace);
    }

    risks
}

/// Risk profile for a failed edit-log entry.
#[derive(Debug, Clone)]
pub struct FailedEditRisk {
    /// Tool that performed the edit.
    pub tool: String,
    /// Path of the edited file.
    pub file_path: String,
    /// Normalised outcome string.
    pub outcome: String,
    /// Risks detected in `old_str`.
    pub risks: Vec<OldStrRisk>,
    /// The `old_str` value (truncated for reporting).
    pub old_str_preview: String,
}

/// Result of analysing the edit log for failure risk characteristics.
#[derive(Debug, Clone, Default)]
pub struct EditLogAnalysis {
    /// Number of failed entries examined.
    pub failure_count: usize,
    /// Number of failed entries where at least one risk was detected.
    pub risky_failure_count: usize,
    /// Count of failures per detected risk characteristic.
    pub risk_counts: HashMap<OldStrRisk, usize>,
    /// Up to a few example failures per risk characteristic.
    pub risk_examples: HashMap<OldStrRisk, Vec<FailedEditRisk>>,
    /// Count of distinct risk-characteristic combinations observed.
    pub combination_counts: HashMap<Vec<OldStrRisk>, usize>,
}

impl EditLogAnalysis {
    /// Return the risk characteristics sorted by descending frequency.
    #[must_use]
    pub fn risks_by_frequency(&self) -> Vec<(OldStrRisk, usize)> {
        let mut v: Vec<(OldStrRisk, usize)> = self
            .risk_counts
            .iter()
            .map(|(risk, count)| (risk.clone(), *count))
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.label().cmp(b.0.label())));
        v
    }
}

/// Scan all failed entries in the edit log and analyse their `old_str` values
/// for characteristics that could explain exact-match failures.
///
/// Returns `None` if the log directory does not exist or cannot be read.
/// Returns an empty analysis if there are no failed entries.
#[must_use]
pub fn edit_log_analyse(working_dir: &Path) -> Option<EditLogAnalysis> {
    let log_dir = log_dir(working_dir);
    if !log_dir.exists() {
        return None;
    }

    let mut analysis = EditLogAnalysis::default();

    let entries = std::fs::read_dir(&log_dir).ok()?;
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
            let value: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let outcome = value
                .get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let normalized = normalize_outcome(outcome);
            if normalized == "success" || normalized == "unknown" {
                continue;
            }

            let tool = value
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let file_path = value
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let old_str = value.get("old_str").and_then(|v| v.as_str()).unwrap_or("");

            let risks = detect_old_str_risks(old_str);
            analysis.failure_count += 1;
            if !risks.is_empty() {
                analysis.risky_failure_count += 1;
            }

            // Count individual risks and collect examples.
            let mut combination = Vec::new();
            for risk in &risks {
                *analysis.risk_counts.entry(risk.clone()).or_insert(0) += 1;
                combination.push(risk.clone());
                let examples = analysis.risk_examples.entry(risk.clone()).or_default();
                if examples.len() < 3 {
                    examples.push(FailedEditRisk {
                        tool: tool.clone(),
                        file_path: file_path.clone(),
                        outcome: normalized.clone(),
                        risks: risks.clone(),
                        old_str_preview: truncate_preview(old_str, 120),
                    });
                }
            }

            if !combination.is_empty() {
                combination.sort_by_key(|r| r.label());
                *analysis.combination_counts.entry(combination).or_insert(0) += 1;
            }
        }
    }

    Some(analysis)
}

/// Truncate a string to a maximum visual length, appending an ellipsis if
/// truncated. Newlines are rendered as `\n` in the preview so the output stays
/// compact.
fn truncate_preview(s: &str, max_len: usize) -> String {
    let escaped = s
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    if escaped.chars().count() <= max_len {
        escaped
    } else {
        let truncated: String = escaped.chars().take(max_len.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

/// Per-tool edit-log statistics.
///
/// `tool_counts` maps tool name → total invocations.
/// `outcome_counts` maps `(tool, outcome)` → count, where `"success"` is the
/// success outcome and any other value is a failure reason.
/// `failure_reasons` maps failure reason → total count across all tools.
#[derive(Debug, Clone, Default)]
pub struct EditLogStats {
    /// Total invocations per tool.
    pub tool_counts: HashMap<String, usize>,
    /// Outcome counts per tool.
    pub outcome_counts: HashMap<(String, String), usize>,
    /// Failure reason counts across all tools.
    pub failure_reasons: HashMap<String, usize>,
}

/// Normalise an outcome string for aggregation.
///
/// Failure messages often contain file paths (e.g. "not found in src/foo.rs"),
/// which makes every failure appear unique in the `/editlog show` rollup.
/// This helper removes any path-like token so that failures with the same
/// underlying reason are counted together.
pub(crate) fn normalize_outcome(outcome: &str) -> String {
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let re = PATH_RE.get_or_init(|| {
        // Match a run of non-whitespace characters that contains a path
        // separator. This catches absolute paths, project-relative paths,
        // and Windows paths without requiring knowledge of the working dir.
        Regex::new(r#"[^\s'"]*[\/\\][^\s'"]*"#).expect("valid regex")
    });
    let normalized = re.replace_all(outcome, "<file>");
    // Collapse consecutive whitespace left behind by removed tokens.
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl EditLogStats {
    /// Total number of logged edit operations across all tools.
    #[must_use]
    pub fn total(&self) -> usize {
        self.tool_counts.values().sum()
    }

    /// Number of successful operations for the given tool.
    #[must_use]
    pub fn success_for(&self, tool: &str) -> usize {
        self.outcome_counts
            .get(&(tool.to_string(), "success".to_string()))
            .copied()
            .unwrap_or(0)
    }

    /// Number of failed operations for the given tool.
    #[must_use]
    pub fn failure_for(&self, tool: &str) -> usize {
        let total = self.tool_counts.get(tool).copied().unwrap_or(0);
        total.saturating_sub(self.success_for(tool))
    }

    /// Success percentage for the given tool.
    #[must_use]
    pub fn success_pct_for(&self, tool: &str) -> f64 {
        let total = self.tool_counts.get(tool).copied().unwrap_or(0);
        if total == 0 {
            0.0
        } else {
            (self.success_for(tool) as f64 / total as f64) * 100.0
        }
    }
}

/// Build detailed edit-log statistics from all `edits-*.jsonl` files.
///
/// Returns `None` if the log directory does not exist or cannot be read.
#[must_use]
pub fn edit_log_stats(working_dir: &Path) -> Option<EditLogStats> {
    let log_dir = log_dir(working_dir);
    if !log_dir.exists() {
        return None;
    }

    let mut stats = EditLogStats::default();

    let entries = std::fs::read_dir(&log_dir).ok()?;
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
            let value: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let tool = value
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let outcome = value
                .get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let normalized = normalize_outcome(outcome);

            *stats.tool_counts.entry(tool.clone()).or_insert(0) += 1;
            *stats
                .outcome_counts
                .entry((tool, normalized.clone()))
                .or_insert(0) += 1;

            if normalized != "success" {
                *stats.failure_reasons.entry(normalized).or_insert(0) += 1;
            }
        }
    }

    Some(stats)
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
            if name_str.starts_with("edits-")
                && name_str.ends_with(".jsonl")
                && let Ok(file) = std::fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(entry.path())
            {
                drop(file);
                cleared += 1;
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
        let prev = is_edit_log_enabled();
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

        set_edit_log_enabled(prev);
        clear_edit_logs(wd);
    }

    #[test]
    fn edit_log_stats_aggregates_by_tool_and_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();
        let prev = is_edit_log_enabled();
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
        // Same underlying reason, different files: should roll up to one entry.
        log_edit_operation(wd, "edit", Path::new("b.rs"), "x", "y", "not found", false);
        log_edit_operation(wd, "edit", Path::new("c.rs"), "x", "y", "not found", false);
        // Path embedded in a longer message: should be normalised to <file>.
        log_edit_operation(
            wd,
            "edit",
            Path::new("d.rs"),
            "x",
            "y",
            "old exact text not found in /work/src/d.rs",
            false,
        );
        log_edit_operation(
            wd,
            "multi_edit",
            Path::new("e.rs"),
            "x",
            "y",
            "success",
            true,
        );
        log_edit_operation(
            wd,
            "multi_edit",
            Path::new("f.rs"),
            "x",
            "y",
            "stale file",
            false,
        );

        let stats = edit_log_stats(wd).unwrap();
        assert_eq!(stats.tool_counts.get("edit").copied().unwrap_or(0), 4);
        assert_eq!(stats.tool_counts.get("multi_edit").copied().unwrap_or(0), 2);
        assert_eq!(stats.success_for("edit"), 1);
        assert_eq!(stats.failure_for("edit"), 3);
        assert!((stats.success_pct_for("edit") - 25.0).abs() < 0.1);
        // All "not found" variants (plain and with path) should collapse.
        assert_eq!(
            stats.failure_reasons.get("not found").copied().unwrap_or(0),
            2
        );
        assert_eq!(
            stats
                .failure_reasons
                .get("old exact text not found in <file>")
                .copied()
                .unwrap_or(0),
            1
        );
        assert_eq!(
            stats
                .failure_reasons
                .get("stale file")
                .copied()
                .unwrap_or(0),
            1
        );

        set_edit_log_enabled(prev);
        clear_edit_logs(wd);
    }

    #[test]
    fn detect_old_str_risks_finds_common_problems() {
        assert!(detect_old_str_risks("café").contains(&OldStrRisk::ContainsUtf));
        assert!(detect_old_str_risks("a\tb").contains(&OldStrRisk::ContainsTabs));
        assert!(detect_old_str_risks("  leading").contains(&OldStrRisk::LeadingTrailingWhitespace));
        assert!(
            detect_old_str_risks("trailing  ").contains(&OldStrRisk::LeadingTrailingWhitespace)
        );
        assert!(detect_old_str_risks("line\n  indented").contains(&OldStrRisk::EscapedWhitespace));
        assert!(detect_old_str_risks("line\n\nline").contains(&OldStrRisk::ContainsBlankLines));
        assert!(detect_old_str_risks("a\r\nb\nc").contains(&OldStrRisk::MixedLineEndings));
        assert!(detect_old_str_risks("plain ascii text.").is_empty());
    }

    #[test]
    fn edit_log_analyse_aggregates_risks_for_failures() {
        let tmp = tempfile::tempdir().unwrap();
        let wd = tmp.path();
        let prev = is_edit_log_enabled();
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
        log_edit_operation(
            wd,
            "edit",
            Path::new("b.rs"),
            "  leading",
            "new",
            "not found",
            false,
        );
        log_edit_operation(
            wd,
            "edit",
            Path::new("c.rs"),
            "line\n\nline",
            "new",
            "stale file",
            false,
        );

        let analysis = edit_log_analyse(wd).unwrap();
        assert_eq!(analysis.failure_count, 2);
        assert_eq!(analysis.risky_failure_count, 2);
        assert!(
            analysis
                .risk_counts
                .get(&OldStrRisk::LeadingTrailingWhitespace)
                .copied()
                .unwrap_or(0)
                >= 1
        );
        assert!(
            analysis
                .risk_counts
                .get(&OldStrRisk::ContainsBlankLines)
                .copied()
                .unwrap_or(0)
                >= 1
        );

        set_edit_log_enabled(prev);
        clear_edit_logs(wd);
    }
}
