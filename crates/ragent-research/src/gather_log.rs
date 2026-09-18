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
//! PERF-049: the log file is opened once, on the first append, and held
//! behind a 64 KiB `BufWriter`, so a sweep no longer performs an `open` plus
//! two `write_all` plus a `flush` syscall per record. The writer is flushed
//! when a `gather_summary` marker is written and again when the `GatherLog`
//! is dropped (`BufWriter`'s drop flushes best-effort), so a completed run is
//! durable. Records written between the last summary and a hard kill
//! (SIGKILL) may still be lost from the buffer; failures are reported via
//! `tracing::warn` by the caller and never abort a gather.
//!
//! PERF-050: per-URL records are serialised from a borrowed
//! `#[derive(Serialize)]` struct rather than a `serde_json::Value` tree, so a
//! record no longer allocates a `Value` map or clones every detail
//! key/value.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::Utc;
use serde::{Serialize, Serializer, ser::SerializeMap};
use uuid::Uuid;

/// Write buffer size for the log file (PERF-049).
const WRITE_BUF_BYTES: usize = 64 * 1024;

/// JSONL logger for one web-gathering pass.
#[derive(Clone)]
///
/// Owns the log file path and a shared buffered writer; construct via
/// [`GatherLog::new`], then append records with [`GatherLog::log_url`] and
/// marker events with [`GatherLog::log_event`]. `GatherLog` is `Clone`; clones
/// share the same writer, so they all append to the one file. The log file is
/// created lazily by the first append.
pub struct GatherLog {
    /// Full path of the JSONL log file.
    path: PathBuf,
    /// Shared buffered writer, opened on the first append (PERF-049).
    writer: Arc<Mutex<Option<BufWriter<File>>>>,
}

impl GatherLog {
    /// Prepare a new gather log inside `log_dir`.
    ///
    /// `research_name` is sanitised so the file name is filesystem-safe and
    /// truncated to 64 characters. A short UUID suffix keeps repeated gather
    /// passes within one research run from clobbering each other. The log file
    /// itself is created by the first append and its handle retained behind a
    /// buffered writer, so per-record appends are in-memory writes (PERF-049).
    ///
    /// # Errors
    ///
    /// Returns an error when the log directory cannot be created.
    pub fn new(log_dir: &Path, research_name: &str) -> anyhow::Result<Self> {
        fs::create_dir_all(log_dir)?;
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let rand = Uuid::new_v4().simple().to_string();
        let name = format!(
            "research-{}-{timestamp}-{rand}-web.jsonl",
            sanitize(research_name)
        );
        let path = log_dir.join(name);
        Ok(Self {
            path,
            writer: Arc::new(Mutex::new(None)),
        })
    }

    /// Path of the underlying log file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Flush buffered records to disk.
    ///
    /// A no-op when nothing has been written yet.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying write fails.
    pub fn flush(&self) -> anyhow::Result<()> {
        Self::run_blocking(|| {
            let mut guard = self
                .writer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(writer) = guard.as_mut() {
                writer.flush()?;
            }
            Ok(())
        })
    }

    /// Append a raw JSON event (used for `gather_start`,
    /// `queries_decomposed`, and `gather_summary` markers).
    ///
    /// A `gather_summary` marker flushes the writer so a completed sweep is
    /// durable without a per-record flush.
    ///
    /// # Errors
    ///
    /// Returns an error when serialisation or the append/flush fails.
    pub fn log_event(&self, event: &serde_json::Value) -> anyhow::Result<()> {
        self.append_line(&serde_json::to_string(event)?)?;
        if event.get("event").and_then(serde_json::Value::as_str) == Some("gather_summary") {
            self.flush()?;
        }
        Ok(())
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
    /// Returns an error when serialisation or the append fails.
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
        let record = UrlRecord {
            timestamp: Rfc3339Now,
            url,
            query,
            status,
            title,
            search_tool,
            search_engine,
            reason,
            detail: FlattenDetail(detail.and_then(serde_json::Value::as_object)),
        };
        self.append_line(&serde_json::to_string(&record)?)
    }

