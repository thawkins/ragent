//! Compaction runner and message replacement (FR-005, FR-007).
//!
//! This module is the Rust port of OpenCode's `SessionCompaction.compactAfterOverflow`
//! in `~/Projects/opencode/packages/core/src/session/compaction.ts`. It ties together
//! the pieces built by the earlier compaction tasks:
//!
//! - [`super::serializer`] — flattens history into a transcript.
//! - [`super::prompt`] — builds the Markdown summarisation prompt.
//! - [`super::estimator`] — fast token estimate + trigger event helper.
//!
//! # Flow
//!
//! 1. [`select`] picks a verbatim "recent" tail within the configured
//!    `keep_tokens` budget; everything before it is the "head" to summarise.
//! 2. The head transcript (plus any previous compaction summary) is fed to
//!    [`super::prompt::build_prompt`].
//! 3. [`summarize_via_client`] sends the prompt to the LLM through a standard
//!    [`LlmClient`] and collects the streamed text into the summary.
//! 4. On success, [`compact`] builds a synthetic [`Role::Compaction`] message
//!    holding the summary and returns the new message list
//!    `[compaction_msg, ...recent]` (FR-005). The session resumes from that
//!    compaction point, so the next turn loads the summary plus the verbatim
//!    recent turns and any later system updates (FR-007).
//!
//! Integration into the agent loop's pre-send path is performed by T-008;
//! emergency overflow invocation + single retry is T-009. This module only
//! provides the runner itself.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, bail};
use futures::StreamExt;
use tokio::time::timeout;
use tracing::{info, warn};

use ragent_config::StreamConfig;
use ragent_config::compaction::CompactionConfig;
use ragent_types::event::{Event, EventBus};
use ragent_types::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent};
use ragent_types::message::{Message, MessagePart, Role};

use crate::compaction::{
    SUMMARY_OUTPUT_TOKENS, build_prompt, estimate_text_tokens, publish_compaction_started,
    serialize_message,
};

/// Maximum characters for the compaction summarisation prompt.
///
/// The prompt consists of the instruction/template plus the serialised head
/// transcript. Capping it prevents local/small models from being swamped with
/// a huge summarisation request that can stall for minutes. The value leaves
/// room for the template (~2.5 k chars), the previous summary (~16 k chars), and
/// a large head transcript.
const MAX_COMPACTION_PROMPT_CHARS: usize = 120_000;

/// Heartbeat interval for long-running compaction summarisation.
const SUMMARY_HEARTBEAT_SECS: u64 = 30;

/// The verbatim-tail selection produced by [`select`].
///
/// `head_messages` are the messages that will be summarised (their serialised
/// transcript feeds the LLM prompt). `recent_messages` are kept verbatim and
/// appended after the synthetic compaction message in the new history.
#[derive(Debug, Clone)]
pub struct SelectedSplit {
    /// Messages to summarise (older prefix), in original order.
    pub head_messages: Vec<Message>,
    /// Messages to keep verbatim (recent tail), in original order.
    pub recent_messages: Vec<Message>,
    /// Estimated token cost of the serialised recent tail.
    pub recent_tokens: usize,
}

/// Result of a successful [`compact`] run.
#[derive(Debug, Clone)]
pub struct CompactionOutcome {
    /// The LLM-produced summary text.
    pub summary: String,
    /// The new message list: `[compaction_msg, ...recent_messages]`.
    pub new_messages: Vec<Message>,
    /// The synthetic compaction message (first element of `new_messages`).
    pub compaction_message: Message,
    /// Number of input messages before compaction.
    pub original_message_count: usize,
    /// Number of messages kept verbatim after compaction (excluding the
    /// compaction message itself).
    pub kept_message_count: usize,
    /// Estimated token cost of the original history.
    pub original_tokens: usize,
    /// Estimated token cost of the new history.
    pub compressed_tokens: usize,
}

