//! OpenCode-derived context-window compaction.
//!
//! This module contains the summarisation-based compaction runner.
//! It currently exposes:
//!
//! - [`estimator`] — fast local token estimator and compaction trigger
//!   (FR-002, FR-003).
//! - [`prompt`] — the summarisation prompt builder.
//! - [`serializer`] — the conversation serialiser that flattens history into
//!   the transcript fed to the summarisation prompt.
//! - [`runner`] — the compaction runner: selects recent turns, calls the LLM
//!   for a summary, and produces the replacement message list (FR-005,
//!   FR-007).
//! - [`convert`] — bidirectional `ChatMessage` ↔ `Message` conversion used by
//!   the runner and the agent loop's pre-send / emergency-overflow paths.

pub mod convert;
pub mod estimator;
pub mod prompt;
pub mod runner;
pub mod serializer;

pub use estimator::{
    CHARS_PER_TOKEN, IMAGE_TOKEN_ESTIMATE, MESSAGE_OVERHEAD_TOKENS, TriggerDecision,
    compaction_threshold, effective_request_tokens, estimate_chat_request_tokens,
    estimate_message_tokens, estimate_request_tokens, estimate_text_tokens, estimate_tool_tokens,
    evaluate_trigger, publish_compaction_started,
};
pub use prompt::{SUMMARY_OUTPUT_TOKENS, build_prompt};
pub use runner::{
    CompactionOutcome, SelectedSplit, build_compaction_message, build_summary_request, compact,
    emergency_compact, select, summarize_via_client,
};
pub use serializer::{TOOL_OUTPUT_MAX_CHARS, serialize_message, serialize_messages, truncate};
