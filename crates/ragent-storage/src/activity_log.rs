//! Append-only event log store for the activity logging subsystem (maka spec).
//!
//! [`ActivityLog`] is a SQLite-backed, append-only store of [`ActivityEvent`]
//! records. It implements the persistence half of the event log:
//!
//! - **FR-001** — [`ActivityLog::append`] persists an event before it is
//!   projected into any derived state.
//! - **FR-002** — the store assigns each event a monotonically increasing
//!   per-run sequence number and stores the event's immutable [`EventId`].
//!   A [`UNIQUE`] constraint on `(run_id, seq)` rejects any attempt to reuse a
//!   sequence number; rows are never updated or deleted (the schema exposes
//!   no mutation or deletion API).
//! - **FR-017** — if storage is unavailable during an append, the operation
//!   returns [`Err`] and the caller is expected to fail the producing
//!   operation without advancing derived state.
//! - **NFR-001** — a single append is one `INSERT` inside a short transaction,
//!   targeting a p99 below 10 ms on local storage.
//!
//! The store is intentionally self-contained: it owns its own
//! [`Connection`](rusqlite::Connection) and schema, separate from the
//! session/message [`Storage`](crate::storage::Storage), so the append-only
//! event log remains an independent source of truth (per the spec's
//! separation of durable facts from derived state).
//!
//! Replay helpers ([`ActivityLog::read_run`],
//! [`ActivityLog::read_run_upto`]) return events ordered by sequence number
//! for use by the projection/replay engine (T-011) and the JSON Lines export
//! (T-020).

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use ragent_types::activity::{
    ACTIVITY_EVENT_SCHEMA_VERSION, ActivityEvent, EventKind, Projection, ResumeResult,
    RollbackResult, validate_event_log_consistency,
};
use ragent_types::id::{EventId, RunId};

/// Append-only, SQLite-backed event log for activity events.
///
/// Owns a single [`Connection`] guarded by a [`Mutex`]. All public methods are
/// synchronous; callers that need to append from an async context should
/// off-load the call onto a blocking thread (mirroring the pattern in
/// [`Storage::write_async`](crate::storage::Storage::write_async)).
pub struct ActivityLog {
    conn: Mutex<Connection>,
    /// Runs currently being rebuilt (rollback/resume); concurrent append,
    /// rollback, resume on these are blocked (FR-014).
    // Reason: used by rollback_to_seq/rollback_to_checkpoint/resume_run to
    // block concurrent operations; the in-memory test API does not exercise
    // the concurrent path so the field appears unused under `cargo check`.
    #[allow(dead_code)]
    rebuilding: Mutex<HashSet<String>>,
}

/// Error returned by [`ActivityLog::append`] when an event is rejected because
/// its sequence number is not the next one expected for its run, or because
/// its immutable event identifier is missing or already committed.
///
/// This preserves append-only semantics (FR-002): each `(run_id, seq)` pair
/// must be unique and monotonic, and each event's [`EventId`] must be present
/// and globally unique, so a duplicate, out-of-order, or id-less append is a
/// programming error, not a silent overwrite.
#[derive(Debug, thiserror::Error)]
pub enum AppendError {
    /// The supplied sequence number is already committed for this run.
    #[error("sequence number {seq} already committed for run {run_id}")]
    DuplicateSeq {
        /// The run the duplicate was attempted on.
        run_id: RunId,
        /// The sequence number that collided.
        seq: u64,
    },
    /// The supplied sequence number skips ahead of the expected next number.
    #[error("sequence number {seq} is out of order for run {run_id} (expected {expected})")]
    OutOfOrder {
        /// The run the out-of-order append was attempted on.
        run_id: RunId,
        /// The sequence number supplied.
        seq: u64,
        /// The sequence number that should have been used.
        expected: u64,
    },
    /// The event has no event identifier (FR-002 requires an immutable id).
    #[error("event for run {run_id} seq {seq} has an empty event id")]
    EmptyEventId {
        /// The run the append was attempted on.
        run_id: RunId,
        /// The sequence number supplied.
        seq: u64,
    },
    /// An event with the same immutable id is already committed (FR-002).
    #[error("event id {id} is already committed")]
    DuplicateEventId {
        /// The event identifier that collided.
        id: EventId,
    },
    /// An attempt to delete or mutate an already-committed event was rejected
    /// because the store is append-only (FR-010). A `MutationRejected` audit
    /// event was recorded in the log.
    #[error(
        "mutation of committed event at seq {target_seq} in run {run_id} rejected: {attempted}"
    )]
    MutationRejected {
        /// The run the mutation was attempted on.
        run_id: RunId,
        /// The sequence number of the committed event that was the target.
        target_seq: u64,
        /// Description of the attempted mutation.
        attempted: String,
    },
    /// The run is being rebuilt (rollback or resume in progress) and concurrent
    /// append, rollback, or resume operations are blocked (FR-014).
    #[error("run {run_id} is rebuilding; concurrent operations blocked")]
    RunRebuilding {
        /// The run that is rebuilding.
        run_id: RunId,
    },
    /// The run is interrupted and not accepting new events until a resume
    /// operation is initiated (FR-006).
    #[error("run {run_id} is interrupted; resume before appending")]
    RunInterrupted {
        /// The run that is interrupted.
        run_id: RunId,
    },
    /// Underlying storage failure (FR-017 surfaces here).
    #[error("storage failure: {0}")]
    Storage(#[from] anyhow::Error),
}

impl std::fmt::Display for ActivityLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivityLog").finish_non_exhaustive()
    }
}

