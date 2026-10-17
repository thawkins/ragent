//! Extra tests-only coverage for `ragent_agent::research_adapter`.
//!
//! Complements `test_research_adapter_more.rs` and the inline suite by driving
//! the branches that are only reachable through `build_research_session` when a
//! `ProviderRegistry` + `ModelRef` are supplied, and through `WebGatherer::gather`
//! with an `mf_fetch` JSON envelope / `mf_search` structured metadata.
//!
//! No network: search hits use non-HTTP URLs (so `extract_head_metadata_for_url`
//! returns before any HTTP client is built) and every tool is an in-process stub
//! returning canned `ToolOutput`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use ragent_agent::agent::ModelRef;
use ragent_agent::event::EventBus;
use ragent_agent::research_adapter::build_research_session;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput, ToolRegistry};
use ragent_agent::{Config, create_default_registry};
use ragent_research::{ResearchManager, Source};
use serde_json::{Value, json};

/// A stub agent tool returning a fixed `ToolOutput`.
struct StubTool {
    name: &'static str,
    content: String,
    metadata: Option<Value>,
}

#[async_trait]
impl Tool for StubTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &'static str {
        "stub"
    }

    fn parameters_schema(&self) -> Value {
        json!({})
    }

    fn permission_category(&self) -> &'static str {
        "read"
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput {
            content: self.content.clone(),
            metadata: self.metadata.clone(),
        })
    }
}

fn registry_of(tools: Vec<Arc<dyn Tool>>) -> Arc<ToolRegistry> {
    let registry = ToolRegistry::new();
    for tool in tools {
        registry.register(tool);
    }
    Arc::new(registry)
}

/// Build a live `ResearchSession` with no provider wiring.
fn session_for(registry: Arc<ToolRegistry>, workdir: PathBuf) -> ragent_research::ResearchSession {
    build_research_session(
        &registry,
        ResearchManager::new("research"),
        "test-session".into(),
        workdir,
        Arc::new(EventBus::new(16)),
        None,
        None,
        None,
        None,
        None,
    )
}

/// A keyword-saturated snippet that clears the lexical relevance filter.
fn body() -> String {
    "widget engine ".repeat(60)
}

// -- build_research_session with a provider registry + active model ----------

/// With a `ProviderRegistry` and an active `ModelRef`, every LLM-backed engine
/// (decomposer, analysis, planner, summarizer) is constructed rather than the
/// heuristic/noop fallbacks, and `resolve_base_url` reads the provider base URL
/// from storage and config. Constructing these engines makes no network call.
#[tokio::test]
async fn provider_model_wiring_uses_storage_and_config_base_url() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = Arc::new(ragent_agent::tool::create_default_registry());

    // Storage supplies the `generic_openai` base URL and an API key.
    let storage = Arc::new(ragent_agent::storage::Storage::open_in_memory().expect("storage"));
    storage
        .set_setting("generic_openai_api_base", "http://127.0.0.1:4321")
        .expect("set base");
    storage
        .set_provider_auth("generic_openai", "sk-test")
        .expect("set key");

    // Config carries a fallback base URL for the same provider (unused when the
    // storage value is present, but exercises the map lookup).
    let mut config = Config::default();
    let provider_config: ragent_agent::ProviderConfig = serde_json::from_value(json!({
        "api": { "base_url": "http://127.0.0.1:9999" }
    }))
    .expect("provider config from json");
    config
        .provider
        .insert("generic_openai".to_string(), provider_config);

    let provider_registry = Arc::new(create_default_registry());
    let model = ModelRef {
        provider_id: "generic_openai".to_string(),
        model_id: "local-model".to_string(),
    };

    let session = build_research_session(
        &registry,
        ResearchManager::new("research"),
        "test-session".into(),
        tmp.path().to_path_buf(),
        Arc::new(EventBus::new(16)),
        Some(storage),
        Some(Arc::new(config)),
        Some(provider_registry),
        Some(model),
        None,
    );

    // The default registry provides websearch+webfetch+glob/grep/read/list.
    let debug = format!("{session:?}");
    assert!(debug.contains("has_web: true"), "{debug}");
    assert!(debug.contains("has_local: true"), "{debug}");
}

/// The `copilot` and `azure_foundry` provider ids resolve their base URL from a
/// storage setting (the `match` arms in `resolve_base_url`).
#[tokio::test]
async fn provider_wiring_resolves_copilot_and_azure_base_urls() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = Arc::new(ragent_agent::tool::create_default_registry());
    let provider_registry = Arc::new(create_default_registry());

    for (provider_id, setting_key) in [
        ("copilot", "copilot_api_base"),
        ("azure_foundry", "azure_foundry_api_base"),
    ] {
        let storage = Arc::new(ragent_agent::storage::Storage::open_in_memory().expect("storage"));
        storage
            .set_setting(setting_key, "https://example.invalid/base")
            .expect("set base");
        let model = ModelRef {
            provider_id: provider_id.to_string(),
            model_id: "m".to_string(),
        };
        let session = build_research_session(
            &registry,
            ResearchManager::new("research"),
            "test-session".into(),
            tmp.path().to_path_buf(),
            Arc::new(EventBus::new(16)),
            Some(storage),
            None,
            Some(provider_registry.clone()),
            Some(model),
            None,
        );
        assert!(format!("{session:?}").contains("has_web: true"));
    }
}

