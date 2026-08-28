//! Activity-log event schema and event types.
//!
//! This module defines the **durable execution facts** recorded by the
//! activity logging subsystem (see the `maka` spec, SPEC.md). An
//! [`ActivityEvent`] is an immutable, self-describing record of a single
//! execution fact — a model message, a tool call, a tool result, a permission
//! decision, a checkpoint, or a termination — appended to a run's append-only
//! event log before it is projected into any user-facing or derived state
//! (FR-001).
//!
//! Each event carries:
//!
//! - an immutable [`EventId`] (FR-002),
//! - a monotonically increasing per-run sequence number [`ActivityEvent::seq`]
//!   (FR-002),
//! - the [`RunId`] it belongs to,
//! - a `schema_version` field so logs remain
//!   replayable across version upgrades (NFR-003),
//! - a UTC timestamp,
//! - a typed [`EventKind`] payload describing what happened.
//!
//! The event is self-describing: the [`EventKind`] discriminator, the schema
//! version, and the run identifier travel with every record (NFR-003), so a
//! log written by one version of the system can still be replayed by a later
//! version that understands the older schema.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::{EventId, RunId};

/// Schema version stamped on every activity event.
///
/// Bump this when the wire format of any [`EventKind`] payload changes in a
/// way that is not backwards-compatible. The version travels with each event
/// (NFR-003) so older logs remain replayable.
pub const ACTIVITY_EVENT_SCHEMA_VERSION: u32 = 1;

/// The lifecycle state of a run, derived from the event log.
///
/// States transition as events are appended; the run begins [`RunStatus::Active`]
/// and ends in [`RunStatus::Completed`] (turn-termination event), or
/// [`RunStatus::Interrupted`] (termination-on-interruption event, FR-003).
/// Resume moves an interrupted run back to [`RunStatus::Active`] (FR-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// The run is accepting new events.
    Active,
    /// The run was interrupted (FR-003) and is resumable but not appendable
    /// until a resume operation is initiated (FR-006).
    Interrupted,
    /// The run finished normally and no longer accepts events.
    Completed,
    /// A resume operation found the log inconsistent and the run cannot be
    /// recovered (FR-011).
    Unrecoverable,
    /// The run's derived state is being rebuilt from the event log (FR-014);
    /// rollback, resume, and append are blocked while in this state.
    Rebuilding,
    /// The run was rolled back to a checkpoint; events after the checkpoint
    /// are retained for audit (FR-007) but ignored for the projection.
    RolledBack,
}

/// Why a run terminated.
///
/// Carried by [`EventKind::Termination`] events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    /// The turn completed normally.
    Completed,
    /// The run was interrupted by a crash, process exit, or explicit abort
    /// (FR-003).
    Interrupted,
    /// The operator explicitly aborted the run.
    Aborted,
}

impl TerminationReason {
    /// The [`RunStatus`] a run takes after terminating with this reason.
    ///
    /// Single source of truth for the reason-to-status mapping shared by
    /// [`Projection::apply`] and the storage layer's status queries.
    #[must_use]
    pub fn derived_status(self) -> RunStatus {
        match self {
            Self::Completed => RunStatus::Completed,
            Self::Interrupted | Self::Aborted => RunStatus::Interrupted,
        }
    }
}

/// A sandbox boundary crossed by a tool, recorded with a permission decision
/// (FR-005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryTarget {
    /// The local file system.
    FileSystem,
    /// A shell command execution.
    Shell,
    /// A network endpoint.
    Network,
    /// An external Model Context Protocol server.
    Mcp,
    /// Another named boundary not covered above.
    Other(String),
}

/// A principal that made a permission decision (FR-005).
///
/// Typically the human operator, but may be an automated policy engine in
/// autonomous modes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Principal {
    /// The human operator approved or denied the action.
    Operator,
    /// An automated policy engine decided the action.
    Policy,
}

