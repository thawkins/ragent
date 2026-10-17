//! Coverage for the registry surface and the model-discovery HTTP paths of
//! the Anthropic, Gemini, Bedrock, and OpenAI providers, driven against local
//! one-shot mock servers (no real network egress).

use std::collections::HashMap;

use ragent_llm::provider::anthropic::list_anthropic_models;
use ragent_llm::provider::bedrock::discover_bedrock_models;
use ragent_llm::provider::bedrock_credentials::AwsCredentials;
use ragent_llm::provider::gemini::list_gemini_models;
use ragent_llm::provider::{Provider, ProviderRegistry, create_default_registry};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

async fn spawn(status: u16, content_type: &str, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let ct = content_type.to_string();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

// ---------------------------------------------------------------------------
// Registry surface
// ---------------------------------------------------------------------------

#[test]
fn test_registry_register_get_list_and_default() {
    let registry = create_default_registry();
    // Every built-in provider is present.
    for id in [
        "anthropic",
        "bedrock",
        "azure_foundry",
        "azure_resource",
        "copilot",
        "gemini",
        "openai",
        "openai_responses",
        "huggingface",
        "generic_openai",
        "ollama_cloud",
        "ollama",
        "openrouter",
        "xai",
        "router",
    ] {
        assert!(registry.get(id).is_some(), "missing provider {id}");
    }
    assert!(registry.get("nonexistent").is_none());
    assert!(registry.get_as_any("router").is_some());
    assert!(registry.get_as_any("nonexistent").is_none());

    let listed = registry.list();
    assert!(listed.len() >= 15);
    assert!(listed.iter().any(|p| p.id == "router"));

    // Default constructor matches `new()`.
    let empty = ProviderRegistry::default();
    assert!(empty.list().is_empty());
}

#[test]
fn test_registry_event_bus_attachment() {
    let registry = create_default_registry();
    let bus = std::sync::Arc::new(ragent_types::event::EventBus::new(4));
    assert!(registry.set_event_bus("openai", Some(bus.clone())));
    assert!(!registry.set_event_bus("does-not-exist", Some(bus.clone())));
    // Attaching to every provider must not panic.
    registry.set_event_bus_all(Some(bus));
    registry.set_event_bus_all(None);
}

#[test]
fn test_registry_resolve_model_static_and_vendor_suffix() {
    let registry = create_default_registry();
    // The OpenAI provider ships an empty catalog, so resolution returns None.
    assert!(registry.resolve_model("openai", "gpt-4o").is_none());
    assert!(registry.resolve_model("missing", "x").is_none());

    // The router has a single static model; vendor-suffix and display-name
    // matching must work.
    let exact = registry
        .resolve_model("router", "router")
        .expect("router model");
    assert_eq!(exact.provider_id, "router");
    assert!(registry.resolve_model("router", "router@x").is_some());
    assert!(registry.resolve_model("router", "MODEL ROUTER").is_some());
    assert!(registry.resolve_model("router", "nope").is_none());
}

#[tokio::test]
async fn test_registry_resolve_model_async_with_static_hit() {
    let registry = create_default_registry();
    let model = registry
        .resolve_model_async("router", "router")
        .await
        .expect("static hit");
    assert_eq!(model.id, "router");
}

// ---------------------------------------------------------------------------
// Anthropic model discovery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_anthropic_models_maps_and_dedups() {
    let payload = serde_json::json!({
        "data": [
            {
                "id": "claude-sonnet-4-20250514",
                "display_name": "Claude Sonnet 4",
                "context_window": 250_000,
                "max_output_tokens": 32_000,
                "capabilities": {"thinking": {"supported": true, "types": ["enabled"]}}
            },
            {
                "id": "claude-3-haiku-20240307",
                "capabilities": {"vision": true, "tools": true}
            },
            // Duplicate id must be removed.
            {"id": "claude-3-haiku-20240307"}
        ]
    })
    .to_string();
    let url = spawn(200, "application/json", payload).await;
    let models = list_anthropic_models("key", Some(&url))
        .await
        .expect("models");
    // Two unique ids after dedup.
    assert_eq!(models.len(), 2);
    let sonnet = models
        .iter()
        .find(|m| m.id.starts_with("claude-sonnet"))
        .expect("sonnet");
    assert!(sonnet.capabilities.reasoning);
    assert_eq!(sonnet.context_window, 250_000);
    assert_eq!(sonnet.max_output, Some(32_000));
    let haiku = models
        .iter()
        .find(|m| m.id.starts_with("claude-3-haiku"))
        .expect("haiku");
    assert!(haiku.capabilities.vision);
}

#[tokio::test]
async fn test_list_anthropic_models_errors_on_non_success() {
    let url = spawn(500, "application/json", "{}".to_string()).await;
    assert!(list_anthropic_models("key", Some(&url)).await.is_err());
}

