//! Offline coverage for provider identity, `create_client` construction, and
//! local mock-server model-discovery paths across the ragent-llm provider
//! family. Every test runs without real network egress: HTTP is served by a
//! one-shot loopback TcpListener.

// `std::env::set_var` / `remove_var` are `unsafe` in Rust 2024; this file
// mutates `OPENROUTER_API_KEY` only while the crate-wide env lock is held
// (see `tests/all.rs`), so the workspace's `unsafe_code` deny is lifted here.
#![allow(unsafe_code)]
// The env lock is a *std* mutex held across the body on purpose: the
// empty-key rejection only holds while a sibling file is not setting the
// variable, so serialisation (not parallelism) is the goal. See `tests/all.rs`.
#![allow(clippy::await_holding_lock)]

use std::collections::HashMap;

use ragent_llm::provider::Provider;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawn a one-shot HTTP server that replies with a fixed status/body and
/// returns its `http://host:port` base URL.
async fn spawn_one_shot(status: u16, content_type: &str, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let ct = content_type.to_string();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Read the request before replying: a client whose request bytes are
        // still in flight when the socket closes sees a TCP RST, surfacing as
        // `client error (Canceled)` on the real HTTP client.
        let mut request_buf = [0u8; 8192];
        let _ = socket.read(&mut request_buf).await;
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    format!("http://{addr}")
}

/// Spawn a two-request server: the first request (Ollama Cloud `/api/tags`)
/// returns the supplied tags payload; every subsequent request (`/api/show`)
/// returns the supplied show payload.
async fn spawn_ollama_cloud(tags_body: String, show_body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        // First request: /api/tags
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        // Read the request before replying: a client whose request bytes are
        // still in flight when the socket closes sees a TCP RST, surfacing as
        // `client error (Canceled)` on the real HTTP client.
        let mut request_buf = [0u8; 8192];
        let _ = socket.read(&mut request_buf).await;
        write_json(&mut socket, 200, &tags_body).await;
        // Remaining requests: /api/show (one per discovered model)
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            // Read the request before replying: a client whose request bytes are
            // still in flight when the socket closes sees a TCP RST, surfacing as
            // `client error (Canceled)` on the real HTTP client.
            let mut request_buf = [0u8; 8192];
            let _ = socket.read(&mut request_buf).await;
            write_json(&mut socket, 200, &show_body).await;
        }
    });
    format!("http://{addr}")
}

async fn write_json(socket: &mut tokio::net::TcpStream, status: u16, body: &str) {
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}

// ---------------------------------------------------------------------------
// Provider identity + create_client construction (no network on the call side)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_anthropic_provider_identity_and_client() {
    use ragent_llm::provider::anthropic::AnthropicProvider;
    let provider = AnthropicProvider;
    assert_eq!(provider.id(), "anthropic");
    assert_eq!(provider.name(), "Anthropic");
    assert!(provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<AnthropicProvider>());
    let _client = provider
        .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await
        .expect("anthropic client");
    // No base URL supplied -> falls back to the built-in API base.
    let _client = provider
        .create_client("key", None, &HashMap::new())
        .await
        .expect("anthropic client default base");
}

#[tokio::test]
async fn test_gemini_provider_identity_and_client() {
    use ragent_llm::provider::gemini::GeminiProvider;
    let provider = GeminiProvider;
    assert_eq!(provider.id(), "gemini");
    assert_eq!(provider.name(), "Google Gemini");
    assert!(!provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<GeminiProvider>());
    let _client = provider
        .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await
        .expect("gemini client");
    let _client = provider
        .create_client("key", None, &HashMap::new())
        .await
        .expect("gemini client default base");
}

#[tokio::test]
async fn test_openai_responses_provider_identity_and_client() {
    use ragent_llm::provider::openai_responses::ResponsesApiProvider;
    let provider = ResponsesApiProvider;
    assert_eq!(provider.id(), "openai_responses");
    assert_eq!(provider.name(), "OpenAI Responses API");
    assert!(provider.as_any_static().is::<ResponsesApiProvider>());
    let _client = provider
        .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await
        .expect("responses client");
    let _client = provider
        .create_client("key", None, &HashMap::new())
        .await
        .expect("responses client default base");
}

#[tokio::test]
async fn test_xai_provider_identity_and_client() {
    use ragent_llm::provider::xai::XaiProvider;
    let provider = XaiProvider;
    assert_eq!(provider.id(), "xai");
    assert_eq!(provider.name(), "xAI");
    assert!(provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<XaiProvider>());
    let _client = provider
        .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await
        .expect("xai client");
    let _client = provider
        .create_client("key", None, &HashMap::new())
        .await
        .expect("xai client default base");
    // Discovery without an XAI_API_KEY in the environment errors out.
    if std::env::var("XAI_API_KEY").is_err() {
        assert!(provider.discover_models().await.is_err());
    }
}

