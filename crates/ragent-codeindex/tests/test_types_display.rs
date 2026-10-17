//! Coverage tests for the `Display` / helper methods on the core data types
//! (`crates/ragent-codeindex/src/types.rs`).
//!
//! The relocated inline tests never exercised `IndexStats`' multi-line
//! `Display`, `StaleDiff::total`, the graph branch of `IndexResult`'s
//! `Display`, or `SearchQuery::new`.

use chrono::{TimeZone, Utc};
use ragent_codeindex::types::{
    IndexResult, IndexStats, ScanConfig, ScannedFile, SearchQuery, StaleDiff,
};
use std::path::PathBuf;

#[test]
fn test_index_stats_display_full_populated() {
    let stats = IndexStats {
        files_indexed: 12,
        total_symbols: 340,
        total_bytes: 2048,
        languages: vec![("rust".to_string(), 10), ("python".to_string(), 2)],
        last_full_index: Some(Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()),
        last_incremental_update: Some(Utc.with_ymd_and_hms(2026, 1, 3, 4, 5, 6).unwrap()),
        index_size_bytes: 4096,
        fts_doc_count: 300,
        total_references: 55,
        graph_total_edges: 7,
        graph_nodes: 6,
        graph_communities: 2,
    };
    let s = format!("{stats}");
    assert!(s.contains("Files indexed: 12"), "{s}");
    assert!(s.contains("Total symbols: 340"), "{s}");
    assert!(s.contains("FTS documents: 300"), "{s}");
    assert!(s.contains("References:    55"), "{s}");
    assert!(s.contains("Total size:"), "{s}");
    // The language breakdown line is emitted with a comma separator.
    assert!(s.contains("Languages:"), "{s}");
    assert!(s.contains("rust (10)"), "{s}");
    assert!(s.contains(", python (2)"), "{s}");
    assert!(s.contains("Last full:"), "{s}");
    assert!(s.contains("Last incr:"), "{s}");
    assert!(s.contains("Index size:"), "{s}");
}

#[test]
fn test_index_stats_display_default() {
    // Default has no languages and no timestamps, so those branches are
    // skipped and only the core lines are emitted.
    let s = format!("{}", IndexStats::default());
    assert!(!s.contains("Languages:"), "{s}");
    assert!(!s.contains("Last full:"), "{s}");
    assert!(!s.contains("Last incr:"), "{s}");
    assert!(s.contains("Files indexed: 0"), "{s}");
}

#[test]
fn test_stale_diff_total_and_is_empty() {
    let mut diff = StaleDiff::default();
    assert!(diff.is_empty());
    assert_eq!(diff.total(), 0);

    diff.to_add.push(ScannedFile {
        path: PathBuf::from("a.rs"),
        hash: "h".to_string(),
        size: 1,
        language: Some("rust".to_string()),
        mtime_ns: 1,
        line_count: 1,
    });
    diff.to_update.push(ScannedFile {
        path: PathBuf::from("b.rs"),
        hash: "h".to_string(),
        size: 1,
        language: Some("rust".to_string()),
        mtime_ns: 1,
        line_count: 1,
    });
    diff.to_remove.push("c.rs".to_string());
    assert!(!diff.is_empty());
    assert_eq!(diff.total(), 3);
}

#[test]
fn test_index_result_display_without_graph() {
    let result = IndexResult {
        files_added: 1,
        files_updated: 0,
        files_removed: 0,
        symbols_extracted: 4,
        elapsed_ms: 10,
        edges_extracted: 0,
        edges_inferred: 0,
    };
    let s = format!("{result}");
    assert!(s.contains("+1"), "{s}");
    assert!(
        !s.contains("graph:"),
        "no graph clause when zero edges: {s}"
    );
}

#[test]
fn test_index_result_display_with_graph() {
    let result = IndexResult {
        files_added: 0,
        files_updated: 2,
        files_removed: 1,
        symbols_extracted: 9,
        elapsed_ms: 33,
        edges_extracted: 5,
        edges_inferred: 3,
    };
    let s = format!("{result}");
    assert!(s.contains("graph:"), "{s}");
    assert!(s.contains("8 edges"), "{s}");
    assert!(s.contains("5 EXTRACTED"), "{s}");
    assert!(s.contains("3 INFERRED"), "{s}");
}

#[test]
fn test_search_query_new_defaults() {
    let q = SearchQuery::new("parse_config");
    assert_eq!(q.query, "parse_config");
    assert_eq!(q.max_results, ragent_codeindex::types::DEFAULT_SEARCH_LIMIT);
    assert!(q.kind.is_none());
    assert!(q.language.is_none());
    assert!(q.file_pattern.is_none());
    assert!(!q.include_body);
}

#[test]
fn test_scan_config_default() {
    let cfg = ScanConfig::default();
    assert_eq!(cfg.max_file_size, 1_048_576);
    assert_eq!(cfg.extra_exclude_dirs.len(), 0);
    assert_eq!(cfg.extra_exclude_patterns.len(), 0);
}