// ---------------------------------------------------------------------------
// Gemini model discovery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_gemini_models_maps_and_dedups() {
    let payload = serde_json::json!({
        "models": [
            {
                "name": "models/gemini-2.5-pro",
                "baseModelId": "gemini-2.5-pro",
                "displayName": "Gemini 2.5 Pro",
                "inputTokenLimit": 2_000_000,
                "outputTokenLimit": 65_536,
                "supportedGenerationMethods": ["generateContent"],
                "thinking": true
            },
            {
                "name": "models/gemini-2.5-pro",
                "displayName": "Gemini 2.5 Pro dup"
            }
        ]
    })
    .to_string();
    let url = spawn(200, "application/json", payload).await;
    let models = list_gemini_models("key", Some(&url)).await.expect("models");
    assert_eq!(models.len(), 1);
    assert!(models[0].capabilities.reasoning);
    assert_eq!(models[0].context_window, 2_000_000);
    assert_eq!(models[0].max_output, Some(65_536));
}

#[tokio::test]
async fn test_list_gemini_models_errors_on_non_success() {
    let url = spawn(403, "application/json", "{}".to_string()).await;
    assert!(list_gemini_models("key", Some(&url)).await.is_err());
}

// ---------------------------------------------------------------------------
// Bedrock model discovery
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_discover_bedrock_models_filters_and_maps() {
    let payload = serde_json::json!({
        "modelSummaries": [
            {
                "modelId": "anthropic.claude-sonnet-4-20250514-v1:0",
                "modelName": "Claude Sonnet 4",
                "providerName": "Anthropic",
                "streaming": true,
                "inputModalities": ["TEXT", "Image"],
                "outputModalities": ["TEXT"],
                "responseStreamingSupported": true
            },
            {
                "modelId": "amazon.titan-x",
                "modelName": "Titan X",
                "providerName": "Amazon",
                "streaming": false,
                "inputModalities": ["TEXT"],
                "responseStreamingSupported": false
            },
            {
                "modelId": "amazon.nova-pro-v1:0",
                "modelName": "Nova Pro",
                "providerName": "Amazon",
                "responseStreamingSupported": true,
                "inputModalities": ["TEXT"]
            }
        ]
    })
    .to_string();
    let url = spawn(200, "application/json", payload).await;
    let creds = AwsCredentials {
        access_key: "AKIATEST".to_string(),
        secret_key: "secret".to_string(),
        session_token: None,
        region: "us-east-1".to_string(),
    };
    let models = discover_bedrock_models(&creds, Some(&url)).await;
    // The non-streaming Titan model is filtered out.
    assert_eq!(models.len(), 2);
    assert!(models.iter().all(|m| m.provider_id == "bedrock"));
    let claude = models
        .iter()
        .find(|m| m.id.starts_with("anthropic.claude"))
        .expect("claude");
    assert!(claude.capabilities.reasoning);
    assert!(claude.capabilities.vision);
    assert_eq!(claude.name, "Bedrock Claude Sonnet 4");
}

#[tokio::test]
async fn test_discover_bedrock_models_returns_empty_on_error() {
    let url = spawn(500, "application/json", "{}".to_string()).await;
    let creds = AwsCredentials {
        access_key: "AKIATEST".to_string(),
        secret_key: "secret".to_string(),
        session_token: None,
        region: "us-east-1".to_string(),
    };
    assert!(discover_bedrock_models(&creds, Some(&url)).await.is_empty());
}

// ---------------------------------------------------------------------------
// OpenAI provider default catalog (cfg(test) helper)
// ---------------------------------------------------------------------------

#[test]
fn test_openai_provider_identity() {
    use ragent_llm::provider::openai::OpenAiProvider;
    let provider = OpenAiProvider;
    assert_eq!(provider.id(), "openai");
    assert_eq!(provider.name(), "OpenAI");
    assert!(provider.default_models().is_empty());
    assert!(
        provider
            .as_any_static()
            .downcast_ref::<OpenAiProvider>()
            .is_some()
    );
}

#[test]
fn test_generic_openai_provider_identity() {
    use ragent_llm::provider::generic_openai::GenericOpenAiProvider;
    let provider = GenericOpenAiProvider;
    assert_eq!(provider.id(), "generic_openai");
    assert_eq!(provider.name(), "Generic OpenAI API");
    assert!(provider.default_models().is_empty());
}

#[tokio::test]
async fn test_generic_openai_create_client_resolves_endpoint() {
    use ragent_llm::provider::generic_openai::GenericOpenAiProvider;
    let provider = GenericOpenAiProvider;
    let mut options = HashMap::new();
    options.insert(
        "endpoint_url".to_string(),
        serde_json::json!("http://127.0.0.1:9100"),
    );
    let _client = provider
        .create_client("k", None, &options)
        .await
        .expect("client");
    // An invalid scheme falls back to the OpenAI default rather than erroring.
    let mut bad = HashMap::new();
    bad.insert("endpoint_url".to_string(), serde_json::json!("file:///etc"));
    let _client = provider
        .create_client("k", None, &bad)
        .await
        .expect("client with fallback");
}

