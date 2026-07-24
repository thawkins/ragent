//! Tests for `RouterClient` prompt routing and provider delegation.
//!
//! These tests verify that the router client classifies prompts into the
//! correct tier (bucket), resolves the primary model for that tier, and
//! delegates the request to a downstream provider's client.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, LlmClient, StreamEvent};
use ragent_llm::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_llm::providers::mock_llm_client::{MockLlmClient, MockScenario};
use ragent_llm::providers::router_client::RouterClient;
use ragent_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};

/// A fake provider that always returns a `MockLlmClient` and records the
/// provider/model it was asked to serve.
struct FakeProvider {
    id: &'static str,
    name: &'static str,
    seen_model: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    models: Vec<ModelInfo>,
}

impl FakeProvider {
    const fn with_models(
        id: &'static str,
        name: &'static str,
        seen_model: std::sync::Arc<std::sync::Mutex<Option<String>>>,
        models: Vec<ModelInfo>,
    ) -> Self {
        Self {
            id,
            name,
            seen_model,
            models,
        }
    }
}

#[async_trait::async_trait]
impl Provider for FakeProvider {
    fn id(&self) -> &str {
        self.id
    }

    fn name(&self) -> &str {
        self.name
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        self.models.clone()
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        if let Some(Value::String(model)) = options.get("model_id") {
            *self.seen_model.lock().unwrap() = Some(model.clone());
        }
        Ok(Box::new(MockLlmClient::with_scenario(
            MockScenario::SimpleTextReply,
        )))
    }
}

fn model_info(id: &str, vision: bool) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        provider_id: "fake".to_string(),
        name: id.to_string(),
        cost: ragent_config::Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: ragent_config::Capabilities {
            reasoning: false,
            streaming: true,
            vision,
            tool_use: true,
            thinking_levels: Vec::new(),
        },
        context_window: 128_000,
        max_output: None,
        request_multiplier: None,
        thinking_config: None,
    }
}

/// Build a `RouterConfig` with a single known model per tier so the selected
/// model is deterministic and easy to assert against.
fn config_with_known_models() -> RouterConfig {
    let mut config = RouterConfig::default();
    let make_tier = |tier: Tier, provider: &str, model: &str| {
        (
            tier.to_string(),
            TierConfig {
                models: vec![TierEntry {
                    provider: provider.to_string(),
                    model: model.to_string(),
                }],
                timeout_ms: None,
            },
        )
    };
    config.tiers.insert(
        make_tier(Tier::Simple, "fake", "qwen2.5:1.5b").0,
        make_tier(Tier::Simple, "fake", "qwen2.5:1.5b").1,
    );
    config.tiers.insert(
        make_tier(Tier::Medium, "fake", "gpt-4.1-mini").0,
        make_tier(Tier::Medium, "fake", "gpt-4.1-mini").1,
    );
    config.tiers.insert(
        make_tier(Tier::Complex, "fake", "claude-sonnet-4-20250514").0,
        make_tier(Tier::Complex, "fake", "claude-sonnet-4-20250514").1,
    );
    config.tiers.insert(
        make_tier(Tier::Reasoning, "fake", "claude-opus-4-20250514").0,
        make_tier(Tier::Reasoning, "fake", "claude-opus-4-20250514").1,
    );
    config
}

fn registry_with_fake_provider() -> (Arc<ProviderRegistry>, Arc<std::sync::Mutex<Option<String>>>) {
    let mut registry = ProviderRegistry::new();
    let seen_model = Arc::new(std::sync::Mutex::new(None));
    registry.register(Box::new(FakeProvider::with_models(
        "fake",
        "Fake Provider",
        Arc::clone(&seen_model),
        vec![],
    )));
    (Arc::new(registry), seen_model)
}

