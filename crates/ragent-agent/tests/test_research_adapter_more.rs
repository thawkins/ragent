//! Additional tests-only coverage for `ragent_agent::research_adapter`.
//!
//! The pure helpers and the private web-fetch parsers are exercised by the
//! existing inline suite (`tests/inline/research_adapter_tests.rs`) and the
//! external helper suite (`test_research_adapter_helpers.rs`). This file adds
//! NEW branches reached only through a live [`ResearchSession`] built by
//! `build_research_session`:
//!
//! - the `AgentLocalTool` adapters (`glob` / `grep` / `read` / `list_specs` /
//!   `spec_title`) driven by `LocalGatherer::gather`, and
//! - the `AgentWebSearchTool` / `AgentWebFetchTool` adapters driven by
//!   `WebGatherer::gather`: the `mf_search` metadata fallback shape, the
//!   metadata-driven legacy `webfetch` readability re-verification (accept and
//!   reject), and the plain-text `websearch` fallback with the non-`mf_search`
//!   exclusion shape.
//!
//! No network: every tool is an in-process stub returning canned `ToolOutput`,
//! and every URL handed to a fetch stub is either non-HTTP (so
//! `extract_head_metadata_for_url` returns early) or rejected by the
//! readability guarantee before the supplementary head fetch is reached.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use ragent_agent::event::EventBus;
use ragent_agent::research_adapter::build_research_session;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput, ToolRegistry};
use ragent_research::{LocalGatherConfig, ResearchManager, Source};
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

/// A stub fetch tool that distinguishes the `format=raw` re-verification call
/// (used by the legacy `webfetch` readability guarantee) from the normal fetch.
struct StubFetchTool {
    name: &'static str,
    body: String,
    raw_html: String,
    metadata: Option<Value>,
}

#[async_trait]
impl Tool for StubFetchTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &'static str {
        "stub-fetch"
    }

    fn parameters_schema(&self) -> Value {
        json!({})
    }

    fn permission_category(&self) -> &'static str {
        "web"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let raw = input.get("format").and_then(Value::as_str) == Some("raw");
        Ok(ToolOutput {
            content: if raw {
                self.raw_html.clone()
            } else {
                self.body.clone()
            },
            metadata: self.metadata.clone(),
        })
    }
}

/// Build a registry from the given tools.
fn registry_of(tools: Vec<Arc<dyn Tool>>) -> Arc<ToolRegistry> {
    let registry = ToolRegistry::new();
    for tool in tools {
        registry.register(tool);
    }
    Arc::new(registry)
}

/// Build a live `ResearchSession` over `registry`.
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

/// A long, keyword-saturated snippet that clears the relevance filters.
fn body() -> String {
    "widget engine ".repeat(60)
}

// -- AgentLocalTool via LocalGatherer::gather --------------------------------

/// `LocalGatherer::gather` drives every `AgentLocalTool` method: `glob`
/// enumerates candidates, `grep` scores them (parsing the tool's `path:line:text`
/// output), `read` extracts the body / spec title, and `list` enumerates specs.
#[tokio::test]
async fn local_gatherer_drives_agent_local_tool_methods() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = registry_of(vec![
        Arc::new(StubTool {
            name: "glob",
            // One candidate file per glob call; de-duplicated to a single path.
            content: "src/widget.rs\n".to_string(),
            metadata: None,
        }) as Arc<dyn Tool>,
        Arc::new(StubTool {
            name: "grep",
            content: "src/widget.rs:12:let widget = 1;\n".to_string(),
            metadata: None,
        }) as Arc<dyn Tool>,
        Arc::new(StubTool {
            name: "read",
            // Serves candidate bodies and the SPEC.md title lookup alike.
            content: "# Alpha Spec\nwidget engine details\n".to_string(),
            metadata: None,
        }) as Arc<dyn Tool>,
        Arc::new(StubTool {
            name: "list",
            content:
                "/project/specs/\n\u{251c}\u{2500}\u{2500} alpha/\n\u{2514}\u{2500}\u{2500} beta/\n"
                    .to_string(),
            metadata: None,
        }) as Arc<dyn Tool>,
    ]);

    let session = session_for(registry, tmp.path().to_path_buf());
    assert!(
        session.local().is_some(),
        "glob/grep/read/list must enable local gathering"
    );
    assert!(session.web().is_none(), "no web tools were registered");

    let gatherer = session.local().expect("local gatherer");
    let config = LocalGatherConfig {
        terms: vec!["widget".to_string(), "engine".to_string()],
        ..LocalGatherConfig::default()
    };

    let sources = gatherer
        .gather(tmp.path(), "widget engine", None, &config)
        .await
        .expect("local gather must succeed");

    let local = sources
        .iter()
        .filter(|s| matches!(s, Source::Local { .. }))
        .count();
    let specs = sources
        .iter()
        .filter(|s| matches!(s, Source::Spec { .. }))
        .count();
    assert_eq!(local, 1, "the one grep-matching candidate is captured");
    assert_eq!(specs, 2, "both specs are cross-referenced: {sources:?}");
}

