#![allow(clippy::assert_is_empty)]
//! Integration tests for the `websearch` tool wrapper.
//!
//! These tests exercise the refactored `WebSearchTool` which delegates to
//! `MfSearchTool` / `SearchOrchestrator`. They verify the tool name, schema,
//! input validation, and metadata shape without making network calls
//! (FR-006, FR-007, FR-008, NFR-003).

use ragent_config::Config;
use ragent_tools_extended::websearch::{WebSearchTool, hits_from_metadata};
use ragent_tools_extended::{Tool, ToolContext};
use serde_json::json;

/// Build a minimal `ToolContext` for tests that do not touch the filesystem.
fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

/// Build a `ToolContext` with the given Tavily API key in its config.
fn ctx_with_tavily_key(key: &str) -> ToolContext {
    let mut config = Config::default();
    config.tavily_api_key = Some(key.to_string());
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[test]
fn test_tool_name_is_websearch() {
    let tool = WebSearchTool;
    assert_eq!(tool.name(), "websearch");
}

#[test]
fn test_permission_category_is_web() {
    let tool = WebSearchTool;
    assert_eq!(tool.permission_category(), "web");
}

#[test]
fn test_parameters_schema_has_required_query() {
    let tool = WebSearchTool;
    let schema = tool.parameters_schema();
    let required = schema["required"]
        .as_array()
        .expect("required should be an array");
    assert!(
        required.iter().any(|v| v == "query"),
        "query should be required"
    );
    assert_eq!(schema["properties"]["query"]["type"], "string");
    assert_eq!(schema["properties"]["num_results"]["type"], "integer");
}

#[tokio::test]
async fn test_execute_rejects_empty_query() {
    let tool = WebSearchTool;
    let result = tool.execute(json!({"query": ""}), &ctx()).await;
    assert!(result.is_err(), "empty query should error");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("empty"),
        "error should mention empty query: {err}"
    );
}

#[tokio::test]
async fn test_execute_rejects_missing_query() {
    let tool = WebSearchTool;
    let result = tool.execute(json!({}), &ctx()).await;
    assert!(result.is_err(), "missing query should error");
}

#[tokio::test]
async fn test_execute_returns_results_with_keyless_backends() {
    // Even without a Tavily key, the websearch wrapper delegates to
    // mf_search which always has DuckDuckGo + Brave. This test verifies
    // the wrapper produces output and metadata in the expected shape.
    let tool = WebSearchTool;
    let result = tool
        .execute(
            json!({"query": "rust programming", "num_results": 3}),
            &ctx(),
        )
        .await;
    // The keyless backends may fail in CI (network), but the wrapper
    // should not panic and should return either Ok or a network error.
    match result {
        Ok(output) => {
            // Verify metadata shape.
            let meta = output.metadata.expect("metadata should be present");
            assert_eq!(meta["query"], "rust programming");
            assert!(meta["count"].as_u64().is_some());
            assert!(meta["results"].is_array());
            assert!(meta["engines_used"].is_array());
            assert!(meta["engine_blocked"].is_array());
            // Verify hits_from_metadata can parse the results.
            let hits = hits_from_metadata(&meta);
            // May be empty if all backends are blocked, but should not panic.
            let _ = hits;
        }
        Err(e) => {
            // Network errors are acceptable in CI; just verify it's not a
            // missing-key error (the wrapper no longer requires a key).
            let msg = e.to_string();
            assert!(
                !msg.contains("No search API key configured"),
                "wrapper should not require a Tavily key: {msg}"
            );
        }
    }
}

#[test]
fn test_metadata_results_shape() {
    // Verify that hits_from_metadata correctly parses the results array
    // shape emitted by the refactored WebSearchTool.
    let metadata = json!({
        "query": "rust async",
        "count": 2,
        "line_count": 6,
        "results": [
            {
                "title": "Rust Async Book",
                "url": "https://rust-lang.github.io/async-book/",
                "snippet": "A guide to async programming in Rust.",
                "search_tool": "websearch",
                "search_engine": "duckduckgo, brave"
            },
            {
                "title": "Tokio Tutorial",
                "url": "https://tokio.rs/tokio/tutorial",
                "snippet": "Learn how to use Tokio for async Rust.",
                "search_tool": "websearch",
                "search_engine": "duckduckgo"
            }
        ],
        "engines_used": ["duckduckgo", "brave"],
        "engine_blocked": [],
        "cached": false,
        "duration_ms": 1234
    });

    let hits = hits_from_metadata(&metadata);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "Rust Async Book");
    assert_eq!(hits[0].url, "https://rust-lang.github.io/async-book/");
    assert_eq!(hits[0].snippet, "A guide to async programming in Rust.");
    assert_eq!(hits[0].search_tool, "websearch");
    assert_eq!(hits[0].search_engine, "duckduckgo, brave");
    assert_eq!(hits[1].title, "Tokio Tutorial");
    assert_eq!(hits[1].search_engine, "duckduckgo");
}

#[test]
fn test_hits_from_metadata_empty_on_missing_results() {
    let metadata = json!({"query": "rust"});
    let hits = hits_from_metadata(&metadata);
    assert!(hits.is_empty());
}

#[test]
fn test_hits_from_metadata_handles_empty_results_array() {
    let metadata = json!({"query": "rust", "results": []});
    let hits = hits_from_metadata(&metadata);
    assert!(hits.is_empty());
}

#[test]
fn test_orchestrator_includes_tavily_when_key_present() {
    // Verify that the orchestrator built by the websearch wrapper includes
    // Tavily when a key is configured (via MfSearchTool::build_orchestrator).
    use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;

    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_tavily_key("tvly-test-key"));
    let names = orchestrator.engine_names();
    assert!(names.contains(&"tavily"));
    assert!(names.contains(&"duckduckgo"));
    assert!(names.contains(&"brave"));
}

#[test]
fn test_orchestrator_omits_tavily_when_no_key() {
    use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;

    let orchestrator = MfSearchTool::build_orchestrator(&ctx());
    let names = orchestrator.engine_names();
    assert!(!names.contains(&"tavily"));
    assert!(names.contains(&"duckduckgo"));
    assert!(names.contains(&"brave"));
}
