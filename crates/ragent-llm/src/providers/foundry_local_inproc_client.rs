//! In-process Microsoft Foundry Local LLM client.
//!
//! Uses the `foundry-local-sdk` native core to load and run models inside the
//! ragent process, avoiding the local web service.  Chat requests are translated
//! to the SDK's [`ChatClient`] and streamed back as ragent [`StreamEvent`]s.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::event::FinishReason;
use crate::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent, ToolDefinition};
use ragent_types::event::{Event, EventBus};

use foundry_local_sdk::{
    ChatCompletionRequestMessage, ChatCompletionTools, CreateChatCompletionStreamResponse,
    DeviceType, FinishReason as SdkFinishReason, Model as SdkModel,
};

/// In-process client for Microsoft Foundry Local inference.
///
/// Holds a reference to the SDK manager singleton and the preferred execution
/// device.  The model is downloaded/loaded on demand before the first chat
/// request and its id is cached so subsequent requests can skip redundant
/// loading when the same model is requested again.
pub struct FoundryLocalInProcClient {
    /// The SDK manager that owns the native core and catalog.
    manager: &'static foundry_local_sdk::FoundryLocalManager,
    /// Optional event bus for publishing download/lifecycle events.
    event_bus: Option<Arc<EventBus>>,
    /// Preferred execution device, if explicitly configured.
    device: Option<DeviceType>,
    /// Id of the model currently loaded in-process, if known.
    loaded_model_id: Mutex<Option<String>>,
}

impl FoundryLocalInProcClient {
    /// Create a new in-process client.
    #[must_use]
    pub fn new(
        manager: &'static foundry_local_sdk::FoundryLocalManager,
        event_bus: Option<Arc<EventBus>>,
        device: Option<DeviceType>,
    ) -> Self {
        Self {
            manager,
            event_bus,
            device,
            loaded_model_id: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for FoundryLocalInProcClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let session_id = request.session_id.clone().unwrap_or_default();

        // Resolve alias/variant, download if necessary, and load via the native core.
        let model = ensure_model_ready(
            self.manager,
            &request.model,
            self.device.as_ref(),
            self.event_bus.clone(),
            &session_id,
        )
        .await
        .with_context(|| {
            format!(
                "Microsoft Foundry Local in-process could not prepare model '{}' for inference",
                request.model
            )
        })?;

        // Remember which model we loaded so we can skip redundant loads.
        {
            let mut guard = self.loaded_model_id.lock().await;
            *guard = Some(model.id().to_string());
        }

        let messages = build_messages(&request)?;
        let tools = build_tools(&request.tools);
        let tools_ref = if tools.is_empty() {
            None
        } else {
            Some(tools.as_slice())
        };

        let mut client = model.create_chat_client();
        if let Some(temp) = request.temperature {
            client = client.temperature(f64::from(temp));
        }
        if let Some(max_tokens) = request.max_tokens {
            client = client.max_tokens(max_tokens);
        }

        let sdk_stream = client
            .complete_streaming_chat(&messages, tools_ref)
            .await
            .with_context(|| "Foundry Local in-process chat request failed")?;

        Ok(Box::pin(translate_stream(sdk_stream)))
    }
}

/// Build OpenAI-compatible messages from a ragent [`ChatRequest`].
///
/// Messages are constructed as JSON first and then deserialized into the SDK's
/// typed `ChatCompletionRequestMessage` enum.  This avoids importing the many
/// content-part types that `foundry-local-sdk` does not re-export, while still
/// producing exactly the wire format the native core expects.
fn build_messages(request: &ChatRequest) -> Result<Vec<ChatCompletionRequestMessage>> {
    let mut json_messages: Vec<Value> = Vec::new();

    if let Some(system) = &request.system {
        json_messages.push(json!({
            "role": "system",
            "content": system,
        }));
    }

    for msg in request.messages.iter() {
        match &msg.content {
            ChatContent::Text(text) => {
                json_messages.push(json!({
                    "role": msg.role,
                    "content": text,
                }));
            }
            ChatContent::Parts(parts) => {
                // Split assistant messages into a single assistant message (text
                // + tool calls) followed by one tool-result message per result.
                if msg.role == "assistant" {
                    let text_parts: Vec<String> = parts
                        .iter()
                        .filter_map(|p| match p {
                            ContentPart::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect();
                    let tool_uses: Vec<&ContentPart> = parts
                        .iter()
                        .filter(|p| matches!(p, ContentPart::ToolUse { .. }))
                        .collect();
                    let tool_results: Vec<&ContentPart> = parts
                        .iter()
                        .filter(|p| matches!(p, ContentPart::ToolResult { .. }))
                        .collect();

                    if !tool_uses.is_empty() {
                        let tool_calls: Vec<Value> = tool_uses
                            .iter()
                            .map(|p| match p {
                                ContentPart::ToolUse { id, name, input } => json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string()
                                    }
                                }),
                                _ => unreachable!(),
                            })
                            .collect();
                        let mut assistant_msg = json!({
                            "role": "assistant",
                            "tool_calls": tool_calls,
                        });
                        if !text_parts.is_empty() {
                            assistant_msg["content"] = json!(text_parts.join("\n"));
                        }
                        json_messages.push(assistant_msg);
                    } else if !text_parts.is_empty() {
                        json_messages.push(json!({
                            "role": "assistant",
                            "content": text_parts.join("\n"),
                        }));
                    }

                    for result in tool_results {
                        if let ContentPart::ToolResult {
                            tool_use_id,
                            content,
                        } = result
                        {
                            json_messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                        }
                    }
                } else {
                    // User/tool messages: keep only text and image parts.  Ignore
                    // tool-use/tool-result parts in non-assistant roles.
                    let mut content_parts: Vec<Value> = Vec::new();
                    for part in parts {
                        match part {
                            ContentPart::Text { text } => {
                                content_parts.push(json!({
                                    "type": "text",
                                    "text": text,
                                }));
                            }
                            ContentPart::ImageUrl { url } => {
                                content_parts.push(json!({
                                    "type": "image_url",
                                    "image_url": { "url": url },
                                }));
                            }
                            _ => {}
                        }
                    }
                    if content_parts.len() == 1 {
                        json_messages.push(json!({
                            "role": msg.role,
                            "content": content_parts[0]["text"].clone(),
                        }));
                    } else {
                        json_messages.push(json!({
                            "role": msg.role,
                            "content": content_parts,
                        }));
                    }
                }
            }
        }
    }