/// The typed payload of an activity-log event.
///
/// Each variant corresponds to one category of execution fact captured by
/// the append-only log. Variants are intentionally exhaustive: future event
/// kinds add new variants and bump [`ACTIVITY_EVENT_SCHEMA_VERSION`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    /// A model message was produced or received (FR-001).
    ModelMessage {
        /// The role of the message author (`user`, `assistant`, etc.).
        role: String,
        /// The message content, as a serialised blob (text or structured).
        content: String,
        /// The provider-assigned message identifier, if any.
        message_id: Option<String>,
    },
    /// A tool was invoked (FR-004). Paired with a later
    /// [`EventKind::ToolResult`] event sharing the same `tool_call_id`.
    ToolCall {
        /// Identifier shared with the matching result event (FR-004).
        tool_call_id: String,
        /// Name of the tool invoked.
        tool: String,
        /// Raw JSON arguments for the call.
        args: String,
    },
    /// A tool call completed (FR-004). Paired with the preceding
    /// [`EventKind::ToolCall`] event sharing the same `tool_call_id`.
    ToolResult {
        /// Identifier shared with the matching invocation event (FR-004).
        tool_call_id: String,
        /// The tool that produced this result.
        tool: String,
        /// Whether the call succeeded.
        success: bool,
        /// The result content, or an error message on failure.
        content: String,
    },
    /// A permission decision was made for a boundary-crossing tool (FR-005).
    PermissionDecision {
        /// The tool the decision applies to.
        tool: String,
        /// Who made the decision.
        principal: Principal,
        /// The boundary the tool crosses.
        boundary: BoundaryTarget,
        /// Whether permission was granted.
        granted: bool,
    },
    /// A named checkpoint was recorded (FR-008).
    Checkpoint {
        /// Operator-assigned or auto-generated checkpoint name.
        name: String,
        /// Sequence number the checkpoint was taken at.
        seq: u64,
    },
    /// A run terminated (FR-003), either by completing or by being
    /// interrupted.
    Termination {
        /// Why the run terminated.
        reason: TerminationReason,
        /// The sequence number at which the run stopped.
        seq: u64,
    },
    /// A new run was branched from a checkpoint of an existing run (FR-018).
    BranchOrigin {
        /// The run the branch was taken from.
        source_run_id: RunId,
        /// The sequence number in the source run the branch was taken at.
        source_seq: u64,
    },
    /// An attempted mutation of a committed event was rejected (FR-010).
    MutationRejected {
        /// The sequence number of the event that was the target of the
        /// rejected mutation.
        target_seq: u64,
        /// Description of the attempted mutation.
        attempted: String,
    },
    /// A lifecycle event such as retention expiry (FR-016) or run completion
    /// retention (FR-015).
    Lifecycle {
        /// What lifecycle event occurred.
        event: String,
    },
}

/// Lifecycle marker appended when a run is resumed (FR-013).
///
/// The string is the shared vocabulary between the storage layer (which
/// writes it) and the projection (which transitions a run back to
/// [`RunStatus::Active`] on seeing it); keep them in sync through this
/// constant rather than repeating the literal.
pub const LIFECYCLE_RESUMED: &str = "resumed";

/// An immutable, self-describing record of a single execution fact.
///
/// Persisted to the append-only event log before being projected into any
/// derived state (FR-001). Every event carries its type, schema version, and
/// run identifier so the log remains replayable across version upgrades
/// (NFR-003).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivityEvent {
    /// Immutable identifier for this event (FR-002).
    pub id: EventId,
    /// The run this event belongs to.
    pub run_id: RunId,
    /// Monotonically increasing per-run sequence number (FR-002).
    pub seq: u64,
    /// Schema version of this event's payload (NFR-003).
    pub schema_version: u32,
    /// UTC timestamp when the event was recorded.
    pub timestamp: DateTime<Utc>,
    /// The typed payload describing what happened.
    pub kind: EventKind,
}

impl ActivityEvent {
    /// Creates a new activity event with the current schema version and a
    /// fresh [`EventId`].
    ///
    /// The caller is responsible for assigning the per-run `seq` (T-003) and
    /// ensuring it is monotonic.
    #[must_use]
    pub fn new(run_id: RunId, seq: u64, kind: EventKind) -> Self {
        Self {
            id: EventId::new(),
            run_id,
            seq,
            schema_version: ACTIVITY_EVENT_SCHEMA_VERSION,
            timestamp: Utc::now(),
            kind,
        }
    }
}

