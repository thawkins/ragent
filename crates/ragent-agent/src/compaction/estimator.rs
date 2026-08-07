//! Fast local token estimator and compaction trigger (FR-002, FR-003).
//!
//! This module is a Rust port of the estimation and trigger logic in
//! `~/Projects/opencode/packages/core/src/session/compaction.ts` and
//! `~/Projects/opencode/packages/core/src/util/token.ts`.
//!
//! # Estimator
//!
//! OpenCode estimates request token load with a single heuristic:
//!
//! ```ts
//! const CHARS_PER_TOKEN = 4
//! export const estimate = (input: string) => Math.max(0, Math.round(input.length / CHARS_PER_TOKEN))
//! ```
//!
//! applied to `JSON.stringify({ system, messages, tools })`. The Rust port
//! mirrors that: [`estimate_text_tokens`] divides character length by 4, and
//! [`estimate_request_tokens`] sums the serialised byte size of the system
//! prompt, every chat message, and every tool definition, then divides by 4.
//! The byte-summation avoids allocating one giant JSON string on every step
//! while staying within a few percent of the `JSON.stringify` result.
//!
//! # Trigger (FR-003)
//!
//! Compaction fires when the *effective* request token count exceeds a
//! threshold. Two trigger models are supported:
//!
//! * **Percentage (recommended)** — when `CompactionConfig::threshold` is set
//!   (e.g. `0.8` for 80%), the threshold is `context_window * threshold`.
//! * **Buffer** — when `threshold` is `None`, the threshold is
//!   `context_window - max(output_tokens, context_window * buffer_fraction)`.
//!
//! The effective count prefers the provider-reported `input_tokens` from the
//! previous turn when available (FR-002), falling back to the local estimate on
//! the first call in a turn or whenever the provider omits usage data.
//!
//! When the trigger fires the runner is expected to emit a compaction-started
//! event (see [`publish_compaction_started`]) and invoke the summarisation
//! pipeline. The actual summarisation runner is implemented in a later task;
//! this module provides only the estimator, the trigger decision, and the
//! event-emission helper.

use ragent_types::event::{Event, EventBus};
use ragent_types::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, ToolDefinition};

use ragent_config::compaction::CompactionConfig;

/// Characters per token for the estimation fallback.
///
/// Matches OpenCode's `Token.CHARS_PER_TOKEN`.
pub const CHARS_PER_TOKEN: usize = 4;

/// Per-message token overhead (approximation for role + JSON wrapper metadata).
///
/// OpenCode folds this into the `JSON.stringify` envelope; we add it explicitly
/// because [`estimate_request_tokens`] sums component byte lengths rather than
/// materialising the full JSON string.
pub const MESSAGE_OVERHEAD_TOKENS: usize = 10;

/// Minimum fraction of the context window that must be filled before automatic
/// pre-send compaction is allowed to fire.
///
/// This floor prevents compaction from running on every small prompt when the
/// configured buffer or a user-supplied percentage threshold would otherwise
/// trigger at a very low usage level. Emergency overflow compaction is not
/// subject to this floor.
pub const MIN_COMPACTION_THRESHOLD_FRACTION: f64 = 0.7;

/// Rough token cost attributed to an image content part for vision models.
pub const IMAGE_TOKEN_ESTIMATE: usize = 1_000;

/// Estimate the token count of an arbitrary piece of text.
///
/// `round(text.len() / CHARS_PER_TOKEN)`, clamped at zero. This is the Rust
/// equivalent of OpenCode's `Token.estimate`.
#[must_use]
pub fn estimate_text_tokens(text: &str) -> usize {
    let chars = text.len();
    if chars == 0 {
        return 0;
    }
    (chars + CHARS_PER_TOKEN / 2) / CHARS_PER_TOKEN
}

/// Estimate the token count of a single [`ChatMessage`].
///
/// Sums the byte length of the role string and every content part (text, tool
/// use input, tool result content, image URL), divides by [`CHARS_PER_TOKEN`],
/// and adds [`MESSAGE_OVERHEAD_TOKENS`] for the JSON envelope.
#[must_use]
pub fn estimate_message_tokens(message: &ChatMessage) -> usize {
    let mut bytes = message.role.len();
    match &message.content {
        ChatContent::Text(text) => bytes += text.len(),
        ChatContent::Parts(parts) => {
            for part in parts {
                match part {
                    ContentPart::Text { text } => bytes += text.len(),
                    ContentPart::ToolUse { id, name, input } => {
                        bytes += id.len() + name.len() + input.to_string().len();
                    }
                    ContentPart::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => bytes += tool_use_id.len() + content.len(),
                    ContentPart::ImageUrl { url } => bytes += url.len(),
                }
            }
        }
    }
    estimate_text_tokens_from_bytes(bytes) + MESSAGE_OVERHEAD_TOKENS
}

/// Estimate the token cost of a slice of tool definitions.
///
/// Each definition contributes its name, description, and serialised JSON
/// schema (`parameters`). This mirrors the tool-size term in
/// [`crate::session::history::estimate_request_bytes`].
#[must_use]
pub fn estimate_tool_tokens(tools: &[ToolDefinition]) -> usize {
    let bytes: usize = tools
        .iter()
        .map(|t| t.name.len() + t.description.len() + t.parameters.to_string().len() + 60)
        .sum();
    estimate_text_tokens_from_bytes(bytes)
}