    let messages: Vec<ChatCompletionRequestMessage> = serde_json::from_value(Value::Array(
        json_messages,
    ))
    .with_context(|| "Failed to convert ragent messages to Foundry Local SDK message format")?;
    Ok(messages)
}

/// Build OpenAI-compatible function tools from ragent [`ToolDefinition`]s.
fn build_tools(tools: &[ToolDefinition]) -> Vec<ChatCompletionTools> {
    if tools.is_empty() {
        return Vec::new();
    }

    let json_tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                }
            })
        })
        .collect();

    serde_json::from_value(Value::Array(json_tools)).unwrap_or_default()
}

/// Resolve a model alias or variant id, ensure it is cached, and load it in-process.
async fn ensure_model_ready(
    manager: &'static foundry_local_sdk::FoundryLocalManager,
    model_id_or_alias: &str,
    device: Option<&DeviceType>,
    event_bus: Option<Arc<EventBus>>,
    session_id: &str,
) -> Result<Arc<SdkModel>> {
    let catalog = manager.catalog();

    let model: Arc<SdkModel> = match catalog.get_model_variant(model_id_or_alias).await {
        Ok(m) => m,
        Err(_) => catalog
            .get_model(model_id_or_alias)
            .await
            .with_context(|| {
                format!(
                    "Model '{model_id_or_alias}' is not known to Microsoft Foundry Local. \
                     Use `foundry model list` to see available models, \
                     or `foundry model download {model_id_or_alias}` to add it."
                )
            })?,
    };

    let model = if model.variants().len() > 1 {
        select_best_variant(&model, model_id_or_alias, device).await?
    } else {
        model
    };

    let resolved_id = strip_model_version_suffix(&model.info().id).to_string();

    if !model.is_cached().await? {
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Downloading Microsoft Foundry Local model for in-process inference"
        );
        publish_download_event(
            &event_bus,
            Event::ModelDownloadStarted {
                provider_id: "foundry_local".to_string(),
                model_id: resolved_id.clone(),
                session_id: session_id.to_string(),
            },
        );

        let bus = event_bus.clone();
        let provider_id = "foundry_local".to_string();
        let model_id_for_event = resolved_id.clone();
        let session_id_for_event = session_id.to_string();
        model
            .download(Some(move |pct: f64| {
                let percent = pct as f32;
                tracing::info!(percent = pct, "Foundry Local in-process download progress");
                if let Some(ref b) = bus {
                    b.publish(Event::ModelDownloadProgress {
                        provider_id: provider_id.clone(),
                        model_id: model_id_for_event.clone(),
                        session_id: session_id_for_event.clone(),
                        percent,
                    });
                }
            }))
            .await
            .with_context(|| {
                format!(
                    "Failed to download model '{model_id_or_alias}' (resolved to '{resolved_id}') for in-process inference"
                )
            })?;

        publish_download_event(
            &event_bus,
            Event::ModelDownloadFinished {
                provider_id: "foundry_local".to_string(),
                model_id: resolved_id.clone(),
                session_id: session_id.to_string(),
                error: None,
            },
        );
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Microsoft Foundry Local model downloaded for in-process inference"
        );
    }

    // For in-process inference we load the model into the native core via the
    // SDK's `Model::load()`.  This is the opposite of the web-service path,
    // which deliberately avoided `load()` because inference happened in the
    // web service process.
    if !model.is_loaded().await? {
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Loading Microsoft Foundry Local model in-process"
        );
        model
            .load()
            .await
            .with_context(|| {
                format!(
                    "Failed to load model '{model_id_or_alias}' (resolved to '{resolved_id}') in-process. \
                     The native core library may be missing or incompatible."
                )
            })?;
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Microsoft Foundry Local model loaded in-process"
        );
    } else {
        tracing::info!(
            requested = %model_id_or_alias,
            resolved = %resolved_id,
            "Microsoft Foundry Local model already loaded in-process"
        );
    }

    Ok(model)
}

