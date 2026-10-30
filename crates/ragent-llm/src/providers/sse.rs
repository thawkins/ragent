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
//!
//! Tool-call bookkeeping is not this module's: the open calls live in
//! [`ToolCallAccumulator`] under the family's declared
//! [`stream_keying`](crate::provider::wire::ToolWireProfile::stream_keying)
//! (FR-004), so "one start per call, one end per started call" holds here for
//! the same reason it holds for every other provider. The old per-provider
//! `dedup_tool_call_start` opt-out is gone: a switch that could turn the
//! guarantee off was a contract with a way out of it.

use futures::StreamExt;
use serde_json::Value;
use std::pin::Pin;

use super::wire::decode::{StreamKey, ToolCallAccumulator};
use super::wire::profile_table::OPENAI_CHAT;
use crate::llm::StreamEvent;
use crate::provider::http_client::redact_frame;
use ragent_types::event::FinishReason;

/// Per-provider switches for [`parse_openai_sse_stream`].
///
/// The defaults describe the canonical OpenAI chat-completions stream;
/// OpenRouter overrides the reasoning and empty-stream-message fields.
pub(crate) struct OpenAiSseSpec {
    /// Provider label used in diagnostic log fields and error messages.
    pub provider_label: String,
    /// The provider id a synthesised tool-call id is built from (FR-019).
    ///
    /// Deliberately separate from [`provider_label`](Self::provider_label): a call
    /// id is recorded in history and replayed in the next request, so it must read
    /// as an id (`openai_0_read`, `openrouter_0_read`), not as a display label
    /// (`OpenAI_0_read`).
    pub provider_id: String,
    /// Optional base URL logged on an empty stream (OpenRouter only).
    pub base_url: Option<String>,
    /// Route `delta.reasoning` / `delta.reasoning_content` into
    /// `Reasoning*` events (OpenRouter).
    pub parse_reasoning: bool,
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
            // The one OpenAI-compatible client that reaches this parser is the
            // OpenAI one; a second caller supplies its own id.
            provider_id: "openai".to_string(),
            base_url: None,
            parse_reasoning: false,
            usage_marks_yielded: false,
            rich_empty_stream_hint: true,
        }
    }

    /// The OpenRouter chat-completions stream.
    pub(crate) fn openrouter(base_url: String) -> Self {
        Self {
            provider_label: "OpenRouter".to_string(),
            provider_id: "openrouter".to_string(),
            base_url: Some(base_url),
            parse_reasoning: true,
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
    let provider_id = spec.provider_id.clone();
    let base_url = spec.base_url.clone();

    let event_stream = async_stream::stream! {
        // PERF-063: pre-size the SSE accumulation buffer so a long stream does
        // not repeatedly realloc/copy as it grows.
        let mut buffer = String::with_capacity(8 * 1024);
        // FUNC-033: hold an incomplete trailing multibyte character from the
        // previous chunk so a UTF-8 sequence split across TCP chunks is not
        // corrupted.
        let mut pending_utf8: Vec<u8> = Vec::new();
        // FR-004: the chat-completions family declares `StreamKeying::ByIndex`,
        // so every tool-call frame is keyed by its `tool_calls[].index` and the
        // accumulator owns the open-call state (no per-provider map, no
        // de-duplication switch).
        // FR-019: the accumulator mints `<provider>_<counter>_<name>` for a call
        // the server left unnamed, so it takes the provider ID - a call id is
        // replayed in the next request and an id, not a display label.
        let mut acc = ToolCallAccumulator::new(OPENAI_CHAT, &provider_id);
        let mut in_reasoning_block = false;
        let mut yielded_event = false;

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

            // SEC-ragent-llm-004 (SECTASKS T-028): fail the stream when a peer
            // dribbles bytes without ever emitting a newline instead of letting
            // the accumulation buffer grow without bound.
            if let Err(e) = super::http_client::append_chunk_guarded(
                &mut buffer,
                &mut pending_utf8,
                &chunk,
                spec.provider_label.as_str(),
            ) {
                yield StreamEvent::Error {
                    message: e.to_string(),
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
                        // SEC-ragent-llm-020: redact secrets and cap the raw frame
                        // before logging.
                        tracing::warn!(
                            provider = %provider_name,
                            frame = %redact_frame(data),
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
                                    frame = %redact_frame(&parsed.to_string()),
                                    "tool_call delta without an index; skipping frame"
                                );
                                continue;
                            };
                            let key = StreamKey::Index(index);

                            if let Some(id) = tc["id"].as_str() {
                                acc.set_id(&key, id);
                            }

                            if let Some(function) = tc.get("function") {
                                if let Some(name) = function["name"].as_str()
                                    && let Some(event) = acc.open(key.clone(), None, name)
                                {
                                    yield event;
                                    yielded_event = true;
                                }

                                // F4: accept both argument forms. String form
                                // preserves delta semantics; object form
                                // (llama.cpp / vLLM servers) is serialised
                                // whole.
                                let args_json = super::tool_cache::tool_arguments_json(function);
                                if let Some(args) = args_json.filter(|args| !args.is_empty())
                                    && let Some(event) = acc.delta(&key, &args)
                                {
                                    yield event;
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
                        // order; `end_all` sorts by index for a `ByIndex` family.
                        for event in acc.end_all() {
                            yield event;
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
