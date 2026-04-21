#![allow(missing_docs)]

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{CodeIndexConfig, ScanConfig, SearchQuery, SymbolFilter, SymbolKind};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Create a temp directory with sample Rust files for indexing.
fn create_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("lib.rs"),
        r#"
//! Main library for the sample project.

/// Application configuration loaded from disk.
pub struct Config {
    pub name: String,
    pub port: u16,
}

/// Parse configuration from a TOML file.
pub fn parse_config(path: &str) -> Config {
    Config {
        name: path.to_string(),
        port: 8080,
    }
}

/// A helper to validate configuration values.
fn validate_config(config: &Config) -> bool {
    !config.name.is_empty() && config.port > 0
}
"#,
    )
    .unwrap();

    fs::write(
        src.join("server.rs"),
        r#"
use crate::Config;

/// An HTTP server that serves requests.
pub struct Server {
    pub config: Config,
    pub running: bool,
}

impl Server {
    /// Create a new server with the given configuration.
    pub fn new(config: Config) -> Self {
        Server {
            config,
            running: false,
        }
    }

    /// Start the server, binding to the configured port.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// Stop the server gracefully.
    pub fn stop(&mut self) {
        self.running = false;
    }
}

/// Available log levels.
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}
"#,
    )
    .unwrap();

    dir
}

/// Build a CodeIndex pointing at the given temp directory.
fn open_index(dir: &TempDir) -> CodeIndex {
    let config = CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir: dir.path().join(".ragent").join("codeindex"),
        scan_config: ScanConfig::default(),
    };
    CodeIndex::open(&config).unwrap()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn test_full_reindex() {
    let dir = create_project();
    let idx = open_index(&dir);

    let result = idx.full_reindex().unwrap();
    assert_eq!(result.files_added, 2, "should add lib.rs and server.rs");
    assert_eq!(result.files_removed, 0);
    assert!(result.symbols_extracted > 0, "should extract symbols");
    assert!(result.elapsed_ms < 30_000, "should complete quickly");

    // Display formatting should work.
    let display = format!("{result}");
    assert!(display.contains("+2"));
}

#[test]
fn test_search_by_name() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let results = idx.search(&SearchQuery::new("parse_config")).unwrap();
    assert!(!results.is_empty(), "should find parse_config");
    assert_eq!(results[0].symbol_name, "parse_config");
}

#[test]
fn test_search_by_doc_comment() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let results = idx.search(&SearchQuery::new("HTTP server")).unwrap();
    assert!(!results.is_empty(), "should find by doc comment");
    // The Server struct's doc comment mentions "HTTP server"
    assert!(
        results
            .iter()
            .any(|r| r.symbol_name == "Server" || r.doc_snippet.contains("HTTP")),
        "should match Server via doc"
    );
}

#[test]
fn test_search_with_kind_filter() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let query = SearchQuery {
        query: "Config".to_string(),
        kind: Some(SymbolKind::Struct),
        max_results: 10,
        ..Default::default()
    };
    let results = idx.search(&query).unwrap();
    assert!(!results.is_empty());
    // All results should be structs.
    for r in &results {
        assert_eq!(r.kind, "struct", "kind filter should work: got {}", r.kind);
    }
}

#[test]
fn test_search_with_file_pattern() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let query = SearchQuery {
        query: "Config".to_string(),
        file_pattern: Some("server".to_string()),
        max_results: 10,
        ..Default::default()
    };
    let results = idx.search(&query).unwrap();
    // Should only return results from server.rs
    for r in &results {
        assert!(
            r.file_path.contains("server"),
            "file pattern filter should work: got {}",
            r.file_path
        );
    }
}

#[test]
fn test_symbols_query() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let syms = idx
        .symbols(&SymbolFilter {
            kind: Some(SymbolKind::Function),
            ..Default::default()
        })
        .unwrap();
    assert!(
        syms.len() >= 2,
        "should find at least parse_config and validate_config"
    );

    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"parse_config"));
}

#[test]
fn test_symbols_query_by_name() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let syms = idx
        .symbols(&SymbolFilter {
            name: Some("Server".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert!(!syms.is_empty());
    assert!(syms.iter().any(|s| s.name == "Server"));
}

#[test]
fn test_status() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let stats = idx.status().unwrap();
    assert_eq!(stats.files_indexed, 2);
    assert!(stats.total_symbols > 0);
    assert!(stats.total_bytes > 0);
    assert!(!stats.languages.is_empty());
    assert!(
        stats.index_size_bytes > 0,
        "on-disk size should be computed"
    );
}

#[test]
fn test_remove_file() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let before = idx.status().unwrap();
    idx.remove_file(std::path::Path::new("src/server.rs"))
        .unwrap();
    let after = idx.status().unwrap();

    assert_eq!(after.files_indexed, before.files_indexed - 1);
    assert!(after.total_symbols < before.total_symbols);

    // FTS should no longer find Server.
    let results = idx.search(&SearchQuery::new("Server")).unwrap();
    assert!(
        results.is_empty(),
        "Server should not appear after remove_file"
    );
}

