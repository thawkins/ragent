//! Per-gather web-URL instrumentation log.
//!
//! Every web-gathering pass appends one JSON object per line to
//! `<log_dir>/research-<name>-<timestamp>-<rand>-web.jsonl`. The caller
//! decides `log_dir`; the agent layer passes `logs/research/` so files
//! land at `logs/research/research-<name>-<ts>-<rand>-web.jsonl`. Each
//! search hit is first recorded with `"status": "considered"` and then,
//! as the fetch/filter pipeline resolves it, with `"status": "captured"`
//! or `"status": "rejected"` plus the rejection `reason`. The file also
//! contains `gather_start`, `queries_decomposed`, and `gather_summary`
//! marker events so a run can be reconstructed end to end. The file name
//! matches the research directory naming (`research-<name>-<ts>-<rand>`)
//! with a `-web` suffix.
//!
//! Writing is best-effort: every record is appended and flushed immediately,
//! so a killed run never silently loses its log entries; failures are
//! reported via `tracing::warn` by the caller and never abort a gather.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

/// JSONL logger for one web-gathering pass.
///
/// Owns the log file path; construct via [`GatherLog::new`], then append
/// records with [`GatherLog::log_url`] and marker events with
/// [`GatherLog::log_event`]. Thread-safe through callers' external
/// synchronisation (each record opens, appends and flushes atomically).
pub struct GatherLog {
    /// Full path of the JSONL log file.
    path: PathBuf,
}

impl GatherLog {
    /// Open (creating if needed) a new gather log inside `log_dir`.
    ///
    /// `research_name` is sanitised so the file name is filesystem-safe and
    /// truncated to 64 characters. A short UUID suffix keeps repeated
    /// gather passes within one research run from clobbering each other.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory cannot be created or the log file
    /// cannot be opened for appending.
    pub fn new(log_dir: &Path, research_name: &str) -> anyhow::Result<Self> {
        fs::create_dir_all(log_dir)?;
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let rand = Uuid::new_v4().simple().to_string();
        let name = format!(
            "research-{}-{timestamp}-{rand}-web.jsonl",
            sanitize(research_name)
        );
        let path = log_dir.join(name);
        Ok(Self { path })
    }

    /// Path of the underlying log file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append a raw JSON event (used for `gather_start`,
    /// `queries_decomposed`, and `gather_summary` markers).
    ///
    /// # Errors
    ///
    /// Returns an error when serialisation or the append/flush fails.
    pub fn log_event(&self, event: &serde_json::Value) -> anyhow::Result<()> {
        self.append_line(&serde_json::to_string(event)?)
    }

    /// Append one per-URL outcome record.
    ///
    /// # Arguments
    ///
    /// * `url` — the candidate page URL.
    /// * `query` — the sub-query that produced this hit.
    /// * `status` — `"considered"`, `"captured"`, or `"rejected"`.
    /// * `title` — page/search title at the time of the record.
    /// * `search_tool` — agent tool that issued the search (e.g. `mf_search`).
    /// * `search_engine` — backend engine(s) that returned the URL.
    /// * `reason` — rejection reason; `None` for considered/captured records.
    /// * `detail` — optional structured extras (relevance label, content
    ///   length) merged into the record.
    ///
    /// # Errors
    ///
    /// Returns an error when serialisation or the append/flush fails.
    #[allow(clippy::too_many_arguments)]
    pub fn log_url(
        &self,
        url: &str,
        query: &str,
        status: &str,
        title: &str,
        search_tool: &str,
        search_engine: &str,
        reason: Option<&str>,
        detail: Option<&serde_json::Value>,
    ) -> anyhow::Result<()> {
        let mut record = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "url": url,
            "query": query,
            "status": status,
            "title": title,
            "search_tool": search_tool,
            "search_engine": search_engine,
            "reason": reason,
        });
        if let (Some(detail), Some(obj)) = (detail, record.as_object_mut())
            && let Some(extra) = detail.as_object()
        {
            for (k, v) in extra {
                obj.insert(k.clone(), v.clone());
            }
        }
        self.append_line(&serde_json::to_string(&record)?)
    }

    fn append_line(&self, line: &str) -> anyhow::Result<()> {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }
}

/// Restrict a research name to filesystem-safe characters (alphanumeric,
/// `-` and `_`); everything else becomes `-`. Truncated to 64 characters.
fn sanitize(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    out.truncate(64);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_log_writes_considered_and_rejected_records() {
        let dir = tempfile::tempdir().unwrap();
        let log = GatherLog::new(dir.path(), "my research").unwrap();
        log.log_event(&json!({"event": "gather_start", "topic": "t"}))
            .unwrap();
        log.log_url(
            "https://a.example",
            "q",
            "considered",
            "A",
            "mf_search",
            "duckduckgo",
            None,
            None,
        )
        .unwrap();
        log.log_url(
            "https://b.example",
            "q",
            "rejected",
            "B",
            "mf_search",
            "duckduckgo",
            Some("relevance too low (Low)"),
            Some(&json!({"relevance": "Low"})),
        )
        .unwrap();

        let contents = fs::read_to_string(log.path()).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);
        let start: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(start["event"], "gather_start");
        let considered: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(considered["status"], "considered");
        assert_eq!(considered["url"], "https://a.example");
        assert!(considered["reason"].is_null());
        let rejected: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(rejected["status"], "rejected");
        assert_eq!(rejected["reason"], "relevance too low (Low)");
        assert_eq!(rejected["relevance"], "Low");
        let file_name = log
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(file_name.starts_with("research-my-research-"));
        assert!(file_name.ends_with("-web.jsonl"));
    }

    #[test]
    fn sanitize_replaces_unsafe_characters() {
        assert_eq!(sanitize("v1 rocket/german?"), "v1-rocket-german-");
        assert_eq!(sanitize("plain_name-1"), "plain_name-1");
    }
}
