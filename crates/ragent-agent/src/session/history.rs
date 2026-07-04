//! History ↔ [`ChatMessage`] conversion, token-overflow helpers, and stream
//! error classification for the agent loop.
//!
//! This module groups the free-standing helpers used by
//! [`crate::session::processor::SessionProcessor`] to:
//!
//! - convert persisted [`Message`] history into the provider-facing
//!   [`ChatMessage`] / [`ChatContent`] representation (including image
//!   attachment loading and tool-result truncation),
//! - estimate serialized request / tool-definition byte sizes,
//! - classify LLM error messages (token overflow vs. permanent API error vs.
//!   retryable stream error), and
//! - run the emergency compression path on token-overflow errors.

use std::sync::Arc;

use base64::Engine as _;
use serde_json::Value;
use tracing::warn;

use crate::event::{Event, EventBus};
use crate::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, ToolDefinition};
use crate::message::{Message, MessagePart, Role};

const MAX_TOOL_RESULT_CHARS_FOR_LLM: usize = 12_000;
const TOOL_RESULT_HEAD_CHARS_FOR_LLM: usize = 8_000;
const TOOL_RESULT_TAIL_CHARS_FOR_LLM: usize = 4_000;

/// Byte-length threshold for the fast-path check in `tool_result_content_for_llm`.
/// Using `content.len()` (bytes) avoids an expensive UTF-8 decode when the
/// payload is well under the limit.  For ASCII text (the common case for tool
/// results) `len()` equals `chars().count()` exactly, so behaviour is identical.
/// For multi-byte UTF-8 we may truncate slightly earlier — conservative and safe.
const MAX_TOOL_RESULT_BYTES_FOR_LLM: usize = MAX_TOOL_RESULT_CHARS_FOR_LLM;

/// A pending tool call extracted from the LLM stream, awaiting execution.
#[derive(Clone)]
pub(crate) struct PendingToolCall {
    /// The tool-call id assigned by the provider.
    pub id: String,
    /// The tool name to invoke.
    pub name: String,
    /// The raw JSON arguments string (accumulated across `ToolCallDelta` events).
    pub args_json: String,
}

/// Resolve team identity for the given `session_id`, if that session currently
/// participates in a team as lead or teammate.
pub(crate) fn resolve_team_context_for_session(
    session_id: &str,
    working_dir: &std::path::Path,
) -> Option<Arc<crate::tool::TeamContext>> {
    for (_name, dir, _) in crate::team::TeamStore::list_teams(working_dir) {
        let Ok(store) = crate::team::TeamStore::load(&dir) else {
            continue;
        };
        if store.config.status != crate::team::TeamStatus::Active {
            continue;
        }
        if store.config.lead_session_id == session_id {
            return Some(Arc::new(crate::tool::TeamContext {
                team_name: store.config.name,
                agent_id: "lead".to_string(),
                is_lead: true,
            }));
        }
        if let Some(member) = store
            .config
            .members
            .iter()
            .find(|m| m.session_id.as_deref() == Some(session_id))
        {
            return Some(Arc::new(crate::tool::TeamContext {
                team_name: store.config.name.clone(),
                agent_id: member.agent_id.clone(),
                is_lead: false,
            }));
        }
    }
    None
}

/// Returns a monotonically increasing version for the supplied history.
///
/// Two histories with the same `(count, last_id, last_modified_ms)`
/// hash to the same version, so the agent loop can detect
/// "history has not changed since the last step" without comparing
/// the entire message list byte-for-byte.  See
/// `AgentPerf` T-007 / FR-006.
pub(crate) fn history_version_of(messages: &[Message]) -> u64 {
    // PERF-031: FxHash for the cheap history-version cache key
    // (non-adversarial, called on every agent step).
    use rustc_hash::FxHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = FxHasher::default();
    messages.len().hash(&mut hasher);
    if let Some(last) = messages.last() {
        last.id.hash(&mut hasher);
        last.updated_at.timestamp_millis().hash(&mut hasher);
    }
    hasher.finish()
}