/// Pick a variant from an alias group, optionally constrained by device type.
///
/// Prefers a variant matching the requested device, then a cached variant, then
/// the first available variant.
async fn select_best_variant(
    model: &SdkModel,
    model_id_or_alias: &str,
    device: Option<&DeviceType>,
) -> Result<Arc<SdkModel>> {
    let variants = model.variants();

    // 1. Try to find a variant whose runtime matches the requested device.
    if let Some(requested) = device {
        let matching: Vec<Arc<SdkModel>> = variants
            .iter()
            .filter(|v| {
                v.info()
                    .runtime
                    .as_ref()
                    .map(|r| r.device_type == *requested)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        if !matching.is_empty() {
            if let Some(selected) = pick_cached_variant(&matching)
                .await
                .or_else(|| matching.first().cloned())
            {
                model.select_variant(selected.as_ref())?;
                return Ok(Arc::new((*selected).clone()));
            }
        }
    }

    // 2. Otherwise fall back to the same logic as the web-service path.
    let selected = pick_cached_variant(&variants)
        .await
        .or_else(|| variants.first().cloned())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Model '{model_id_or_alias}' has no variants available in the Foundry Local catalog"
            )
        })?;

    model
        .select_variant(selected.as_ref())
        .with_context(|| format!("Failed to select variant for '{model_id_or_alias}'"))?;

    Ok(Arc::new((*selected).clone()))
}

/// Return the first cached variant in the list, if any.
async fn pick_cached_variant(variants: &[Arc<SdkModel>]) -> Option<Arc<SdkModel>> {
    for v in variants {
        if v.is_cached().await.unwrap_or(false) {
            return Some(Arc::clone(v));
        }
    }
    None
}

/// Strip a trailing `:N` version suffix from a Foundry Local model id.
///
/// The OpenAI-compatible chat endpoint expects the unsuffixed base id, while
/// the SDK catalog stores variant ids with the version suffix.
fn strip_model_version_suffix(id: &str) -> &str {
    id.rsplit_once(':')
        .and_then(|(base, suffix)| {
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                Some(base)
            } else {
                None
            }
        })
        .unwrap_or(id)
}

/// Publish a download-related event if an event bus is available.
fn publish_download_event(event_bus: &Option<Arc<EventBus>>, event: Event) {
    if let Some(bus) = event_bus {
        bus.publish(event);
    }
}