fn registry_with_fake_models(
    models: Vec<ModelInfo>,
) -> (Arc<ProviderRegistry>, Arc<std::sync::Mutex<Option<String>>>) {
    let mut registry = ProviderRegistry::new();
    let seen_model = Arc::new(std::sync::Mutex::new(None));
    registry.register(Box::new(FakeProvider::with_models(
        "fake",
        "Fake Provider",
        Arc::clone(&seen_model),
        models,
    )));
    (Arc::new(registry), seen_model)
}

fn request_with_prompt(text: &str) -> ChatRequest {
    ChatRequest {
        model: "router".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(text.to_string()),
        }]),
        tools: Arc::new(vec![]),
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

async fn collect_events(
    mut stream: std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>,
) -> Vec<StreamEvent> {
    use futures::StreamExt;
    let mut collected = Vec::new();
    while let Some(event) = stream.next().await {
        collected.push(event);
    }
    collected
}

/// A trivial prompt should classify into SIMPLE and delegate to the simple-tier
/// model, rewriting the request `model` field to the downstream model id.
#[tokio::test]
async fn test_router_simple_prompt_delegates_to_downstream_model() {
    let (registry, seen_model) = registry_with_fake_provider();
    let client = RouterClient::new(config_with_known_models(), Some(registry), None);
    let result = client.chat(request_with_prompt("hi")).await;
    assert!(
        result.is_ok(),
        "router chat should delegate, got: {:?}",
        result.as_ref().err()
    );
    let events = collect_events(result.unwrap()).await;
    assert!(!events.is_empty(), "downstream mock should emit events");
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("qwen2.5:1.5b"),
        "router should rewrite request model to downstream model id"
    );
}

/// A reasoning-heavy prompt should classify into REASONING when the classifier
/// is configured with strong reasoning-weighting boundaries and delegate to the
/// reasoning-tier model.
#[tokio::test]
async fn test_router_reasoning_prompt_delegates_to_reasoning_model() {
    let mut config = config_with_known_models();
    // Lower the reasoning boundary so this proof prompt crosses into REASONING.
    config.boundaries.medium_complex = 0.05;
    config.boundaries.complex_reasoning = 0.10;
    let (registry, seen_model) = registry_with_fake_provider();
    let client = RouterClient::new(config, Some(registry), None);
    let result = client
        .chat(request_with_prompt("prove the infinitude of primes"))
        .await;
    assert!(
        result.is_ok(),
        "router chat should delegate, got: {:?}",
        result.as_ref().err()
    );
    let events = collect_events(result.unwrap()).await;
    assert!(!events.is_empty(), "downstream mock should emit events");
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("claude-opus-4-20250514"),
        "router should select the reasoning-tier model"
    );
}

/// When a tier has no models, the router falls back to other tiers before giving
/// up. A SIMPLE prompt with an empty SIMPLE tier should still find the MEDIUM
/// tier's model.
#[tokio::test]
async fn test_router_empty_tier_falls_back_to_other_tiers() {
    let mut config = config_with_known_models();
    // Empty the SIMPLE tier so the router must fall back to MEDIUM.
    config.tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models: vec![],
            timeout_ms: None,
        },
    );
    let (registry, seen_model) = registry_with_fake_provider();
    let client = RouterClient::new(config, Some(registry), None);
    let result = client.chat(request_with_prompt("hi")).await;
    assert!(
        result.is_ok(),
        "router should fall back to another tier, got: {:?}",
        result.as_ref().err()
    );
    let events = collect_events(result.unwrap()).await;
    assert!(!events.is_empty(), "downstream mock should emit events");
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("gpt-4.1-mini"),
        "router should fall back to the medium-tier model"
    );
}

/// When every tier is empty, the router reports that no suitable model is
/// configured.
#[tokio::test]
async fn test_router_all_empty_tiers_reports_no_model() {
    let mut config = RouterConfig::default();
    for tier in Tier::all() {
        config.tiers.insert(
            tier.to_string(),
            TierConfig {
                models: vec![],
                timeout_ms: None,
            },
        );
    }
    let (registry, _seen_model) = registry_with_fake_provider();
    let client = RouterClient::new(config, Some(registry), None);
    let result = client.chat(request_with_prompt("hi")).await;
    let msg = match result {
        Ok(_) => panic!("router chat should fail when every tier is empty"),
        Err(e) => e.to_string(),
    };
    assert!(
        msg.contains("no suitable model"),
        "error should indicate no model is configured, got: {msg}"
    );
}