/// Estimate the total request token load for an LLM call.
///
/// Adds the system prompt, every chat message, and every tool definition.
/// This is the Rust equivalent of OpenCode's
/// `Token.estimate(JSON.stringify({ system, messages, tools }))`.
///
/// # Arguments
///
/// * `system` — optional system prompt text.
/// * `messages` — provider-facing chat history.
/// * `tools` — tool definitions included in the request.
#[must_use]
pub fn estimate_request_tokens(
    system: Option<&str>,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> usize {
    let mut total = 0usize;
    if let Some(sys) = system {
        total += estimate_text_tokens(sys);
    }
    for message in messages {
        total += estimate_message_tokens(message);
    }
    total += estimate_tool_tokens(tools);
    total
}

/// Estimate the request token load directly from a [`ChatRequest`].
#[must_use]
pub fn estimate_chat_request_tokens(request: &ChatRequest) -> usize {
    estimate_request_tokens(request.system.as_deref(), &request.messages, &request.tools)
}

/// Convert a raw byte count into a token estimate using [`CHARS_PER_TOKEN`].
fn estimate_text_tokens_from_bytes(bytes: usize) -> usize {
    if bytes == 0 {
        return 0;
    }
    (bytes + CHARS_PER_TOKEN / 2) / CHARS_PER_TOKEN
}

/// Outcome of a compaction-trigger evaluation.
///
/// Returned by [`evaluate_trigger`]; carries both the decision and the
/// intermediate values so callers (and tests) can inspect why compaction did or
/// did not fire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerDecision {
    /// `true` when compaction should run this turn.
    pub should_compact: bool,
    /// Local estimate of the request token load.
    pub estimated_tokens: usize,
    /// Token count actually used for the decision: the provider-reported
    /// `input_tokens` when available, otherwise [`Self::estimated_tokens`].
    pub effective_tokens: usize,
    /// Compaction threshold: `context_window * threshold` when the user
    /// configured a percentage, otherwise `context_window -
    /// max(output_tokens, buffer)`.
    pub threshold: usize,
}

/// Resolve the effective token count for compaction decisions (FR-002).
///
/// When the provider reported a non-zero `input_tokens` for the previous turn,
/// that value is preferred because it matches the provider's tokenizer exactly.
/// Otherwise the local [`estimate_request_tokens`] estimate is used.
#[must_use]
pub fn effective_request_tokens(estimated_tokens: usize, last_reported_input_tokens: u64) -> usize {
    if last_reported_input_tokens > 0 {
        last_reported_input_tokens as usize
    } else {
        estimated_tokens
    }
}

/// Compute the compaction threshold (FR-003) with a 70 % usage floor.
///
/// When `config.threshold` is set (a fraction such as `0.8` for 80%), the
/// threshold is `context_window * threshold` — the user-configured trigger
/// point. Otherwise it falls back to the buffer-based model
/// `context_window - max(output_tokens, context_window * buffer_fraction)`,
/// saturating at zero so a tiny context window never produces an underflow.
///
/// In either case the result is raised to at least
/// [`MIN_COMPACTION_THRESHOLD_FRACTION`] of the context window (70 %). This
/// ensures that automatic pre-send compaction only runs once the conversation
/// has reached 70 % of the model's context window, preventing premature
/// summarisation after a routine prompt. Emergency overflow compaction bypasses
/// this function, so it is not affected by the floor.
#[must_use]
pub fn compaction_threshold(
    context_window: usize,
    output_tokens: usize,
    buffer_fraction: f64,
    threshold: Option<f64>,
) -> usize {
    let candidate = match threshold {
        Some(frac) if (0.0..=1.0).contains(&frac) => ((context_window as f64) * frac) as usize,
        _ => {
            let buffer_tokens =
                ((context_window as f64) * buffer_fraction.clamp(0.0, 1.0)) as usize;
            context_window.saturating_sub(output_tokens.max(buffer_tokens))
        }
    };
    let floor = ((context_window as f64) * MIN_COMPACTION_THRESHOLD_FRACTION) as usize;
    candidate.max(floor).min(context_window)
}

/// Evaluate whether compaction should fire for the upcoming LLM request.
///
/// Combines the local estimate, the provider-reported token count, and the
/// [`CompactionConfig`] buffer into a [`TriggerDecision`]. Returns
/// `should_compact = true` when the effective token count exceeds the
/// threshold.
///
/// # Arguments
///
/// * `config` — compaction configuration (supplies `buffer_fraction`).
/// * `estimated_tokens` — local [`estimate_request_tokens`] result.
/// * `last_reported_input_tokens` — provider-reported `input_tokens` from the
///   previous turn, or `0` if unavailable.
/// * `context_window` — the model's context window in tokens.
/// * `output_tokens` — max output tokens for the request (the
///   `max_tokens` / `output` limit).
#[must_use]
pub fn evaluate_trigger(
    config: &CompactionConfig,
    estimated_tokens: usize,
    last_reported_input_tokens: u64,
    context_window: usize,
    output_tokens: usize,
) -> TriggerDecision {
    let effective = effective_request_tokens(estimated_tokens, last_reported_input_tokens);
    let threshold = compaction_threshold(
        context_window,
        output_tokens,
        config.buffer,
        config.threshold,
    );
    TriggerDecision {
        should_compact: effective > threshold,
        estimated_tokens,
        effective_tokens: effective,
        threshold,
    }
}

/// Publish the compaction-started event for a session (FR-003).
///
/// Emits [`Event::CompressionStarted`] — the existing compaction-lifecycle
/// carrier event — with the supplied reason (`"auto"` for pre-send triggers,
/// `"overflow"` for emergency triggers). The summarisation pipeline itself is
/// invoked by the compaction runner (later task); this helper only signals that
/// compaction has begun so the TUI, SSE stream, and telemetry can react.
pub fn publish_compaction_started(event_bus: &EventBus, session_id: &str, reason: &str) {
    event_bus.publish(Event::CompressionStarted {
        session_id: session_id.to_string(),
        reason: reason.to_string(),
    });
}

#[cfg(test)]
#[path = "../../tests/inline/test_compaction_estimator.rs"]
mod tests;