/// Translate the SDK's `ChatCompletionStream` into ragent `StreamEvent`s.
fn translate_stream<S>(sdk_stream: S) -> impl futures::Stream<Item = StreamEvent>
where
    S: futures::Stream<
            Item = Result<CreateChatCompletionStreamResponse, foundry_local_sdk::FoundryLocalError>,
        > + Send
        + 'static,
{
    async_stream::stream! {
        let mut tool_call_ids: HashMap<u32, String> = HashMap::new();
        let mut tool_call_names: HashMap<u32, String> = HashMap::new();
        let mut yielded_event = false;

        futures::pin_mut!(sdk_stream);

        while let Some(chunk_result) = sdk_stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    let message = format!("Foundry Local in-process stream error: {e}");
                    yield StreamEvent::Error { message };
                    break;
                }
            };

            // Usage may appear on any chunk, most commonly the last one.
            if let Some(usage) = chunk.usage {
                let input_tokens = u64::from(usage.prompt_tokens);
                let output_tokens = u64::from(usage.completion_tokens);
                if input_tokens > 0 || output_tokens > 0 {
                    yield StreamEvent::Usage { input_tokens, output_tokens };
                    yielded_event = true;
                }
            }

            for choice in chunk.choices {
                let delta = choice.delta;

                if let Some(content) = delta.content.filter(|c| !c.is_empty()) {
                    yield StreamEvent::TextDelta { text: content };
                    yielded_event = true;
                }

                if let Some(tool_calls) = delta.tool_calls {
                    for tc in tool_calls {
                        let index = tc.index;

                        if let Some(id) = tc.id.as_ref() {
                            tool_call_ids.insert(index, id.clone());
                        }

                        if let Some(function) = tc.function.as_ref() {
                            if let Some(name) = function.name.as_ref() {
                                let tc_id = tool_call_ids.get(&index).cloned().unwrap_or_else(|| format!("tc_{index}"));
                                tool_call_names.insert(index, name.clone());
                                yield StreamEvent::ToolCallStart {
                                    id: tc_id,
                                    name: name.clone(),
                                };
                                yielded_event = true;
                            }

                            if let Some(args) = function.arguments.as_ref().filter(|a| !a.is_empty()) {
                                let tc_id = tool_call_ids.get(&index).cloned().unwrap_or_else(|| format!("tc_{index}"));
                                yield StreamEvent::ToolCallDelta {
                                    id: tc_id,
                                    args_json: args.clone(),
                                };
                                yielded_event = true;
                            }
                        }
                    }
                }

                if let Some(finish_reason) = choice.finish_reason {
                    for id in tool_call_ids.values().cloned().collect::<Vec<_>>() {
                        yield StreamEvent::ToolCallEnd { id };
                    }
                    tool_call_ids.clear();
                    tool_call_names.clear();

                    yield StreamEvent::Finish {
                        reason: map_finish_reason(finish_reason),
                    };
                    yielded_event = true;
                }
            }
        }

        if !yielded_event {
            tracing::warn!("Foundry Local in-process stream ended without yielding any events");
            yield StreamEvent::Error {
                message: "Foundry Local in-process response stream ended without producing any events. \
                          The model may not have loaded correctly.".to_string(),
            };
        }
    }
}

/// Map the SDK/async_openai `FinishReason` to ragent's `FinishReason`.
fn map_finish_reason(reason: SdkFinishReason) -> FinishReason {
    match reason {
        SdkFinishReason::ToolCalls | SdkFinishReason::FunctionCall => FinishReason::ToolUse,
        SdkFinishReason::Length => FinishReason::Length,
        SdkFinishReason::ContentFilter => FinishReason::ContentFilter,
        SdkFinishReason::Stop => FinishReason::Stop,
    }
}