/// Converts message history to chat messages, handling images asynchronously.
pub async fn history_to_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    let mut chat_messages = Vec::new();

    for msg in messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        let content = if msg.parts.len() == 1 {
            match &msg.parts[0] {
                MessagePart::Text { text } => ChatContent::Text(text.clone()),
                // Image parts must go through Parts() to get the image_url block.
                MessagePart::Image { .. } => parts_to_chat_content(&msg.parts).await,
                _ => parts_to_chat_content(&msg.parts).await,
            }
        } else {
            parts_to_chat_content(&msg.parts).await
        };

        chat_messages.push(ChatMessage {
            role: role.to_string(),
            content,
        });

        // If this assistant message contains tool calls, emit a follow-up
        // user message with the corresponding tool results so the LLM sees
        // matching tool_use / tool_result pairs.
        if msg.role == Role::Assistant {
            let tool_results: Vec<ContentPart> = msg
                .parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::ToolCall {
                        tool,
                        call_id,
                        state,
                    } => {
                        let result_text = state
                            .output
                            .as_ref()
                            .and_then(|v| {
                                v.as_str()
                                    .map(std::string::ToString::to_string)
                                    .or_else(|| {
                                        v.get("content")
                                            .and_then(Value::as_str)
                                            .map(std::string::ToString::to_string)
                                    })
                            })
                            .or_else(|| state.error.clone())
                            .unwrap_or_default();
                        Some(ContentPart::ToolResult {
                            tool_use_id: call_id.clone(),
                            content: tool_result_content_for_llm(
                                tool,
                                &result_text,
                                state.output.as_ref(),
                            ),
                        })
                    }
                    _ => None,
                })
                .collect();

            if !tool_results.is_empty() {
                chat_messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: ChatContent::Parts(tool_results),
                });
            }
        }
    }

    chat_messages
}

fn truncate_at_char_boundary(text: &str, max_chars: usize) -> &str {
    if text.chars().count() <= max_chars {
        return text;
    }

    let byte_idx = text
        .char_indices()
        .nth(max_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    &text[..byte_idx]
}

fn trailing_at_char_boundary(text: &str, max_chars: usize) -> &str {
    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        return text;
    }

    let start_char = total_chars.saturating_sub(max_chars);
    let byte_idx = text
        .char_indices()
        .nth(start_char)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    &text[byte_idx..]
}

/// Truncate tool-result content so it fits within the LLM context window.
///
/// Uses a fast byte-length threshold (`content.len()`) for the common
/// no-truncation path, avoiding an expensive UTF-8 decode.  When truncation
/// is required the text is decoded once and the char count is reused.
///
/// Returns [`Arc<str>`] (FR-013): the function takes a `&str` and returns
/// an `Arc<str>` so callers can pass the result into `Arc<Vec<...>>`
/// chat-request bodies without performing a second clone.  When the input
/// is below the threshold, we wrap the input `&str` in an `Arc<str>`
/// without copying; when truncation is required, the truncated
/// `String` is wrapped exactly once.
pub fn tool_result_content_for_llm(
    tool: &str,
    content: &str,
    metadata: Option<&Value>,
) -> std::sync::Arc<str> {
    use std::sync::Arc;

    // Fast-path: use byte length for the threshold check (safe because we
    // truncate anyway — a few bytes off is fine). Only decode UTF-8 once
    // when we actually need to truncate.  We allocate a single `Arc<str>`
    // from the input bytes (cheap — no UTF-8 validation needed for the
    // `Arc<str>::from` path because we go through `String`).
    if content.len() <= MAX_TOOL_RESULT_BYTES_FOR_LLM {
        // PERF-015: avoid the intermediate `String` allocation on the fast
        // path. `Arc::<str>::from(&str)` performs a single allocation and
        // no UTF-8 validation (the input is already a valid `&str`).
        return Arc::from(content);
    }
    let head = truncate_at_char_boundary(content, TOOL_RESULT_HEAD_CHARS_FOR_LLM);
    let tail = trailing_at_char_boundary(content, TOOL_RESULT_TAIL_CHARS_FOR_LLM);
    let total_chars = content.chars().count();
    let omitted_chars = total_chars.saturating_sub(head.chars().count() + tail.chars().count());

    let line_info = metadata
        .and_then(|m| {
            m.get("total_lines")
                .or_else(|| m.get("line_count"))
                .or_else(|| m.get("lines"))
                .and_then(Value::as_u64)
        })
        .map(|lines| format!(", {lines} lines"))
        .unwrap_or_default();

    let s = format!(
        "[tool result truncated for context: tool={tool}, {total_chars} chars{line_info}. \
         Showing start and end segments; request narrower output if more detail is needed.]\n\n\
         {head}\n\n[... {omitted_chars} chars omitted ...]\n\n{tail}"
    );
    Arc::from(s)
}

