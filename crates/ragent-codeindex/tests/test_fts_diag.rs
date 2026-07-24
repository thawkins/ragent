//! Diagnostic tests for FTS search issues
use std::path::Path;

/// Locate the project root by walking up from the current file.
fn project_root() -> std::path::PathBuf {
    // `CARGO_MANIFEST_DIR` points to crates/ragent-codeindex.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_or_else(
        |_| {
            Path::new(file!())
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf()
        },
        std::path::PathBuf::from,
    );
    // Workspace root is two levels up from the crate manifest dir.
    manifest_dir
        .parent()
        .expect("crate dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Copy a directory tree recursively.
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.as_ref().join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &dest)?;
        } else {
            std::fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

/// Test raw FTS search on actual index
#[test]
fn diag_raw_fts_search() {
    let project_root = project_root();
    let fts_path = project_root.join(".ragent/codeindex/fts");
    if !fts_path.exists() {
        eprintln!("SKIP: FTS directory not found");
        return;
    }

    // Work on a snapshot so a concurrently-running ragent process cannot lock the live index.
    let temp = tempfile::tempdir().unwrap();
    let snapshot_fts = temp.path().join("fts");
    copy_dir_all(&fts_path, &snapshot_fts).expect("copy fts snapshot");

    let fts = ragent_codeindex::search::FtsIndex::open(&snapshot_fts).unwrap();
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

/// Test `CodeIndex` search (full pipeline)
#[test]
fn diag_codeindex_search() {
    let project_root = project_root();
    let live_index_dir = project_root.join(".ragent/codeindex");
    if !live_index_dir.exists() {
        eprintln!("SKIP: codeindex directory not found");
        return;
    }

    // Work on a snapshot so a concurrently-running ragent process cannot lock the live index.
    let temp = tempfile::tempdir().unwrap();
    let index_dir = temp.path().join("codeindex");
    copy_dir_all(&live_index_dir, &index_dir).expect("copy codeindex snapshot");

    let config = ragent_codeindex::types::CodeIndexConfig {
        enabled: true,
        project_root,
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
