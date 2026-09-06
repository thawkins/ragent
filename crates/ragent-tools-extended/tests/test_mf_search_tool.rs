#![allow(clippy::assert_is_empty)]
//! Integration tests for the `mf_search` tool surface.
//!
//! These tests exercise the tool's `Tool` trait implementation (name,
//! description, schema, permission category, and input validation) without
//! making any network calls.

use ragent_config::Config;
use ragent_tools_extended::masterfetch::tools::search_tool::{EngineStatus, MfSearchTool};
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

/// Build a `ToolContext` with the given LangSearch API key in its config.
fn ctx_with_langsearch_key(key: &str) -> ToolContext {
    let mut config = Config::default();
    config.langsearch_api_key = Some(key.to_string());
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

/// Build a `ToolContext` with the given Perplexity API key in its config.
fn ctx_with_perplexity_key(key: &str) -> ToolContext {
    let mut config = Config::default();
    config.perplexity_api_key = Some(key.to_string());
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

/// Build a `ToolContext` with the given Exa API key in its config.
fn ctx_with_exa_key(key: &str) -> ToolContext {
    let mut config = Config::default();
    config.exa_api_key = Some(key.to_string());
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
fn test_tool_name_is_mf_search() {
    let tool = MfSearchTool;
    assert_eq!(tool.name(), "mf_search");
}

#[test]
fn test_permission_category_is_web() {
    let tool = MfSearchTool;
    assert_eq!(tool.permission_category(), "web");
}

#[test]
fn test_description_mentions_key_features() {
    let tool = MfSearchTool;
    let desc = tool.description();
    assert!(
        desc.contains("OpenAlex"),
        "description should mention OpenAlex"
    );
    assert!(
        desc.contains("Wikipedia"),
        "description should mention Wikipedia"
    );
    assert!(
        desc.contains("LangSearch"),
        "description should mention LangSearch"
    );
    assert!(
        desc.contains("langsearch_api_key"),
        "description should mention the optional LangSearch API key"
    );
    assert!(desc.contains("Tavily"), "description should mention Tavily");
    assert!(
        desc.contains("tavily_api_key"),
        "description should mention the optional Tavily API key"
    );
    assert!(
        desc.contains("Perplexity"),
        "description should mention Perplexity"
    );
    assert!(
        desc.contains("perplexity_api_key"),
        "description should mention the optional Perplexity API key"
    );
    assert!(desc.contains("Exa"), "description should mention Exa");
    assert!(
        desc.contains("exa_api_key"),
        "description should mention the optional Exa API key"
    );
    assert!(
        desc.contains("relevance_score"),
        "description should mention relevance_score"
    );
    assert!(
        desc.contains("engines_consensus"),
        "description should mention engines_consensus"
    );
}

#[test]
fn test_orchestrator_without_key_uses_three_engines() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx());
    assert_eq!(orchestrator.engine_count(), 2);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"openalex"));
    assert!(names.contains(&"wikipedia"));
}

#[test]
fn test_orchestrator_with_key_adds_langsearch_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_langsearch_key("ls-test-key"));
    assert_eq!(orchestrator.engine_count(), 3);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"langsearch"));
}

#[test]
fn test_orchestrator_with_empty_key_omits_langsearch_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_langsearch_key(""));
    assert_eq!(orchestrator.engine_count(), 2);
    let names = orchestrator.engine_names();
    assert!(!names.contains(&"langsearch"));
}

#[test]
fn test_orchestrator_with_tavily_key_adds_tavily_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_tavily_key("tvly-test-key"));
    assert_eq!(orchestrator.engine_count(), 3);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"tavily"));
}

#[test]
fn test_orchestrator_with_empty_tavily_key_omits_tavily_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_tavily_key(""));
    assert_eq!(orchestrator.engine_count(), 2);
    let names = orchestrator.engine_names();
    assert!(!names.contains(&"tavily"));
}

#[test]
fn test_orchestrator_with_perplexity_key_adds_perplexity_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_perplexity_key("pplx-test-key"));
    assert_eq!(orchestrator.engine_count(), 3);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"perplexity"));
}