// ---------------------------------------------------------------------------
// Registry async resolution with a fake discovering provider
// ---------------------------------------------------------------------------

/// A provider whose `discover_models` returns a fixed catalog or an error.
struct FakeDiscoverProvider {
    catalog: Vec<ragent_llm::provider::ModelInfo>,
    fail: bool,
}

#[async_trait::async_trait]
impl Provider for FakeDiscoverProvider {
    fn id(&self) -> &'static str {
        "fake"
    }
    fn name(&self) -> &'static str {
        "Fake"
    }
    fn default_models(&self) -> Vec<ragent_llm::provider::ModelInfo> {
        Vec::new()
    }
    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }
    async fn discover_models(&self) -> anyhow::Result<Vec<ragent_llm::provider::ModelInfo>> {
        if self.fail {
            anyhow::bail!("discovery failed");
        }
        Ok(self.catalog.clone())
    }
    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Box<dyn ragent_llm::llm::LlmClient>> {
        anyhow::bail!("not implemented")
    }
}

fn fake_model(id: &str) -> ragent_llm::provider::ModelInfo {
    ragent_llm::provider::ModelInfo {
        id: id.to_string(),
        provider_id: "fake".to_string(),
        name: format!("Fake {id}"),
        cost: ragent_config::Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: ragent_config::Capabilities {
            reasoning: false,
            streaming: true,
            vision: false,
            tool_use: true,
            thinking_levels: Vec::new(),
        },
        context_window: 1000,
        max_output: None,
        request_multiplier: None,
        thinking_config: None,
    }
}

#[tokio::test]
async fn test_registry_resolve_model_async_discovery_paths() {
    // Success path: discovery populates the cache and resolves by display name.
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(FakeDiscoverProvider {
        catalog: vec![fake_model("fake-1")],
        fail: false,
    }));
    let m = registry
        .resolve_model_async("fake", "Fake fake-1")
        .await
        .expect("display-name resolution");
    assert_eq!(m.id, "fake-1");
    // The cache now serves a synchronous lookup without re-discovery.
    assert!(registry.resolve_model("fake", "fake-1").is_some());

    // Unknown provider -> None.
    assert!(registry.resolve_model_async("missing", "x").await.is_none());

    // Failure path: discovery errors, no cache entry is created.
    let mut failing = ProviderRegistry::new();
    failing.register(Box::new(FakeDiscoverProvider {
        catalog: vec![],
        fail: true,
    }));
    assert!(failing.resolve_model_async("fake", "x").await.is_none());
}

// ---------------------------------------------------------------------------
// OpenAI discovery error paths
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_discover_openai_models_error_paths() {
    use ragent_llm::provider::openai::discover_openai_models;
    // Non-success status.
    let url = spawn(500, "application/json", "{}".to_string()).await;
    assert!(discover_openai_models("k", &url, "openai").await.is_err());
    // Missing `data` array.
    let url = spawn(200, "application/json", r#"{"nope":true}"#.to_string()).await;
    assert!(discover_openai_models("k", &url, "openai").await.is_err());
}

// ---------------------------------------------------------------------------
// Provider create_client coverage (offline, no network on construction)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_gemini_and_anthropic_create_client_offline() {
    use ragent_llm::provider::anthropic::AnthropicProvider;
    use ragent_llm::provider::gemini::GeminiProvider;
    let _ = GeminiProvider
        .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await
        .expect("gemini client");
    let _ = AnthropicProvider
        .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await
        .expect("anthropic client");

    // Gemini discovery errors on a non-success endpoint.
    use ragent_llm::provider::gemini::list_gemini_models;
    let url = spawn(404, "application/json", "{}".to_string()).await;
    assert!(list_gemini_models("k", Some(&url)).await.is_err());
}

#[tokio::test]
async fn test_huggingface_create_client_requires_key_and_accepts_options() {
    use ragent_llm::provider::huggingface::HuggingFaceProvider;
    // Empty key is rejected.
    assert!(
        HuggingFaceProvider
            .create_client("", None, &HashMap::new())
            .await
            .is_err()
    );
    // Valid key with explicit options builds a client.
    let mut options = HashMap::new();
    options.insert("wait_for_model".to_string(), serde_json::json!(false));
    options.insert("use_cache".to_string(), serde_json::json!(false));
    let _ = HuggingFaceProvider
        .create_client("token", Some("http://127.0.0.1:1/"), &options)
        .await
        .expect("hf client");
}
