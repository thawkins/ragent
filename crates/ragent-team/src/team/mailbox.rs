//! Mailbox types and I/O for inter-agent messaging.
//!
//! Each agent (lead and each teammate) has a dedicated mailbox file at
//! `mailbox/{agent-id}.json` inside the team directory.  Messages are
//! appended by senders and drained by the recipient.
//!
//! A global [`MailboxNotifierRegistry`] allows poll loops to be woken
//! instantly when a message is pushed, instead of relying solely on
//! periodic polling (see Milestone T6).

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub read: bool,
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
        }
    }
}

// ── Mailbox notifier registry ────────────────────────────────────────────────

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
    team_dir: PathBuf,
    agent_id: String,
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

    /// Read all messages from the mailbox without modifying it.
    pub fn read_all(&self) -> Result<Vec<MailboxMessage>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("read mailbox {}", self.path.display()))?;
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&raw).with_context(|| format!("parse mailbox {}", self.path.display()))
    }

    fn write_locked(file: &mut File, messages: &[MailboxMessage]) -> Result<()> {
        let json = serde_json::to_string_pretty(messages)?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(json.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    /// Append a message to the mailbox (acquires an exclusive lock).
    ///
    /// After writing, signals the in-process [`Notify`] handle (if
    /// registered) so the recipient's poll loop wakes immediately.
    pub fn push(&self, message: MailboxMessage) -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open mailbox {}", self.path.display()))?;

        file.lock_exclusive()?;

        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        let mut messages: Vec<MailboxMessage> = if raw.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&raw)?
        };

        messages.push(message);
        Self::write_locked(&mut file, &messages)?;
        file.unlock()?;

        // Wake the recipient's poll loop if one is registered.
        signal_notifier(&self.team_dir, &self.agent_id);

        Ok(())
    }

    /// Return all unread messages and mark them as read (acquires an exclusive lock).
    pub fn drain_unread(&self) -> Result<Vec<MailboxMessage>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open mailbox {}", self.path.display()))?;

        file.lock_exclusive()?;

        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        if raw.trim().is_empty() {
            file.unlock()?;
            return Ok(Vec::new());
        }

        let mut messages: Vec<MailboxMessage> = serde_json::from_str(&raw)?;
        let unread: Vec<MailboxMessage> = messages.iter().filter(|m| !m.read).cloned().collect();

        if !unread.is_empty() {
            for m in &mut messages {
                m.read = true;
            }
            Self::write_locked(&mut file, &messages)?;
        }

        file.unlock()?;
        Ok(unread)
    }

    /// Mark a specific message as read by `message_id`.
    pub fn mark_read(&self, message_id: &str) -> Result<bool> {
        if !self.path.exists() {
            return Ok(false);
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open mailbox {}", self.path.display()))?;

        file.lock_exclusive()?;

        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        if raw.trim().is_empty() {
            file.unlock()?;
            return Ok(false);
        }

        let mut messages: Vec<MailboxMessage> = serde_json::from_str(&raw)?;
        let found = messages.iter_mut().find(|m| m.message_id == message_id);

        if let Some(m) = found {
            if !m.read {
                m.read = true;
                Self::write_locked(&mut file, &messages)?;
            }
            file.unlock()?;
            Ok(true)
        } else {
            file.unlock()?;
            Ok(false)
        }
    }
}
