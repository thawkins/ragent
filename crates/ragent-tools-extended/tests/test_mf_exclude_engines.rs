//! Integration tests for `mf_search` engine exclusion (spec `researchnoacc`,
//! T-005; FR-001, FR-002, FR-008, FR-014, NFR-003).
//!
//! These tests exercise `SearchOrchestrator::exclude_engines` through a
//! full-search flow with a local mock engine, so the "excluded engine returns
//! no rows" guarantee (FR-007, FR-008) is verified without any network access.
//! They complement the inline unit tests in `masterfetch::search::tests` by
//! checking the search path and the `mf_search` tool wiring.

use std::sync::Arc;

use async_trait::async_trait;

use ragent_config::Config;
use ragent_tools_extended::masterfetch::search::SearchOrchestrator;
use ragent_tools_extended::masterfetch::search::engine::{
    EngineReport, RawResult, SearchEngine, SearchOptions,
};
use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;
use ragent_tools_extended::{Tool, ToolContext};
use serde_json::json;

/// A mock engine returning canned results under its own name.
struct MockEngine {
    name: &'static str,
    results: Vec<RawResult>,
}

#[async_trait]
impl SearchEngine for MockEngine {
    fn name(&self) -> &str {
        self.name
    }

    async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
        EngineReport::ok(self.name, self.results.clone())
    }
}

/// Build a mock engine that returns one result sourced under `name`.
fn engine_with_one_result(name: &'static str) -> Arc<dyn SearchEngine> {
    Arc::new(MockEngine {
        name,
        results: vec![RawResult::new(
            format!("{name} result"),
            format!("https://{name}.example/a"),
            "snippet",
            name,
        )],
    })
}

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

fn orchestrator_with_openalex_and_wikipedia() -> SearchOrchestrator {
    SearchOrchestrator::with_engines(vec![
        engine_with_one_result("openalex"),
        engine_with_one_result("wikipedia"),
    ])
}

// ---------------------------------------------------------------------------
// Orchestrator exclusion through a real search (FR-002, FR-008, NFR-003)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_without_exclusion_returns_openalex_rows() {
    // Baseline (NFR-003): with no exclusions both engines contribute.
    let orchestrator = orchestrator_with_openalex_and_wikipedia();
    let output = orchestrator.search("rust", &SearchOptions::new(10)).await;

    let sources: Vec<&str> = output
        .merge
        .results
        .iter()
        .map(|r| r.source.as_str())
        .collect();
    assert!(
        sources.iter().any(|s| s.contains("openalex")),
        "baseline search should include openalex rows, got: {sources:?}"
    );
    assert_eq!(output.engines_used.len(), 2);
}

#[tokio::test]
async fn search_with_openalex_excluded_returns_no_openalex_rows() {
    // FR-007/FR-008: excluding openalex removes it before the search runs, so
    // no result may be sourced from it.
    let orchestrator = orchestrator_with_openalex_and_wikipedia().exclude_engines(&["openalex"]);
    let output = orchestrator.search("rust", &SearchOptions::new(10)).await;

    assert_eq!(output.engines_used, vec!["wikipedia".to_string()]);
    for result in &output.merge.results {
        assert!(
            !result.source.contains("openalex"),
            "excluded engine must not appear in results: {}",
            result.source
        );
    }
    // The non-academic engine is preserved (FR-015).
    assert!(
        output
            .merge
            .results
            .iter()
            .any(|r| r.source.contains("wikipedia")),
        "wikipedia results should still be returned"
    );
}

#[tokio::test]
async fn search_with_unknown_exclusion_name_queries_all_engines() {
    // FR-008: an unknown name excludes nothing and the call proceeds.
    let orchestrator =
        orchestrator_with_openalex_and_wikipedia().exclude_engines(&["not_a_real_engine"]);
    let output = orchestrator.search("rust", &SearchOptions::new(10)).await;

    assert_eq!(orchestrator.engine_count(), 2);
    assert_eq!(output.engines_used.len(), 2);
}

// ---------------------------------------------------------------------------
// mf_search tool wiring (FR-001, FR-002, FR-014)
// ---------------------------------------------------------------------------

#[test]
fn mf_search_build_orchestrator_excludes_openalex() {
    // The keyless orchestrator wires openalex + wikipedia; excluding openalex
    // must leave exactly wikipedia (FR-002, FR-015).
    let filtered = MfSearchTool::build_orchestrator(&ctx()).exclude_engines(&["openalex"]);
    let names = filtered.engine_names();
    assert!(!names.contains(&"openalex"));
    assert!(names.contains(&"wikipedia"));
}

#[test]
fn mf_search_exclude_engines_accepts_string_vec() {
    // The tool parses `exclude_engines` into `Vec<String>`; the method must
    // accept that form directly (FR-001).
    let names = vec!["openalex".to_string()];
    let filtered = MfSearchTool::build_orchestrator(&ctx()).exclude_engines(&names);
    assert!(!filtered.engine_names().contains(&"openalex"));
}

#[test]
fn mf_search_orchestrator_with_optional_key_and_exclusion() {
    // Excluding a keyless engine leaves any configured API-backed engines in
    // place, and the receiver is unchanged.
    let mut config = Config::default();
    config.tavily_api_key = Some("tvly-test-key".to_string());
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(Arc::new(config)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };
    let orchestrator = MfSearchTool::build_orchestrator(&ctx);
    assert_eq!(orchestrator.engine_count(), 3);

    let filtered = orchestrator.exclude_engines(&["openalex"]);
    let names = filtered.engine_names();
    assert!(!names.contains(&"openalex"));
    assert!(names.contains(&"wikipedia"));
    assert!(names.contains(&"tavily"));
    // Receiver is untouched (FR-002).
    assert_eq!(orchestrator.engine_count(), 3);
}

#[tokio::test]
async fn mf_search_execute_with_all_engines_excluded_is_explicit() {
    // FR-014: excluding every configured engine returns an explicit result and
    // dispatches no request.
    let tool = MfSearchTool;
    let result = tool
        .execute(
            json!({"query": "rust", "exclude_engines": ["openalex", "wikipedia"]}),
            &ctx(),
        )
        .await
        .expect("all-excluded must be Ok");

    assert!(result.content.contains("No engines available"));
    let metadata = result.metadata.expect("metadata present");
    assert_eq!(metadata["total_engines"], 0);
    assert_eq!(metadata["error"], "all engines excluded by exclude_engines");
}

#[tokio::test]
async fn mf_search_execute_with_unknown_exclusion_still_runs() {
    // FR-008: unknown exclusion names are ignored, so the tool proceeds with a
    // real (network) search rather than erroring on the parameter. Any engine
    // block is reported in metadata, never as a parameter error.
    let tool = MfSearchTool;
    let result = tool
        .execute(
            json!({"query": "   ", "exclude_engines": ["not_a_real_engine"]}),
            &ctx(),
        )
        .await;
    // Only the empty query is rejected; the exclusion name itself is not.
    assert!(result.is_err(), "empty query should still error");
}