/// Select the verbatim recent tail to preserve after compaction.
///
/// Walks the non-compaction messages in reverse, accumulating the estimated
/// token cost of each serialised message until the `keep_tokens` budget is
/// exhausted. At least the last message is always kept verbatim so the active
/// turn is never dropped. Compaction messages in the input are dropped (their
/// summary is passed separately via `previous_summary` to [`compact`]).
///
/// # Arguments
///
/// * `messages` — full conversation history in ragent internal format.
/// * `config` — compaction configuration (supplies `keep_tokens` and
///   `tool_output_max_chars`).
#[must_use]
pub fn select(messages: &[Message], config: &CompactionConfig) -> SelectedSplit {
    let tool_max = config.tool_output_max_chars();
    let keep_tokens = config.keep_tokens();

    // (original_index, serialised_text, token_cost) for every non-compaction
    // message with non-empty serialised content.
    let conv: Vec<(usize, String, usize)> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role != Role::Compaction)
        .map(|(i, m)| {
            let serialized = serialize_message(m, tool_max);
            let cost = estimate_text_tokens(&serialized);
            (i, serialized, cost)
        })
        .filter(|(_, s, _)| !s.is_empty())
        .collect();

    if conv.is_empty() {
        return SelectedSplit {
            head_messages: Vec::new(),
            recent_messages: Vec::new(),
            recent_tokens: 0,
        };
    }

    // Always keep at least the last message verbatim.
    let mut total = conv.last().expect("non-empty conv").2;
    let mut split_idx = conv.len() - 1;
    for (idx, &(_, _, cost)) in conv.iter().enumerate().rev().skip(1) {
        if total + cost > keep_tokens {
            break;
        }
        total += cost;
        split_idx = idx;
    }

    let recent_messages: Vec<Message> = conv[split_idx..]
        .iter()
        .map(|(i, _, _)| messages[*i].clone())
        .collect();
    let head_messages: Vec<Message> = conv[..split_idx]
        .iter()
        .map(|(i, _, _)| messages[*i].clone())
        .collect();

    SelectedSplit {
        head_messages,
        recent_messages,
        recent_tokens: total,
    }
}

/// Drive an LLM streaming request to completion, collecting every
/// [`StreamEvent::TextDelta`] into a single summary string.
///
/// Returns `Err` if the provider emits an [`StreamEvent::Error`] or the stream
/// ends abnormally. A successful stream that produces no text yields an empty
/// string (the caller treats an empty summary as failure).
///
/// # Arguments
///
/// * `client` — the LLM client to call.
/// * `request` — the summarisation request (a single user message).
pub async fn summarize_via_client(
    client: &Arc<dyn crate::llm::LlmClient>,
    request: ChatRequest,
    stream_config: &StreamConfig,
    event_bus: &EventBus,
    session_id: &str,
) -> Result<String> {
    // Bound the total wall time for a summarisation call. Local models in
    // particular can stall on huge prompts; this prevents the UI from freezing
    // indefinitely. Defaults: 300s initial + 120s stall budget -> 420s cap.
    let overall_timeout_secs =
        (stream_config.initial_response_timeout_secs + stream_config.timeout_secs).min(300);
    let overall_timeout = std::time::Duration::from_secs(overall_timeout_secs);

    let summary_fut = async {
        let started = Instant::now();
        let mut next_heartbeat = SUMMARY_HEARTBEAT_SECS;
        let mut stream = client.chat(request).await?;
        let mut chunks: Vec<String> = Vec::new();
        while let Some(event) = stream.next().await {
            let elapsed_secs = started.elapsed().as_secs();
            if elapsed_secs >= next_heartbeat {
                info!(
                    session_id,
                    elapsed_secs, "compaction summarisation still in progress"
                );
                event_bus.publish(Event::AgentNotice {
                    session_id: session_id.to_string(),
                    message: format!("Context compression still running after {elapsed_secs}s..."),
                });
                next_heartbeat = elapsed_secs + SUMMARY_HEARTBEAT_SECS;
            }
            match event {
                StreamEvent::TextDelta { text } => chunks.push(text),
                StreamEvent::Error { message } => {
                    bail!("compaction summarisation failed: {message}")
                }
                StreamEvent::Finish { .. } => break,
                // Tool calls, reasoning deltas, usage, and rate-limit events are
                // not expected from a no-tools summarisation request and are
                // ignored.
                _ => {}
            }
        }
        Ok(chunks.join(""))
    };

    match timeout(overall_timeout, summary_fut).await {
        Ok(result) => result,
        Err(_) => bail!(
            "compaction summarisation timed out after {overall_timeout_secs}s; \
             the model may be overloaded or the compaction prompt is too large"
        ),
    }
}

