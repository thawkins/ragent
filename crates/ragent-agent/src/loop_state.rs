//! Stateful loop cron mode — cross-run state and triage inbox tag protocol.
//!
//! Implements FR-004 of the pie-gap spec: when a cron job is created with the
//! `--stateful` flag, the system maintains a cross-run state file and parses
//! `<loop-state>` / `<inbox>` output protocol tags from the sub-agent's
//! response.
//!
//! ## Tag Protocol
//!
//! The sub-agent's text output may contain these tags:
//!
//! - `<loop-state>...</loop-state>` — notes carried forward to the next run.
//!   Capped at [`LoopState::MAX_CHARS`] (2000) characters.
//! - `<inbox>...</inbox>` — findings reported to the global triage inbox.
//!   Each entry capped at [`InboxEntry::MAX_CHARS`] (500) characters, with at
//!   most [`InboxEntry::MAX_PER_RUN`] (16) findings honored per run.
//!
//! ## State Persistence
//!
//! Loop state is stored as a plain-text file at:
//! ```text
//! <data_dir>/loop-state/<event_id>.txt
//! ```
//!
//! ## Inbox Persistence
//!
//! Inbox entries are appended to a global JSONL file at:
//! ```text
//! <data_dir>/log/inbox/inbox.jsonl
//! ```
//! This file is shared across all sessions and all stateful cron events.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// ── Constants ─────────────────────────────────────────────────────────

/// Maximum loop-state content length in characters (FR-004).
pub const LOOP_STATE_MAX_CHARS: usize = 2000;

/// Maximum inbox entry content length in characters (FR-004).
pub const INBOX_ENTRY_MAX_CHARS: usize = 500;

/// Maximum number of inbox findings honored per run (FR-004).
pub const INBOX_MAX_PER_RUN: usize = 16;

// ── Types ─────────────────────────────────────────────────────────────

/// The cross-run state for a stateful cron event.
///
/// Stored as plain text at `<data_dir>/loop-state/<event_id>.txt`.
/// Content is capped at [`LOOP_STATE_MAX_CHARS`] characters.
#[derive(Debug, Clone, Default)]
pub struct LoopState {
    /// The state content (notes from the previous run).
    pub content: String,
}

impl LoopState {
    /// Maximum length of the state content in characters.
    pub const MAX_CHARS: usize = LOOP_STATE_MAX_CHARS;

    /// Load the loop state for the given event ID from the data directory.
    ///
    /// Returns an empty `LoopState` if the state file does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error only if the file exists but cannot be read.
    pub fn load(data_dir: &Path, event_id: &str) -> std::io::Result<Self> {
        let path = loop_state_path(data_dir, event_id);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let truncated = truncate_chars(&content, Self::MAX_CHARS);
                Ok(Self { content: truncated })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Save the loop state to the data directory.
    ///
    /// The content is truncated to [`MAX_CHARS`] before writing. The parent
    /// directory is created if it does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, data_dir: &Path, event_id: &str) -> std::io::Result<()> {
        let path = loop_state_path(data_dir, event_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let truncated = truncate_chars(&self.content, Self::MAX_CHARS);
        let char_count = truncated.chars().count();
        std::fs::write(&path, &truncated)?;
        debug!(event_id = %event_id, "loop state saved ({} chars)", char_count);
        Ok(())
    }
}

/// A single triage inbox finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxEntry {
    /// Unique identifier (UUID).
    pub id: String,
    /// The cron event ID that produced this finding.
    pub source_event_id: String,
    /// The finding content (capped at [`INBOX_ENTRY_MAX_CHARS`] chars).
    pub content: String,
    /// When the finding was reported.
    pub timestamp: DateTime<Utc>,
    /// Whether the finding has been claimed/dismissed (managed by `/inbox`).
    pub status: String,
}

impl InboxEntry {
    /// Maximum content length in characters.
    pub const MAX_CHARS: usize = INBOX_ENTRY_MAX_CHARS;

    /// Maximum number of findings honored per run.
    pub const MAX_PER_RUN: usize = INBOX_MAX_PER_RUN;

    /// Create a new inbox entry with the given fields.
    pub fn new(source_event_id: &str, content: &str) -> Self {
        let truncated = truncate_chars(content, Self::MAX_CHARS);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_event_id: source_event_id.to_string(),
            content: truncated,
            timestamp: Utc::now(),
            status: "open".to_string(),
        }
    }
}

