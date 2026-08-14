//! Trigger runtime — deduplication and cycle suppression for trigger events.
//!
//! The [`TriggerRuntime`] is the central component that receives trigger
//! envelopes from all sources (dynamic rules, MCP notification hooks) and
//! applies two protective mechanisms before dispatching:
//!
//! 1. **Deduplication** — if an envelope with the same `dedup_hash` has
//!    already been processed within the dedup window, the new envelope is
//!    silently dropped. This prevents repeated notifications from the same
//!    source with identical content from spamming the chat.
//!
//! 2. **Cycle suppression** — if a source fires the same action repeatedly
//!    (detected via `source_id` + `dedup_hash`), the runtime suppresses
//!    further firings after `max_cycles` consecutive duplicates. This
//!    prevents infinite loops where a trigger's output re-triggers itself.
//!
//! See `specs/piegap/SPEC.md` FR-002 and FR-003 for the full specification.

pub mod dynamic;
pub mod mcp_notification;
pub mod runtime;

pub use dynamic::{
    ActionDispatcher, ConditionEvaluator, DynamicTriggerEngine, DynamicTriggerError,
    NoopActionDispatcher, ParsedTriggerRequest, SimpleConditionEvaluator,
};
pub use mcp_notification::{
    McpNotification, McpNotificationAdapter, McpNotificationError, NotificationInjector,
    RecordingNotificationInjector,
};
pub use runtime::{TriggerRuntime, TriggerRuntimeConfig};