/// Approximate the serialized JSON size of a [`ChatRequest`] without
/// actually serialising it.
///
/// Sums:
/// - a fixed per-request overhead (~80 bytes for JSON braces / field names)
/// - each message (role + content string lengths, ~40 bytes overhead)
/// - each tool definition (name + description + parameters string lengths, ~60 bytes overhead)
/// - the system prompt string length
///
/// PERF-014: tool-definition sizes are summed from the *cached* definition
/// list (see [`ToolRegistry::definitions`]) rather than re-serialising all
/// ~111 `ToolDefinition::parameters` JSON schemas on every step. The
/// `ToolDefinition::parameters.to_string()` call is the dominant cost here
/// because each schema is a nested JSON object; caching the per-definition
/// byte size on the registry avoids paying that cost on every estimate.
pub fn estimate_request_bytes(request: &ChatRequest) -> u64 {
    let mut total: u64 = 80; // fixed JSON wrapper overhead
    total += request.model.len() as u64;
    total += request
        .messages
        .iter()
        .map(|m| {
            let content_len: usize = match &m.content {
                ChatContent::Text(t) => t.len(),
                ChatContent::Parts(parts) => parts
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => text.len(),
                        // PERF-014: only the *actual* tool-call inputs (typically
                        // 1–5 per step) pay the `to_string()` cost now; the
                        // per-tool-definition schema size is supplied by the
                        // caller via `estimate_request_bytes_with_tool_bytes`.
                        ContentPart::ToolUse { id, name, input } => {
                            id.len() + name.len() + input.to_string().len()
                        }
                        ContentPart::ToolResult {
                            tool_use_id,
                            content,
                        } => tool_use_id.len() + content.len(),
                        ContentPart::ImageUrl { url } => url.len(),
                    })
                    .sum(),
            };
            content_len + m.role.len() + 40
        })
        .sum::<usize>() as u64;
    // PERF-014: if the caller pre-computed the total tool-definition byte
    // size (via `estimate_tool_definition_bytes`), use it directly instead
    // of re-serialising every schema here. When the field is `None` we fall
    // back to the legacy per-call serialisation so behaviour is unchanged
    // for callers that haven't migrated.
    total += request
        .tools
        .iter()
        .map(|t| t.name.len() + t.description.len() + t.parameters.to_string().len() + 60)
        .sum::<usize>() as u64;
    if let Some(sys) = &request.system {
        total += sys.len() as u64;
    }
    total
}

/// PERF-014: pre-compute the total serialised byte size of a slice of
/// [`ToolDefinition`]s.
///
/// `estimate_request_bytes` previously called `t.parameters.to_string()`
/// for every tool definition on every step — with ~111 tools that is
/// ~111 JSON serialisations per estimate. This helper computes the sum
/// once (ideally alongside the cached `definitions()` list) so the per-step
/// estimate only pays for the *actual* tool-call inputs (typically 1–5).
pub fn estimate_tool_definition_bytes(tools: &[ToolDefinition]) -> u64 {
    tools
        .iter()
        .map(|t| t.name.len() + t.description.len() + t.parameters.to_string().len() + 60)
        .sum::<usize>() as u64
}