impl EventKind {
    /// JSON discriminator for [`EventKind::ToolCall`] (the `kind` tag value in
    /// the serialised form, `"tool_call"`), used by the activity store's
    /// targeted lookup (M-003).
    #[must_use]
    pub fn tool_call_discriminator() -> &'static str {
        "tool_call"
    }

    /// JSON discriminator for [`EventKind::ToolResult`] (the `kind` tag value
    /// in the serialised form, `"tool_result"`), used by the activity store's
    /// targeted lookup (M-003).
    #[must_use]
    pub fn tool_result_discriminator() -> &'static str {
        "tool_result"
    }

    /// JSON discriminator for [`EventKind::Checkpoint`] (the `kind` tag value
    /// in the serialised form, `"checkpoint"`), used by the activity store's
    /// targeted lookup (M-003).
    #[must_use]
    pub fn checkpoint_discriminator() -> &'static str {
        "checkpoint"
    }

    /// JSON discriminator for [`EventKind::Termination`] (the `kind` tag value
    /// in the serialised form, `"termination"`), used by the activity store's
    /// targeted lookup (M-003).
    #[must_use]
    pub fn termination_discriminator() -> &'static str {
        "termination"
    }

    /// JSON discriminator for the `"resumed"` lifecycle event (the `kind` tag
    /// value in the serialised form, `"lifecycle"`), used by the activity
    /// store's targeted status lookup (M-003 / FR-013).
    #[must_use]
    pub fn lifecycle_resumed_discriminator() -> &'static str {
        "lifecycle"
    }
}

/// A model-message entry in a replayed [`Projection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedMessage {
    /// The role of the message author.
    pub role: String,
    /// The message content.
    pub content: String,
    /// The provider-assigned message identifier, if any.
    pub message_id: Option<String>,
}

/// A tool-call entry in a replayed [`Projection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedToolCall {
    /// The shared tool-call identifier linking to the result.
    pub tool_call_id: String,
    /// The tool name.
    pub tool: String,
    /// The raw JSON arguments.
    pub args: String,
}

/// A tool-result entry in a replayed [`Projection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedToolResult {
    /// The shared tool-call identifier linking to the call.
    pub tool_call_id: String,
    /// Whether the call succeeded.
    pub success: bool,
    /// The result content or error message.
    pub content: String,
}

/// A permission-decision entry in a replayed [`Projection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedPermission {
    /// The tool the decision applies to.
    pub tool: String,
    /// Who made the decision.
    pub principal: Principal,
    /// The boundary the tool crosses.
    pub boundary: BoundaryTarget,
    /// Whether permission was granted.
    pub granted: bool,
}

/// A checkpoint entry in a replayed [`Projection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedCheckpoint {
    /// The checkpoint name.
    pub name: String,
    /// The sequence number the checkpoint was taken at.
    pub seq: u64,
}

/// The derived state (active context) rebuilt by replaying a run's event log
/// (FR-012, FR-013).
///
/// A [`Projection`] is a pure, in-memory snapshot of everything an agent
/// would see after replaying events up to a chosen point. It is computed by
/// [`Projection::replay`] (or [`Projection::replay_upto`] for a partial
/// replay up to a rollback/resume target). All fields are derived from the
/// append-only event log — the projection holds no independent truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    /// Model messages in replay order.
    pub messages: Vec<ProjectedMessage>,
    /// Tool invocations in replay order.
    pub tool_calls: Vec<ProjectedToolCall>,
    /// Tool results keyed by `tool_call_id`.
    pub tool_results: Vec<ProjectedToolResult>,
    /// Permission decisions in replay order.
    pub permissions: Vec<ProjectedPermission>,
    /// Named checkpoints, keyed by name (last one wins on duplicate name).
    pub checkpoints: Vec<ProjectedCheckpoint>,
    /// The termination event, if the replayed range includes one.
    pub termination: Option<(TerminationReason, u64)>,
    /// The highest sequence number applied to this projection.
    pub last_seq: u64,
    /// The derived run status.
    pub status: RunStatus,
}

