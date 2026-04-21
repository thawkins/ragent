//! Diagnostic tests for FTS search issues
use std::path::Path;

/// Test raw FTS search on actual index
#[test]
fn diag_raw_fts_search() {
    let fts_path = Path::new("/home/thawkins/Projects/ragent/.ragent/codeindex/fts");
    if !fts_path.exists() {
        eprintln!("SKIP: FTS directory not found");
        return;
    }

    let fts = ragent_codeindex::search::FtsIndex::open(fts_path).unwrap();
    let count = fts.doc_count().unwrap();
    eprintln!("FTS doc_count: {count}");
    assert!(count > 0, "FTS should have docs");

    let results = fts.search("append_assistant_text", 10).unwrap();
    eprintln!(
        "FTS search('append_assistant_text'): {} results",
        results.len()
    );
    for r in &results {
        eprintln!(
            "  {:.3} {} ({}) @ {}:{}",
            r.score, r.symbol_name, r.kind, r.file_path, r.line
        );
    }
    assert!(
        !results.is_empty(),
        "should find results for 'append_assistant_text'"
    );
}

/// Test CodeIndex search (full pipeline)
#[test]
fn diag_codeindex_search() {
    let cwd = Path::new("/home/thawkins/Projects/ragent");
    let index_dir = cwd.join(".ragent/codeindex");
    if !index_dir.exists() {
        eprintln!("SKIP: codeindex directory not found");
        return;
    }

    let config = ragent_codeindex::types::CodeIndexConfig {
        enabled: true,
        project_root: cwd.to_path_buf(),
        index_dir,
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };

    let idx = ragent_codeindex::CodeIndex::open(&config).unwrap();

    // Check status
    let status = idx.status().unwrap();
    eprintln!(
        "CodeIndex status: files={}, symbols={}, fts_docs={}",
        status.files_indexed, status.total_symbols, status.fts_doc_count
    );

    // Search
    let query = ragent_codeindex::types::SearchQuery {
        query: "append_assistant_text".to_string(),
        kind: None,
        language: None,
        file_pattern: None,
        max_results: 10,
        include_body: false,
    };
    let results = idx.search(&query).unwrap();
    eprintln!(
        "CodeIndex search('append_assistant_text'): {} results",
        results.len()
    );
    for r in &results {
        eprintln!(
            "  {:.3} {} ({}) @ {}:{}",
            r.score, r.symbol_name, r.kind, r.file_path, r.line
        );
    }
    assert!(
        !results.is_empty(),
        "CodeIndex should find results for 'append_assistant_text'"
    );
}