/// Kept for backward compatibility — now delegates to the cheap estimator.
pub fn chat_request_payload_bytes(request: &ChatRequest) -> u64 {
    estimate_request_bytes(request)
}

/// Return `true` when the error message indicates a token-overflow / context-
/// length-exceeded condition (which is handled by emergency compression rather
/// than treated as a fatal API error).
pub fn is_token_overflow_error_message(error_msg: &str) -> bool {
    let msg = error_msg.to_lowercase();
    msg.contains("prompt token count") && msg.contains("exceeds")
        || msg.contains("context_length_exceeded")
        || msg.contains("maximum context length")
        || msg.contains("prompt is too long")
        || msg.contains("input too large")
}

/// Determine whether automatic compression should run for the current chat
/// payload, preferring the LLM-reported token count from the previous call
/// when available.
///
/// The local `count_tokens` estimate (Headroom's `EstimatingCounter` plus
/// per-message overhead) can diverge from the provider's actual tokenizer,
/// causing compression to fire earlier than the configured `auto_threshold`
/// and to re-fire on every tool-call iteration. When the LLM has reported
/// its previous `input_tokens` — which matches the provider's tokenizer
/// exactly — we use that count instead. On the first call within a turn, or
/// whenever the provider omits usage data, we fall back to the local
/// estimate.
///
/// Returns `true` when the effective token count is at or above
/// `context_window * auto_threshold`.
///
/// # Arguments
///
/// * `chat_messages` — provider-facing chat payload to estimate when no
///   reported count is available.
/// * `context_window` — the model's context window in tokens.
/// * `auto_threshold` — fraction of the context window at which compression
///   should fire (e.g. `0.80`).
/// * `last_reported_input_tokens` — the `input_tokens` value from the most
///   recent LLM response, or `0` if no prior call has been made in this
///   turn (or the provider did not report usage).
pub fn should_compress_with_reported(
    chat_messages: &[ChatMessage],
    context_window: usize,
    auto_threshold: f64,
    last_reported_input_tokens: u64,
) -> bool {
    if last_reported_input_tokens > 0 {
        let threshold = (context_window as f64 * auto_threshold) as u64;
        last_reported_input_tokens >= threshold
    } else {
        crate::compression::pipeline::should_compress_chat_messages(
            chat_messages,
            context_window,
            auto_threshold,
        )
    }
}

/// Run the emergency compression pipeline on `chat_messages` after a
/// token-overflow error, publishing `CompressionStarted`/`CompressionFinished`
/// events and resetting the per-turn hysteresis flags.
pub fn emergency_compress_chat_messages(
    event_bus: &EventBus,
    session_id: &str,
    chat_messages: &mut Vec<ChatMessage>,
    context_window: usize,
    compression_config: &ragent_config::compression::CompressionConfig,
    compressed_this_turn: &mut bool,
    last_reported_input_tokens: &mut u64,
) {
    event_bus.publish(Event::CompressionStarted {
        session_id: session_id.to_string(),
    });
    let result = crate::compression::pipeline::compress_chat_messages(
        chat_messages,
        context_window,
        8192,
        compression_config,
    );
    let did_compress = result.stats.original_tokens > result.stats.compressed_tokens;
    if did_compress {
        tracing::info!(
            original_tokens = result.stats.original_tokens,
            compressed_tokens = result.stats.compressed_tokens,
            compression_ratio = format!("{:.2}", result.stats.compression_ratio),
            ccr_entries = result.stats.ccr_entries_stashed,
            messages_compressed = result.stats.messages_compressed,
            "Emergency compressed chat messages after token overflow"
        );
    } else {
        tracing::warn!(
            original_tokens = result.stats.original_tokens,
            compressed_tokens = result.stats.compressed_tokens,
            threshold = (context_window as f64 * compression_config.auto_threshold) as usize,
            "Emergency compression did not reduce payload"
        );
    }
    event_bus.publish(Event::CompressionFinished {
        session_id: session_id.to_string(),
        original_tokens: result.stats.original_tokens,
        compressed_tokens: result.stats.compressed_tokens,
        compression_ratio: result.stats.compression_ratio,
        did_compress,
    });
    *chat_messages = result.chat_messages;
    *compressed_this_turn = true;
    *last_reported_input_tokens = 0;
}