impl Projection {
    /// Creates an empty projection (no events applied).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            messages: Vec::new(),
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            permissions: Vec::new(),
            checkpoints: Vec::new(),
            termination: None,
            last_seq: 0,
            status: RunStatus::Active,
        }
    }

    /// Replays the full event log into a [`Projection`] (FR-013).
    ///
    /// Events are applied in the given order. The caller is responsible for
    /// ensuring the slice is in ascending sequence-number order (e.g. from
    /// the storage layer's `ActivityLog::read_run`).
    #[must_use]
    pub fn replay(events: &[ActivityEvent]) -> Self {
        Self::replay_upto(events, u64::MAX)
    }

    /// Replays events up to and including `upto_seq` into a [`Projection`]
    /// (FR-012 rollback / FR-013 resume).
    ///
    /// Events with `seq > upto_seq` are ignored, so a rollback to a
    /// checkpoint or sequence number rebuilds the projection from the start
    /// of the run up to (and including) the target, discarding all later
    /// events for this projection.
    #[must_use]
    pub fn replay_upto(events: &[ActivityEvent], upto_seq: u64) -> Self {
        let mut proj = Self::empty();
        for event in events {
            if event.seq > upto_seq {
                break;
            }
            proj.apply(event);
        }
        proj
    }

    /// Applies a single event to this projection, advancing `last_seq` and
    /// updating derived state.
    pub fn apply(&mut self, event: &ActivityEvent) {
        self.last_seq = event.seq;
        match &event.kind {
            EventKind::ModelMessage {
                role,
                content,
                message_id,
            } => {
                self.messages.push(ProjectedMessage {
                    role: role.clone(),
                    content: content.clone(),
                    message_id: message_id.clone(),
                });
            }
            EventKind::ToolCall {
                tool_call_id,
                tool,
                args,
            } => {
                self.tool_calls.push(ProjectedToolCall {
                    tool_call_id: tool_call_id.clone(),
                    tool: tool.clone(),
                    args: args.clone(),
                });
            }
            EventKind::ToolResult {
                tool_call_id,
                success,
                content,
                ..
            } => {
                self.tool_results.push(ProjectedToolResult {
                    tool_call_id: tool_call_id.clone(),
                    success: *success,
                    content: content.clone(),
                });
            }
            EventKind::PermissionDecision {
                tool,
                principal,
                boundary,
                granted,
            } => {
                self.permissions.push(ProjectedPermission {
                    tool: tool.clone(),
                    principal: principal.clone(),
                    boundary: boundary.clone(),
                    granted: *granted,
                });
            }
            EventKind::Checkpoint { name, seq } => {
                self.checkpoints.push(ProjectedCheckpoint {
                    name: name.clone(),
                    seq: *seq,
                });
            }
            EventKind::Termination { reason, seq } => {
                self.termination = Some((*reason, *seq));
                self.status = reason.derived_status();
            }
            // FR-013: a "resumed" lifecycle event transitions an interrupted
            // run back to Active, matching [`crate::activity::LIFECYCLE_RESUMED`].
            EventKind::Lifecycle { event } if event == LIFECYCLE_RESUMED => {
                self.status = RunStatus::Active;
            }
            // Audit/meta events have no direct projection effect.
            EventKind::BranchOrigin { .. }
            | EventKind::MutationRejected { .. }
            | EventKind::Lifecycle { .. } => {}
        }
    }

    /// Returns the model messages that form the active context for the next
    /// model invocation (FR-012, FR-013).
    ///
    /// On resume (FR-013), these are the messages replayed from the event log
    /// up to the resume point. On rollback (FR-012), these are the messages up
    /// to (and including) the rollback target.
    #[must_use]
    pub fn model_messages(&self) -> &[ProjectedMessage] {
        &self.messages
    }

    /// Returns the sequence number from which execution should continue after
    /// a resume (FR-013: "continue execution from the event following the last
    /// committed sequence number").
    ///
    /// This is `last_seq + 1` — the next event to append after resuming.
    #[must_use]
    pub fn resume_from_seq(&self) -> u64 {
        self.last_seq + 1
    }

    /// Returns `true` if this projection represents an interrupted run that
    /// can be resumed (FR-013).
    ///
    /// A run is resumable when its last termination event has reason
    /// [`TerminationReason::Interrupted`] or [`TerminationReason::Aborted`]
    /// (i.e. the derived status is [`RunStatus::Interrupted`]).
    #[must_use]
    pub fn is_resumable(&self) -> bool {
        self.status == RunStatus::Interrupted
    }

    /// Returns tool calls that have no matching result event — the calls that
    /// were in-flight when the run was interrupted (FR-013).
    ///
    /// On resume, these are the tool calls whose results need to be obtained
    /// before the next model invocation can proceed.
    #[must_use]
    pub fn pending_tool_calls(&self) -> Vec<&ProjectedToolCall> {
        let completed: std::collections::HashSet<&str> = self
            .tool_results
            .iter()
            .map(|r| r.tool_call_id.as_str())
            .collect();
        self.tool_calls
            .iter()
            .filter(|c| !completed.contains(c.tool_call_id.as_str()))
            .collect()
    }

    /// Looks up a checkpoint by name, returning the sequence number it was
    /// taken at (FR-012 rollback target).
    ///
    /// If multiple checkpoints share a name, the most recent (highest seq) is
    /// returned. Returns `None` if no checkpoint with that name exists in the
    /// replayed range.
    #[must_use]
    pub fn checkpoint_seq(&self, name: &str) -> Option<u64> {
        self.checkpoints
            .iter()
            .rev()
            .find(|c| c.name == name)
            .map(|c| c.seq)
    }

    /// Returns the tool results to include in the next model prompt, optionally
    /// pruning old tool results (FR-009).
    ///
    /// When the operator enables context pruning, `keep_last` specifies how
    /// many of the most recent tool results to keep; older tool results are
    /// omitted from the returned slice. The full [`Projection`] (with all tool
    /// results) is unchanged — pruning affects only what is sent to the model,
    /// not the events in the log (which are never deleted, per FR-015).
    ///
    /// Pass `usize::MAX` (or any value >= `tool_results.len()`) to keep all
    /// results (no pruning). Pass `0` to omit all tool results.
    #[must_use]
    pub fn pruned_tool_results(&self, keep_last: usize) -> &[ProjectedToolResult] {
        let start = self.tool_results.len().saturating_sub(keep_last);
        &self.tool_results[start..]
    }
}

