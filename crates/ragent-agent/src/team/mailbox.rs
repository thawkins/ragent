//! Mailbox types and I/O for inter-agent messaging.
//!
//! Each agent (lead and each teammate) has a dedicated mailbox file at
//! `mailbox/{agent-id}.json` inside the team directory.  Messages are
//! appended by senders and drained by the recipient.
//!
//! A global [`MailboxNotifierRegistry`] allows poll loops to be woken
//! instantly when a message is pushed, instead of relying solely on
//! periodic polling (see Milestone T6).
//!
//! # On-disk format (PERF-022)
//!
//! Mailbox files use **newline-delimited JSON (JSONL)** — one
//! `MailboxMessage` serialised as a single JSON object per line. This makes
//! [`Mailbox::push`] an O(1) append (a single `write_line` under the advisory
//! lock) instead of the legacy read-modify-write cycle that re-serialised
//! the whole message array on every message.
//!
//! **Legacy single-JSON-array format is still supported for reading** so
//! existing mailbox files continue to load. The first non-whitespace
//! character of the file distinguishes the two:
//! - `[` → legacy single JSON array (read with `serde_json::from_str::<Vec<_>>`)
//! - `{` → JSONL (parse line by line)
//!
//! Any mutation path (push, mark_read, drain_unread) rewrites the file in
//! the new JSONL format, so legacy files are transparently migrated on the
//! first write after an upgrade.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use uuid::Uuid;

// ── Message type ─────────────────────────────────────────────────────────────

/// The semantic category of a mailbox message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// A free-form direct message.
    Message,
    /// A broadcast from the lead to all teammates.
    Broadcast,
    /// Teammate submits a plan for lead approval.
    PlanRequest,
    /// Lead approved a teammate plan.
    PlanApproved,
    /// Lead rejected a teammate plan.
    PlanRejected,
    /// Teammate reports it is idle.
    IdleNotify,
    /// Lead requests graceful shutdown of a teammate.
    ShutdownRequest,
    /// Teammate acknowledges shutdown.
    ShutdownAck,
}

// ── Message ───────────────────────────────────────────────────────────────────

/// A single mailbox message.
///
/// `correlation_id` (M5-T4) links request/reply pairs: set on `PlanRequest`,
/// `ShutdownRequest`, and copied into the corresponding `PlanApproved` /
/// `PlanRejected` / `ShutdownAck` reply. `#[serde(deny_unknown_fields)]`
/// rejects unknown fields on manual edits (M5-T3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailboxMessage {
    /// Unique message identifier (UUID v4).
    pub message_id: String,
    /// Sender's agent ID or `"lead"`.
    pub from: String,
    /// Recipient's agent ID or `"lead"`.
    pub to: String,
    /// Semantic type of the message.
    #[serde(rename = "type")]
    pub message_type: MessageType,
    /// Human-readable content (plan text, feedback, free-form text, etc.).
    pub content: String,
    /// When the message was sent.
    pub sent_at: DateTime<Utc>,
    /// Whether the recipient has read this message.
    #[serde(default)]
    pub read: bool,
    /// Optional correlation id linking a request to its reply (M5-T4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl MailboxMessage {
    /// Create a new unread message with a freshly generated UUID.
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        message_type: MessageType,
        content: impl Into<String>,
    ) -> Self {
        Self {
            message_id: Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
            message_type,
            content: content.into(),
            sent_at: Utc::now(),
            read: false,
            correlation_id: None,
        }
    }

    /// Create a new unread message with a correlation id (M5-T4).
    ///
    /// Used for request messages (`PlanRequest`, `ShutdownRequest`) so the
    /// corresponding reply can copy the correlation id.
    pub fn new_correlated(
        from: impl Into<String>,
        to: impl Into<String>,
        message_type: MessageType,
        content: impl Into<String>,
        correlation_id: impl Into<String>,
    ) -> Self {
        let mut msg = Self::new(from, to, message_type, content);
        msg.correlation_id = Some(correlation_id.into());
        msg
    }

    /// Validate the message's invariants (M5-T3).
    ///
    /// Returns `Ok(())` if the message is well-formed, or an error describing
    /// the first violation. Checks:
    /// - `from` and `to` are non-empty.
    /// - `message_id` is a plausible UUID (non-empty).
    pub fn validate(&self) -> anyhow::Result<()> {
        use anyhow::anyhow;
        if self.message_id.is_empty() {
            return Err(anyhow!("message_id is empty"));
        }
        if self.from.is_empty() {
            return Err(anyhow!("message {} has empty from", self.message_id));
        }
        if self.to.is_empty() {
            return Err(anyhow!("message {} has empty to", self.message_id));
        }
        Ok(())
    }
}