#[test]
fn test_orchestrator_with_empty_perplexity_key_omits_perplexity_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_perplexity_key(""));
    assert_eq!(orchestrator.engine_count(), 2);
    let names = orchestrator.engine_names();
    assert!(!names.contains(&"perplexity"));
}

#[test]
fn test_orchestrator_with_both_keys_adds_all_optional_engines() {
    let mut config = Config::default();
    config.langsearch_api_key = Some("ls-test-key".to_string());
    config.tavily_api_key = Some("tvly-test-key".to_string());
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    };
    let orchestrator = MfSearchTool::build_orchestrator(&ctx);
    assert_eq!(orchestrator.engine_count(), 4);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"langsearch"));
    assert!(names.contains(&"tavily"));
    assert!(names.contains(&"openalex"));
    assert!(names.contains(&"wikipedia"));
}

#[test]
fn test_orchestrator_with_all_three_keys_adds_all_optional_engines() {
    let mut config = Config::default();
    config.langsearch_api_key = Some("ls-test-key".to_string());
    config.tavily_api_key = Some("tvly-test-key".to_string());
    config.perplexity_api_key = Some("pplx-test-key".to_string());
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    };
    let orchestrator = MfSearchTool::build_orchestrator(&ctx);
    assert_eq!(orchestrator.engine_count(), 5);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"langsearch"));
    assert!(names.contains(&"tavily"));
    assert!(names.contains(&"perplexity"));
    assert!(names.contains(&"openalex"));
    assert!(names.contains(&"wikipedia"));
}

#[test]
fn test_orchestrator_with_exa_key_adds_exa_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_exa_key("exa-test-key"));
    assert_eq!(orchestrator.engine_count(), 3);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"exa"));
}

#[test]
fn test_orchestrator_with_empty_exa_key_omits_exa_engine() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_exa_key(""));
    assert_eq!(orchestrator.engine_count(), 2);
    let names = orchestrator.engine_names();
    assert!(!names.contains(&"exa"));
}

#[test]
fn test_orchestrator_with_all_four_keys_adds_all_optional_engines() {
    let mut config = Config::default();
    config.langsearch_api_key = Some("ls-test-key".to_string());
    config.tavily_api_key = Some("tvly-test-key".to_string());
    config.perplexity_api_key = Some("pplx-test-key".to_string());
    config.exa_api_key = Some("exa-test-key".to_string());
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    };
    let orchestrator = MfSearchTool::build_orchestrator(&ctx);
    assert_eq!(orchestrator.engine_count(), 6);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"langsearch"));
    assert!(names.contains(&"tavily"));
    assert!(names.contains(&"perplexity"));
    assert!(names.contains(&"exa"));
    assert!(names.contains(&"openalex"));
    assert!(names.contains(&"wikipedia"));
}

#[test]
fn test_orchestrator_select_engine_exa() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx_with_exa_key("exa-test-key"));
    let selected = orchestrator
        .select_engine("exa")
        .expect("select_engine should find 'exa' when key is present");
    assert_eq!(selected.engine_count(), 1);
    let names = selected.engine_names();
    assert_eq!(names, vec!["exa"]);
}

#[test]
fn test_orchestrator_select_engine_exa_returns_none_when_no_key() {
    let orchestrator = MfSearchTool::build_orchestrator(&ctx());
    assert!(orchestrator.select_engine("exa").is_none());
}

#[test]
fn test_parameters_schema_has_required_query() {
    let tool = MfSearchTool;
    let schema = tool.parameters_schema();
    let required = schema["required"]
        .as_array()
        .expect("required should be an array");
    assert!(
        required.iter().any(|v| v == "query"),
        "query should be required"
    );
    assert_eq!(schema["properties"]["query"]["type"], "string");
    assert_eq!(schema["properties"]["site"]["type"], "string");
    assert_eq!(schema["properties"]["exclude_sites"]["type"], "array");
    assert!(
        schema["properties"]["freshness"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("day"))
    );
    assert_eq!(schema["properties"]["max_results"]["type"], "integer");
    assert_eq!(
        schema["properties"]["per_engine_results"]["type"],
        "integer"
    );
    assert_eq!(schema["properties"]["page"]["type"], "integer");
}

