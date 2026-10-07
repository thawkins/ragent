//! Shared OpenAI-compatible SSE stream parser (audit T-402).
//!
//! `OpenAiClient::parse_sse_stream` and `OpenRouterClient::parse_sse_stream`
//! were near-identical copies that had drifted: the OpenAI copy enforced an
//! explicit tool-call `index` and de-duplicated `ToolCallStart`, while the
//! OpenRouter copy still used `unwrap_or(0)` and emitted duplicate starts but
//! additionally routed `reasoning` / `reasoning_content` deltas. This module
//! owns one parser parameterised by [`OpenAiSseSpec`]; both providers delegate
//! so the fixed behaviours (explicit index, start de-duplication) apply to
//! both and the remaining OpenRouter-specific reasoning routing is explicit.

use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::llm::StreamEvent;
use ragent_types::event::FinishReason;

/// Per-provider switches for [`parse_openai_sse_stream`].
///
/// The defaults describe the canonical OpenAI chat-completions stream;
/// OpenRouter overrides the reasoning and empty-stream-message fields.
pub(crate) struct OpenAiSseSpec {
    /// Provider label used in diagnostic log fields and error messages.
    pub provider_label: String,
    /// Optional base URL logged on an empty stream (OpenRouter only).
    pub base_url: Option<String>,
    /// Route `delta.reasoning` / `delta.reasoning_content` into
    /// `Reasoning*` events (OpenRouter).
    pub parse_reasoning: bool,
    /// Emit `ToolCallStart` at most once per tool-call index (OpenAI).
    pub dedup_tool_call_start: bool,
    /// Treat a `usage`-only frame as a yielded event (OpenRouter).
    pub usage_marks_yielded: bool,
    /// Append the "requested model is not loaded" hint to the empty-stream
    /// message (OpenAI; local OpenAI-compatible endpoints).
    pub rich_empty_stream_hint: bool,
}

impl OpenAiSseSpec {
    /// The canonical OpenAI chat-completions stream.
    pub(crate) fn openai(provider_label: String) -> Self {
        Self {
            provider_label,
            base_url: None,
            parse_reasoning: false,
            dedup_tool_call_start: true,
            usage_marks_yielded: false,
            rich_empty_stream_hint: true,
        }
    }

    /// The OpenRouter chat-completions stream.
    pub(crate) fn openrouter(base_url: String) -> Self {
        Self {
            provider_label: "OpenRouter".to_string(),
            base_url: Some(base_url),
            parse_reasoning: true,
            dedup_tool_call_start: false,
            usage_marks_yielded: true,
            rich_empty_stream_hint: false,
        }
    }
}