/// A `research_name` with characters that must be sanitised drives the
/// `GatherLog::new` branch: the `log/research` directory is created eagerly.
#[tokio::test]
async fn research_name_creates_the_gather_log_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = Arc::new(ragent_agent::tool::create_default_registry());
    let _session = build_research_session(
        &registry,
        ResearchManager::new("research"),
        "test-session".into(),
        tmp.path().to_path_buf(),
        Arc::new(EventBus::new(16)),
        None,
        None,
        None,
        None,
        Some("My Research / 2025!"),
    );
    assert!(
        tmp.path().join("log").join("research").is_dir(),
        "the gather log directory is created for a named run"
    );
}

// -- AgentWebFetchTool: mf_fetch JSON envelope via WebGatherer::gather --------

/// A `file://` search hit (non-HTTP, so the head-metadata fetch returns early)
/// whose page is served as an `mf_fetch` JSON envelope. This drives
/// `parse_mf_fetch_output`'s envelope branch: the body, title, content type,
/// page type, detected language, and author are all read from the envelope.
#[tokio::test]
async fn web_gather_parses_mf_fetch_json_envelope() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let envelope = json!({
        "content": body(),
        "content_type": "text/html",
        "page_type": "article",
        "detected_language": "en",
        "extraction_method": "readability",
        "metadata": { "title": "Widget Guide", "author": "Alice Example" }
    })
    .to_string();
    let registry = registry_of(vec![
        Arc::new(StubTool {
            name: "mf_search",
            content: "ignored".to_string(),
            metadata: Some(json!({
                "results": [{
                    "title": "widget engine",
                    "url": "file:///widget-guide",
                    "snippet": body(),
                    "search_engine": "tavily"
                }]
            })),
        }) as Arc<dyn Tool>,
        Arc::new(StubTool {
            name: "mf_fetch",
            content: envelope,
            metadata: Some(json!({ "extraction_method": "readability" })),
        }) as Arc<dyn Tool>,
    ]);

    let session = session_for(registry, tmp.path().to_path_buf());
    let gatherer = session.web().expect("web gatherer");
    let sources = gatherer
        .gather("widget engine", 5)
        .await
        .expect("web gather must succeed");
    assert_eq!(sources.len(), 1, "the envelope-served page is captured");
    match &sources[0] {
        Source::Web { title, author, .. } => {
            assert_eq!(title, "Widget Guide", "title read from the envelope");
            assert_eq!(author.as_deref(), Some("Alice Example"));
        }
        other => panic!("expected a Web source, got {other:?}"),
    }
}

/// A `search_engine` of `tavily` routes the hit through the fetch stage (not
/// the encyclopedia/scholarly synthesis), and the structured `results`
/// metadata is deserialised by `hits_from_metadata` rather than the plain-text
/// fallback.
#[tokio::test]
async fn search_metadata_results_feed_the_gatherer() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = registry_of(vec![
        Arc::new(StubTool {
            name: "mf_search",
            content: "plain text that must be ignored".to_string(),
            metadata: Some(json!({
                "results": [{
                    "title": "widget engine page",
                    "url": "file:///w",
                    "snippet": body(),
                    "search_tool": "mf_search",
                    "search_engine": "tavily"
                }]
            })),
        }) as Arc<dyn Tool>,
        Arc::new(StubTool {
            name: "mf_fetch",
            content: body(),
            metadata: Some(json!({
                "content_type": "text/html",
                "extraction_method": "readability",
                "page_type": "article"
            })),
        }) as Arc<dyn Tool>,
    ]);

    let session = session_for(registry, tmp.path().to_path_buf());
    let sources = session
        .web()
        .expect("web gatherer")
        .gather("widget engine", 5)
        .await
        .expect("web gather must succeed");
    assert_eq!(sources.len(), 1, "the metadata hit is fetched and captured");
}

/// An `mf_fetch` result with `content_ok = false` and `page_type = paywall`
/// is converted to an `AuthWall` `FetchFailure` before the readability gate,
/// so the page is suppressed with the real reason.
#[tokio::test]
async fn web_gather_rejects_paywalled_fetch() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = registry_of(vec![
        Arc::new(StubTool {
            name: "mf_search",
            content: "ignored".to_string(),
            metadata: Some(json!({
                "results": [{
                    "title": "widget engine",
                    "url": "file:///paywalled",
                    "snippet": body(),
                    "search_engine": "tavily"
                }]
            })),
        }) as Arc<dyn Tool>,
        Arc::new(StubTool {
            name: "mf_fetch",
            content: "placeholder".to_string(),
            metadata: Some(json!({
                "content_ok": false,
                "page_type": "paywall",
                "error": "premium content requires a subscription"
            })),
        }) as Arc<dyn Tool>,
    ]);

    let session = session_for(registry, tmp.path().to_path_buf());
    let sources = session
        .web()
        .expect("web gatherer")
        .gather("widget engine", 5)
        .await
        .expect("web gather must succeed");
    assert!(
        sources.is_empty(),
        "a paywalled fetch is refused: {sources:?}"
    );
}