// ── Lock-file helpers ────────────────────────────────────────────────────────

/// Return the companion lock file for a data file (e.g. `foo.json` ->
/// `foo.json.lock`).  The lock file is a stable file whose inode never
/// changes, so `flock` remains effective across atomic renames of the data
/// file.
fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

/// Acquire an advisory `flock` on the companion lock file.
///
/// `exclusive` selects `lock_exclusive` (writers) or `lock_shared` (readers).
fn acquire_lock(path: &Path, exclusive: bool) -> Result<File> {
    let lock = lock_path(path);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock)
        .with_context(|| format!("open lock file {}", lock.display()))?;
    if exclusive {
        file.lock_exclusive()
            .with_context(|| format!("acquire exclusive lock on {}", lock.display()))?;
    } else {
        file.lock_shared()
            .with_context(|| format!("acquire shared lock on {}", lock.display()))?;
    }
    Ok(file)
}

// ── Mailbox notifier registry ───────────────────��────────────────────────────

type NotifyKey = (PathBuf, String);

/// Process-wide registry that maps `(team_dir, agent_id)` to a
/// [`tokio::sync::Notify`] handle.  When [`Mailbox::push`] writes a
/// message it calls [`signal_notifier`] so that the recipient's poll
/// loop wakes immediately instead of waiting for the fallback interval.
fn notifier_map() -> &'static RwLock<HashMap<NotifyKey, Arc<Notify>>> {
    static MAP: OnceLock<RwLock<HashMap<NotifyKey, Arc<Notify>>>> = OnceLock::new();
    MAP.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a [`Notify`] handle for the given agent so that
/// [`Mailbox::push`] can wake its poll loop.
pub fn register_notifier(team_dir: &Path, agent_id: &str, notify: Arc<Notify>) {
    let key: NotifyKey = (team_dir.to_path_buf(), agent_id.to_string());
    if let Ok(mut map) = notifier_map().write() {
        map.insert(key, notify);
    }
}

/// Remove a previously registered notifier (called on teammate shutdown).
pub fn deregister_notifier(team_dir: &Path, agent_id: &str) {
    let key: NotifyKey = (team_dir.to_path_buf(), agent_id.to_string());
    if let Ok(mut map) = notifier_map().write() {
        map.remove(&key);
    }
}

/// Wake the poll loop for `agent_id` if a notifier is registered.
fn signal_notifier(team_dir: &Path, agent_id: &str) {
    let key: NotifyKey = (team_dir.to_path_buf(), agent_id.to_string());
    if let Ok(map) = notifier_map().read()
        && let Some(notify) = map.get(&key)
    {
        notify.notify_one();
    }
}

// ── Mailbox ───────────────────────────────────────────────────────────────────

/// File-backed per-agent mailbox stored at `mailbox/{agent-id}.json`.
///
/// Stores the `team_dir` and `agent_id` so that [`Self::push`] can
/// signal the in-process notifier after writing.
pub struct Mailbox {
    path: PathBuf,
    /// PERF-016: exposed so the `*_blocking` async wrappers can reconstruct
    /// a `Mailbox` inside a `spawn_blocking` closure without re-walking
    /// the path logic.
    pub team_dir: PathBuf,
    /// PERF-016: exposed for the same reason as `team_dir`.
    pub agent_id: String,
}