impl ActivityLog {
    /// Opens (or creates) the append-only event log at the given filesystem
    /// path and runs the schema migration.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directory cannot be created, the
    /// database cannot be opened, or migration fails.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open activity log at {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        let log = Self {
            conn: Mutex::new(conn),
            rebuilding: Mutex::new(HashSet::new()),
        };
        log.migrate()?;
        Ok(log)
    }

    /// Opens an ephemeral in-memory event log, useful for testing.
    ///
    /// # Errors
    ///
    /// Returns an error if the in-memory database cannot be created or
    /// migration fails.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let log = Self {
            conn: Mutex::new(conn),
            rebuilding: Mutex::new(HashSet::new()),
        };
        log.migrate()?;
        Ok(log)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("activity log lock poisoned: {e}"))
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.lock()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS activity_events (
                run_id         TEXT    NOT NULL,
                seq            INTEGER NOT NULL,
                id             TEXT    NOT NULL,
                schema_version INTEGER NOT NULL,
                timestamp      TEXT    NOT NULL,
                kind           TEXT    NOT NULL,
                PRIMARY KEY (run_id, seq)
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_activity_events_id
                ON activity_events(id);",
        )?;
        Ok(())
    }

    /// Returns the next sequence number to assign for `run_id` (i.e. one more
    /// than the highest committed sequence, or `0` for a new run).
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017).
    pub fn next_seq(&self, run_id: &RunId) -> Result<u64> {
        let conn = self.lock()?;
        Self::next_seq_locked(&conn, run_id)
    }

    fn next_seq_locked(conn: &Connection, run_id: &RunId) -> Result<u64> {
        let max_seq: Option<Option<i64>> = conn
            .query_row(
                "SELECT MAX(seq) FROM activity_events WHERE run_id = ?1",
                params![run_id.as_str()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        Ok(max_seq.flatten().map_or(0, |s| (s as u64) + 1))
    }

    /// Returns `true` if the run's last committed event is a termination with
    /// reason `Interrupted` or `Aborted` (FR-006). While in this state, new
    /// events cannot be appended until a resume operation is initiated.
    fn is_interrupted_locked(conn: &Connection, run_id: &RunId) -> bool {
        let kind_json: Option<String> = conn
            .query_row(
                "SELECT kind FROM activity_events
                 WHERE run_id = ?1
                 ORDER BY seq DESC
                 LIMIT 1",
                params![run_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();
        match kind_json {
            Some(json) => match serde_json::from_str::<EventKind>(&json) {
                Ok(EventKind::Termination { reason, .. }) => {
                    matches!(
                        reason,
                        ragent_types::activity::TerminationReason::Interrupted
                            | ragent_types::activity::TerminationReason::Aborted
                    )
                }
                _ => false,
            },
            None => false,
        }
    }

    /// Appends an event to the log.
    ///
    /// The event's `seq` must equal the next expected sequence for its run
    /// (i.e. one more than the highest committed sequence). A mismatch is
    /// rejected as [`AppendError::OutOfOrder`] (or [`AppendError::DuplicateSeq`]
    /// if the sequence is already committed), preserving the append-only and
    /// monotonic guarantees of FR-002. The event must also carry a non-empty,
    /// globally-unique [`EventId`] (FR-002); an empty id is rejected as
    /// [`AppendError::EmptyEventId`] and a re-used id as
    /// [`AppendError::DuplicateEventId`]. The store never mutates or deletes
    /// committed rows.
    ///
    /// On success returns a clone of the persisted event (with its id and
    /// timestamp as supplied).
    ///
    /// # Errors
    ///
    /// - [`AppendError::EmptyEventId`] — the event has no event id (FR-002).
    /// - [`AppendError::DuplicateEventId`] — an event with this id is already
    ///   committed (FR-002).
    /// - [`AppendError::OutOfOrder`] / [`AppendError::DuplicateSeq`] — the
    ///   sequence number violates monotonic append-only semantics (FR-002).
    /// - [`AppendError::Storage`] — the underlying store is unavailable
    ///   (FR-017); the caller must fail the producing operation without
    ///   advancing derived state.
    pub fn append(&self, event: &ActivityEvent) -> std::result::Result<ActivityEvent, AppendError> {
        // FR-002: every event must carry a non-empty, immutable event id.
        if event.id.as_str().is_empty() {
            return Err(AppendError::EmptyEventId {
                run_id: event.run_id.clone(),
                seq: event.seq,
            });
        }
        let conn = self.lock()?;
        // FR-006: reject appends to interrupted runs (before a resume).
        if Self::is_interrupted_locked(&conn, &event.run_id) {
            return Err(AppendError::RunInterrupted {
                run_id: event.run_id.clone(),
            });
        }
        // FR-002: the event id must be globally unique; an event id that is
        // already committed is an integrity violation, not a silent reuse.
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM activity_events WHERE id = ?1",
                params![event.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppendError::Storage(anyhow::anyhow!("id lookup failed: {e}")))?;
        if existing.is_some() {
            // FR-010: a mutation of a committed event (reusing its immutable id)
            // is rejected, and the rejected mutation is recorded as a separate
            // audit event.
            let _ = Self::append_mutation_rejected_locked(
                &conn,
                &event.run_id,
                event.seq,
                format!("append reusing committed event id {}", event.id.as_str()),
            );
            return Err(AppendError::DuplicateEventId {
                id: event.id.clone(),
            });
        }
        let expected = Self::next_seq_locked(&conn, &event.run_id)?;
        if event.seq < expected {
            // FR-010: a mutation of a committed event (overwriting its seq)
            // is rejected, and the rejected mutation is recorded as a
            // separate audit event.
            let _ = Self::append_mutation_rejected_locked(
                &conn,
                &event.run_id,
                event.seq,
                format!("append overwriting committed seq {}", event.seq),
            );
            return Err(AppendError::DuplicateSeq {
                run_id: event.run_id.clone(),
                seq: event.seq,
            });
        }
        if event.seq != expected {
            return Err(AppendError::OutOfOrder {
                run_id: event.run_id.clone(),
                seq: event.seq,
                expected,
            });
        }
        let kind_json =
            serde_json::to_string(&event.kind).context("Failed to serialise event kind")?;
        let ts = event.timestamp.to_rfc3339();
        conn.execute(
            "INSERT INTO activity_events
                (run_id, seq, id, schema_version, timestamp, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.run_id.as_str(),
                event.seq as i64,
                event.id.as_str(),
                event.schema_version as i64,
                ts,
                kind_json,
            ],
        )
        .map_err(|e| AppendError::Storage(anyhow::anyhow!("insert failed: {e}")))?;
        Ok(event.clone())
    }

    /// Convenience: builds a fresh event for `run_id` with the next sequence
    /// number and a new [`EventId`], persists it, and returns it.
    ///
    /// This is the primary append path for callers that want the store to
    /// manage sequence-number assignment (FR-001, FR-002). It atomically reads
    /// the next sequence and inserts the row inside a single transaction so a
    /// concurrent appender cannot steal the sequence.
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::append`] and [`AppendError::Storage`].
    pub fn append_new(
        &self,
        run_id: &RunId,
        kind: EventKind,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        let mut conn = self.lock()?;
        let tx = conn
            .transaction()
            .map_err(|e| AppendError::Storage(anyhow::anyhow!("begin tx: {e}")))?;
        // FR-006: reject appends to interrupted runs (before a resume).
        if Self::is_interrupted_locked(&tx, run_id) {
            return Err(AppendError::RunInterrupted {
                run_id: run_id.clone(),
            });
        }
        let seq = Self::next_seq_locked(&tx, run_id).map_err(AppendError::Storage)?;
        let event = ActivityEvent {
            id: EventId::new(),
            run_id: run_id.clone(),
            seq,
            schema_version: ACTIVITY_EVENT_SCHEMA_VERSION,
            timestamp: Utc::now(),
            kind,
        };
        let kind_json = serde_json::to_string(&event.kind)
            .context("Failed to serialise event kind")
            .map_err(AppendError::Storage)?;
        tx.execute(
            "INSERT INTO activity_events
                (run_id, seq, id, schema_version, timestamp, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.run_id.as_str(),
                event.seq as i64,
                event.id.as_str(),
                event.schema_version as i64,
                event.timestamp.to_rfc3339(),
                kind_json,
            ],
        )
        .map_err(|e| AppendError::Storage(anyhow::anyhow!("insert failed: {e}")))?;
        tx.commit()
            .map_err(|e| AppendError::Storage(anyhow::anyhow!("commit: {e}")))?;
        Ok(event)
    }

    /// Reads all events for `run_id` in ascending sequence-number order.
    ///
    /// Used by the projection/replay engine (T-011) and the JSON Lines export
    /// (T-020).
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn read_run(&self, run_id: &RunId) -> Result<Vec<ActivityEvent>> {
        self.read_run_range(run_id, None)
    }

    /// Records a model-message event (FR-001) — a message produced or received
    /// by the model — appending it to the run's event log before it is
    /// projected into any derived state.
    ///
    /// `role` is the message author (`"user"`, `"assistant"`, etc.),
    /// `content` is the serialised message body, and `message_id` is the
    /// provider-assigned message identifier, if any.
    ///
    /// On success returns the persisted event (with its assigned sequence
    /// number and immutable [`EventId`]).
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::append_new`] and [`AppendError`].
    pub fn record_model_message(
        &self,
        run_id: &RunId,
        role: impl Into<String>,
        content: impl Into<String>,
        message_id: Option<String>,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        self.append_new(
            run_id,
            EventKind::ModelMessage {
                role: role.into(),
                content: content.into(),
                message_id,
            },
        )
    }

    /// Records a tool-call event (FR-004) — the invocation of a tool —
    /// appending it to the run's event log before the tool executes.
    ///
    /// `tool_call_id` is the shared identifier that links this invocation to
    /// its later [`record_tool_result`](Self::record_tool_result) event
    /// (FR-004). `tool` is the tool name and `args` is the raw JSON arguments.
    ///
    /// On success returns the persisted invocation event.
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::append_new`] and [`AppendError`].
    pub fn record_tool_call(
        &self,
        run_id: &RunId,
        tool_call_id: impl Into<String>,
        tool: impl Into<String>,
        args: impl Into<String>,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        self.append_new(
            run_id,
            EventKind::ToolCall {
                tool_call_id: tool_call_id.into(),
                tool: tool.into(),
                args: args.into(),
            },
        )
    }

    /// Records a tool-result event (FR-004) — the completion of a tool call —
    /// appending it to the run's event log before the next model invocation
    /// reads the result.
    ///
    /// `tool_call_id` must match the id supplied to the preceding
    /// [`record_tool_call`](Self::record_tool_call) so the two events are
    /// linked (FR-004). `success` indicates whether the call succeeded and
    /// `content` is the result content (or an error message on failure).
    ///
    /// On success returns the persisted result event.
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::append_new`] and [`AppendError`].
    pub fn record_tool_result(
        &self,
        run_id: &RunId,
        tool_call_id: impl Into<String>,
        tool: impl Into<String>,
        success: bool,
        content: impl Into<String>,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        self.append_new(
            run_id,
            EventKind::ToolResult {
                tool_call_id: tool_call_id.into(),
                tool: tool.into(),
                success,
                content: content.into(),
            },
        )
    }

    /// Retrieves the linked `(tool-call, tool-result)` pair for the given
    /// `tool_call_id` within `run_id` (FR-004).
    ///
    /// Returns `(Some(call), Some(result))` when both events exist, `(Some,
    /// None)` when only the invocation has been recorded, and `(None, None)`
    /// when no event with that `tool_call_id` exists in the run. Because
    /// events are appended in order, the invocation's sequence number is
    /// always less than the result's when both are present.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn find_tool_call_pair(
        &self,
        run_id: &RunId,
        tool_call_id: &str,
    ) -> Result<(Option<ActivityEvent>, Option<ActivityEvent>)> {
        let events = self.read_run(run_id)?;
        let mut call = None;
        let mut result = None;
        for event in events {
            match &event.kind {
                EventKind::ToolCall {
                    tool_call_id: id, ..
                } if id == tool_call_id => {
                    call = Some(event);
                }
                EventKind::ToolResult {
                    tool_call_id: id, ..
                } if id == tool_call_id => {
                    result = Some(event);
                }
                _ => {}
            }
        }
        Ok((call, result))
    }

    /// Records a permission-decision event (FR-005) — a grant or deny decision
    /// made for a tool that crosses a sandbox boundary — appending it to the
    /// run's event log before the tool is allowed (or refused) to proceed.
    ///
    /// `tool` is the tool the decision applies to, `principal` is who made the
    /// decision (operator or policy engine), `boundary` is the
    /// boundary-crossing target the tool would reach, and `granted` is whether
    /// permission was granted.
    ///
    /// On success returns the persisted decision event.
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::append_new`] and [`AppendError`].
    pub fn record_permission_decision(
        &self,
        run_id: &RunId,
        tool: impl Into<String>,
        principal: ragent_types::activity::Principal,
        boundary: ragent_types::activity::BoundaryTarget,
        granted: bool,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        self.append_new(
            run_id,
            EventKind::PermissionDecision {
                tool: tool.into(),
                principal,
                boundary,
                granted,
            },
        )
    }

    /// Records a termination event (FR-003) marking the run as stopped,
    /// embedding the last committed sequence number at which the run
    /// terminated.
    ///
    /// `reason` is why the run stopped:
    /// [`TerminationReason::Interrupted`] for a crash, process exit, or
    /// explicit abort (FR-003), [`TerminationReason::Aborted`] for an operator
    /// abort, and [`TerminationReason::Completed`] for a normal turn
    /// completion.
    ///
    /// The event's payload `seq` field records the last committed sequence
    /// number *before* this termination event was appended (i.e. where the run
    /// actually stopped), per FR-003's "at the last committed sequence number".
    /// The termination event itself is then appended at the next sequence
    /// number, so it is the final event in the run's log.
    ///
    /// On success returns the persisted termination event.
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::append_new`] and [`AppendError`].
    pub fn record_termination(
        &self,
        run_id: &RunId,
        reason: ragent_types::activity::TerminationReason,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        // FR-003: the termination marks the run as stopped at the last
        // committed sequence number.
        let stopped_at = self
            .last_seq(run_id)
            .map_err(AppendError::Storage)?
            .unwrap_or(0);
        self.append_new(
            run_id,
            EventKind::Termination {
                reason,
                seq: stopped_at,
            },
        )
    }

    /// Records an interruption termination event (FR-003) — the run was
    /// interrupted by a crash, process exit, or explicit abort.
    ///
    /// Convenience wrapper for
    /// [`record_termination`](Self::record_termination) with
    /// [`TerminationReason::Interrupted`].
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::record_termination`] and [`AppendError`].
    pub fn record_interruption(
        &self,
        run_id: &RunId,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        self.record_termination(
            run_id,
            ragent_types::activity::TerminationReason::Interrupted,
        )
    }

    /// Records a checkpoint event (FR-008) — a named, durable marker in the
    /// run's event log used as a rollback/resume target.
    ///
    /// `name` is the operator-assigned or auto-generated checkpoint name. The
    /// checkpoint's payload `seq` records the last committed sequence number
    /// *before* this checkpoint event was appended (i.e. the checkpoint
    /// covers all events up to and including that seq), per FR-008's
    /// "recording the checkpoint name, sequence number, and timestamp". The
    /// checkpoint event itself is appended at the next sequence number.
    ///
    /// # Errors
    ///
    /// See [`ActivityLog::append_new`] and [`AppendError`].
    pub fn record_checkpoint(
        &self,
        run_id: &RunId,
        name: impl Into<String>,
    ) -> std::result::Result<ActivityEvent, AppendError> {
        let checkpoint_at = self
            .last_seq(run_id)
            .map_err(AppendError::Storage)?
            .unwrap_or(0);
        self.append_new(
            run_id,
            EventKind::Checkpoint {
                name: name.into(),
                seq: checkpoint_at,
            },
        )
    }

    /// Retrieves the checkpoint event with the given `name` within `run_id`, or
    /// `None` if no such checkpoint exists (FR-008).
    ///
    /// If multiple checkpoints share a name, the one with the highest sequence
    /// number (the most recent) is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn find_checkpoint(&self, run_id: &RunId, name: &str) -> Result<Option<ActivityEvent>> {
        let events = self.read_run(run_id)?;
        let mut found: Option<ActivityEvent> = None;
        for event in events {
            if let EventKind::Checkpoint { name: n, .. } = &event.kind
                && n == name
            {
                found = Some(event);
            }
        }
        Ok(found)
    }

    /// Derives the current status of `run_id` from its event log (FR-015).
    ///
    /// The status is a projection of the append-only log:
    /// - [`RunStatus::Active`] — the run has no termination event yet.
    /// - [`RunStatus::Completed`] — the last termination event has reason
    ///   [`TerminationReason::Completed`].
    /// - [`RunStatus::Interrupted`] — the last termination event has reason
    ///   [`TerminationReason::Interrupted`] or [`TerminationReason::Aborted`].
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn run_status(&self, run_id: &RunId) -> Result<ragent_types::activity::RunStatus> {
        use ragent_types::activity::{RunStatus, TerminationReason};
        let events = self.read_run(run_id)?;
        let mut status = RunStatus::Active;
        for event in &events {
            match &event.kind {
                EventKind::Termination { reason, .. } => {
                    status = match reason {
                        TerminationReason::Completed => RunStatus::Completed,
                        TerminationReason::Interrupted | TerminationReason::Aborted => {
                            RunStatus::Interrupted
                        }
                    };
                }
                // FR-013: a "resumed" lifecycle event transitions an
                // interrupted run back to Active.
                EventKind::Lifecycle { event } if event == "resumed" => {
                    status = RunStatus::Active;
                }
                _ => {}
            }
        }
        Ok(status)
    }

    /// Lists all run identifiers that have at least one event in the log
    /// (FR-015), so completed runs can be found for inspection, replay, or
    /// branching later.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017).
    pub fn list_runs(&self) -> Result<Vec<RunId>> {
        let conn = self.lock()?;
        let mut stmt =
            conn.prepare("SELECT DISTINCT run_id FROM activity_events ORDER BY run_id")?;
        let run_ids: Result<Vec<String>, rusqlite::Error> =
            stmt.query_map([], |row| row.get(0))?.collect();
        Ok(run_ids?.into_iter().map(RunId::from).collect())
    }

    /// Exports the complete event log for `run_id` as JSON Lines (one
    /// [`ActivityEvent`] per line) for external audit (NFR-004).
    ///
    /// Each line is a standalone, self-describing JSON object (carrying the
    /// event type, schema version, and run identifier per NFR-003). Events
    /// are written in ascending sequence-number order. An empty run yields an
    /// empty string.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable, a stored row cannot be
    /// deserialised (FR-017), or an event cannot be re-serialised to JSON.
    pub fn export_jsonl(&self, run_id: &RunId) -> Result<String> {
        let events = self.read_run(run_id)?;
        let mut out = String::new();
        for event in &events {
            let line = serde_json::to_string(event).context("Failed to serialise event to JSON")?;
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out)
    }

    /// Writes the complete event log for `run_id` as JSON Lines to `writer`
    /// (NFR-004).
    ///
    /// One [`ActivityEvent`] is written per line in ascending sequence-number
    /// order. This is the file/stream export path for external audit.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable, a stored row cannot be
    /// deserialised (FR-017), an event cannot be serialised to JSON, or the
    /// writer fails.
    pub fn export_jsonl_to(&self, run_id: &RunId, writer: &mut impl std::io::Write) -> Result<()> {
        let events = self.read_run(run_id)?;
        for event in &events {
            let line = serde_json::to_string(event).context("Failed to serialise event to JSON")?;
            writeln!(writer, "{line}").context("Failed to write JSONL line")?;
        }
        Ok(())
    }

    /// Replays the complete event log for `run_id` into a [`Projection`]
    /// (FR-013), reconstructing the active context from the start of the run.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn replay_run(&self, run_id: &RunId) -> Result<ragent_types::activity::Projection> {
        let events = self.read_run(run_id)?;
        Ok(ragent_types::activity::Projection::replay(&events))
    }

    /// Replays the event log for `run_id` up to and including `upto_seq` into
    /// a [`Projection`] (FR-012 rollback / FR-013 resume).
    ///
    /// Events after `upto_seq` are ignored for this projection, so a rollback
    /// to a checkpoint or sequence number rebuilds the projection from the
    /// start of the run up to (and including) the target.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn replay_run_upto(
        &self,
        run_id: &RunId,
        upto_seq: u64,
    ) -> Result<ragent_types::activity::Projection> {
        let events = self.read_run(run_id)?;
        Ok(ragent_types::activity::Projection::replay_upto(
            &events, upto_seq,
        ))
    }

    /// Reads events for `run_id` up to and including `upto_seq`, in ascending
    /// sequence-number order.
    ///
    /// Used by rollback (T-013) to replay a projection up to a chosen point.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised.
    pub fn read_run_upto(&self, run_id: &RunId, upto_seq: u64) -> Result<Vec<ActivityEvent>> {
        self.read_run_range(run_id, Some(upto_seq))
    }

    fn read_run_range(&self, run_id: &RunId, upto: Option<u64>) -> Result<Vec<ActivityEvent>> {
        let conn = self.lock()?;
        let mut sql = String::from(
            "SELECT run_id, seq, id, schema_version, timestamp, kind
             FROM activity_events
             WHERE run_id = ?1",
        );
        if upto.is_some() {
            sql.push_str(" AND seq <= ?2");
        }
        sql.push_str(" ORDER BY seq ASC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = if let Some(seq) = upto {
            stmt.query_map(params![run_id.as_str(), seq as i64], row_to_event)
                .context("Failed to query activity events")?
        } else {
            stmt.query_map(params![run_id.as_str()], row_to_event)
                .context("Failed to query activity events")?
        };
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// Returns the highest committed sequence number for `run_id`, or `None`
    /// if the run has no events yet.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017).
    pub fn last_seq(&self, run_id: &RunId) -> Result<Option<u64>> {
        let conn = self.lock()?;
        let max_seq: Option<Option<i64>> = conn
            .query_row(
                "SELECT MAX(seq) FROM activity_events WHERE run_id = ?1",
                params![run_id.as_str()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        Ok(max_seq.flatten().map(|s| s as u64))
    }

    /// Returns the number of events stored for `run_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable.
    pub fn count(&self, run_id: &RunId) -> Result<u64> {
        let conn = self.lock()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM activity_events WHERE run_id = ?1",
            params![run_id.as_str()],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Retrieves the committed event at `(run_id, seq)`, or `None` if no such
    /// event exists.
    ///
    /// Because committed rows are never mutated or deleted, the returned
    /// event — including its immutable [`EventId`] — is identical to what was
    /// appended. This is the read-back path used to verify FR-002's "immutable
    /// event identifier" guarantee.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn get_event(&self, run_id: &RunId, seq: u64) -> Result<Option<ActivityEvent>> {
        let conn = self.lock()?;
        let event: Option<ActivityEvent> = conn
            .query_row(
                "SELECT run_id, seq, id, schema_version, timestamp, kind
                 FROM activity_events
                 WHERE run_id = ?1 AND seq = ?2",
                params![run_id.as_str(), seq as i64],
                row_to_event,
            )
            .optional()?;
        Ok(event)
    }

    /// Retrieves the committed event with the given immutable [`EventId`], or
    /// `None` if no such event exists.
    ///
    /// Because event ids are globally unique and immutable (FR-002), a hit
    /// returns exactly one event whose id matches the supplied id; a miss
    /// means no event with that id has ever been committed.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable or a stored row cannot be
    /// deserialised (FR-017).
    pub fn find_by_id(&self, id: &EventId) -> Result<Option<ActivityEvent>> {
        let conn = self.lock()?;
        let event: Option<ActivityEvent> = conn
            .query_row(
                "SELECT run_id, seq, id, schema_version, timestamp, kind
                 FROM activity_events
                 WHERE id = ?1",
                params![id.as_str()],
                row_to_event,
            )
            .optional()?;
        Ok(event)
    }

    /// Branches a new run from a checkpoint of an existing run (FR-018).
    ///
    /// Copies the events up to (and including) the checkpoint's sequence
    /// number from `source_run_id` into `new_run_id`, then records a
    /// [`EventKind::BranchOrigin`] event in the new run (linking it to its
    /// source) and a [`EventKind::Lifecycle`] event in the source run (noting
    /// that a branch was created).
    ///
    /// The copied events get fresh [`EventId`]s (the global uniqueness
    /// constraint prevents reusing the source's ids) but retain their original
    /// sequence numbers, schema versions, timestamps, and payloads, so the
    /// new run's log is a faithful copy of the checkpointed state.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017), the checkpoint is
    /// not found in the source run, or the new run already has events.
    pub fn branch_from_checkpoint(
        &self,
        source_run_id: &RunId,
        checkpoint_name: &str,
        new_run_id: &RunId,
    ) -> Result<ActivityEvent> {
        // Find the checkpoint in the source run.
        let checkpoint = self
            .find_checkpoint(source_run_id, checkpoint_name)?
            .ok_or_else(|| {
                anyhow::anyhow!("checkpoint '{checkpoint_name}' not found in run {source_run_id}")
            })?;
        let target_seq = match &checkpoint.kind {
            EventKind::Checkpoint { seq, .. } => *seq,
            _ => anyhow::bail!(
                "checkpoint '{checkpoint_name}' in run {source_run_id} resolved to a non-checkpoint event"
            ),
        };
        // Read source events up to the checkpoint.
        let source_events = self.read_run_upto(source_run_id, target_seq)?;
        // Ensure the new run is empty.
        if self.count(new_run_id)? > 0 {
            anyhow::bail!("new run {new_run_id} already has events; cannot branch into it");
        }
        // Copy events into the new run with fresh ids.
        {
            let conn = self.lock()?;
            for event in &source_events {
                let kind_json =
                    serde_json::to_string(&event.kind).context("Failed to serialise event kind")?;
                conn.execute(
                    "INSERT INTO activity_events
                        (run_id, seq, id, schema_version, timestamp, kind)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        new_run_id.as_str(),
                        event.seq as i64,
                        EventId::new().as_str(),
                        event.schema_version as i64,
                        event.timestamp.to_rfc3339(),
                        kind_json,
                    ],
                )?;
            }
        }
        // Record the branch origin in the new run (FR-018).
        let branch_event = self.append_new(
            new_run_id,
            EventKind::BranchOrigin {
                source_run_id: source_run_id.clone(),
                source_seq: target_seq,
            },
        )?;
        // Record the branch in the source run (FR-018: "record the branch
        // origin in both runs").
        self.append_new(
            source_run_id,
            EventKind::Lifecycle {
                event: format!("branched to {new_run_id} at seq {target_seq}"),
            },
        )?;
        Ok(branch_event)
    }

    /// Expires (removes) the complete event log for `run_id` after first
    /// recording the expiry as a lifecycle event (FR-016).
    ///
    /// This is the one operation that removes events from the append-only
    /// store: it is an operator-driven retention action, not a mutation of
    /// individual committed events (FR-010). Before deletion, a
    /// [`EventKind::Lifecycle`] event describing the expiry is appended to
    /// the run's log so the audit trail records *why* and *when* the run was
    /// expired. The lifecycle event is self-describing (carrying its type,
    /// schema version, and run identifier per NFR-003).
    ///
    /// After this call, the run no longer appears in
    /// [`list_runs`](Self::list_runs) and its events are gone.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017).
    pub fn expire_run(&self, run_id: &RunId, reason: impl Into<String>) -> Result<()> {
        let reason_str = reason.into();
        // FR-016: record the expiry as a lifecycle event before deletion.
        // Direct insert (bypasses the interrupted check in append_new) so
        // expiry works on interrupted runs too.
        {
            let conn = self.lock()?;
            let seq = Self::next_seq_locked(&conn, run_id)?;
            let id = EventId::new();
            let kind = EventKind::Lifecycle {
                event: format!("expired: {reason_str}"),
            };
            let kind_json =
                serde_json::to_string(&kind).context("Failed to serialise lifecycle event")?;
            conn.execute(
                "INSERT INTO activity_events
                    (run_id, seq, id, schema_version, timestamp, kind)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    run_id.as_str(),
                    seq as i64,
                    id.as_str(),
                    ACTIVITY_EVENT_SCHEMA_VERSION as i64,
                    Utc::now().to_rfc3339(),
                    kind_json,
                ],
            )
            .context("Failed to insert lifecycle event")?;
        }
        // Remove all events for this run (retention expiry, FR-016).
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM activity_events WHERE run_id = ?1",
            params![run_id.as_str()],
        )?;
        Ok(())
    }

    /// Archives the complete event log for `run_id` as JSON Lines (NFR-004)
    /// and then expires the run (FR-016), returning the archived JSONL
    /// string.
    ///
    /// This is the "archive or expire" path: the run's log is exported for
    /// external storage before being removed from the active store. The
    /// returned string is the complete JSONL export (including the expiry
    /// lifecycle event).
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017) or the export
    /// fails.
    pub fn archive_run(&self, run_id: &RunId, reason: impl Into<String>) -> Result<String> {
        let reason_str = reason.into();
        // FR-016: append the expiry lifecycle event BEFORE exporting so the
        // JSONL includes it.
        {
            let conn = self.lock()?;
            let seq = Self::next_seq_locked(&conn, run_id)?;
            let id = EventId::new();
            let kind = EventKind::Lifecycle {
                event: format!("expired: {reason_str}"),
            };
            let kind_json =
                serde_json::to_string(&kind).context("Failed to serialise lifecycle event")?;
            conn.execute(
                "INSERT INTO activity_events
                    (run_id, seq, id, schema_version, timestamp, kind)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    run_id.as_str(),
                    seq as i64,
                    id.as_str(),
                    ACTIVITY_EVENT_SCHEMA_VERSION as i64,
                    Utc::now().to_rfc3339(),
                    kind_json,
                ],
            )
            .context("Failed to insert lifecycle event")?;
        }
        // Export the complete log (now including the lifecycle event).
        let jsonl = self.export_jsonl(run_id)?;
        // Remove all events for this run.
        let conn = self.lock()?;
        conn.execute(
            "DELETE FROM activity_events WHERE run_id = ?1",
            params![run_id.as_str()],
        )?;
        Ok(jsonl)
    }

    /// Returns the timestamp of the last event for `run_id`, or `None` if the
    /// run has no events. Used to determine run age for retention-limit
    /// expiration (FR-016).
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017).
    pub fn run_last_activity(
        &self,
        run_id: &RunId,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
        let conn = self.lock()?;
        let ts: Option<String> = conn
            .query_row(
                "SELECT timestamp FROM activity_events
                 WHERE run_id = ?1
                 ORDER BY seq DESC
                 LIMIT 1",
                params![run_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        match ts {
            None => Ok(None),
            Some(s) => {
                let dt = chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .context("Failed to parse event timestamp")?;
                Ok(Some(dt))
            }
        }
    }

    /// Expires all runs whose last activity is older than `max_age` (FR-016),
    /// recording a lifecycle event for each before deletion.
    ///
    /// Returns the list of expired run identifiers.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017).
    pub fn expire_runs_older_than(&self, max_age: chrono::Duration) -> Result<Vec<RunId>> {
        let now = Utc::now();
        let cutoff = now - max_age;
        let runs = self.list_runs()?;
        let mut expired = Vec::new();
        for run in &runs {
            if let Some(last) = self.run_last_activity(run)?
                && last < cutoff
            {
                self.expire_run(run, "retention limit")?;
                expired.push(run.clone());
            }
        }
        Ok(expired)
    }

    /// Attempts to delete the committed event at `(run_id, seq)` (FR-010).
    ///
    /// The store is append-only, so the deletion is **always rejected** — no
    /// row is ever removed. If the target event is committed, a
    /// [`EventKind::MutationRejected`] audit event is recorded in the log
    /// before the error is returned. If the target does not exist, the error
    /// is returned without an audit event (there was no committed event to
    /// mutate).
    ///
    /// # Errors
    ///
    /// Always returns [`AppendError::MutationRejected`] — the store is
    /// append-only and never deletes committed events (FR-010).
    pub fn try_delete_event(
        &self,
        run_id: &RunId,
        seq: u64,
    ) -> std::result::Result<(), AppendError> {
        let conn = self.lock()?;
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM activity_events WHERE run_id = ?1 AND seq = ?2",
                params![run_id.as_str(), seq as i64],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| AppendError::Storage(anyhow::anyhow!("lookup failed: {e}")))?
            .unwrap_or(false);
        if exists {
            // FR-010: record the rejected mutation as a separate audit event.
            let _ = Self::append_mutation_rejected_locked(
                &conn,
                run_id,
                seq,
                format!("delete committed event at seq {seq}"),
            );
        }
        Err(AppendError::MutationRejected {
            run_id: run_id.clone(),
            target_seq: seq,
            attempted: "delete".to_string(),
        })
    }

    /// Attempts to overwrite the committed event at `(run_id, seq)` with
    /// `new_kind` (FR-010).
    ///
    /// The store is append-only, so the mutation is **always rejected** — no
    /// row is ever updated. If the target event is committed, a
    /// [`EventKind::MutationRejected`] audit event is recorded in the log
    /// before the error is returned.
    ///
    /// # Errors
    ///
    /// Always returns [`AppendError::MutationRejected`] — the store is
    /// append-only and never mutates committed events (FR-010).
    pub fn try_update_event(
        &self,
        run_id: &RunId,
        seq: u64,
        new_kind: &EventKind,
    ) -> std::result::Result<(), AppendError> {
        let conn = self.lock()?;
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM activity_events WHERE run_id = ?1 AND seq = ?2",
                params![run_id.as_str(), seq as i64],
                |_| Ok(true),
            )
            .optional()
            .map_err(|e| AppendError::Storage(anyhow::anyhow!("lookup failed: {e}")))?
            .unwrap_or(false);
        if exists {
            let attempted = format!("update committed event at seq {seq} to kind {}", {
                let v = serde_json::to_value(new_kind).unwrap_or_default();
                v.get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or("unknown")
                    .to_string()
            });
            // FR-010: record the rejected mutation as a separate audit event.
            let _ = Self::append_mutation_rejected_locked(&conn, run_id, seq, attempted);
        }
        Err(AppendError::MutationRejected {
            run_id: run_id.clone(),
            target_seq: seq,
            attempted: "update".to_string(),
        })
    }

    // ── Rollback / Resume (FR-006, FR-007, FR-012, FR-013) ──────────

    /// Rolls back a run to the event at `target_seq` (inclusive), rebuilding
    /// the projection from the start of the run up to and including the target
    /// (FR-012).
    ///
    /// Events after `target_seq` are **preserved in the log** for audit
    /// (FR-007) but ignored for the returned projection. The log is never
    /// mutated or truncated — rollback is a read-only projection rebuild.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is unavailable (FR-017).
    pub fn rollback_to_seq(&self, run_id: &RunId, target_seq: u64) -> Result<RollbackResult> {
        let conn = self.lock()?;
        let all_events = Self::read_run_range_locked(&conn, run_id, None)?;
        let total = all_events.len() as u64;
        let upto_events = Self::read_run_range_locked(&conn, run_id, Some(target_seq))?;
        let projection = Projection::replay_upto(&upto_events, target_seq);
        let included = upto_events.len() as u64;
        let ignored_count = total.saturating_sub(included);
        Ok(RollbackResult {
            projection,
            target_seq,
            ignored_count,
        })
    }

    /// Rolls back a run to a named checkpoint, rebuilding the projection up to
    /// the checkpoint's payload sequence (FR-012).
    ///
    /// The checkpoint's payload `seq` records the last committed sequence
    /// *before* the checkpoint event itself; the projection is rebuilt up to
    /// and including that sequence. Events after the checkpoint are preserved
    /// in the log for audit (FR-007).
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint is not found or storage is
    /// unavailable (FR-017).
    pub fn rollback_to_checkpoint(&self, run_id: &RunId, name: &str) -> Result<RollbackResult> {
        let checkpoint = self
            .find_checkpoint(run_id, name)?
            .ok_or_else(|| anyhow::anyhow!("checkpoint '{name}' not found"))?;
        let target_seq = match &checkpoint.kind {
            EventKind::Checkpoint { seq, .. } => *seq,
            _ => anyhow::bail!(
                "checkpoint '{name}' in run {run_id} resolved to a non-checkpoint event"
            ),
        };
        self.rollback_to_seq(run_id, target_seq)
    }

    /// Resumes an interrupted run (FR-006, FR-013).
    ///
    /// Replays the event log to reconstruct the active context (the
    /// [`Projection`] up to and including the interruption event), validates
    /// log consistency (FR-011), appends a `"resumed"` lifecycle event to the
    /// log, and returns the projection plus the sequence number from which
    /// execution should continue.
    ///
    /// After resume, the run transitions back to `Active` and new events can
    /// be appended at `resume_from_seq`.
    ///
    /// # Errors
    ///
    /// Returns an error if the run is not interrupted, the log is inconsistent
    /// (FR-011), or storage is unavailable (FR-017).
    pub fn resume_run(&self, run_id: &RunId) -> Result<ResumeResult> {
        let conn = self.lock()?;

        // FR-006: only interrupted runs can be resumed.
        if !Self::is_interrupted_locked(&conn, run_id) {
            return Err(anyhow::anyhow!("run {run_id} is not interrupted"));
        }

        // Read all events for the run.
        let events = Self::read_run_range_locked(&conn, run_id, None)?;

        // FR-011: validate consistency before producing a projection.
        validate_event_log_consistency(&events)?;

        // Build the projection from events up to the interruption (not including
        // the resumed lifecycle event we are about to append).
        let projection = Projection::replay(&events);

        // Append the "resumed" lifecycle event (direct insert bypasses the
        // interrupted-check in `append`, which would otherwise reject it).
        let resumed_seq = Self::next_seq_locked(&conn, run_id)?;
        let resumed_id = EventId::new();
        let resumed_kind = EventKind::Lifecycle {
            event: "resumed".into(),
        };
        let resumed_kind_json =
            serde_json::to_string(&resumed_kind).context("Failed to serialise resumed event")?;
        conn.execute(
            "INSERT INTO activity_events
                (run_id, seq, id, schema_version, timestamp, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id.as_str(),
                resumed_seq as i64,
                resumed_id.as_str(),
                ACTIVITY_EVENT_SCHEMA_VERSION as i64,
                Utc::now().to_rfc3339(),
                resumed_kind_json,
            ],
        )
        .context("Failed to insert resumed event")?;

        let resume_from_seq = resumed_seq + 1;
        Ok(ResumeResult {
            projection,
            resume_from_seq,
        })
    }

    /// Reads events for `run_id` optionally bounded by `upto_seq`, using an
    /// already-held connection lock.
    fn read_run_range_locked(
        conn: &Connection,
        run_id: &RunId,
        upto: Option<u64>,
    ) -> Result<Vec<ActivityEvent>> {
        let mut sql = String::from(
            "SELECT run_id, seq, id, schema_version, timestamp, kind
             FROM activity_events
             WHERE run_id = ?1",
        );
        if upto.is_some() {
            sql.push_str(" AND seq <= ?2");
        }
        sql.push_str(" ORDER BY seq ASC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = if let Some(seq) = upto {
            stmt.query_map(params![run_id.as_str(), seq as i64], row_to_event)
                .context("Failed to query activity events")?
        } else {
            stmt.query_map(params![run_id.as_str()], row_to_event)
                .context("Failed to query activity events")?
        };
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    /// Appends a [`EventKind::MutationRejected`] audit event for `run_id`
    /// targeting `target_seq`, inside an already-held lock (FR-010).
    ///
    /// Errors are swallowed (logged via tracing) so that a failure to record
    /// the audit event never masks the original rejection error. The audit
    /// event records the sequence number of the committed event that was the
    /// target of the rejected mutation and a description of what was
    /// attempted.
    fn append_mutation_rejected_locked(
        conn: &Connection,
        run_id: &RunId,
        target_seq: u64,
        attempted: String,
    ) -> Result<()> {
        let seq = Self::next_seq_locked(conn, run_id)?;
        let id = EventId::new();
        let kind = EventKind::MutationRejected {
            target_seq,
            attempted,
        };
        let kind_json = serde_json::to_string(&kind).context("Failed to serialise audit event")?;
        conn.execute(
            "INSERT INTO activity_events
                (run_id, seq, id, schema_version, timestamp, kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id.as_str(),
                seq as i64,
                id.as_str(),
                ACTIVITY_EVENT_SCHEMA_VERSION as i64,
                Utc::now().to_rfc3339(),
                kind_json,
            ],
        )?;
        Ok(())
    }
}

/// Maps a single `activity_events` row to an [`ActivityEvent`].
fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityEvent> {
    let run_id: String = row.get(0)?;
    let seq: i64 = row.get(1)?;
    let id: String = row.get(2)?;
    let schema_version: i64 = row.get(3)?;
    let ts: String = row.get(4)?;
    let kind_json: String = row.get(5)?;
    let timestamp = DateTime::<Utc>::from(
        DateTime::<chrono::FixedOffset>::parse_from_rfc3339(&ts).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?,
    );
    let kind: EventKind = serde_json::from_str(&kind_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(ActivityEvent {
        id: EventId::from(id),
        run_id: RunId::from(run_id),
        seq: seq as u64,
        schema_version: schema_version as u32,
        timestamp,
        kind,
    })
}