/// Map a ragent configuration device string to a `foundry-local-sdk` `DeviceType`.
pub fn device_type_from_str(device: &str) -> Result<DeviceType> {
    match device {
        "auto" => Ok(DeviceType::Invalid), // interpreted as "no preference" elsewhere
        "cpu" => Ok(DeviceType::CPU),
        "gpu" => Ok(DeviceType::GPU),
        "npu" => Ok(DeviceType::NPU),
        _ => bail!("Invalid Foundry Local device '{device}'. Must be one of: auto, cpu, gpu, npu"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatMessage, ChatRequest};

    #[test]
    fn test_device_type_mapping() {
        assert_eq!(device_type_from_str("cpu").unwrap(), DeviceType::CPU);
        assert_eq!(device_type_from_str("gpu").unwrap(), DeviceType::GPU);
        assert_eq!(device_type_from_str("npu").unwrap(), DeviceType::NPU);
        assert_eq!(device_type_from_str("auto").unwrap(), DeviceType::Invalid);
        assert!(device_type_from_str("cuda").is_err());
    }

    #[test]
    fn test_build_messages_text_only() {
        let request = ChatRequest {
            model: "phi-4".to_string(),
            messages: Arc::new(vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: ChatContent::Text("s".to_string()),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: ChatContent::Text("u".to_string()),
                },
            ]),
            system: None,
            ..default_request()
        };
        let messages = build_messages(&request).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_build_messages_with_system_override() {
        let request = ChatRequest {
            model: "phi-4".to_string(),
            system: Some("override".to_string()),
            messages: Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("u".to_string()),
            }]),
            ..default_request()
        };
        let messages = build_messages(&request).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_build_tools_empty() {
        let tools = build_tools(&[]);
        assert!(tools.is_empty());
    }

    #[test]
    fn test_build_tools_one() {
        let tools = build_tools(&[ToolDefinition {
            name: "foo".to_string(),
            description: "bar".to_string(),
            parameters: json!({"type": "object"}),
        }]);
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn test_strip_model_version_suffix() {
        assert_eq!(
            strip_model_version_suffix("qwen3-0.6b-generic-cpu:4"),
            "qwen3-0.6b-generic-cpu"
        );
        assert_eq!(strip_model_version_suffix("phi-4"), "phi-4");
    }

    #[test]
    fn test_map_finish_reason() {
        assert_eq!(map_finish_reason(SdkFinishReason::Stop), FinishReason::Stop);
        assert_eq!(
            map_finish_reason(SdkFinishReason::Length),
            FinishReason::Length
        );
        assert_eq!(
            map_finish_reason(SdkFinishReason::ToolCalls),
            FinishReason::ToolUse
        );
    }

    #[test]
    #[allow(deprecated)]
    fn test_translate_stream_text_delta_and_finish() {
        use futures::StreamExt;

        let chunk = CreateChatCompletionStreamResponse {
            id: String::new(),
            choices: vec![foundry_local_sdk::ChatChoiceStream {
                index: 0,
                delta: foundry_local_sdk::ChatCompletionStreamResponseDelta {
                    content: Some("Hello".to_string()),
                    function_call: None,
                    tool_calls: None,
                    role: None,
                    refusal: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            created: 0,
            model: String::new(),
            service_tier: None,
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: None,
        };
        let finish_chunk = CreateChatCompletionStreamResponse {
            id: String::new(),
            choices: vec![foundry_local_sdk::ChatChoiceStream {
                index: 0,
                delta: foundry_local_sdk::ChatCompletionStreamResponseDelta {
                    content: None,
                    function_call: None,
                    tool_calls: None,
                    role: None,
                    refusal: None,
                },
                finish_reason: Some(SdkFinishReason::Stop),
                logprobs: None,
            }],
            created: 0,
            model: String::new(),
            service_tier: None,
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
            usage: Some(foundry_local_sdk::CompletionUsage {
                prompt_tokens: 5,
                completion_tokens: 1,
                total_tokens: 6,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
        };

        let stream = futures::stream::iter(vec![Ok(chunk), Ok(finish_chunk)]);
        let events: Vec<StreamEvent> =
            futures::executor::block_on(async { translate_stream(stream).collect().await });

        assert!(
            events
                .iter()
                .any(|e| matches!(e, StreamEvent::TextDelta { text } if text == "Hello")),
            "expected TextDelta"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                StreamEvent::Usage {
                    input_tokens: 5,
                    output_tokens: 1
                }
            )),
            "expected Usage"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                StreamEvent::Finish {
                    reason: FinishReason::Stop
                }
            )),
            "expected Finish(Stop)"
        );
    }

    fn default_request() -> ChatRequest {
        ChatRequest {
            model: String::new(),
            messages: Arc::new(Vec::new()),
            tools: Arc::new(Vec::new()),
            temperature: None,
            top_p: None,
            max_tokens: None,
            system: None,
            options: HashMap::new(),
            session_id: None,
            request_id: None,
            stream_timeout_secs: None,
            thinking: None,
        }
    }
}