impl Mailbox {
    /// Open a mailbox for `agent_id` inside `team_dir/mailbox/`.
    pub fn open(team_dir: &Path, agent_id: &str) -> Result<Self> {
        let dir = team_dir.join("mailbox");
        fs::create_dir_all(&dir)
            .with_context(|| format!("create mailbox dir {}", dir.display()))?;
        let path = dir.join(format!("{agent_id}.json"));
        Ok(Self {
            path,
            team_dir: team_dir.to_path_buf(),
            agent_id: agent_id.to_string(),
        })
    }

    /// Read all messages from the mailbox (acquires a shared lock on the
    /// companion lock file).
    ///
    /// M6-T5: if the mailbox file cannot be parsed (corrupt JSON), it is
    /// moved aside to `<path>.corrupt.<timestamp>` and an empty mailbox is
    /// returned, rather than propagating the parse error and permanently
    /// disabling the inbox. The incident is logged via `tracing::warn`.
    ///
    /// PERF-022: supports both the legacy single-JSON-array format (file
    /// starts with `[`) and the current newline-delimited JSON / JSONL
    /// format (each line is one `MailboxMessage`). See the crate-level
    /// docs for the migration story.
    pub fn read_all(&self) -> Result<Vec<MailboxMessage>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let lock = acquire_lock(&self.path, false)?;
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("read mailbox {}", self.path.display()))?;
        drop(lock);
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        match parse_messages(&raw) {
            Ok(msgs) => Ok(msgs),
            Err(e) => {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "M6-T5: mailbox is corrupt; moving aside and returning empty"
                );
                // M6-T5: move the corrupt file aside so the inbox can recover.
                let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
                let corrupt_path = self.path.with_extension(format!("corrupt.{ts}.json"));
                let _ = fs::rename(&self.path, &corrupt_path);
                Ok(Vec::new())
            }
        }
    }

    /// Write `messages` to `path` in the current JSONL format while holding
    /// the companion lock.
    ///
    /// The temp file name includes a UUID so concurrent writers cannot collide
    /// on the same temp path (Milestone 1, M1-T4).
    ///
    /// PERF-022: the on-disk format is now newline-delimited JSON (one
    /// `MailboxMessage` per line). This is what every mutation path writes;
    /// legacy single-array files are transparently migrated the first time
    /// they are written.
    fn write_locked(path: &Path, messages: &[MailboxMessage]) -> Result<()> {
        let mut json = String::new();
        for m in messages {
            // Each line is one serialised message; a trailing `\n` separates
            // records so the file is valid JSONL.
            let line = serde_json::to_string(m)?;
            json.push_str(&line);
            json.push('\n');
        }
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "mailbox.json".to_string());
        let temp_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".{file_name}.{}", Uuid::new_v4()));
        fs::write(&temp_path, json)?;
        let result: Result<()> = (|| {
            let temp = OpenOptions::new().read(true).open(&temp_path)?;
            temp.sync_all()
                .with_context(|| format!("sync temp file {}", temp_path.display()))?;
            fs::rename(&temp_path, path)
                .with_context(|| format!("rename {} -> {}", temp_path.display(), path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    /// PERF-022: append a single `MailboxMessage` to the mailbox file as a
    /// new JSONL line, **without** re-reading and re-serialising the whole
    /// file. This converts `push` from an O(N) full-file rewrite (where N
    /// is the existing mailbox size) into an O(1) append under the
    /// advisory lock.
    ///
    /// Legacy single-JSON-array files are detected by their leading `[` and
    /// transparently migrated to JSONL on the first `push`: the file is read
    /// once, the existing messages are re-serialised line-by-line, and the
    /// new message is appended, all in a single atomic rename. After the
    /// first `push`, the file is JSONL and subsequent `push`es are pure
    /// appends.
    ///
    /// After writing, signals the in-process [`Notify`] handle (if
    /// registered) so the recipient's poll loop wakes immediately.
    pub fn push(&self, message: MailboxMessage) -> Result<()> {
        let lock = acquire_lock(&self.path, true)?;

        // Detect legacy format by inspecting the first non-whitespace byte.
        let needs_migration = self.path.exists()
            && match fs::read(&self.path) {
                Ok(bytes) => bytes
                    .iter()
                    .find(|b| !b.is_ascii_whitespace())
                    .is_some_and(|b| *b == b'['),
                Err(_) => false,
            };

        if needs_migration {
            // Read existing messages (legacy array), then write everything
            // back in the new JSONL format with the new message appended.
            let raw = fs::read_to_string(&self.path)?;
            let mut messages: Vec<MailboxMessage> = if raw.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str(&raw)?
            };
            messages.push(message);
            let result = Self::write_locked(&self.path, &messages);
            drop(lock);
            result?;
            signal_notifier(&self.team_dir, &self.agent_id);
            return Ok(());
        }

        // JSONL fast path: serialise one message and append it as a line.
        let line = serde_json::to_string(&message)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)
            .with_context(|| format!("open mailbox for append {}", self.path.display()))?;
        if !line.ends_with('\n') {
            writeln!(file, "{line}")?;
        } else {
            file.write_all(line.as_bytes())?;
        }
        file.sync_all()
            .with_context(|| format!("sync mailbox {}", self.path.display()))?;
        drop(lock);

        // Wake the recipient's poll loop if one is registered.
        signal_notifier(&self.team_dir, &self.agent_id);

        Ok(())
    }

    /// Return all unread messages **without** marking them as read (acquires
    /// a shared lock).
    ///
    /// This is the "peek" half of the read-vs-processed split (M4-T1). Callers
    /// that successfully forward the messages to the model should follow up
    /// with [`Mailbox::acknowledge`] to mark each message as read. Callers
    /// that fail to process the messages can leave them unread so they are
    /// redelivered on the next peek.
    ///
    /// Use [`Mailbox::drain_unread`] for the legacy "read and mark" behaviour
    /// (used by the mailbox poll loop, which treats event publishing as the
    /// processing step).
    pub fn peek_unread(&self) -> Result<Vec<MailboxMessage>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let lock = acquire_lock(&self.path, false)?;
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("read mailbox {}", self.path.display()))?;
        drop(lock);
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        // M6-T5: recover from corruption instead of disabling the inbox.
        match parse_messages(&raw) {
            Ok(messages) => Ok(messages.into_iter().filter(|m| !m.read).collect()),
            Err(e) => {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "M6-T5: mailbox is corrupt (peek_unread); moving aside"
                );
                let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
                let corrupt_path = self.path.with_extension(format!("corrupt.{ts}.json"));
                let _ = fs::rename(&self.path, &corrupt_path);
                Ok(Vec::new())
            }
        }
    }

    /// Return all unread messages and mark them as read (acquires an exclusive lock).
    ///
    /// This is the legacy "read and mark" path. It is still used by the
    /// mailbox poll loop in [`crate::team::manager::TeamManager`], which
    /// treats publishing an [`Event::TeammateMessage`] as the successful
    /// processing step. Tools that want at-least-once delivery semantics
    /// (redelivery on failure) should use [`peek_unread`] + [`acknowledge`]
    /// instead.
    ///
    /// M6-T5: if the mailbox file is corrupt it is moved aside and an empty
    /// list is returned, so the poll loop can continue with a fresh inbox
    /// instead of looping forever on a parse error.
    pub fn drain_unread(&self) -> Result<Vec<MailboxMessage>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let lock = acquire_lock(&self.path, true)?;

        let raw = fs::read_to_string(&self.path)?;
        if raw.trim().is_empty() {
            drop(lock);
            return Ok(Vec::new());
        }

        let mut messages: Vec<MailboxMessage> = match parse_messages(&raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "M6-T5: mailbox is corrupt (drain_unread); moving aside"
                );
                drop(lock);
                let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
                let corrupt_path = self.path.with_extension(format!("corrupt.{ts}.json"));
                let _ = fs::rename(&self.path, &corrupt_path);
                return Ok(Vec::new());
            }
        };
        let unread: Vec<MailboxMessage> = messages.iter().filter(|m| !m.read).cloned().collect();

        let result = if !unread.is_empty() {
            for m in &mut messages {
                m.read = true;
            }
            Self::write_locked(&self.path, &messages)
        } else {
            Ok(())
        };
        drop(lock);
        result?;
        Ok(unread)
    }

    /// Acknowledge that a message has been processed, marking it as read by
    /// `message_id` (acquires an exclusive lock).
    ///
    /// This is the "ack" half of the read-vs-processed split (M4-T1). Returns
    /// `true` if the message existed and was transitioned from unread to read,
    /// `false` if it was already read or not found. This is semantically
    /// equivalent to [`mark_read`] but is named to make the
    /// peek → process → acknowledge flow explicit.
    pub fn acknowledge(&self, message_id: &str) -> Result<bool> {
        self.mark_read(message_id)
    }

    /// Mark a specific message as read by `message_id`.
    pub fn mark_read(&self, message_id: &str) -> Result<bool> {
        let ids = [message_id.to_string()];
        let changed = self.mark_all_read(&ids)?;
        Ok(changed > 0)
    }

    /// PERF-020: Mark multiple messages as read in a **single**
    /// lock → read → mark all → write → unlock cycle.
    ///
    /// Previously [`team_read_messages`](crate::tools::team_read_messages)
    /// called [`acknowledge`](Self::acknowledge) (which delegates to
    /// [`mark_read`](Self::mark_read)) once per unread message, producing
    /// N full read-modify-write cycles for N messages. This method collapses
    /// those N cycles into one.
    ///
    /// Returns the number of messages that were transitioned from unread to
    /// read. Already-read or unknown `message_id`s are counted as unchanged
    /// (idempotent, matching the single-message `mark_read` contract).
    pub fn mark_all_read(&self, message_ids: &[String]) -> Result<usize> {
        if message_ids.is_empty() {
            return Ok(0);
        }
        if !self.path.exists() {
            return Ok(0);
        }

        let lock = acquire_lock(&self.path, true)?;

        let raw = fs::read_to_string(&self.path)?;
        if raw.trim().is_empty() {
            drop(lock);
            return Ok(0);
        }

        let mut messages: Vec<MailboxMessage> = parse_messages(&raw)?;
        let target: std::collections::HashSet<&str> =
            message_ids.iter().map(String::as_str).collect();

        let mut changed = 0usize;
        for m in &mut messages {
            if target.contains(m.message_id.as_str()) && !m.read {
                m.read = true;
                changed += 1;
            }
        }

        let result = if changed > 0 {
            Self::write_locked(&self.path, &messages)
        } else {
            Ok(())
        };
        drop(lock);
        result?;
        Ok(changed)
    }
}

