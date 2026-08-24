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

use crate::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, ToolDefinition};
use crate::message::{Message, MessagePart, Role};

const MAX_TOOL_RESULT_CHARS_FOR_LLM: usize = 12_000;
const TOOL_RESULT_HEAD_CHARS_FOR_LLM: usize = 10_000;
const TOOL_RESULT_TAIL_CHARS_FOR_LLM: usize = 4_000;

/// Tools whose output may contain sub-agent reports that must never be
/// silently cut by the generic 12 000-char budget. For these tools, the
/// full `content` is written to disk when it exceeds the budget and only
/// per-task summaries plus file paths are left inline, so no sub-agent
/// finding is lost even when 8 background tasks finish in one batch.
const TOOL_RESULT_EXEMPT_TOOLS: &[&str] = &["wait_agents", "list_agents"];

/// Size above which exempt-tool results are diverted to a sidecar file.
/// 32 000 chars comfortably holds ~4 full reports inline; beyond that the
/// per-task preview form is more useful anyway (the model was never going
/// to read eight raw 10 KB reports in one tool result).
const EXEMPT_TOOL_INLINE_LIMIT: usize = 32_000;

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
/// Convert a slice of [`Message`] into provider-facing [`ChatMessage`]s.
///
/// This is async because image attachments require a blocking read of the
/// source file. For sessions without images (the common coder-agent case)
/// it uses a synchronous fast path that avoids a per-turn yield point.
pub async fn history_to_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    let needs_async = messages.iter().any(|m| {
        m.parts
            .iter()
            .any(|p| matches!(p, MessagePart::Image { .. }))
    });
    if !needs_async {
        return history_to_chat_messages_sync(messages);
    }

    let mut chat_messages = Vec::new();

    for msg in messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant | Role::Compaction => "assistant",
        };
        let content = parts_to_chat_content(&msg.parts).await;

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

/// Synchronous fast path of [`history_to_chat_messages`] for sessions that do
/// not contain any image attachments. Avoids a per-turn async yield point
/// while producing the same provider-facing representation.
fn history_to_chat_messages_sync(messages: &[Message]) -> Vec<ChatMessage> {
    let mut chat_messages = Vec::new();

    for msg in messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant | Role::Compaction => "assistant",
        };
        let content = parts_to_chat_content_sync(&msg.parts);

        chat_messages.push(ChatMessage {
            role: role.to_string(),
            content,
        });

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

    // Exempt-path: `wait_agents` / `list_agents` outputs hold sub-agent
    // findings that the caller explicitly asked us never to cut. When the
    // combined report grows past the inline budget we divert the full text
    // to `log/subagents/wait-batch-<ts>.md` and hand the model a summary
    // that names every task and the file to re-read, so nothing is lost.
    if TOOL_RESULT_EXEMPT_TOOLS.contains(&tool) {
        return exempt_tool_result_for_llm(tool, content, metadata);
    }

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
         For `wait_agents` the full per-agent reports are kept in the tool's metadata \
         `\"results\"` array (one object per agent with its complete `\"output\"`) AND \
         under `log/subagents/<task-id>.md` (path shown in the results entry as \
         `\"output_file\"`) — read that file with the `read` tool if the middle \
         segment below omitted findings you need.]\n\n\
         {head}\n\n[... {omitted_chars} chars omitted ...]\n\n{tail}"
    );
    Arc::from(s)
}

