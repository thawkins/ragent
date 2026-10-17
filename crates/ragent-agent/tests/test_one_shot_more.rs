//! Additional tests-only coverage for the one-shot LLM dispatch entry point
//! (`ragent_agent::one_shot::send_one_shot`).
//!
//! The sibling `test_one_shot.rs` covers only the unknown-provider rejection.
//! Here a scripted in-process provider (no network, no real LLM) drives the
//! remaining branches: the `router` and `ollama` short-circuits of
//! `resolve_api_key`, a text-stream happy path, an unexpected-event warn, the
//! stream-error bail, and a provider whose client cannot be built.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::ModelRef;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::send_one_shot;
use ragent_config::{Capabilities, Cost};
use serde_json::Value;

/// A scripted reply list replayed in order by the fake client.
#[derive(Clone)]
enum Reply {
    /// Text deltas then an unexpected `Usage` then a terminal `Finish`.
    Text(String),
    /// The stream emits an error mid-way.
    StreamError(String),
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<Reply>>>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let reply = self
            .replies
            .lock()
            .expect("replies lock")
            .pop()
            .unwrap_or(Reply::Text(String::new()));
        match reply {
            Reply::Text(text) => {
                let events: [StreamEvent; 4] = [
                    StreamEvent::TextDelta { text: text.clone() },
                    StreamEvent::Usage {
                        input_tokens: 1,
                        output_tokens: 2,
                    },
                    StreamEvent::TextDelta {
                        text: "-tail".to_string(),
                    },
                    StreamEvent::Finish {
                        reason: LlmFinishReason::Stop,
                    },
                ];
                Ok(Box::pin(stream::iter(events)))
            }
            Reply::StreamError(message) => {
                let events: [StreamEvent; 2] = [
                    StreamEvent::TextDelta {
                        text: "partial".to_string(),
                    },
                    StreamEvent::Error { message },
                ];
                Ok(Box::pin(stream::iter(events)))
            }
        }
    }
}

struct ScriptedProvider {
    id: &'static str,
    replies: Arc<Mutex<Vec<Reply>>>,
    fail_client: bool,
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &str {
        self.id
    }

    fn name(&self) -> &'static str {
        "Scripted one-shot provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "scripted-1".to_string(),
            provider_id: self.id.to_string(),
            name: "Scripted".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                vision: false,
                tool_use: false,
                thinking_levels: vec![],
                reasoning: false,
                streaming: true,
            },
            context_window: 8192,
            max_output: Some(1024),
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        if self.fail_client {
            anyhow::bail!("scripted client construction failure");
        }
        Ok(Box::new(ScriptedClient {
            replies: Arc::clone(&self.replies),
        }))
    }
}

fn registry(id: &'static str, replies: Vec<Reply>, fail_client: bool) -> Arc<ProviderRegistry> {
    let mut reg = ProviderRegistry::new();
    reg.register(Box::new(ScriptedProvider {
        id,
        replies: Arc::new(Mutex::new(replies)),
        fail_client,
    }));
    Arc::new(reg)
}

fn model(provider: &str) -> ModelRef {
    ModelRef {
        provider_id: provider.to_string(),
        model_id: "scripted-1".to_string(),
    }
}

#[tokio::test]
async fn send_one_shot_router_provider_collects_text_deltas() {
    // The `router` provider id short-circuits `resolve_api_key` to an empty key;
    // the client streams text deltas (with an interleaved unexpected event) and
    // the function returns their concatenation.
    let registry = registry("router", vec![Reply::Text("hello".to_string())], false);
    let out = send_one_shot(
        registry,
        None,
        model("router"),
        Some("you are a helper".to_string()),
        "hi".to_string(),
        Some(64),
    )
    .await
    .expect("the scripted stream must succeed");
    assert_eq!(out, "hello-tail");
}

#[tokio::test]
async fn send_one_shot_ollama_provider_reads_the_env_key_branch() {
    // The `ollama` id resolves its key from `OLLAMA_API_KEY` (absent -> empty),
    // exercising that branch of `resolve_api_key`.
    let registry = registry("ollama", vec![Reply::Text("ok".to_string())], false);
    let out = send_one_shot(
        registry,
        None,
        model("ollama"),
        None,
        "prompt".to_string(),
        None,
    )
    .await
    .expect("the ollama branch must reach the client");
    assert_eq!(out, "ok-tail");
}

#[tokio::test]
async fn send_one_shot_surfaces_a_stream_error() {
    let registry = registry(
        "router",
        vec![Reply::StreamError("provider hiccup".to_string())],
        false,
    );
    let err = send_one_shot(registry, None, model("router"), None, "x".to_string(), None)
        .await
        .expect_err("a stream error must abort the call");
    assert!(err.to_string().contains("LLM stream error"), "err: {err}");
}

#[tokio::test]
async fn send_one_shot_surfaces_a_client_construction_failure() {
    let registry = registry("router", Vec::new(), true);
    let err = send_one_shot(registry, None, model("router"), None, "x".to_string(), None)
        .await
        .expect_err("a client build failure must surface");
    assert!(
        err.to_string().contains("failed to create LLM client"),
        "err: {err}"
    );
}

#[tokio::test]
async fn send_one_shot_registered_provider_without_env_key_errors() {
    // A real provider id with no env mapping and no storage -> key resolution
    // fails before any client is built. `huggingface` reads HUGGINGFACE_API_KEY,
    // which is not set in the test environment.
    if std::env::var("HUGGINGFACE_API_KEY").is_ok() {
        return;
    }
    let mut reg = ProviderRegistry::new();
    reg.register(Box::new(ragent_llm::HuggingFaceProvider));
    let registry = Arc::new(reg);
    let err = send_one_shot(
        registry,
        None,
        ModelRef {
            provider_id: "huggingface".to_string(),
            model_id: "meta-llama/Llama-3-8B".to_string(),
        },
        None,
        "hello".to_string(),
        None,
    )
    .await
    .expect_err("a missing key must be reported");
    assert!(err.to_string().contains("No API key found"), "err: {err}");
}