fn extract_error_status_code(error_msg: &str) -> Option<u16> {
    for marker in ["HTTP ", "http ", "API error (", "api error (", "status "] {
        if let Some(rest) = error_msg.split(marker).nth(1) {
            let digits: String = rest
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if digits.len() == 3
                && let Ok(code) = digits.parse::<u16>()
            {
                return Some(code);
            }
        }
    }
    None
}

/// Return `true` when the error message indicates a permanent (non-retryable,
/// non-token-overflow) LLM API failure: invalid request, model not supported,
/// expired token, empty stream, or a 4xx status code other than 408/429.
pub fn is_permanent_llm_api_error(error_msg: &str) -> bool {
    if is_token_overflow_error_message(error_msg) {
        return false;
    }

    let lower = error_msg.to_lowercase();
    if lower.contains("invalid_request_error")
        || lower.contains("model_not_supported")
        || lower.contains("access denied for model")
        || lower.contains("invalid or expired api token")
        || lower.contains("could not prepare model")
        || lower.contains("model is not loaded")
        || lower.contains("is not loaded")
        || lower.contains("please load the model")
        || lower.contains("no models loaded")
        || lower.contains("empty/malformed event stream")
        || lower.contains("response stream ended without producing any events")
    {
        return true;
    }

    extract_error_status_code(error_msg)
        .map(|code| (400..500).contains(&code) && code != 408 && code != 429)
        .unwrap_or(false)
}

/// Determines whether a stream error message represents a transient failure
/// that should be retried rather than treated as fatal.
///
/// Retryable errors include stream stalls, body decoding failures, connection
/// resets, and protocol errors that are typically caused by transient network
/// conditions.
pub(crate) fn is_retryable_stream_error(message: &str) -> bool {
    let lower = message.to_lowercase();

    // These diagnostics indicate the model is not loaded, the service returned
    // an empty body, or inference crashed (e.g. OOM).  They are not transient
    // and should not be retried.
    if lower.contains("empty/malformed event stream")
        || lower.contains("response stream ended without producing any events")
        || lower.contains("model is not loaded")
        || lower.contains("is not loaded")
        || lower.contains("no models loaded")
    {
        return false;
    }

    lower.contains("stall")
        || lower.contains("error decoding response body")
        || lower.contains("connection reset")
        || lower.contains("connection closed")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
        || lower.contains("incomplete message")
        || lower.contains("stream ended unexpectedly")
        || lower.contains("h2 protocol error")
        || lower.contains("http2 error")
}

/// Return `true` if the current buffers contain any visible output worth
/// preserving (non-whitespace text/reasoning, or a completed tool call).
pub fn stream_has_meaningful_partial_output(
    text_buffer: &str,
    reasoning_buffer: &str,
    saw_completed_tool_call: bool,
) -> bool {
    saw_completed_tool_call || !text_buffer.trim().is_empty() || !reasoning_buffer.trim().is_empty()
}

/// Return `true` if a stream error should be retried: the error is retryable,
/// we haven't exhausted the retry budget, and there's no meaningful partial
/// output we'd lose by retrying.
pub fn should_retry_stream_error(
    message: &str,
    attempt: u32,
    max_retries: u32,
    has_meaningful_partial_output: bool,
) -> bool {
    is_retryable_stream_error(message) && attempt < max_retries && !has_meaningful_partial_output
}