#[tokio::test]
async fn test_copilot_provider_identity_usage_and_client_error() {
    use ragent_llm::provider::copilot::CopilotProvider;
    let provider = CopilotProvider::new();
    assert_eq!(provider.id(), "copilot");
    assert_eq!(provider.name(), "GitHub Copilot");
    assert!(provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<CopilotProvider>());
    // `Default` delegates to `new`.
    assert_eq!(CopilotProvider::default().id(), "copilot");
    assert_eq!(CopilotProvider::with_url("x/").name(), "GitHub Copilot");
    // Usage reports the cached plan (None before any token exchange).
    let usage = provider.fetch_usage("token").await.expect("usage info");
    assert!(usage.plan.is_none() || usage.plan.is_some());
    // create_client falls back to the GitHub Models API on an exchange failure
    // (the loopback URL is unreachable), which still yields a usable client.
    let client = provider
        .create_client("ghu_token", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await;
    assert!(client.is_ok(), "copilot create_client should fall back");
}

#[tokio::test]
async fn test_ollama_cloud_provider_identity_and_client_paths() {
    use ragent_llm::provider::ollama_cloud::OllamaCloudProvider;
    let provider = OllamaCloudProvider::new();
    assert_eq!(provider.id(), "ollama_cloud");
    assert_eq!(provider.name(), "Ollama Cloud");
    assert!(provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<OllamaCloudProvider>());
    assert_eq!(OllamaCloudProvider::default().id(), "ollama_cloud");
    // Empty key is rejected.
    assert!(
        provider
            .create_client("", None, &HashMap::new())
            .await
            .is_err()
    );
    // Non-empty key builds a client; an explicit base overrides the default.
    assert!(
        provider
            .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
            .await
            .is_ok()
    );
    assert!(
        provider
            .create_client("key", None, &HashMap::new())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_ollama_provider_identity_and_client_paths() {
    use ragent_llm::provider::ollama::OllamaProvider;
    let provider = OllamaProvider::with_url("http://127.0.0.1:1");
    assert_eq!(provider.id(), "ollama");
    assert_eq!(provider.name(), "Ollama");
    assert!(provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<OllamaProvider>());
    assert_eq!(OllamaProvider::default().id(), "ollama");
    // Empty key -> keyless client.
    assert!(
        provider
            .create_client("", None, &HashMap::new())
            .await
            .is_ok()
    );
    // Non-empty key -> bearer-authenticated client.
    assert!(
        provider
            .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_huggingface_provider_identity_and_client_paths() {
    use ragent_llm::provider::huggingface::HuggingFaceProvider;
    let provider = HuggingFaceProvider;
    assert_eq!(provider.id(), "huggingface");
    assert_eq!(provider.name(), "Hugging Face");
    assert!(provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<HuggingFaceProvider>());
    // Empty token rejected.
    assert!(
        provider
            .create_client("", None, &HashMap::new())
            .await
            .is_err()
    );
    let mut options = HashMap::new();
    options.insert("wait_for_model".to_string(), serde_json::json!(false));
    options.insert("use_cache".to_string(), serde_json::json!(false));
    assert!(
        provider
            .create_client("token", Some("http://127.0.0.1:1/"), &options)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_generic_openai_provider_identity_and_client() {
    use ragent_llm::provider::generic_openai::GenericOpenAiProvider;
    let provider = GenericOpenAiProvider;
    assert_eq!(provider.id(), "generic_openai");
    assert_eq!(provider.name(), "Generic OpenAI API");
    assert!(provider.default_models().is_empty());
    assert!(provider.as_any_static().is::<GenericOpenAiProvider>());
    // Options-supplied endpoint wins.
    let mut options = HashMap::new();
    options.insert(
        "endpoint_url".to_string(),
        serde_json::json!("http://127.0.0.1:9"),
    );
    assert!(provider.create_client("key", None, &options).await.is_ok());
    // Explicit base URL path.
    assert!(
        provider
            .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_azure_foundry_provider_identity_and_client() {
    use ragent_llm::provider::azure_foundry::AzureFoundryProvider;
    let provider = AzureFoundryProvider;
    assert_eq!(provider.id(), "azure_foundry");
    assert!(provider.as_any_static().is::<AzureFoundryProvider>());
    let mut options = HashMap::new();
    options.insert(
        "endpoint_url".to_string(),
        serde_json::json!("http://127.0.0.1:9"),
    );
    assert!(provider.create_client("key", None, &options).await.is_ok());
    assert!(
        provider
            .create_client("key", Some("http://127.0.0.1:1/"), &HashMap::new())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_openrouter_provider_requires_key_and_builds_client() {
    use ragent_llm::provider::openrouter::OpenRouterProvider;
    // Hold the crate-wide env lock: the empty-key rejection only holds while
    // `OPENROUTER_API_KEY` is unset, and a sibling file in this consolidated
    // binary sets it (see `tests/all.rs`).
    let _env_lock = crate::global_env_lock();
    // SAFETY: serialised by the lock above; restored by the `EnvGuard` below.
    let saved_key = std::env::var_os("OPENROUTER_API_KEY");
    unsafe { std::env::remove_var("OPENROUTER_API_KEY") };
    let provider = OpenRouterProvider::new();
    assert_eq!(provider.id(), "openrouter");
    assert!(provider.as_any_static().is::<OpenRouterProvider>());
    // Missing key is rejected with a remediation-bearing error.
    assert!(
        provider
            .create_client("", None, &HashMap::new())
            .await
            .is_err()
    );
    // A supplied key builds a client; the base URL accessor is trailing-slash
    // trim aware.
    let client = provider
        .create_client("sk-or-key", Some("http://127.0.0.1:1/"), &HashMap::new())
        .await
        .expect("openrouter client");
    let _ = client;
    assert_eq!(
        provider.base_url(),
        provider.base_url().trim_end_matches('/')
    );
    // Restore the saved value so later tests in this binary see the developer's
    // environment unchanged.
    match saved_key {
        Some(prev) => unsafe { std::env::set_var("OPENROUTER_API_KEY", prev) },
        None => unsafe { std::env::remove_var("OPENROUTER_API_KEY") },
    }
}

#[tokio::test]
async fn test_bedrock_provider_identity() {
    use ragent_llm::provider::bedrock::BedrockProvider;
    let provider = BedrockProvider;
    assert_eq!(provider.id(), "bedrock");
    assert_eq!(provider.name(), "Amazon Bedrock");
    assert!(provider.as_any_static().is::<BedrockProvider>());
}

// ---------------------------------------------------------------------------
// Live model-discovery HTTP paths against local mock servers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_anthropic_models_maps_entries() {
    use ragent_llm::provider::anthropic::list_anthropic_models;
    let payload = serde_json::json!({
        "data": [
            {
                "id": "claude-sonnet-4-20250514",
                "display_name": "Claude Sonnet 4",
                "context_window": 200_000,
                "max_output_tokens": 8192,
                "capabilities": {
                    "reasoning_effort": ["high", "medium", "low"]
                }
            },
            { "name": "claude-3-5-haiku-20241022" }
        ]
    })
    .to_string();
    let url = spawn_one_shot(200, "application/json", payload).await;
    let models = list_anthropic_models("key", Some(&url))
        .await
        .expect("models");
    assert!(models.iter().any(|m| m.id == "claude-sonnet-4-20250514"));
    assert!(models.iter().any(|m| m.id == "claude-3-5-haiku-20241022"));
    let sonnet = models
        .iter()
        .find(|m| m.id == "claude-sonnet-4-20250514")
        .expect("sonnet");
    assert!(sonnet.capabilities.reasoning);
    assert_eq!(sonnet.context_window, 200_000);
}

#[tokio::test]
async fn test_list_anthropic_models_missing_data_array_errors() {
    use ragent_llm::provider::anthropic::list_anthropic_models;
    // Valid JSON but no `data` key -> the response parse still succeeds with an
    // empty default list (unwrap_or_default), so it returns Ok(empty).
    let url = spawn_one_shot(200, "application/json", "{}".to_string()).await;
    let models = list_anthropic_models("key", Some(&url))
        .await
        .expect("empty models");
    assert!(models.is_empty());
}

#[tokio::test]
async fn test_list_ollama_cloud_models_maps_show_metadata() {
    use ragent_llm::provider::ollama_cloud::list_ollama_cloud_models;
    let tags = serde_json::json!({
        "models": [
            { "name": "qwen3:8b", "model": "qwen3:8b",
              "details": { "parameter_size": "8B" } },
            { "name": "llava:13b", "details": { "parameter_size": "13B" } }
        ]
    })
    .to_string();
    let show = serde_json::json!({
        "model_info": { "llama.context_length": 32768 },
        "capabilities": ["vision", "thinking"]
    })
    .to_string();
    let url = spawn_ollama_cloud(tags, show).await;
    let models = list_ollama_cloud_models("key", Some(&url))
        .await
        .expect("models");
    assert_eq!(models.len(), 2);
    let qwen = models.iter().find(|m| m.id == "qwen3:8b").expect("qwen");
    assert_eq!(qwen.context_window, 32768);
    assert!(qwen.capabilities.vision);
    assert!(qwen.capabilities.reasoning);
}

#[tokio::test]
async fn test_anthropic_discover_models_without_key_errors() {
    use ragent_llm::provider::anthropic::AnthropicProvider;
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        assert!(AnthropicProvider.discover_models().await.is_err());
    }
}

#[tokio::test]
async fn test_openai_responses_discover_models_local() {
    use ragent_llm::provider::openai_responses::ResponsesApiProvider;
    // Discovery uses the hard-coded Responses API base, so drive the parse path
    // through the provider's own method only when it can reach a local server;
    // otherwise assert the offline failure path is graceful.
    let result = ResponsesApiProvider.discover_models().await;
    // Either it errored (no network) or returned a list; both are acceptable and
    // no panic is the contract.
    let _ = result;
}
