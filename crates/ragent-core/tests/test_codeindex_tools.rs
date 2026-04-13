#![allow(missing_docs)]
//! Integration tests for M5 codeindex tools.

use ragent_core::event::EventBus;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Build a ToolContext with code_index = None (disabled).
fn ctx_disabled() -> ToolContext {
    ToolContext {
        session_id: "test-codeindex".to_string(),
        working_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

/// Build a ToolContext with an in-memory CodeIndex populated with test data.
fn ctx_with_index() -> (ToolContext, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    // Create a Rust source file with known symbols.
    std::fs::write(
        src_dir.join("lib.rs"),
        r#"
/// A configuration parser.
pub fn parse_config(path: &str) -> Result<Config, Error> {
    todo!()
}

/// The application configuration.
pub struct Config {
    pub name: String,
    pub port: u16,
}

/// Custom error type.
pub enum Error {
    Io(std::io::Error),
    Parse(String),
}

impl Config {
    pub fn default_port() -> u16 {
        8080
    }
}

use std::io;
use std::path::Path;
"#,
    )
    .unwrap();

    // Create a second file.
    std::fs::write(
        src_dir.join("server.rs"),
        r#"
use crate::Config;

/// Start the HTTP server.
pub fn start_server(config: &Config) -> Result<(), crate::Error> {
    let port = Config::default_port();
    todo!()
}
"#,
    )
    .unwrap();

    let config = ragent_code::types::CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir: dir.path().join(".ragent").join("index"),
        scan_config: ragent_code::types::ScanConfig::default(),
    };

    std::fs::create_dir_all(&config.index_dir).unwrap();

    let idx = ragent_code::CodeIndex::open(&config).expect("open index");
    idx.full_reindex().expect("reindex");

    let ctx = ToolContext {
        session_id: "test-codeindex".to_string(),
        working_dir: dir.path().to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: Some(Arc::new(idx)),
    };

    (ctx, dir)
}

// ── Disabled (code_index = None) tests ──────────────────────────────────────

#[tokio::test]
async fn test_search_disabled() {
    use ragent_core::tool::codeindex_search::CodeIndexSearchTool;
    let out = CodeIndexSearchTool
        .execute(json!({"query": "parse"}), &ctx_disabled())
        .await
        .unwrap();
    assert!(out.content.contains("not available"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["error"], "codeindex_disabled");
    let fallbacks = meta["fallback_tools"].as_array().unwrap();
    assert!(!fallbacks.is_empty());
}

#[tokio::test]
async fn test_symbols_disabled() {
    use ragent_core::tool::codeindex_symbols::CodeIndexSymbolsTool;
    let out = CodeIndexSymbolsTool
        .execute(json!({}), &ctx_disabled())
        .await
        .unwrap();
    assert!(out.content.contains("not available"));
}

#[tokio::test]
async fn test_references_disabled() {
    use ragent_core::tool::codeindex_references::CodeIndexReferencesTool;
    let out = CodeIndexReferencesTool
        .execute(json!({"symbol": "Config"}), &ctx_disabled())
        .await
        .unwrap();
    assert!(out.content.contains("not available"));
}

#[tokio::test]
async fn test_dependencies_disabled() {
    use ragent_core::tool::codeindex_dependencies::CodeIndexDependenciesTool;
    let out = CodeIndexDependenciesTool
        .execute(json!({"path": "src/lib.rs"}), &ctx_disabled())
        .await
        .unwrap();
    assert!(out.content.contains("not available"));
}

#[tokio::test]
async fn test_status_disabled() {
    use ragent_core::tool::codeindex_status::CodeIndexStatusTool;
    let out = CodeIndexStatusTool
        .execute(json!({}), &ctx_disabled())
        .await
        .unwrap();
    assert!(out.content.contains("not available"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["enabled"], false);
}

#[tokio::test]
async fn test_reindex_disabled() {
    use ragent_core::tool::codeindex_reindex::CodeIndexReindexTool;
    let out = CodeIndexReindexTool
        .execute(json!({}), &ctx_disabled())
        .await
        .unwrap();
    assert!(out.content.contains("not available"));
}

// ── Enabled (code_index populated) tests ────────────────────────────────────

#[tokio::test]
async fn test_search_finds_function() {
    use ragent_core::tool::codeindex_search::CodeIndexSearchTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexSearchTool
        .execute(json!({"query": "parse_config"}), &ctx)
        .await
        .unwrap();
    assert!(
        out.content.contains("parse_config"),
        "Expected parse_config in results: {}",
        out.content
    );
    let meta = out.metadata.unwrap();
    assert!(meta["total_results"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_search_with_kind_filter() {
    use ragent_core::tool::codeindex_search::CodeIndexSearchTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexSearchTool
        .execute(
            json!({"query": "Config", "kind": "struct"}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        out.content.contains("Config"),
        "Expected Config struct: {}",
        out.content
    );
}

#[tokio::test]
async fn test_search_no_results() {
    use ragent_core::tool::codeindex_search::CodeIndexSearchTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexSearchTool
        .execute(json!({"query": "nonexistent_xyz_1234"}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("No results"));
}

#[tokio::test]
async fn test_symbols_lists_all() {
    use ragent_core::tool::codeindex_symbols::CodeIndexSymbolsTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexSymbolsTool
        .execute(json!({}), &ctx)
        .await
        .unwrap();
    // Should contain at least parse_config and Config
    assert!(
        out.content.contains("parse_config") || out.content.contains("Config"),
        "Expected symbols: {}",
        out.content
    );
    let meta = out.metadata.unwrap();
    assert!(meta["total_results"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_symbols_filter_by_name() {
    use ragent_core::tool::codeindex_symbols::CodeIndexSymbolsTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexSymbolsTool
        .execute(json!({"name": "Config"}), &ctx)
        .await
        .unwrap();
    assert!(
        out.content.contains("Config"),
        "Expected Config: {}",
        out.content
    );
}

#[tokio::test]
async fn test_symbols_no_match() {
    use ragent_core::tool::codeindex_symbols::CodeIndexSymbolsTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexSymbolsTool
        .execute(json!({"name": "zzz_no_match_1234"}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("No symbols"));
}

#[tokio::test]
async fn test_status_shows_stats() {
    use ragent_core::tool::codeindex_status::CodeIndexStatusTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexStatusTool
        .execute(json!({}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("Files indexed"));
    assert!(out.content.contains("Total symbols"));
    let meta = out.metadata.unwrap();
    assert_eq!(meta["enabled"], true);
    assert!(meta["files_indexed"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn test_reindex_succeeds() {
    use ragent_core::tool::codeindex_reindex::CodeIndexReindexTool;
    let (ctx, _dir) = ctx_with_index();
    let out = CodeIndexReindexTool
        .execute(json!({}), &ctx)
        .await
        .unwrap();
    assert!(out.content.contains("Re-index complete"));
    let meta = out.metadata.unwrap();
    // Second reindex may find 0 stale files — just verify it ran successfully.
    assert!(meta.get("elapsed_ms").is_some());
}

// ── Tool metadata tests ─────────────────────────────────────────────────────

#[test]
fn test_tool_names_and_schemas() {
    use ragent_core::tool::codeindex_search::CodeIndexSearchTool;
    use ragent_core::tool::codeindex_symbols::CodeIndexSymbolsTool;
    use ragent_core::tool::codeindex_references::CodeIndexReferencesTool;
    use ragent_core::tool::codeindex_dependencies::CodeIndexDependenciesTool;
    use ragent_core::tool::codeindex_status::CodeIndexStatusTool;
    use ragent_core::tool::codeindex_reindex::CodeIndexReindexTool;

    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(CodeIndexSearchTool),
        Box::new(CodeIndexSymbolsTool),
        Box::new(CodeIndexReferencesTool),
        Box::new(CodeIndexDependenciesTool),
        Box::new(CodeIndexStatusTool),
        Box::new(CodeIndexReindexTool),
    ];

    let expected_names = [
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_reindex",
    ];

    for (tool, expected_name) in tools.iter().zip(expected_names.iter()) {
        assert_eq!(tool.name(), *expected_name);
        assert!(!tool.description().is_empty());
        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(tool.permission_category().starts_with("codeindex:"));
    }
}

#[test]
fn test_tools_registered_in_default_registry() {
    let registry = ragent_core::tool::create_default_registry();
    let expected = [
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_reindex",
    ];
    for name in &expected {
        assert!(
            registry.get(name).is_some(),
            "Tool '{}' not found in default registry",
            name
        );
    }
}