/// PERF-022: parse a mailbox file body into a `Vec<MailboxMessage>`,
/// transparently supporting both on-disk formats:
///
/// - **Legacy single JSON array** — file starts (after leading whitespace)
///   with `[`. Parsed with `serde_json::from_str::<Vec<MailboxMessage>>`.
/// - **JSONL** — newline-delimited JSON, one `MailboxMessage` per non-empty
///   line. Parsed line by line; blank lines are skipped.
///
/// Returns the first parse error encountered (the caller is responsible for
/// the M6-T5 corruption-recovery path: moving the file aside and returning
/// an empty vec).
fn parse_messages(raw: &str) -> Result<Vec<MailboxMessage>> {
    // Peek at the first non-whitespace byte to decide the format.
    let first = raw.trim_start().chars().next();
    match first {
        Some('[') => {
            // Legacy single-JSON-array format.
            Ok(serde_json::from_str::<Vec<MailboxMessage>>(raw)?)
        }
        Some('{') => {
            // JSONL: one serialised message per line.
            let mut messages = Vec::new();
            for (idx, line) in raw.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let msg: MailboxMessage = serde_json::from_str(trimmed)
                    .with_context(|| format!("parse mailbox JSONL line {}", idx + 1))?;
                messages.push(msg);
            }
            Ok(messages)
        }
        // Empty file or unrecognised leading byte → treat as empty.
        // (Empty files are common before the first push.)
        Some(_) if raw.trim().is_empty() => Ok(Vec::new()),
        Some(c) => {
            // Anything else is either empty (handled above) or malformed.
            anyhow::bail!("unrecognised mailbox format (first non-whitespace char: {c:?})");
        }
        None => Ok(Vec::new()),
    }
}