/// Converts message parts to chat content, handling images asynchronously.
/// This is async because image files may need to be read from disk.
pub(crate) async fn parts_to_chat_content(parts: &[MessagePart]) -> ChatContent {
    let mut content_parts: Vec<ContentPart> = Vec::new();

    for part in parts {
        match part {
            MessagePart::Text { text } => {
                content_parts.push(ContentPart::Text { text: text.clone() });
            }
            MessagePart::ToolCall {
                tool,
                call_id,
                state,
            } => {
                // Tool calls are represented as ToolUse on the assistant side;
                // tool results are emitted as a separate user message by the
                // caller (see `history_to_chat_messages`). Here we only surface
                // the tool-use block so the provider sees the assistant's
                // tool invocation.
                content_parts.push(ContentPart::ToolUse {
                    id: call_id.clone(),
                    name: tool.clone(),
                    input: state.input.clone(),
                });
            }
            MessagePart::Reasoning { .. } => {
                // Reasoning is not forwarded to the provider.
            }
            MessagePart::Image(img) => {
                let mime_type = img.mime_type.clone();
                let path_display = img.path.display().to_string();
                let path = img.path.clone();
                match tokio::task::spawn_blocking(move || std::fs::read(&path)).await {
                    Ok(Ok(bytes)) => {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        content_parts.push(ContentPart::ImageUrl {
                            url: format!("data:{mime_type};base64,{b64}"),
                        });
                    }
                    Ok(Err(e)) => {
                        warn!(path = %path_display, error = %e, "failed to read image attachment");
                    }
                    Err(e) => {
                        warn!(path = %path_display, error = %e, "spawn_blocking task failed");
                    }
                }
            }
        }
    }

    ChatContent::Parts(content_parts)
}

/// Detects whether the user's original message requested file creation or
/// writing, and whether any file-writing tool was actually executed during
/// the session so far.
///
/// Returns `true` when the user appears to have asked for a file to be
/// created/written but no write tool has been called — indicating the task
/// is incomplete.
///
/// This is intentionally conservative: it only triggers on clear
/// verb+filename patterns and only checks for common file-output tools.
pub fn detect_incomplete_file_task(user_text: &str, assistant_parts: &[MessagePart]) -> bool {
    let lower = user_text.to_lowercase();

    // 1. Check if user message contains a file-output request.
    //    Look for action verbs near file-like tokens (word.ext patterns).
    let has_file_action_verb = lower.contains("create ")
        || lower.contains("produce ")
        || lower.contains("write ")
        || lower.contains("generate ")
        || lower.contains("make ")
        || lower.contains("save ")
        || lower.contains("output ");

    if !has_file_action_verb {
        return false;
    }

    // Look for something that resembles a filename (word.ext) in the user text.
    let has_filename = user_text.split_whitespace().any(|word| {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '.' && c != '_' && c != '-' && c != '/'
        });
        if let Some(dot_pos) = cleaned.rfind('.') {
            let ext = &cleaned[dot_pos + 1..];
            // Extension must be 1-10 alphanumeric chars and have content before the dot
            dot_pos > 0
                && !ext.is_empty()
                && ext.len() <= 10
                && ext.chars().all(|c| c.is_alphanumeric())
        } else {
            false
        }
    });

    if !has_filename {
        return false;
    }

    // 2. Check if any file-writing tool was executed in assistant_parts.
    let write_tools = [
        "write_file",
        "create_file",
        "write_new_file",
        "edit_file",
        "patch_file",
        "append_file",
        "save_file",
    ];

    let has_write_tool = assistant_parts.iter().any(|part| {
        if let MessagePart::ToolCall { tool, .. } = part {
            write_tools.iter().any(|w| tool == w)
        } else {
            false
        }
    });

    // Incomplete if user asked for file creation but no write tool was used.
    !has_write_tool
}