/// Exempt-tool variant of [`tool_result_content_for_llm`] used for
/// `wait_agents` / `list_agents`. Up to [`EXEMPT_TOOL_INLINE_LIMIT`] the
/// content passes through unchanged (these results are how the parent model
/// actually reads sub-agent reports). Past that limit the full payload is
/// persisted to `log/subagents/wait-batch-<unix millis>.md` and replaced by
/// a per-task index: one line per task with id, agent, status and the
/// `log/subagents/<task-id>.md` path for the full report, so every finding
/// remains one `read` call away without a silent middle-cut.
fn exempt_tool_result_for_llm(
    tool: &str,
    content: &str,
    metadata: Option<&Value>,
) -> std::sync::Arc<str> {
    use std::sync::Arc;

    if content.len() <= EXEMPT_TOOL_INLINE_LIMIT {
        return Arc::from(content);
    }

    let batch_path = persist_exempt_batch(content);
    let mut index = String::with_capacity(1024);
    index.push_str(&format!(
        "[{tool} output ({chars} chars) exceeded the inline budget; full text was written \
         to {path}. Use `read` on that file (or on the per-agent `output_file` paths below) \
         to recover any finding. The per-task summaries follow.]\n\n",
        chars = content.len(),
        path = batch_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(unavailable — filesystem error)".to_string())
    ));

    if let Some(meta) = metadata
        && let Some(entries) = meta.get("results").and_then(Value::as_array)
    {
        for entry in entries {
            let task_id = entry.get("task_id").and_then(Value::as_str).unwrap_or("?");
            let agent = entry.get("agent").and_then(Value::as_str).unwrap_or("?");
            let success = entry
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let status = entry
                .get("report_status")
                .and_then(Value::as_str)
                .unwrap_or("complete");
            let out_file = entry
                .get("output_file")
                .and_then(Value::as_str)
                .unwrap_or("");
            let preview = entry
                .get("output")
                .and_then(Value::as_str)
                .map(|s| {
                    let head = truncate_at_char_boundary(s, 1_000);
                    head.to_string()
                })
                .unwrap_or_default();
            index.push_str(&format!(
                "- {} `{}` — {} — report {} — full text: {}\n  preview: {}\n\n",
                if success { "ok" } else { "ERR" },
                task_id,
                agent,
                status,
                if out_file.is_empty() {
                    "(none)"
                } else {
                    out_file
                },
                preview
            ));
        }
    } else {
        // Not the shape we expected (should be unreachable — the tool always
        // sends `results`) — fall back to head+tail rather than dropping
        // everything.
        let head = truncate_at_char_boundary(content, TOOL_RESULT_HEAD_CHARS_FOR_LLM);
        let tail = trailing_at_char_boundary(content, TOOL_RESULT_TAIL_CHARS_FOR_LLM);
        index.push_str(head);
        index.push_str("\n\n[... middle omitted ...]\n\n");
        index.push_str(tail);
    }

    Arc::from(index)
}

/// Persist a diverted `wait_agents`/`list_agents` payload to
/// `log/subagents/wait-batch-<millis>.md` under the crate working
/// directory. Returns the path on success; on any I/O failure the caller
/// falls back to the head+tail summary. Temp-file + rename so a crash
/// mid-write never leaves a half-written batch file.
fn persist_exempt_batch(content: &str) -> Option<std::path::PathBuf> {
    let dir = std::path::Path::new("log").join("subagents");
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let file = dir.join(format!("wait-batch-{ts}.md"));
    let tmp = dir.join(format!(".wait-batch-{ts}.md.tmp"));
    std::fs::write(&tmp, content)
        .and_then(|()| std::fs::rename(&tmp, &file))
        .ok()
        .map(|()| file)
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
    // A raw "error decoding response body" before any output almost always
    // indicates an empty/malformed stream (e.g. a local model that is not
    // loaded). Treat it as fatal unless we have already received useful output,
    // in which case it may be a transient mid-stream disconnect worth keeping.
    let is_early_decode_failure = message
        .to_lowercase()
        .contains("error decoding response body")
        && !has_meaningful_partial_output;

    is_retryable_stream_error(message)
        && attempt < max_retries
        && !has_meaningful_partial_output
        && !is_early_decode_failure
}

/// Converts message parts to chat content, handling images asynchronously.
/// This is async because image files may need to be read from disk.
/// Convert message parts into provider-facing [`ChatContent`] without any
/// blocking I/O. Panics if called on an [`Image`] part; callers must first
/// verify the session has no images (see [`history_to_chat_messages`]).
fn parts_to_chat_content_sync(parts: &[MessagePart]) -> ChatContent {
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
                content_parts.push(ContentPart::ToolUse {
                    id: call_id.clone(),
                    name: tool.clone(),
                    input: state.input.clone(),
                });
            }
            MessagePart::Reasoning { .. } => {
                // Reasoning is not forwarded to the provider.
            }
            MessagePart::Image { .. } => {
                unreachable!("parts_to_chat_content_sync must not be called for image sessions")
            }
        }
    }

    ChatContent::Parts(content_parts)
}

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
        "write",
        "create",
        "edit",
        "multiedit",
        "multi_edit",
        "patch",
        "apply_patch",
        "append_to_file",
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