/// A `/reasoning` modifier should force the REASONING tier regardless of the
/// underlying prompt complexity, and the selected model should be the
/// reasoning-tier model.
#[tokio::test]
async fn test_router_reasoning_modifier_delegates_to_reasoning_model() {
    let (registry, seen_model) = registry_with_fake_provider();
    let client = RouterClient::new(config_with_known_models(), Some(registry), None);
    let result = client
        .chat(request_with_prompt("/reasoning explain rust lifetimes"))
        .await;
    assert!(
        result.is_ok(),
        "router chat should delegate, got: {:?}",
        result.as_ref().err()
    );
    let events = collect_events(result.unwrap()).await;
    assert!(!events.is_empty(), "downstream mock should emit events");
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("claude-opus-4-20250514"),
        "router should select the reasoning-tier model for /reasoning modifier"
    );
}

/// A request with an image attachment should set `requires_vision` and cause
/// the router to skip non-vision models in the resolved tier.
#[tokio::test]
async fn test_router_image_attachment_requires_vision_model() {
    use ragent_llm::llm::ContentPart;
    use ragent_llm::providers::router_client::extract_attachments;

    let request = ChatRequest {
        model: "router".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![
                ContentPart::ImageUrl {
                    url: "data:image/png;base64,iVBORw0KGgo=".to_string(),
                },
                ContentPart::Text {
                    text: "What is in this image?".to_string(),
                },
            ]),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let attachments = extract_attachments(&request);
    assert!(
        attachments.has_media(),
        "image attachment should be detected"
    );
    assert_eq!(attachments.image_count, 1);

    let config = config_with_known_models();
    let (registry, seen_model) = registry_with_fake_models(vec![
        model_info("qwen2.5:1.5b", false),
        model_info("gpt-4.1-mini", false),
        model_info("claude-sonnet-4-20250514", true),
        model_info("claude-opus-4-20250514", true),
    ]);

    let client = RouterClient::new(config, Some(registry), None);
    let result = client.chat(request).await;
    assert!(
        result.is_ok(),
        "router chat should delegate to vision model, got: {:?}",
        result.as_ref().err()
    );
    let _ = collect_events(result.unwrap()).await;
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("claude-sonnet-4-20250514"),
        "router should select the vision-capable COMPLEX tier model"
    );
}