/// The result of a rollback operation (FR-012, FR-007).
///
/// Contains the rebuilt [`Projection`] (events up to and including the target,
/// per FR-012) and the number of events *after* the target that were ignored
/// for this projection but **preserved in the log** for audit (FR-007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackResult {
    /// The rebuilt projection (events up to and including the target).
    pub projection: Projection,
    /// The sequence number the rollback targeted.
    pub target_seq: u64,
    /// The number of events after `target_seq` that were ignored for this
    /// projection but preserved in the log for audit (FR-007).
    pub ignored_count: u64,
}

/// The result of a resume operation (FR-013).
///
/// Contains the reconstructed active-context [`Projection`] (replayed from the
/// event log) and the sequence number from which execution should continue
/// (the event following the resume marker).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeResult {
    /// The reconstructed active context (replayed from the event log, FR-013).
    pub projection: Projection,
    /// The sequence number from which execution continues after resume
    /// (FR-013: "the event following the last committed sequence number").
    pub resume_from_seq: u64,
}

/// An inconsistency found in a run's event log during resume validation
/// (FR-011).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConsistencyError {
    /// A gap in the sequence numbers (e.g. 0, 1, 3 — missing 2).
    #[error("sequence gap: expected {expected}, found {found}")]
    SeqGap {
        /// The expected sequence number.
        expected: u64,
        /// The sequence number that was found instead.
        found: u64,
    },
    /// A tool result event has no matching tool call event (FR-011 example).
    #[error("orphaned tool result: tool_call_id {tool_call_id} has no matching tool call")]
    OrphanedToolResult {
        /// The tool-call identifier with no matching invocation.
        tool_call_id: String,
    },
}

/// Validates an event log for consistency (FR-011).
///
/// Checks that:
/// - Sequence numbers are contiguous (0, 1, 2, ... with no gaps).
/// - Every [`EventKind::ToolResult`] has a preceding
///   [`EventKind::ToolCall`] with the same `tool_call_id`.
///
/// Returns `Ok(())` if the log is consistent, or the first
/// [`ConsistencyError`] found.
pub fn validate_event_log_consistency(events: &[ActivityEvent]) -> Result<(), ConsistencyError> {
    let mut seen_tool_calls: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (i, event) in events.iter().enumerate() {
        // Check for sequence gaps.
        if event.seq != i as u64 {
            return Err(ConsistencyError::SeqGap {
                expected: i as u64,
                found: event.seq,
            });
        }
        // Track tool calls and check for orphaned results.
        match &event.kind {
            EventKind::ToolCall { tool_call_id, .. } => {
                seen_tool_calls.insert(tool_call_id.as_str());
            }
            EventKind::ToolResult { tool_call_id, .. }
                if !seen_tool_calls.contains(tool_call_id.as_str()) =>
            {
                return Err(ConsistencyError::OrphanedToolResult {
                    tool_call_id: tool_call_id.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

impl Default for Projection {
    fn default() -> Self {
        Self::empty()
    }
}