    /// Run `f`, yielding the async worker to a blocking thread first when
    /// called from inside a multi-thread tokio runtime (FUNC-051).
    ///
    /// The log's synchronous entry points do blocking file I/O (a one-time
    /// `create_dir_all` + `open`, then buffered writes). Calling them directly
    /// from an async task occupies a tokio worker for the duration of the
    /// syscall. Inside a multi-thread runtime we use `block_in_place`, which
    /// hands the worker's other tasks to a sibling thread and runs `f` on a
    /// dedicated blocking thread; outside a runtime (or on a current-thread
    /// runtime, where `block_in_place` is illegal) we call `f` directly.
    fn run_blocking<T>(f: impl FnOnce() -> T) -> T {
        use tokio::runtime::RuntimeFlavor;
        match tokio::runtime::Handle::try_current() {
            Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(f)
            }
            _ => f(),
        }
    }

    /// Append one already-serialised JSON line (FUNC-051).
    ///
    /// The lock-guarded buffered write runs directly: after PERF-049 the
    /// first-use `open` and the explicit `flush` (the only blocking syscalls
    /// on this path) are wrapped in [`Self::run_blocking`]. Per call, the
    /// write is an in-memory [`BufWriter`] append *until the buffer fills* —
    /// at that point `write_all` performs a bounded one-buffer flush while
    /// still holding the lock. Because lines are short and the buffer is
    /// 8 KiB, that flush is rare and amortised.
    fn append_line(&self, line: &str) -> anyhow::Result<()> {
        let mut guard = self.lock_writer();
        self.ensure_open_blocking(&mut guard)?;
        let writer = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("gather log writer not initialised"))?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        Ok(())
    }

    /// Lock the writer, opening the log file on first use (blocking).
    ///
    /// Only the first-use `create_dir_all` + `open` is offloaded to a blocking
    /// thread via [`Self::run_blocking`]; subsequent calls return immediately.
    fn ensure_open_blocking(
        &self,
        guard: &mut MutexGuard<'_, Option<BufWriter<File>>>,
    ) -> anyhow::Result<()> {
        if guard.is_none() {
            let path = self.path.clone();
            let file = Self::run_blocking(move || {
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    fs::create_dir_all(parent)?;
                }
                fs::OpenOptions::new().create(true).append(true).open(&path)
            })?;
            **guard = Some(BufWriter::with_capacity(WRITE_BUF_BYTES, file));
        }
        Ok(())
    }

    /// Lock the writer without opening it.
    fn lock_writer(&self) -> MutexGuard<'_, Option<BufWriter<File>>> {
        self.writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Borrowed-fields record for one per-URL outcome (PERF-050).
#[derive(Serialize)]
struct UrlRecord<'a> {
    timestamp: Rfc3339Now,
    url: &'a str,
    query: &'a str,
    status: &'a str,
    title: &'a str,
    search_tool: &'a str,
    search_engine: &'a str,
    reason: Option<&'a str>,
    #[serde(flatten)]
    detail: FlattenDetail<'a>,
}

/// Serialises `detail` entries as top-level record fields.
struct FlattenDetail<'a>(Option<&'a serde_json::Map<String, serde_json::Value>>);

impl Serialize for FlattenDetail<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(detail) = self.0 {
            for (key, value) in detail {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

/// Serialises the current UTC instant as an RFC 3339 string.
struct Rfc3339Now;

impl Serialize for Rfc3339Now {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&Utc::now().to_rfc3339())
    }
}

/// Restrict a research name to filesystem-safe characters (alphanumeric,
/// `-` and `_`); everything else becomes `-`. Truncated to 64 characters.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_log_writes_considered_and_rejected_records() {
        let dir = tempfile::tempdir().unwrap();
        let log = GatherLog::new(dir.path(), "my research").unwrap();
        log.log_event(&serde_json::json!({"event": "gather_start", "topic": "t"}))
            .unwrap();
        log.log_url(
            "https://a.example",
            "q",
            "considered",
            "A",
            "mf_search",
            "openalex",
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
            "openalex",
            Some("relevance too low (Low)"),
            Some(&serde_json::json!({"relevance": "Low"})),
        )
        .unwrap();
        log.flush().unwrap();

        let contents = fs::read_to_string(log.path()).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);
        let start: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(start["event"], "gather_start");
        let considered: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(considered["status"], "considered");
        assert_eq!(considered["url"], "https://a.example");
        assert!(considered["reason"].is_null());
        assert!(considered["timestamp"].is_string());
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
    fn new_does_not_create_the_file_until_the_first_append() {
        let dir = tempfile::tempdir().unwrap();
        let log = GatherLog::new(dir.path(), "lazy").unwrap();
        assert!(!log.path().exists());
        log.flush().unwrap();
        assert!(!log.path().exists());

        log.log_url("u", "q", "considered", "t", "tool", "engine", None, None)
            .unwrap();
        assert!(log.path().exists());
    }

    #[test]
    fn gather_summary_flushes_without_explicit_flush() {
        let dir = tempfile::tempdir().unwrap();
        let log = GatherLog::new(dir.path(), "flush-test").unwrap();
        log.log_url("u", "q", "considered", "t", "tool", "engine", None, None)
            .unwrap();
        log.log_event(&serde_json::json!({"event": "gather_summary", "captured": 1}))
            .unwrap();

        // No explicit flush / drop: the summary marker must have flushed both
        // the preceding record and itself.
        let contents = fs::read_to_string(log.path()).unwrap();
        assert_eq!(contents.lines().count(), 2);
    }

    #[test]
    fn sanitize_replaces_unsafe_characters() {
        assert_eq!(sanitize("v1 rocket/german?"), "v1-rocket-german-");
        assert_eq!(sanitize("plain_name-1"), "plain_name-1");
    }
}