/// A continuation call (last user message is tool results, not a fresh prompt)
/// should reuse the previously selected model and NOT re-classify.
///
/// The agent loop appends tool results as a "user"-role message after every
/// tool execution. Without the continuation guard, the router would
/// re-classify the tool output and re-log on every iteration.
#[tokio::test]
async fn test_router_continuation_reuses_cached_model_without_reclassifying() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Track how many times the downstream provider's create_client is called.
    // The router calls create_client once per chat() invocation.
    let create_count = Arc::new(AtomicUsize::new(0));

    struct CountingProvider {
        id: &'static str,
        name: &'static str,
        seen_model: Arc<std::sync::Mutex<Option<String>>>,
        create_count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Provider for CountingProvider {
        fn id(&self) -> &str {
            self.id
        }
        fn name(&self) -> &str {
            self.name
        }
        fn default_models(&self) -> Vec<ModelInfo> {
            vec![]
        }
        fn as_any_static(&self) -> &dyn std::any::Any {
            self
        }
        async fn create_client(
            &self,
            _api_key: &str,
            _base_url: Option<&str>,
            options: &HashMap<String, Value>,
        ) -> Result<Box<dyn LlmClient>> {
            self.create_count.fetch_add(1, Ordering::SeqCst);
            if let Some(Value::String(model)) = options.get("model_id") {
                *self.seen_model.lock().unwrap() = Some(model.clone());
            }
            Ok(Box::new(MockLlmClient::with_scenario(
                MockScenario::SimpleTextReply,
            )))
        }
    }

    let mut registry = ProviderRegistry::new();
    let seen_model = Arc::new(std::sync::Mutex::new(None));
    let create_count = Arc::clone(&create_count);
    registry.register(Box::new(CountingProvider {
        id: "fake",
        name: "Fake Provider",
        seen_model: Arc::clone(&seen_model),
        create_count: Arc::clone(&create_count),
    }));
    let registry = Arc::new(registry);

    let client = RouterClient::new(config_with_known_models(), Some(registry), None);

    // First call: a fresh user prompt — should classify into SIMPLE and
    // delegate to the simple-tier model.
    let result = client.chat(request_with_prompt("hi")).await;
    assert!(result.is_ok(), "first call should succeed");
    let _ = collect_events(result.unwrap()).await;
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("qwen2.5:1.5b"),
        "first call should select the simple-tier model"
    );

    // Second call: a continuation — last user message is tool results.
    // The router should reuse the cached simple-tier model WITHOUT
    // re-classifying. We verify this by checking the downstream model is
    // still the simple-tier one.
    let tool_result_msg = ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![ragent_llm::llm::ContentPart::ToolResult {
            tool_use_id: "tool-1".to_string(),
            content: "result of the tool".into(),
        }]),
    };
    let mut messages = vec![ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text("hi".to_string()),
    }];
    messages.push(ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Text("let me check".to_string()),
    });
    messages.push(tool_result_msg);

    let continuation_request = ChatRequest {
        model: "router".to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let result = client.chat(continuation_request).await;
    assert!(result.is_ok(), "continuation call should succeed");
    let _ = collect_events(result.unwrap()).await;
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("qwen2.5:1.5b"),
        "continuation should reuse the cached simple-tier model"
    );

    // If the router re-classified the tool-result "prompt", the classifier
    // would likely pick a different tier (tool output is not a simple "hi").
    // The fact that we still get qwen2.5:1.5b proves the cached decision was
    // reused.
}

/// Regression test: a text-only follow-up in a conversation that previously
/// contained an image should not force the router to keep using a vision model.
///
/// The router's `extract_attachments` used to scan the entire message history,
/// so once an image appeared in the conversation every subsequent request was
/// treated as `requires_vision`, even when the current user prompt had no
/// attachment. The non-vision model is listed first in the MEDIUM tier and
/// should be selected for a plain-text prompt.
#[tokio::test]
async fn test_router_text_followup_after_image_uses_non_vision_model() {
    let mut config = RouterConfig::default();
    config.tiers.insert(
        Tier::Medium.to_string(),
        TierConfig {
            models: vec![
                TierEntry {
                    provider: "fake".to_string(),
                    model: "glm-5.2".to_string(),
                },
                TierEntry {
                    provider: "fake".to_string(),
                    model: "glm-5.2-vision".to_string(),
                },
            ],
            timeout_ms: None,
        },
    );

    let (registry, seen_model) = registry_with_fake_models(vec![
        model_info("glm-5.2", false),
        model_info("glm-5.2-vision", true),
    ]);
    let client = RouterClient::new(config, Some(registry), None);

    // First turn: image prompt selects the vision model.
    let image_request = ChatRequest {
        model: "router".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![
                ContentPart::ImageUrl {
                    url: "data:image/png;base64,iVBORw0KGgo=".to_string(),
                },
                ContentPart::Text {
                    text: "What is in this image?".to_string(),
                },
            ]),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let result = client.chat(image_request).await;
    assert!(
        result.is_ok(),
        "image chat should delegate, got: {:?}",
        result.err()
    );
    let _ = collect_events(result.unwrap()).await;
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("glm-5.2-vision"),
        "image prompt should select the vision-capable model"
    );

    // Second turn: text-only follow-up in the same conversation history.
    let text_request = ChatRequest {
        model: "router".to_string(),
        messages: Arc::new(vec![
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![
                    ContentPart::ImageUrl {
                        url: "data:image/png;base64,iVBORw0KGgo=".to_string(),
                    },
                    ContentPart::Text {
                        text: "What is in this image?".to_string(),
                    },
                ]),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text("It shows a cat.".to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Summarise Rust ownership.".to_string()),
            },
        ]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    let result = client.chat(text_request).await;
    assert!(
        result.is_ok(),
        "text follow-up chat should delegate, got: {:?}",
        result.err()
    );
    let _ = collect_events(result.unwrap()).await;
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("glm-5.2"),
        "text-only follow-up should revert to the non-vision model listed first in the tier"
    );
}