/// The result of parsing `<loop-state>` and `<inbox>` tags from agent output.
#[derive(Debug, Clone, Default)]
pub struct ParsedTags {
    /// The extracted loop-state content (empty if no `<loop-state>` tag found).
    pub loop_state: String,
    /// The extracted inbox findings (empty if no `<inbox>` tags found).
    pub inbox_entries: Vec<String>,
}

// ── Tag Parsing ───────────────────────────────────────────────────────

/// Open tag for loop-state.
const LOOP_STATE_OPEN: &str = "<loop-state>";
/// Close tag for loop-state.
const LOOP_STATE_CLOSE: &str = "</loop-state>";
/// Open tag for inbox.
const INBOX_OPEN: &str = "<inbox>";
/// Close tag for inbox.
const INBOX_CLOSE: &str = "</inbox>";

/// Parse `<loop-state>` and `<inbox>` tags from agent output text.
///
/// - `<loop-state>` content is extracted and truncated to
///   [`LOOP_STATE_MAX_CHARS`] characters. If multiple tags are present, the
///   last one wins (the agent may update its state mid-run).
/// - `<inbox>` entries are extracted in order, each truncated to
///   [`INBOX_ENTRY_MAX_CHARS`] characters. Only the first
///   [`INBOX_MAX_PER_RUN`] entries are kept; the rest are silently dropped.
///
/// Tags that are malformed (e.g. missing close tag) are silently ignored.
pub fn parse_tags(output: &str) -> ParsedTags {
    let loop_state = extract_last_tag(output, LOOP_STATE_OPEN, LOOP_STATE_CLOSE)
        .map(|s| truncate_chars(&s, LOOP_STATE_MAX_CHARS))
        .unwrap_or_default();

    let inbox_entries = extract_all_tags(output, INBOX_OPEN, INBOX_CLOSE)
        .into_iter()
        .take(INBOX_MAX_PER_RUN)
        .map(|s| truncate_chars(&s, INBOX_ENTRY_MAX_CHARS))
        .collect();

    ParsedTags {
        loop_state,
        inbox_entries,
    }
}

/// Extract the content of the last occurrence of a tag pair from `text`.
///
/// Returns `None` if the tag pair is not found or malformed.
fn extract_last_tag(text: &str, open: &str, close: &str) -> Option<String> {
    let last_open = text.rfind(open)?;
    let after_open = last_open + open.len();
    let close_pos = text[after_open..].find(close)?;
    Some(text[after_open..after_open + close_pos].trim().to_string())
}

/// Extract the content of all occurrences of a tag pair from `text`.
///
/// Malformed pairs (missing close tag) are skipped.
fn extract_all_tags(text: &str, open: &str, close: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(open_pos) = text[search_from..].find(open) {
        let abs_open = search_from + open_pos;
        let after_open = abs_open + open.len();
        match text[after_open..].find(close) {
            Some(close_pos) => {
                let content = text[after_open..after_open + close_pos].trim().to_string();
                results.push(content);
                search_from = after_open + close_pos + close.len();
            }
            None => break, // malformed — no close tag
        }
    }
    results
}

// ── Inbox Writer ──────────────────────────────────────────────────────

/// Append inbox entries to the global JSONL file.
///
/// The file is created if it does not exist. Each entry is serialized as a
/// single JSON object on its own line.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or written.
pub fn write_inbox_entries(data_dir: &Path, entries: &[InboxEntry]) -> std::io::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let path = inbox_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write;
    for entry in entries {
        let json = serde_json::to_string(entry).map_err(std::io::Error::other)?;
        writeln!(file, "{json}")?;
    }
    debug!(count = entries.len(), "inbox entries written");
    Ok(())
}

/// Read all inbox entries from the global JSONL file.
///
/// Returns an empty vector if the file does not exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read or parsed.
pub fn read_inbox(data_dir: &Path) -> std::io::Result<Vec<InboxEntry>> {
    let path = inbox_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let mut entries = Vec::new();
            for (line_num, line) in content.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<InboxEntry>(line) {
                    Ok(entry) => entries.push(entry),
                    Err(e) => {
                        warn!(line = line_num + 1, error = %e, "skipping malformed inbox entry");
                    }
                }
            }
            Ok(entries)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

// ── Inbox Management ──────────────────────────────────────────────────

/// Update the status of a single inbox entry by ID.
///
/// Reads the entire inbox JSONL file, updates the matching entry, and
/// rewrites the file in place. Returns `true` if the entry was found and
/// updated, `false` if no entry with the given ID exists.
///
/// # Errors
///
/// Returns an error if the file cannot be read or written.
pub fn update_inbox_entry_status(
    data_dir: &Path,
    entry_id: &str,
    new_status: &str,
) -> std::io::Result<bool> {
    let mut entries = read_inbox(data_dir)?;
    let mut found = false;
    for entry in &mut entries {
        if entry.id == entry_id {
            entry.status = new_status.to_string();
            found = true;
            break;
        }
    }
    if found {
        rewrite_inbox(data_dir, &entries)?;
    }
    Ok(found)
}