/// Build the summarisation [`ChatRequest`] for a given prompt.
///
/// The request carries a single user message (the summary prompt), no tools,
/// and a `max_tokens` cap of `summary_output`.
#[must_use]
pub fn build_summary_request(
    model: &str,
    prompt: &str,
    summary_output: u32,
    stream_timeout_secs: Option<u64>,
) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(prompt.to_string()),
        }]),
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: Some(summary_output),
        system: None,
        options: std::collections::HashMap::new(),
        thinking: None,
        session_id: None,
        request_id: None,
        stream_timeout_secs,
    }
}

/// Build the synthetic [`Role::Compaction`] message holding the summary.
///
/// The message carries the summary as a single [`MessagePart::Text`]. The
/// verbatim recent turns are kept as separate messages in the new history
/// (see [`compact`]), so the compaction message itself only needs the summary
/// text — this matches how the history loader (FR-007) reconstructs context:
/// the compaction summary plus every message from the compaction point onward.
#[must_use]
pub fn build_compaction_message(session_id: &str, summary: &str) -> Message {
    Message::new(
        session_id,
        Role::Compaction,
        vec![MessagePart::Text {
            text: summary.to_string(),
        }],
    )
}

/// Run a single compaction pass and return the replacement message list.
///
/// This is the Rust equivalent of OpenCode's `compactAfterOverflow`. It:
///
/// 1. Selects the verbatim recent tail via [`select`].
/// 2. Bails out when there is nothing to summarise (empty head and no previous
///    compaction summary to update) — mirroring OpenCode's guard.
/// 3. Bails out when the summary prompt itself would overflow the context
///    window minus the summary output budget.
/// 4. Emits a compaction-started event, calls the LLM, and collects the summary.
/// 5. On a non-empty summary, builds the synthetic compaction message and
///    returns `[compaction_msg, ...recent]`.
/// 6. Emits a compaction-finished event with the before/after token estimates.
///
/// # Arguments
///
/// * `session_id` — the session being compacted.
/// * `messages` — full conversation history (consumed).
/// * `model` — the model id to use for the summarisation call.
/// * `context_window` — the model's context window in tokens (used for the
///   overflow guard).
/// * `output_tokens` — the request's max output tokens; the summary output cap
///   is `min(output_tokens, SUMMARY_OUTPUT_TOKENS)`.
/// * `config` — compaction configuration.
/// * `previous_summary` — an existing compaction summary to update, if any
///   (FR-010).
/// * `client` — the LLM client used for the summarisation call.
/// * `event_bus` — event bus for compaction-lifecycle events.
/// * `reason` — compaction reason (`"auto"` for pre-send triggers, `"overflow"`
///   for emergency triggers).
///
/// # Errors
///
/// Returns `Err` when there is nothing to summarise, the summary prompt would
/// overflow, the LLM call fails, or the summary comes back empty.
pub async fn compact(
    session_id: &str,
    messages: Vec<Message>,
    model: &str,
    context_window: usize,
    output_tokens: usize,
    config: &CompactionConfig,
    previous_summary: Option<&str>,
    client: &Arc<dyn crate::llm::LlmClient>,
    event_bus: &EventBus,
    reason: &str,
    stream_config: &StreamConfig,
) -> Result<CompactionOutcome> {
    let original_message_count = messages.len();
    let tool_max = config.tool_output_max_chars();

    // 1. Select verbatim recent tail + head to summarise.
    let split = select(&messages, config);

    // 2. Nothing-to-summarise guard (OpenCode:
    //    `if (!selected || (selected.head.length === 0 && previousSummary?.
    //    type !== "compaction")) return false`).
    if split.head_messages.is_empty() && previous_summary.is_none() {
        bail!("compaction has nothing to summarise: empty head and no previous summary");
    }

    // 3. Build the summarisation prompt. Context = [previous recent (if any),
    //    head transcript].
    let head_transcript = split
        .head_messages
        .iter()
        .map(|m| serialize_message(m, tool_max))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    // Cap the prompt length so the summarisation request stays tractable for
    // the configured model. If the head transcript is too long, keep the most
    // recent portion and note the truncation.
    let head_transcript = cap_head_transcript(&head_transcript);

    let prompt = build_prompt(previous_summary, &[head_transcript.as_str()]);

    // 4. Overflow guard: bail if the prompt alone would not leave room for the
    //    summary output. The summary output cap mirrors OpenCode's
    //    `Math.min(output || SUMMARY_OUTPUT_TOKENS, SUMMARY_OUTPUT_TOKENS)`.
    let summary_output_cap = if output_tokens == 0 {
        SUMMARY_OUTPUT_TOKENS
    } else {
        output_tokens.min(SUMMARY_OUTPUT_TOKENS)
    };
    let prompt_tokens = estimate_text_tokens(&prompt);
    let prompt_budget = context_window.saturating_sub(summary_output_cap);
    if context_window > 0 && prompt_tokens > prompt_budget {
        warn!(
            prompt_tokens,
            context_window,
            summary_output_cap,
            "compaction summary prompt would overflow context window"
        );
        bail!(
            "compaction summary prompt ({prompt_tokens} tokens) exceeds context budget ({prompt_budget})"
        );
    }

    // 5. Emit compaction-started and call the LLM.
    publish_compaction_started(event_bus, session_id, reason);
    let request = build_summary_request(
        model,
        &prompt,
        summary_output_cap as u32,
        Some(stream_config.initial_response_timeout_secs),
    );
    let summary =
        summarize_via_client(client, request, stream_config, event_bus, session_id).await?;
    let summary = summary.trim();
    if summary.is_empty() {
        event_bus.publish(Event::CompressionFinished {
            session_id: session_id.to_string(),
            original_tokens: 0,
            compressed_tokens: 0,
            compression_ratio: 1.0,
            did_compress: false,
            reason: reason.to_string(),
        });
        bail!("compaction summarisation produced an empty summary");
    }

    // 6. Build the replacement message list: [compaction_msg, ...recent].
    let compaction_message = build_compaction_message(session_id, summary);
    let kept_message_count = split.recent_messages.len();
    let mut new_messages = Vec::with_capacity(1 + kept_message_count);
    new_messages.push(compaction_message.clone());
    new_messages.extend(split.recent_messages.iter().cloned());

    // 7. Token estimates for the finished event.
    let original_tokens: usize = messages
        .iter()
        .map(|m| estimate_text_tokens(&serialize_message(m, tool_max)))
        .sum();
    let compressed_tokens: usize = new_messages
        .iter()
        .map(|m| estimate_text_tokens(&serialize_message(m, tool_max)))
        .sum();
    let did_compress = compressed_tokens < original_tokens;
    let compression_ratio = if compressed_tokens == 0 {
        1.0
    } else {
        original_tokens as f64 / compressed_tokens as f64
    };

    info!(
        original_tokens,
        compressed_tokens,
        kept_messages = kept_message_count,
        summary_len = summary.len(),
        "compaction complete"
    );
    event_bus.publish(Event::CompressionFinished {
        session_id: session_id.to_string(),
        original_tokens,
        compressed_tokens,
        compression_ratio,
        did_compress,
        reason: reason.to_string(),
    });

    Ok(CompactionOutcome {
        summary: summary.to_string(),
        new_messages,
        compaction_message,
        original_message_count,
        kept_message_count,
        original_tokens,
        compressed_tokens,
    })
}