/// A fake provider with an empty static catalog and a dynamic `discover_models`
/// implementation. Used to test the router's vision fallback path for providers
/// such as Ollama Cloud whose capabilities are only known after async discovery.
struct DynamicFakeProvider {
    seen_model: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    models: Vec<ModelInfo>,
}

#[async_trait::async_trait]
impl Provider for DynamicFakeProvider {
    fn id(&self) -> &'static str {
        "dynamic_fake"
    }

    fn name(&self) -> &'static str {
        "Dynamic Fake"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        Vec::new()
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }

    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(self.models.clone())
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        if let Some(Value::String(model)) = options.get("model_id") {
            *self.seen_model.lock().unwrap() = Some(model.clone());
        }
        Ok(Box::new(MockLlmClient::with_scenario(
            MockScenario::SimpleTextReply,
        )))
    }
}

fn image_request_with_prompt(text: &str) -> ChatRequest {
    ChatRequest {
        model: "router".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![
                ContentPart::ImageUrl {
                    url: "data:image/png;base64,iVBORw0KGgo=".to_string(),
                },
                ContentPart::Text {
                    text: text.to_string(),
                },
            ]),
        }]),
        tools: Arc::new(vec![]),
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

/// Regression: when a vision prompt classifies into a tier that has no
/// vision-capable models, the router must fall back to another tier whose
/// model is provided by a dynamic-discovery provider. Before this fix the
/// router only warmed the resolved tier, so the fallback tier's model could
/// not be resolved and the request failed with "no suitable model is
/// configured".
#[tokio::test]
async fn test_router_vision_fallback_warms_all_tiers_for_dynamic_providers() {
    let mut config = RouterConfig::default();
    // Force the prompt into MEDIUM so the router has to fall back to SIMPLE.
    config.boundaries.simple_medium = 0.99;
    config.tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models: vec![TierEntry {
                provider: "dynamic_fake".to_string(),
                model: "kimi-k2.7-code".to_string(),
            }],
            timeout_ms: None,
        },
    );
    // MEDIUM is intentionally empty — this is where classification lands.
    config.tiers.insert(
        Tier::Medium.to_string(),
        TierConfig {
            models: vec![],
            timeout_ms: None,
        },
    );

    let mut registry = ProviderRegistry::new();
    let seen_model = Arc::new(std::sync::Mutex::new(None));
    registry.register(Box::new(DynamicFakeProvider {
        seen_model: Arc::clone(&seen_model),
        models: vec![model_info("kimi-k2.7-code", true)],
    }));
    let registry = Arc::new(registry);

    let client = RouterClient::new(config, Some(registry), None);
    let result = client
        .chat(image_request_with_prompt("describe this"))
        .await;
    assert!(
        result.is_ok(),
        "router should fall back to the SIMPLE tier's discovered vision model, got: {:?}",
        result.as_ref().err()
    );
    let _ = collect_events(result.unwrap()).await;
    assert_eq!(
        seen_model.lock().unwrap().as_deref(),
        Some("kimi-k2.7-code"),
        "fallback should select the dynamic provider's vision model"
    );
}