impl Mailbox {
    /// PERF-016: `spawn_blocking` wrapper around [`Mailbox::push`].
    ///
    /// `push` performs a synchronous `fs::read_to_string` + `serde_json`
    /// deserialise + `fs::write` under a `flock`. On the async path this
    /// stalls the tokio worker thread for the duration of the I/O. This
    /// helper moves the whole read-modify-write cycle onto a blocking-pool
    /// thread so the async executor stays free to drive other futures
    /// (teammate event forwarding, watchdog, etc.).
    ///
    /// The `Mailbox` handle itself is cheap to reconstruct (`Mailbox::open`
    /// is just two `PathBuf` clones), so we pass the `team_dir` + `agent_id`
    /// into the blocking closure and re-open the mailbox there.
    pub async fn push_blocking(&self, message: MailboxMessage) -> Result<()> {
        let team_dir = self.team_dir.clone();
        let agent_id = self.agent_id.clone();
        tokio::task::spawn_blocking(move || {
            let mbox = Self::open(&team_dir, &agent_id)?;
            mbox.push(message)
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`Mailbox::peek_unread`].
    pub async fn peek_unread_blocking(&self) -> Result<Vec<MailboxMessage>> {
        let team_dir = self.team_dir.clone();
        let agent_id = self.agent_id.clone();
        tokio::task::spawn_blocking(move || {
            let mbox = Self::open(&team_dir, &agent_id)?;
            mbox.peek_unread()
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`Mailbox::drain_unread`].
    pub async fn drain_unread_blocking(&self) -> Result<Vec<MailboxMessage>> {
        let team_dir = self.team_dir.clone();
        let agent_id = self.agent_id.clone();
        tokio::task::spawn_blocking(move || {
            let mbox = Self::open(&team_dir, &agent_id)?;
            mbox.drain_unread()
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`Mailbox::mark_read`] /
    /// [`Mailbox::acknowledge`].
    pub async fn mark_read_blocking(&self, message_id: String) -> Result<bool> {
        let team_dir = self.team_dir.clone();
        let agent_id = self.agent_id.clone();
        tokio::task::spawn_blocking(move || {
            let mbox = Self::open(&team_dir, &agent_id)?;
            mbox.mark_read(&message_id)
        })
        .await?
    }

    /// PERF-016 / PERF-020: `spawn_blocking` wrapper around
    /// [`Mailbox::mark_all_read`].
    pub async fn mark_all_read_blocking(&self, message_ids: Vec<String>) -> Result<usize> {
        let team_dir = self.team_dir.clone();
        let agent_id = self.agent_id.clone();
        tokio::task::spawn_blocking(move || {
            let mbox = Self::open(&team_dir, &agent_id)?;
            mbox.mark_all_read(&message_ids)
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`Mailbox::read_all`].
    pub async fn read_all_blocking(&self) -> Result<Vec<MailboxMessage>> {
        let team_dir = self.team_dir.clone();
        let agent_id = self.agent_id.clone();
        tokio::task::spawn_blocking(move || {
            let mbox = Self::open(&team_dir, &agent_id)?;
            mbox.read_all()
        })
        .await?
    }
}