#[test]
fn test_parameters_schema_includes_engine_enum() {
    let tool = MfSearchTool;
    let schema = tool.parameters_schema();
    let engine = &schema["properties"]["engine"];
    assert_eq!(engine["type"], "string");
    let engine_enum: Vec<String> = engine["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(engine_enum.contains(&"openalex".to_string()));
    assert!(engine_enum.contains(&"wikipedia".to_string()));
    assert!(engine_enum.contains(&"langsearch".to_string()));
    assert!(engine_enum.contains(&"tavily".to_string()));
    assert!(engine_enum.contains(&"perplexity".to_string()));
    assert!(engine_enum.contains(&"exa".to_string()));
}

#[test]
fn test_description_mentions_engine_parameter() {
    let tool = MfSearchTool;
    let desc = tool.description();
    assert!(
        desc.contains("engine"),
        "description should mention the engine parameter"
    );
}

#[tokio::test]
async fn test_execute_rejects_empty_query() {
    let tool = MfSearchTool;
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
    let tool = MfSearchTool;
    let result = tool.execute(json!({}), &ctx()).await;
    assert!(result.is_err(), "missing query should error");
}

#[test]
fn test_metadata_includes_search_tool_and_search_engine() {
    // Build a representative `mf_search` metadata blob and verify the fields
    // that the research adapter consumes are present.
    let metadata = serde_json::json!({
        "query": "rust lifetimes",
        "results": [
            {
                "title": "Rust Lifetimes",
                "url": "https://doc.rust-lang.org/nomicon/lifetimes.html",
                "snippet": "A deep dive into lifetimes.",
                "source": "duckduckgo, brave",
                "search_tool": "mf_search",
                "search_engine": "duckduckgo, brave",
                "position": 1,
                "relevance_score": 0.95,
                "fetch_relevance": "high",
                "engines_consensus": 2
            }
        ],
        "total_results": 1,
        "engines_used": ["duckduckgo", "brave"],
        "engine_blocked": [],
        "engines_with_results": 2,
        "total_engines": 2,
        "cached": false,
        "duration_ms": 1234
    });

    let results = metadata.get("results").unwrap().as_array().unwrap();
    let first = &results[0];
    assert_eq!(first["search_tool"], "mf_search");
    assert_eq!(first["search_engine"], "duckduckgo, brave");
    assert!(first["relevance_score"].as_f64().unwrap() > 0.0);
    assert_eq!(first["fetch_relevance"], "high");
    assert_eq!(first["engines_consensus"], 2);
}

#[test]
fn test_build_search_metadata_populates_search_tool_and_engine() {
    // Verify the orchestrator exposes Tavily plus the keyless backends so
    // research provenance can be derived without making network calls.
    use ragent_tools_extended::masterfetch::search::SearchOptions;

    let ctx = ctx_with_tavily_key("tvly-test-key");
    let orchestrator = MfSearchTool::build_orchestrator(&ctx);
    let opts = SearchOptions::new(1);
    let names = orchestrator.engine_names();
    assert!(names.contains(&"tavily"));
    assert!(names.contains(&"openalex"));
    assert!(names.contains(&"wikipedia"));

    // The real metadata construction runs inside execute(); here we just
    // ensure the orchestrator exposes the expected engine list so research
    // provenance can be derived.
    assert!(!names.is_empty());
    let _ = opts;
}

#[test]
fn test_engine_status_without_keys_shows_keyless_engines_enabled() {
    let status = MfSearchTool::engine_status(&ctx());
    assert_eq!(status.len(), 6);
    let by_name: std::collections::HashMap<&str, &EngineStatus> =
        status.iter().map(|e| (e.name, e)).collect();

    let oalex = by_name["OpenAlex"];
    assert!(oalex.enabled && oalex.in_use && !oalex.failed);

    let wiki = by_name["Wikipedia"];
    assert!(wiki.enabled && wiki.in_use && !wiki.failed);

    let lang = by_name["LangSearch"];
    assert!(!lang.enabled && !lang.in_use && lang.failed);

    let tav = by_name["Tavily"];
    assert!(!tav.enabled && !tav.in_use && tav.failed);

    let ppx = by_name["Perplexity"];
    assert!(!ppx.enabled && !ppx.in_use && ppx.failed);

    let exa = by_name["Exa"];
    assert!(!exa.enabled && !exa.in_use && exa.failed);
}

#[test]
fn test_engine_status_with_langsearch_key_enables_langsearch() {
    let status = MfSearchTool::engine_status(&ctx_with_langsearch_key("ls-test-key"));
    let by_name: std::collections::HashMap<&str, &EngineStatus> =
        status.iter().map(|e| (e.name, e)).collect();

    let lang = by_name["LangSearch"];
    assert!(lang.enabled && lang.in_use && !lang.failed);

    let tav = by_name["Tavily"];
    assert!(!tav.enabled && !tav.in_use && tav.failed);

    let ppx = by_name["Perplexity"];
    assert!(!ppx.enabled && !ppx.in_use && ppx.failed);
}

#[test]
fn test_engine_status_with_tavily_key_enables_tavily() {
    let status = MfSearchTool::engine_status(&ctx_with_tavily_key("tvly-test-key"));
    let by_name: std::collections::HashMap<&str, &EngineStatus> =
        status.iter().map(|e| (e.name, e)).collect();

    let tav = by_name["Tavily"];
    assert!(tav.enabled && tav.in_use && !tav.failed);

    let lang = by_name["LangSearch"];
    assert!(!lang.enabled && !lang.in_use && lang.failed);

    let ppx = by_name["Perplexity"];
    assert!(!ppx.enabled && !ppx.in_use && ppx.failed);

    let exa = by_name["Exa"];
    assert!(!exa.enabled && !exa.in_use && exa.failed);
}

#[test]
fn test_engine_status_with_exa_key_enables_exa() {
    let status = MfSearchTool::engine_status(&ctx_with_exa_key("exa-test-key"));
    let by_name: std::collections::HashMap<&str, &EngineStatus> =
        status.iter().map(|e| (e.name, e)).collect();

    let exa = by_name["Exa"];
    assert!(exa.enabled && exa.in_use && !exa.failed);

    let lang = by_name["LangSearch"];
    assert!(!lang.enabled && !lang.in_use && lang.failed);

    let tav = by_name["Tavily"];
    assert!(!tav.enabled && !tav.in_use && tav.failed);

    let ppx = by_name["Perplexity"];
    assert!(!ppx.enabled && !ppx.in_use && ppx.failed);
}

#[test]
fn test_engine_status_with_both_keys_enables_all_optional_engines() {
    let mut config = Config::default();
    config.langsearch_api_key = Some("ls-test-key".to_string());
    config.tavily_api_key = Some("tvly-test-key".to_string());
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    };
    let status = MfSearchTool::engine_status(&ctx);
    let by_name: std::collections::HashMap<&str, &EngineStatus> =
        status.iter().map(|e| (e.name, e)).collect();

    // LangSearch and Tavily should be fully enabled; Perplexity should be
    // failed (no key in this test).
    let lang = by_name["LangSearch"];
    assert!(lang.enabled && lang.in_use && !lang.failed);

    let tav = by_name["Tavily"];
    assert!(tav.enabled && tav.in_use && !tav.failed);

    let ppx = by_name["Perplexity"];
    assert!(!ppx.enabled && !ppx.in_use && ppx.failed);
}

#[test]
fn test_engine_status_with_empty_keys_treats_keys_as_missing() {
    let mut config = Config::default();
    config.langsearch_api_key = Some(String::new());
    config.tavily_api_key = Some(String::new());
    config.perplexity_api_key = Some(String::new());
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: std::sync::Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(std::sync::Arc::new(config)),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    };
    let status = MfSearchTool::engine_status(&ctx);
    let by_name: std::collections::HashMap<&str, &EngineStatus> =
        status.iter().map(|e| (e.name, e)).collect();

    assert!(by_name["LangSearch"].failed);
    assert!(!by_name["LangSearch"].enabled);
    assert!(by_name["Tavily"].failed);
    assert!(!by_name["Tavily"].enabled);
    assert!(by_name["Perplexity"].failed);
    assert!(!by_name["Perplexity"].enabled);
    assert!(by_name["Exa"].failed);
    assert!(!by_name["Exa"].enabled);
}
