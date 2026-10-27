//! The one shape a tool call's terminal outcome takes (spec `uniformtools`
//! FR-012, FR-014, FR-021).
//!
//! Every terminal path of the tool phase - a call that ran, a call a guard
//! denied, a call refused before dispatch, a call whose task panicked or was
//! aborted by the watchdog - produces the same nine-field tuple, and the loop
//! turns that tuple into the recorded `ToolCallState`, the LLM-visible
//! `ContentPart::ToolResult` and the batch event. Three of those paths never
//! reach a tool at all, and before this module each one assembled its own tuple
//! inline: the pre-dispatch gate hand-built one around a `ToolInputError`
//! sentence, the loop-restriction / hook / analyzer denials shared a
//! `denied_tool_call` helper that lived at the bottom of a 4,600-line processor
//! module, and the panic and watchdog wrappers each built a third. The three
//! disagreed where it matters:
//!
//! * a refusal published `Error: <sentence>` while every other path published
//!   the bare sentence, so the text the model read depended on which stage
//!   stopped the call and not on what the call did;
//! * the panic and watchdog paths published `ToolCallEnd` themselves in some
//!   places and not in others, which left a TUI spinner spinning on an aborted
//!   call;
//! * the duration field was `0` in one copy and the real elapsed time in
//!   another, so a timed-out call reported the same latency as one refused
//!   before it started.
//!
//! The three constructors here are the whole of that policy, next to the one
//! refusal type they render ([`ToolInputError`]). They publish the
//! `ToolCallEnd` that closes the call out, because a terminal outcome and the
//! event that announces it are one fact and not two.
//!
//! The same three constructors, and the stage *ordering*, now live in
//! [`crate::session::dispatch::ToolDispatchPipeline`], which the loop calls for
//! every call whether the step runs it sequentially or concurrently (T-014,
//! FR-012). This module keeps the third constructor - the one a task that
//! panicked or failed to join needs, which the loop still wraps around its
//! `tokio::spawn` handle - beside the refusal type it renders.

use std::sync::Arc;

use serde_json::Value;

use ragent_types::message::ToolCallStatus;

use crate::event::Event;
use crate::session::history::PendingToolCall;
use crate::session::tool_input_error::ToolInputError;

/// The outcome one terminal tool call produces, consumed by
/// [`SessionProcessor`](crate::session::processor::SessionProcessor)'s tool
/// phase to append history, publish the batch event and record the activity.
///
/// The fields are exactly the ones the loop needs, so a refusal, a denial, a
/// synthesised failure and a completed call are indistinguishable in shape -
/// which is why a stage cannot "return early" in a different format from any
/// other.
///
/// Field order: the call; the decoded input the call ran with (`Value::Null`
/// for a refusal); the record status whose
/// [`is_error`](ToolCallStatus::is_error) is the result flag (FR-014); the
/// tool's metadata (present on success); the failure text (present on a refusal
/// or a failed run); the wall-clock duration; the content the model sees; the
/// metadata attached to the LLM-visible result; and the `PostToolUse` hook
/// `additionalContext` blocks to fold in.
pub(crate) type ToolOutcome = (
    PendingToolCall,
    Value,
    ToolCallStatus,
    Option<Value>,
    Option<String>,
    u64,
    String,
    Option<Value>,
    Vec<String>,
);

/// Builds the outcome for a call refused *before* a guard ran: an unregistered
/// name, an undecodable payload, or a schema violation (FR-008, FR-009,
/// FR-021).
///
/// The sentence, the status and the `is_error` flag all come from the
/// [`ToolInputError`] value, so this adapter states the content of the refusal
/// nowhere - it cannot drift from the constructor. The model-facing text is
/// `Error: <sentence>`, the same prefix every other failure uses: a refusal
/// used to reach the model as a bare statement, so a client keyed on the marker
/// saw the same status spelled two ways.
#[allow(dead_code)] // superseded by `ToolDispatchPipeline::refusal`; the pipeline owns ordering
pub(crate) fn refusal(
    tc: &PendingToolCall,
    refusal: &ToolInputError,
    input: Value,
    event_bus: &Arc<crate::event::EventBus>,
    session_id: &str,
) -> ToolOutcome {
    let message = refusal.to_string();
    event_bus.publish(Event::ToolCallEnd {
        session_id: session_id.to_string(),
        call_id: tc.id.clone(),
        tool: tc.name.clone(),
        error: Some(message.clone()),
        duration_ms: 0,
    });
    (
        tc.clone(),
        input,
        refusal.status(),
        None,
        Some(message.clone()),
        0u64,
        format!("Error: {message}"),
        None,
        Vec::new(),
    )
}

/// Builds the outcome for a call a guard denied, and closes the call out on the
/// UI first.
///
/// Every denial in the pipeline goes through here, so a denial always publishes
/// `ToolCallEnd` (leaving no spinner spinning) and always returns the same shape
/// with [`ToolCallStatus::Error`] - whose
/// [`is_error`](ToolCallStatus::is_error) is the result flag (FR-014).
#[allow(dead_code)] // superseded by `ToolDispatchPipeline::denial`; the pipeline owns ordering
pub(crate) fn denial(
    tc: &PendingToolCall,
    reason: &str,
    input: Value,
    event_bus: &Arc<crate::event::EventBus>,
    session_id: &str,
) -> ToolOutcome {
    event_bus.publish(Event::ToolCallEnd {
        session_id: session_id.to_string(),
        call_id: tc.id.clone(),
        tool: tc.name.clone(),
        error: Some(reason.to_string()),
        duration_ms: 0,
    });
    (
        tc.clone(),
        input,
        ToolCallStatus::Error,
        None,
        Some(reason.to_string()),
        0u64,
        String::new(),
        None,
        Vec::new(),
    )
}

/// Builds the outcome for a call whose task never produced a result: it
/// panicked, failed to join, or was aborted by the watchdog.
///
/// A join failure is not a tool result, but the `tool_use` record still has to
/// be answered - an orphaned record leaves the conversation with a call whose
/// outcome the model never sees. This is the one place that answers it, so the
/// sequential and the concurrent shape cannot disagree about the text, the
/// status, the flag or the duration.
///
/// The pipeline carries its own copy ([`ToolDispatchPipeline::synthesised`])
/// so that both execution arms reach the same constructor; until that copy is
/// the only one, this one keeps the shape pinned for a direct caller.
///
/// [`ToolDispatchPipeline::synthesised`]: crate::session::dispatch::ToolDispatchPipeline::synthesised
#[allow(dead_code)] // superseded by `ToolDispatchPipeline::synthesised`; the pipeline owns ordering
pub(crate) fn synthesised(
    tc: &PendingToolCall,
    message: &str,
    duration_ms: u64,
    event_bus: &Arc<crate::event::EventBus>,
    session_id: &str,
) -> ToolOutcome {
    event_bus.publish(Event::ToolCallEnd {
        session_id: session_id.to_string(),
        call_id: tc.id.clone(),
        tool: tc.name.clone(),
        error: Some(message.to_string()),
        duration_ms,
    });
    (
        tc.clone(),
        Value::Null,
        ToolCallStatus::Error,
        None,
        Some(message.to_string()),
        duration_ms,
        format!("Error: {message}"),
        None,
        Vec::new(),
    )
}