// -- AgentWebSearchTool / AgentWebFetchTool via WebGatherer::gather ----------

/// The `mf_search`-specific fallback: a `results` entry whose `search_engine`
/// field is not a string fails the generic `hits_from_metadata` deserialisation,
/// so `parse_mf_search_metadata` handles it (reading the `source` engine field).
/// A `wikipedia` engine is captured as an encyclopedia summary without a fetch.
#[tokio::test]
async fn web_gather_captures_mf_search_source_fallback() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = registry_of(vec![
        Arc::new(StubTool {
            name: "mf_search",
            content: "ignored".to_string(),
            metadata: Some(json!({
                "results": [{
                    "title": "widget engine",
                    "url": "file:///widget-guide",
                    "snippet": "widget engine details. ".repeat(10),
                    // Non-string engine forces the generic parser to fail.
                    "search_engine": 1,
                    "source": "wikipedia"
                }]
            })),
        }) as Arc<dyn Tool>,
        Arc::new(StubFetchTool {
            name: "mf_fetch",
            body: String::new(),
            raw_html: String::new(),
            metadata: None,
        }) as Arc<dyn Tool>,
    ]);

    let session = session_for(registry, tmp.path().to_path_buf());
    let gatherer = session.web().expect("web gatherer");
    let sources = gatherer
        .gather("widget engine", 5)
        .await
        .expect("web gather must succeed");
    assert_eq!(
        sources.len(),
        1,
        "encyclopedia hit is synthesized from its snippet: {sources:?}"
    );
}

/// A metadata-driven hit with a non-HTTP URL (so `extract_head_metadata_for_url`
/// returns without a network call) served by the legacy `webfetch` tool: the
/// adapter re-runs readability on the raw HTML and ACCEPTS a real article.
#[tokio::test]
async fn web_gather_legacy_webfetch_verify_accepts_article() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let article = format!(
        "<html><head><title>widget guide</title></head><body><article><h1>widget</h1><p>{}</p></article></body></html>",
        "widget engine details. ".repeat(50)
    );
    let registry = registry_of(vec![
        Arc::new(StubTool {
            name: "websearch",
            content: "ignored".to_string(),
            metadata: Some(json!({
                "results": [{
                    "title": "widget guide",
                    "url": "file:///widget-guide",
                    "snippet": "widget engine overview"
                }]
            })),
        }) as Arc<dyn Tool>,
        Arc::new(StubFetchTool {
            name: "webfetch",
            body: body(),
            raw_html: article,
            metadata: None,
        }) as Arc<dyn Tool>,
    ]);

    let session = session_for(registry, tmp.path().to_path_buf());
    let gatherer = session.web().expect("web gatherer");
    let sources = gatherer
        .gather("widget engine", 5)
        .await
        .expect("web gather must succeed");
    assert_eq!(
        sources.len(),
        1,
        "the readability-verified legacy page is captured: {sources:?}"
    );
}

/// The plain-text `websearch` fallback (`metadata: None`) is parsed by
/// `parse_websearch_output`. With `disable_scholarly` the gatherer calls the
/// adapter's `search_with_exclusions`; because the backing tool is NOT
/// `mf_search` the exclusions are dropped (the non-`mf_search` shape). The
/// legacy `webfetch` re-verification then REJECTS a non-article HTML page,
/// bailing before any head-metadata fetch.
#[tokio::test]
async fn web_gather_legacy_webfetch_verify_rejects_non_article() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let registry = registry_of(vec![
        Arc::new(StubTool {
            name: "websearch",
            content: "1. widget guide\nhttps://widget.example/page\nwidget engine overview\n"
                .to_string(),
            metadata: None,
        }) as Arc<dyn Tool>,
        Arc::new(StubFetchTool {
            name: "webfetch",
            body: body(),
            // Nav-only HTML fails the readability minimum-length gate.
            raw_html: "<html><body><nav><p>hi</p></nav></body></html>".to_string(),
            metadata: None,
        }) as Arc<dyn Tool>,
    ]);

    let session = session_for(registry, tmp.path().to_path_buf());
    let gatherer = session
        .web()
        .expect("web gatherer")
        .with_disable_scholarly(true);
    let sources = gatherer
        .gather("widget engine", 5)
        .await
        .expect("web gather must succeed");
    assert!(
        sources.is_empty(),
        "the legacy fetch must be rejected by the readability guarantee: {sources:?}"
    );
}