/// Emergency overflow compaction (FR-004).
///
/// Invoked by the agent loop when a provider response fails with a context-
/// overflow error before any assistant tokens have been produced. Runs the
/// OpenCode-derived summarisation runner with `reason = "overflow"`, replacing
/// `chat_messages` in place with `[compaction_msg, ...recent]`. The caller then
/// retries the turn once with the compacted history.
///
/// This is the drop-in replacement for the legacy Headroom
/// `emergency_compress_chat_messages` helper. Unlike the Headroom version it
/// is `async` (it calls the LLM to produce the summary) and never silently
/// drops structured message parts (FR-014) — the serialiser represents them
/// textually inside the summary prompt.
///
/// # Arguments
///
/// * `session_id` — the session being compacted.
/// * `chat_messages` — provider-facing chat history; replaced in place on
///   success.
/// * `model` — the model id used for the summarisation call.
/// * `context_window`, `output_tokens`, `config` — forwarded to [`compact`].
/// * `client` — the LLM client used for the summarisation call.
/// * `event_bus` — event bus for compaction-lifecycle events.
///
/// # Errors
///
/// Returns `Err` when there is nothing to summarise, the summary prompt would
/// overflow, the LLM call fails, or the summary comes back empty. On error the
/// `chat_messages` slice is left unchanged so the caller can surface the
/// original overflow error.
pub async fn emergency_compact(
    session_id: &str,
    chat_messages: &mut Vec<ChatMessage>,
    model: &str,
    context_window: usize,
    output_tokens: usize,
    config: &CompactionConfig,
    client: &Arc<dyn crate::llm::LlmClient>,
    event_bus: &EventBus,
    stream_config: &StreamConfig,
) -> Result<CompactionOutcome> {
    // Convert the provider-facing chat messages into the internal `Message`
    // form the compaction runner expects.
    let messages = crate::compaction::convert::chat_messages_to_messages(chat_messages);
    let previous_summary = messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Compaction)
        .map(|m| m.text_content());
    let outcome = compact(
        session_id,
        messages,
        model,
        context_window,
        output_tokens,
        config,
        previous_summary.as_deref(),
        client,
        event_bus,
        "overflow",
        stream_config,
    )
    .await?;
    // Replace the in-memory history in place so the caller's retry attempt
    // sends the compacted payload.
    let new_chat = crate::compaction::convert::messages_to_chat_messages(&outcome.new_messages);
    *chat_messages = new_chat;
    Ok(outcome)
}

/// Truncate the serialised head transcript so the final compaction prompt does
/// not exceed [`MAX_COMPACTION_PROMPT_CHARS`].
///
/// Keeps the most recent conversation content (the end of the string) and
/// prepends a truncation marker when content is dropped.
#[must_use]
fn cap_head_transcript(head_transcript: &str) -> String {
    if head_transcript.len() <= MAX_COMPACTION_PROMPT_CHARS {
        return head_transcript.to_string();
    }
    let marker = "[Earlier conversation omitted due to length]\n\n";
    let keep_len = MAX_COMPACTION_PROMPT_CHARS.saturating_sub(marker.len());
    let truncated = &head_transcript[head_transcript.len() - keep_len..];
    // Try to start at a message boundary so we don't cut mid-message.
    let boundary = truncated.find("\n\n").unwrap_or(0);
    let truncated = &truncated[boundary..];
    format!("{marker}{truncated}")
}

#[cfg(test)]
#[path = "../../tests/inline/test_compaction_runner.rs"]
mod tests;