/// Parse an OpenAI-compatible SSE response into a [`StreamEvent`] stream.
///
/// Handles `data: {...}` lines, `[DONE]`, `choices[].delta` text, optional
/// `reasoning` / `reasoning_content` deltas, incremental `tool_calls`,
/// final-chunk `usage`, and `finish_reason` mapping.
pub(crate) fn parse_openai_sse_stream(
    response: reqwest::Response,
    spec: OpenAiSseSpec,
) -> Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> {
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let rate_limit_event = super::openai::parse_openai_rate_limit_headers(response.headers());
    let stream = response.bytes_stream();
    let provider_name = spec.provider_label.clone();
    let base_url = spec.base_url.clone();

    let event_stream = async_stream::stream! {
        // PERF-063: pre-size the SSE accumulation buffer so a long stream does
        // not repeatedly realloc/copy as it grows.
        let mut buffer = String::with_capacity(8 * 1024);
        // FUNC-033: hold an incomplete trailing multibyte character from the
        // previous chunk so a UTF-8 sequence split across TCP chunks is not
        // corrupted.
        let mut pending_utf8: Vec<u8> = Vec::new();
        let mut tool_call_ids: HashMap<u64, String> = HashMap::new();
        let mut in_reasoning_block = false;
        let mut yielded_event = false;
        // A2: indices whose ToolCallStart has already been emitted, so a
        // provider repeating `function.name` across delta frames cannot
        // produce duplicate Start events.
        let mut started_tool_call_indices: std::collections::HashSet<u64> =
            std::collections::HashSet::new();

        if let Some(ev) = rate_limit_event {
            yield ev;
            yielded_event = true;
        }

        futures::pin_mut!(stream);

        loop {
            let chunk = match tokio::time::timeout(
                std::time::Duration::from_secs(
                    super::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS,
                ),
                stream.next(),
            )
            .await
            {
                Ok(Some(r)) => match r {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            provider = %provider_name,
                            status = %status,
                            content_type = %content_type,
                            yielded_events = yielded_event,
                            error = %e,
                            "SSE stream decode error"
                        );
                        let err_text = e.to_string();
                        let is_decode_failure =
                            err_text.to_lowercase().contains("error decoding response body");
                        // A successful HTTP status with an immediate decode failure
                        // almost always means the endpoint returned an empty or
                        // non-stream body (e.g. a local model that is not loaded).
                        // Surface that as a clear, non-retryable error instead of
                        // the raw reqwest diagnostic.
                        let message = if is_decode_failure
                            && !yielded_event
                            && status.is_success()
                        {
                            if spec.rich_empty_stream_hint {
                                format!(
                                    "{provider_name} returned an empty/malformed event stream (status {status}, content-type {content_type}). \
                                     For local OpenAI-compatible providers this usually means the requested model is not loaded."
                                )
                            } else {
                                format!(
                                    "{provider_name} returned an empty/malformed event stream (status {status}, content-type {content_type})."
                                )
                            }
                        } else {
                            err_text
                        };
                        yield StreamEvent::Error { message };
                        break;
                    }
                },
                Ok(None) => break,
                Err(_) => {
                    yield StreamEvent::Error {
                        message: format!(
                            "{}: stream stalled - no data received for {}s",
                            provider_name,
                            super::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS
                        ),
                    };
                    break;
                }
            };

            super::http_client::append_stream_chunk(&mut buffer, &mut pending_utf8, &chunk);

            // SEC-ragent-llm-004 (SECTASKS T-028): fail the stream when a peer
            // dribbles bytes without ever emitting a newline instead of letting
            // the accumulation buffer grow without bound.
            if super::http_client::sse_buffer_exceeded(&buffer) {
                tracing::warn!(
                    limit = super::http_client::MAX_SSE_BUFFER_BYTES,
                    "SSE accumulation buffer exceeded the cap; aborting the stream"
                );
                yield StreamEvent::Error {
                    message: format!(
                        "SSE buffer exceeded {} bytes without a complete frame",
                        super::http_client::MAX_SSE_BUFFER_BYTES
                    ),
                };
                return;
            }

            while let Some(line) = super::http_client::take_sse_line(&mut buffer) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let data = match line.strip_prefix("data: ") {
                    Some(d) => d.trim(),
                    None => continue,
                };

                if data == "[DONE]" {
                    if spec.parse_reasoning && in_reasoning_block {
                        yield StreamEvent::ReasoningEnd;
                    }
                    yield StreamEvent::Finish { reason: FinishReason::Stop };
                    return;
                }

                let parsed: Value = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(e) => {
                        // FUNC-032: a corrupt frame must be logged, not silently
                        // dropped - it can carry tool-call deltas.
                        tracing::warn!(
                            provider = %provider_name,
                            frame = %data,
                            error = %e,
                            "dropping malformed SSE data frame"
                        );
                        continue;
                    }
                };

                // Handle usage info (sent with stream_options.include_usage)
                if let Some(usage) = parsed.get("usage")
                    && !usage.is_null()
                {
                    let input_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
                    let output_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
                    if input_tokens > 0 || output_tokens > 0 {
                        yield StreamEvent::Usage { input_tokens, output_tokens };
                        if spec.usage_marks_yielded {
                            yielded_event = true;
                        }
                    }
                }

                let choices = match parsed["choices"].as_array() {
                    Some(c) => c,
                    None => continue,
                };

                for choice in choices {
                    let delta = &choice["delta"];

                    if spec.parse_reasoning {
                        // Providers expose reasoning text under either
                        // `delta.reasoning` or `delta.reasoning_content`
                        // depending on the upstream model.
                        let reasoning_text = delta
                            .get("reasoning")
                            .or_else(|| delta.get("reasoning_content"))
                            .and_then(|v| v.as_str());
                        if let Some(text) = reasoning_text {
                            if !text.is_empty() {
                                if !in_reasoning_block {
                                    yield StreamEvent::ReasoningStart;
                                    in_reasoning_block = true;
                                }
                                yield StreamEvent::ReasoningDelta {
                                    text: text.to_string(),
                                };
                                yielded_event = true;
                            }
                        }
                    }

                    // Text content. Close any open reasoning block first so
                    // consumers see contiguous content phases.
                    if let Some(content) = delta["content"].as_str() {
                        if spec.parse_reasoning && in_reasoning_block && !content.is_empty() {
                            yield StreamEvent::ReasoningEnd;
                            in_reasoning_block = false;
                        }
                        if !content.is_empty() {
                            yield StreamEvent::TextDelta {
                                text: content.to_string(),
                            };
                            yielded_event = true;
                        }
                    }

                    // Tool calls
                    if let Some(tool_calls) = delta["tool_calls"].as_array() {
                        for tc in tool_calls {
                            // FUNC-032: require an explicit `index`. The old
                            // `unwrap_or(0)` mapped every missing index onto
                            // stream 0, merging distinct parallel tool calls.
                            let Some(index) = tc["index"].as_u64() else {
                                tracing::warn!(
                                    provider = %provider_name,
                                    frame = %parsed,
                                    "tool_call delta without an index; skipping frame"
                                );
                                continue;
                            };

                            if let Some(id) = tc["id"].as_str() {
                                tool_call_ids.insert(index, id.to_string());
                            }

                            if let Some(function) = tc.get("function") {
                                let tc_id = tool_call_ids
                                    .get(&index)
                                    .cloned()
                                    .unwrap_or_else(|| format!("tc_{index}"));
                                if let Some(name) = function["name"].as_str() {
                                    // A2: guard against duplicate Start events
                                    // when `function.name` repeats across delta
                                    // frames for the same index. Scoped to the
                                    // Start emission only so a repeated-name
                                    // frame that also carries arguments still
                                    // yields its delta below.
                                    let emit_start = !spec.dedup_tool_call_start
                                        || started_tool_call_indices.insert(index);
                                    if emit_start {
                                        yield StreamEvent::ToolCallStart {
                                            id: tc_id.clone(),
                                            name: name.to_string(),
                                        };
                                        yielded_event = true;
                                    }
                                }

                                // F4: accept both argument forms. String form
                                // preserves delta semantics; object form
                                // (llama.cpp / vLLM servers) is serialised
                                // whole.
                                let args_json = super::tool_cache::tool_arguments_json(function);
                                if let Some(args) = args_json.filter(|args| !args.is_empty()) {
                                    yield StreamEvent::ToolCallDelta {
                                        id: tc_id,
                                        args_json: args,
                                    };
                                    yielded_event = true;
                                }
                            }
                        }
                    }

                    // Finish reason
                    if let Some(finish_reason) = choice["finish_reason"].as_str() {
                        if spec.parse_reasoning && in_reasoning_block {
                            yield StreamEvent::ReasoningEnd;
                            in_reasoning_block = false;
                        }

                        // End any pending tool calls in index order so consumers
                        // pairing Start/End events by sequence see a deterministic
                        // order (HashMap iteration is arbitrary).
                        let mut ends: Vec<(u64, String)> = tool_call_ids.drain().collect();
                        ends.sort_unstable_by_key(|(idx, _)| *idx);
                        for (_, id) in ends {
                            yield StreamEvent::ToolCallEnd { id };
                        }

                        let reason = match finish_reason {
                            "tool_calls" => FinishReason::ToolUse,
                            "length" => FinishReason::Length,
                            "content_filter" => FinishReason::ContentFilter,
                            _ => FinishReason::Stop,
                        };
                        yield StreamEvent::Finish { reason };
                        yielded_event = true;
                    }
                }
            }
        }

        if spec.parse_reasoning && in_reasoning_block {
            yield StreamEvent::ReasoningEnd;
        }

        if !yielded_event {
            warn_empty_stream(&provider_name, status, &content_type, base_url.as_deref());
            let message = if spec.rich_empty_stream_hint {
                format!(
                    "{provider_name} response stream ended without producing any events (status {status}, content-type {content_type}). \
                     For local OpenAI-compatible providers this usually means the requested model is not loaded or the service returned an empty body."
                )
            } else {
                format!(
                    "{provider_name} response stream ended without producing any events (status {status}, content-type {content_type})."
                )
            };
            yield StreamEvent::Error { message };
        }
    };

    Box::pin(event_stream)
}

/// Log an empty stream once, keeping the OpenRouter `base_url` field optional.
fn warn_empty_stream(
    provider_name: &str,
    status: reqwest::StatusCode,
    content_type: &str,
    base_url: Option<&str>,
) {
    match base_url {
        Some(base_url) => tracing::warn!(
            provider = %provider_name,
            status = %status,
            content_type = %content_type,
            base_url = %base_url,
            "response stream ended without producing any events"
        ),
        None => tracing::warn!(
            provider = %provider_name,
            status = %status,
            content_type = %content_type,
            "SSE stream ended without yielding any events"
        ),
    }
}