/// Remove all inbox entries, deleting the JSONL file.
///
/// Returns the number of entries that were removed.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be deleted.
pub fn clear_inbox(data_dir: &Path) -> std::io::Result<usize> {
    let path = inbox_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let count = content.lines().filter(|l| !l.trim().is_empty()).count();
            std::fs::remove_file(&path)?;
            debug!(count, "inbox cleared");
            Ok(count)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

/// Rewrite the entire inbox JSONL file with the given entries (overwrite mode).
fn rewrite_inbox(data_dir: &Path, entries: &[InboxEntry]) -> std::io::Result<()> {
    let path = inbox_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;
    use std::io::Write;
    for entry in entries {
        let json = serde_json::to_string(entry).map_err(std::io::Error::other)?;
        writeln!(file, "{json}")?;
    }
    Ok(())
}

// ── Prompt Injection ──────────────────────────────────────────────────

/// Inject loop state into a prompt, returning the augmented prompt.
///
/// If the state content is empty, the original prompt is returned unchanged.
/// Otherwise, a `<loop-state>` block is prepended to the prompt so the
/// sub-agent sees its previous run's notes.
pub fn inject_state_into_prompt(prompt: &str, state: &LoopState) -> String {
    if state.content.trim().is_empty() {
        return prompt.to_string();
    }
    format!(
        "<loop-state>\n{}\n</loop-state>\n\n{}",
        state.content.trim(),
        prompt
    )
}

// ── Tag Removal ──────────────────────────────────────────��────────────

/// Strip `<loop-state>` and `<inbox>` tag pairs from `output`, returning
/// the clean text.
///
/// This is used to produce the user-visible output without the protocol tags.
pub fn strip_tags(output: &str) -> String {
    let mut result = output.to_string();
    // Strip loop-state tags (last occurrence first to avoid index shifts).
    while let Some(start) = result.find(LOOP_STATE_OPEN) {
        let after_open = start + LOOP_STATE_OPEN.len();
        if let Some(close_rel) = result[after_open..].find(LOOP_STATE_CLOSE) {
            let end = after_open + close_rel + LOOP_STATE_CLOSE.len();
            result.replace_range(start..end, "");
        } else {
            break; // malformed
        }
    }
    // Strip inbox tags.
    while let Some(start) = result.find(INBOX_OPEN) {
        let after_open = start + INBOX_OPEN.len();
        if let Some(close_rel) = result[after_open..].find(INBOX_CLOSE) {
            let end = after_open + close_rel + INBOX_CLOSE.len();
            result.replace_range(start..end, "");
        } else {
            break; // malformed
        }
    }
    result.trim().to_string()
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Compute the file path for a cron event's loop state.
fn loop_state_path(data_dir: &Path, event_id: &str) -> PathBuf {
    data_dir.join("loop-state").join(format!("{event_id}.txt"))
}

/// Compute the file path for the global inbox JSONL file.
///
/// The inbox lives at `<data_dir>/log/inbox/inbox.jsonl`, keeping it separate
/// from the cron execution logs in `<data_dir>/log/`.
fn inbox_path(data_dir: &Path) -> PathBuf {
    data_dir.join("log").join("inbox").join("inbox.jsonl")
}

/// Truncate a string to at most `max` characters, appending an ellipsis if
/// truncation occurs.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tags_empty_output() {
        let parsed = parse_tags("");
        assert!(parsed.loop_state.is_empty());
        assert!(parsed.inbox_entries.is_empty());
    }

    #[test]
    fn test_parse_tags_no_tags() {
        let parsed = parse_tags("Just some text without any tags.");
        assert!(parsed.loop_state.is_empty());
        assert!(parsed.inbox_entries.is_empty());
    }

    #[test]
    fn test_parse_tags_loop_state() {
        let output = "Some work done.\n<loop-state>\nRemember to check X.\n</loop-state>\nDone.";
        let parsed = parse_tags(output);
        assert_eq!(parsed.loop_state, "Remember to check X.");
        assert!(parsed.inbox_entries.is_empty());
    }

    #[test]
    fn test_parse_tags_multiple_loop_state_last_wins() {
        let output = "<loop-state>first</loop-state> middle <loop-state>second</loop-state>";
        let parsed = parse_tags(output);
        assert_eq!(parsed.loop_state, "second");
    }

    #[test]
    fn test_parse_tags_inbox_entries() {
        let output = "<inbox>finding 1</inbox>\n<inbox>finding 2</inbox>";
        let parsed = parse_tags(output);
        assert_eq!(parsed.inbox_entries.len(), 2);
        assert_eq!(parsed.inbox_entries[0], "finding 1");
        assert_eq!(parsed.inbox_entries[1], "finding 2");
    }

    #[test]
    fn test_parse_tags_inbox_max_per_run() {
        let mut output = String::new();
        for i in 0..(INBOX_MAX_PER_RUN + 5) {
            output.push_str(&format!("<inbox>finding {i}</inbox>\n"));
        }
        let parsed = parse_tags(&output);
        assert_eq!(parsed.inbox_entries.len(), INBOX_MAX_PER_RUN);
        assert_eq!(parsed.inbox_entries[0], "finding 0");
        assert_eq!(
            parsed.inbox_entries[INBOX_MAX_PER_RUN - 1],
            format!("finding {}", INBOX_MAX_PER_RUN - 1)
        );
    }

    #[test]
    fn test_parse_tags_loop_state_truncated() {
        let long_content = "x".repeat(LOOP_STATE_MAX_CHARS + 500);
        let output = format!("<loop-state>{long_content}</loop-state>");
        let parsed = parse_tags(&output);
        assert!(parsed.loop_state.chars().count() <= LOOP_STATE_MAX_CHARS);
        assert!(parsed.loop_state.ends_with('…'));
    }

    #[test]
    fn test_parse_tags_inbox_entry_truncated() {
        let long_content = "y".repeat(INBOX_ENTRY_MAX_CHARS + 200);
        let output = format!("<inbox>{long_content}</inbox>");
        let parsed = parse_tags(&output);
        assert_eq!(parsed.inbox_entries.len(), 1);
        assert!(parsed.inbox_entries[0].chars().count() <= INBOX_ENTRY_MAX_CHARS);
        assert!(parsed.inbox_entries[0].ends_with('…'));
    }

    #[test]
    fn test_parse_tags_malformed_no_close() {
        let parsed = parse_tags("<loop-state>no close tag");
        assert!(parsed.loop_state.is_empty());
    }

    #[test]
    fn test_parse_tags_mixed() {
        let output = "Work done.\n<loop-state>notes for next run</loop-state>\nMore text.\n<inbox>issue found</inbox>\n<inbox>another issue</inbox>";
        let parsed = parse_tags(output);
        assert_eq!(parsed.loop_state, "notes for next run");
        assert_eq!(parsed.inbox_entries.len(), 2);
        assert_eq!(parsed.inbox_entries[0], "issue found");
        assert_eq!(parsed.inbox_entries[1], "another issue");
    }

    #[test]
    fn test_inject_state_empty() {
        let state = LoopState::default();
        let prompt = "do something";
        let result = inject_state_into_prompt(prompt, &state);
        assert_eq!(result, prompt);
    }

    #[test]
    fn test_inject_state_with_content() {
        let state = LoopState {
            content: "previous notes".to_string(),
        };
        let prompt = "do something";
        let result = inject_state_into_prompt(prompt, &state);
        assert!(result.starts_with("<loop-state>"));
        assert!(result.contains("previous notes"));
        assert!(result.contains("do something"));
    }

    #[test]
    fn test_strip_tags() {
        let output =
            "Work done.\n<loop-state>notes</loop-state>\nMore text.\n<inbox>finding</inbox>";
        let stripped = strip_tags(output);
        assert!(!stripped.contains("<loop-state>"));
        assert!(!stripped.contains("</loop-state>"));
        assert!(!stripped.contains("<inbox>"));
        assert!(!stripped.contains("</inbox>"));
        assert!(stripped.contains("Work done."));
        assert!(stripped.contains("More text."));
    }

    #[test]
    fn test_strip_tags_no_tags() {
        let output = "Just plain text.";
        assert_eq!(strip_tags(output), "Just plain text.");
    }

    #[test]
    fn test_loop_state_save_load() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-loop-state-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let state = LoopState {
            content: "test notes".to_string(),
        };
        state.save(&dir, "event-1").unwrap();

        let loaded = LoopState::load(&dir, "event-1").unwrap();
        assert_eq!(loaded.content, "test notes");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_loop_state_load_missing() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-loop-state-test-missing-{}",
            uuid::Uuid::new_v4()
        ));
        let loaded = LoopState::load(&dir, "nonexistent").unwrap();
        assert!(loaded.content.is_empty());
    }

    #[test]
    fn test_loop_state_truncated_on_load() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-loop-state-test-trunc-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let long_content = "x".repeat(LOOP_STATE_MAX_CHARS + 500);
        let path = dir.join("loop-state").join("event-trunc.txt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &long_content).unwrap();

        let loaded = LoopState::load(&dir, "event-trunc").unwrap();
        assert!(loaded.content.chars().count() <= LOOP_STATE_MAX_CHARS);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_inbox_write_read() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-inbox-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let entries = vec![
            InboxEntry::new("event-1", "first finding"),
            InboxEntry::new("event-1", "second finding"),
        ];
        write_inbox_entries(&dir, &entries).unwrap();

        let read = read_inbox(&dir).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].content, "first finding");
        assert_eq!(read[1].content, "second finding");
        assert_eq!(read[0].source_event_id, "event-1");
        assert_eq!(read[0].status, "open");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_inbox_read_missing() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-inbox-test-missing-{}",
            uuid::Uuid::new_v4()
        ));
        let read = read_inbox(&dir).unwrap();
        assert!(read.is_empty());
    }

    #[test]
    fn test_inbox_entry_truncation() {
        let long_content = "z".repeat(INBOX_ENTRY_MAX_CHARS + 100);
        let entry = InboxEntry::new("event-1", &long_content);
        assert!(entry.content.chars().count() <= INBOX_ENTRY_MAX_CHARS);
        assert!(entry.content.ends_with('…'));
    }

    #[test]
    fn test_write_inbox_empty_no_file() {
        let dir =
            std::env::temp_dir().join(format!("ragent-inbox-test-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        write_inbox_entries(&dir, &[]).unwrap();
        assert!(!dir.join("inbox.jsonl").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_truncate_chars_no_truncation() {
        assert_eq!(truncate_chars("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_chars_exact() {
        assert_eq!(truncate_chars("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_chars_truncated() {
        let result = truncate_chars("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_update_inbox_entry_status_found() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-inbox-update-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let entry = InboxEntry::new("event-1", "test finding");
        let entry_id = entry.id.clone();
        write_inbox_entries(&dir, &[entry]).unwrap();

        let found = update_inbox_entry_status(&dir, &entry_id, "claimed").unwrap();
        assert!(found);

        let read = read_inbox(&dir).unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].status, "claimed");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_update_inbox_entry_status_not_found() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-inbox-update-nf-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let entry = InboxEntry::new("event-1", "test finding");
        write_inbox_entries(&dir, &[entry]).unwrap();

        let found = update_inbox_entry_status(&dir, "nonexistent-id", "dismissed").unwrap();
        assert!(!found);

        // File should be unchanged
        let read = read_inbox(&dir).unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].status, "open");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_update_inbox_entry_status_no_file() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-inbox-update-nofile-{}",
            uuid::Uuid::new_v4()
        ));
        let found = update_inbox_entry_status(&dir, "any-id", "claimed").unwrap();
        assert!(!found);
    }

    #[test]
    fn test_clear_inbox_with_entries() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-inbox-clear-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let entries = vec![
            InboxEntry::new("event-1", "first"),
            InboxEntry::new("event-2", "second"),
            InboxEntry::new("event-1", "third"),
        ];
        write_inbox_entries(&dir, &entries).unwrap();

        let count = clear_inbox(&dir).unwrap();
        assert_eq!(count, 3);
        assert!(!dir.join("inbox.jsonl").exists());

        // Clearing again should return 0
        let count2 = clear_inbox(&dir).unwrap();
        assert_eq!(count2, 0);
    }

    #[test]
    fn test_clear_inbox_empty() {
        let dir =
            std::env::temp_dir().join(format!("ragent-inbox-clear-empty-{}", uuid::Uuid::new_v4()));
        let count = clear_inbox(&dir).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_update_inbox_preserves_other_entries() {
        let dir = std::env::temp_dir().join(format!(
            "ragent-inbox-preserve-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let entry1 = InboxEntry::new("event-1", "first");
        let entry2 = InboxEntry::new("event-2", "second");
        let entry1_id = entry1.id.clone();
        write_inbox_entries(&dir, &[entry1, entry2]).unwrap();

        update_inbox_entry_status(&dir, &entry1_id, "dismissed").unwrap();

        let read = read_inbox(&dir).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].status, "dismissed");
        assert_eq!(read[1].status, "open");

        std::fs::remove_dir_all(&dir).ok();
    }
}
