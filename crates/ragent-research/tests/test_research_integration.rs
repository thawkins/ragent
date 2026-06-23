//! End-to-end integration tests for `ragent-research` (T-052).
//!
//! Exercises the full create → list → show → search → delete flow against a
//! real on-disk `research/` directory, plus the FR-016 duplicate-create and
//! FR-018 closest-match paths.

use ragent_research::{ResearchItem, ResearchManager, ResearchName, ResearchStatus, Source};
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn full_create_list_show_delete_flow() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());

    // Create
    let item = mgr
        .create("rust-async", "Rust Async", "async/await idioms")
        .await
        .unwrap();
    assert_eq!(item.name, ResearchName::new("rust-async").unwrap());

    // List
    let list = mgr.list(false).await.unwrap();
    assert_eq!(list.len(), 1);

    // Show
    let shown = mgr.show("rust-async").await.unwrap();
    assert_eq!(shown.status, ResearchStatus::Draft);
    assert_eq!(shown.title, "Rust Async");
    assert!(shown.topic.contains("async/await"));

    // INDEX.md exists.
    let index_path = ragent_research::ResearchIo::index_path(tmp.path());
    assert!(index_path.is_file());

    // Delete
    mgr.delete("rust-async").await.unwrap();
    let list = mgr.list(true).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn create_rejects_duplicate_with_fr016_error() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    let err = mgr
        .create("rust-async", "Other", "Other")
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("already exists"));
    assert!(msg.contains("/research open"));
}

#[tokio::test]
async fn write_document_persists_supports_files_and_index() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    let mut item: ResearchItem = mgr.show("rust-async").await.unwrap();
    item.add_source(Source::Web {
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
    });
    item.add_source(Source::Local {
        path: "src/lib.rs".into(),
        kind: ragent_research::LocalSourceKind::InProject,
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "anchor file".into(),
    });
    let doc = ragent_research::ResearchDocument {
        item,
        summary: "Captured one web source and one local cross-reference.".into(),
        findings: vec!["Finding 1".into()],
        cross_references: vec![ragent_research::CrossReference {
            path: "src/lib.rs".into(),
            relevance: "anchor file".into(),
        }],
        open_questions: vec!["What about errors?".into()],
        template_body: None,
    };
    mgr.write_document(&doc).await.unwrap();

    // RESEARCH.md on disk has the full body.
    let body = std::fs::read_to_string(ragent_research::ResearchIo::research_md_path(
        tmp.path(),
        &ResearchName::new("rust-async").unwrap(),
    ))
    .unwrap();
    assert!(body.contains("Captured one web source"));
    assert!(body.contains("What about errors?"));
    assert!(body.contains("src/lib.rs"));

    // INDEX.md was refreshed.
    let index =
        std::fs::read_to_string(ragent_research::ResearchIo::index_path(tmp.path())).unwrap();
    assert!(index.contains("rust-async"));
}

#[tokio::test]
async fn archive_then_default_list_excludes_item() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.archive("rust-async").await.unwrap();
    assert!(mgr.list(false).await.unwrap().is_empty());
    let all = mgr.list(true).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].status, ResearchStatus::Archived);
}

#[tokio::test]
async fn not_found_suggests_closest_match_per_fr018() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.create("tokio-runtime", "Tokio", "topic").await.unwrap();
    let err = mgr.show("rust-asynx").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Closest matches"));
    assert!(msg.contains("rust-async"));
}

#[tokio::test]
async fn search_finds_text_across_research_items() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.create("serde-json", "Serde JSON", "topic")
        .await
        .unwrap();
    let hits = mgr.search("Rust", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "rust-async");
}