#[test]
fn test_incremental_reindex() {
    let dir = create_project();
    let idx = open_index(&dir);
    let r1 = idx.full_reindex().unwrap();
    assert_eq!(r1.files_added, 2);

    // Second reindex with no changes should be a no-op.
    let r2 = idx.full_reindex().unwrap();
    assert_eq!(r2.files_added, 0);
    assert_eq!(r2.files_updated, 0);
    assert_eq!(r2.files_removed, 0);
}

#[test]
fn test_incremental_after_file_change() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    // Modify server.rs — add a new function.
    let server_path = dir.path().join("src/server.rs");
    let mut content = fs::read_to_string(&server_path).unwrap();
    content.push_str("\n/// A brand new function.\npub fn new_endpoint() {}\n");
    fs::write(&server_path, content).unwrap();

    let r2 = idx.full_reindex().unwrap();
    assert_eq!(r2.files_updated, 1, "server.rs should be updated");
    assert_eq!(r2.files_added, 0);

    // The new function should be searchable.
    let results = idx.search(&SearchQuery::new("new_endpoint")).unwrap();
    assert!(!results.is_empty(), "new_endpoint should be in FTS");
}

#[test]
fn test_incremental_after_file_delete() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    // Delete server.rs.
    fs::remove_file(dir.path().join("src/server.rs")).unwrap();

    let r2 = idx.full_reindex().unwrap();
    assert_eq!(r2.files_removed, 1, "server.rs should be removed");

    let stats = idx.status().unwrap();
    assert_eq!(stats.files_indexed, 1);
}

#[test]
fn test_index_file_directly() {
    let dir = create_project();
    let idx = open_index(&dir);

    // Index just one file.
    idx.index_file(&PathBuf::from("src/lib.rs")).unwrap();

    let stats = idx.status().unwrap();
    assert_eq!(stats.files_indexed, 1);
    assert!(stats.total_symbols > 0);

    // Should be searchable via FTS.
    let results = idx.search(&SearchQuery::new("parse_config")).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_search_result_display() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let results = idx.search(&SearchQuery::new("parse_config")).unwrap();
    assert!(!results.is_empty());

    // Compact display.
    let compact = format!("{}", results[0]);
    assert!(compact.contains("parse_config"));

    // Detailed display.
    let detailed = format!("{:#}", results[0]);
    assert!(detailed.contains("parse_config"));
}

#[test]
fn test_in_memory_index() {
    let dir = create_project();
    let config = CodeIndexConfig {
        project_root: dir.path().to_path_buf(),
        ..Default::default()
    };
    let idx = CodeIndex::open_in_memory(&config).unwrap();

    // Cannot full_reindex with in-memory because it needs project_root on disk.
    // But we can index individual files.
    idx.index_file(&PathBuf::from("src/lib.rs")).unwrap();

    let results = idx.search(&SearchQuery::new("parse_config")).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_search_max_results() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let query = SearchQuery {
        query: "Config".to_string(),
        max_results: 1,
        ..Default::default()
    };
    let results = idx.search(&query).unwrap();
    assert!(results.len() <= 1);
}

#[test]
fn test_methods_found() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let methods = idx
        .symbols(&SymbolFilter {
            kind: Some(SymbolKind::Method),
            ..Default::default()
        })
        .unwrap();
    let names: Vec<&str> = methods.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"new"), "should find Server::new");
    assert!(names.contains(&"start"), "should find Server::start");
    assert!(names.contains(&"stop"), "should find Server::stop");
}

#[test]
fn test_enum_found_via_search() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    let results = idx.search(&SearchQuery::new("LogLevel")).unwrap();
    assert!(!results.is_empty(), "should find LogLevel enum via FTS");
    assert_eq!(results[0].symbol_name, "LogLevel");
}

#[test]
fn test_ensure_fts_sync_rebuilds_empty_fts() {
    let dir = create_project();
    let idx = open_index(&dir);
    idx.full_reindex().unwrap();

    // Verify search works initially.
    let results = idx.search(&SearchQuery::new("parse_config")).unwrap();
    assert!(
        !results.is_empty(),
        "should find parse_config after reindex"
    );

    // Simulate FTS loss by clearing it.
    {
        let fts = idx.fts_for_test();
        fts.clear().unwrap();
        assert_eq!(
            fts.doc_count().unwrap(),
            0,
            "FTS should be empty after clear"
        );
    }

    // Verify search fails with empty FTS.
    let results = idx.search(&SearchQuery::new("parse_config")).unwrap();
    assert!(
        results.is_empty(),
        "search should return empty after FTS clear"
    );

    // ensure_fts_sync should rebuild the FTS from SQLite.
    idx.ensure_fts_sync().unwrap();

    // Search should work again.
    let results = idx.search(&SearchQuery::new("parse_config")).unwrap();
    assert!(
        !results.is_empty(),
        "should find parse_config after FTS sync"
    );
    assert_eq!(results[0].symbol_name, "parse_config");
}
