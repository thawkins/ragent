//! Integration tests for the Hyperresearch source vault (T-003).
//!
//! These tests exercise the public [`ragent_research::source_vault`] API using
//! a temporary directory so they do not interfere with the project research
//! tree.

use ragent_research::source_vault::{NewVaultSource, SourceVault, SourceVaultError};
use std::path::PathBuf;
use tempfile::TempDir;

fn sample_source(url: &str, title: &str, body: &str) -> NewVaultSource {
    NewVaultSource {
        url: url.to_string(),
        title: title.to_string(),
        fetch_timestamp: None,
        search_tool: "mf_search".to_string(),
        search_engine: "duckduckgo, brave".to_string(),
        media_type: "page".to_string(),
        content_type: None,
        body_text: body.to_string(),
    }
}

fn open_vault(temp: &TempDir, run_tag: &str) -> SourceVault {
    SourceVault::open_with_root(temp.path(), run_tag).expect("open vault")
}

#[test]
fn vault_creates_directory_tree_and_database() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "glp1-20250101-120000");

    assert!(vault.run_dir().exists());
    assert!(vault.raw_dir().exists());
    assert!(vault.db_path().exists());
}

#[test]
fn vault_stores_source_and_writes_raw_file() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-a");
    let source = sample_source(
        "https://example.com/article",
        "Example Article",
        "The quick brown fox studied cardiovascular outcomes.",
    );

    let stored = vault.store(&source).expect("store source");
    assert_eq!(stored.url, source.url);
    assert_eq!(stored.title, source.title);
    assert_eq!(stored.search_tool, source.search_tool);
    assert_eq!(stored.search_engine, source.search_engine);
    assert_eq!(stored.media_type, source.media_type);
    assert!(!stored.source_id.is_empty());
    assert!(stored.content_path.exists());
    assert!(stored.content_path.starts_with(vault.raw_dir()));

    let body = vault.read_content(&stored.source_id).expect("read content");
    assert_eq!(body, source.body_text);
}

#[test]
fn vault_deduplicates_by_url_within_run() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-b");
    let source = sample_source("https://example.com/dedup", "First", "original body");

    let first = vault.store(&source).expect("first store");
    let mut duplicate = source.clone();
    duplicate.title = "Second".to_string();
    duplicate.body_text = "different body".to_string();

    let second = vault.store(&duplicate).expect("second store");
    assert_eq!(first.id, second.id);
    assert_eq!(first.source_id, second.source_id);
    assert_eq!(first.title, "First");

    assert_eq!(vault.count().expect("count"), 1);
}

#[test]
fn vault_lists_newest_first() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-c");

    let one = sample_source("https://example.com/one", "One", "one body");
    let two = sample_source("https://example.com/two", "Two", "two body");
    vault.store(&one).unwrap();
    vault.store(&two).unwrap();

    let list = vault.list(10).expect("list");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].title, "Two");
    assert_eq!(list[1].title, "One");
}

#[test]
fn vault_searches_by_body_text() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-d");

    let relevant = sample_source(
        "https://example.com/glp1",
        "GLP-1 Study",
        "Patients taking GLP-1 agonists showed reduced cardiovascular events.",
    );
    let unrelated = sample_source(
        "https://example.com/cars",
        "Cars",
        "Electric vehicles are becoming more popular every year.",
    );
    vault.store(&relevant).unwrap();
    vault.store(&unrelated).unwrap();

    let hits = vault.search("cardiovascular GLP-1", 10).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "https://example.com/glp1");
}

#[test]
fn vault_search_handles_punctuation_without_errors() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-e");
    let source = sample_source(
        "https://example.com/punct",
        "Punctuation",
        "GLP-1 drugs (e.g. semaglutide) improve HbA1c!",
    );
    vault.store(&source).unwrap();

    let hits = vault
        .search("GLP-1, semaglutide; HbA1c?", 10)
        .expect("search");
    assert_eq!(hits.len(), 1);
}

#[test]
fn vault_find_by_url_exact_match() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-f");
    let source = sample_source("https://example.com/findme", "Find Me", "body");
    vault.store(&source).unwrap();

    let found = vault
        .find_by_url("https://example.com/findme")
        .expect("find");
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Find Me");

    let missing = vault
        .find_by_url("https://example.com/nope")
        .expect("missing");
    assert!(missing.is_none());
}

#[test]
fn vault_read_content_rejects_unknown_source() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-g");

    let err = vault
        .read_content("not-a-real-uuid")
        .expect_err("expected error");
    assert!(matches!(err, SourceVaultError::SourceNotFound(_)));
}

#[test]
fn vault_rejects_invalid_run_tag() {
    let temp = TempDir::new().unwrap();
    let err = SourceVault::open_with_root(temp.path(), "../escape")
        .expect_err("expected invalid run tag");
    assert!(matches!(err, SourceVaultError::InvalidRunTag(_)));
}

#[test]
fn vault_content_path_uses_pdf_extension_for_pdf_media() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-h");
    let path: PathBuf = vault.content_path("abc-123", "pdf");
    assert_eq!(path.extension().and_then(|s| s.to_str()), Some("pdf"));
}

#[test]
fn vault_content_path_defaults_to_markdown_for_page_media() {
    let temp = TempDir::new().unwrap();
    let vault = open_vault(&temp, "run-i");
    let path: PathBuf = vault.content_path("abc-123", "page");
    assert_eq!(path.extension().and_then(|s| s.to_str()), Some("md"));
}
