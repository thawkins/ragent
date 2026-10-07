//! Inline tests for `manager.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use tempfile::TempDir;

fn sample_name() -> ResearchName {
    ResearchName::new("rust-async").expect("name must validate")
}

#[tokio::test]
async fn create_then_list_returns_item() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    let item = mgr
        .create("rust-async", "Rust Async", "async/await idioms")
        .await
        .unwrap();
    assert_eq!(item.name, sample_name());
    assert_eq!(item.title, "Rust Async");
    assert_eq!(item.status, ResearchStatus::Draft);

    let list = mgr.list(false).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, sample_name());
}

#[tokio::test]
async fn create_rejects_duplicate_with_fr016_error() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    let err = mgr
        .create("rust-async", "Different Title", "Different topic")
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("already exists"), "msg was: {msg}");
    assert!(msg.contains("/research open"), "msg was: {msg}");
}

#[tokio::test]
async fn show_returns_not_found_with_suggestions_for_close_match() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.create("tokio-runtime", "Tokio Runtime", "topic")
        .await
        .unwrap();
    let err = mgr.show("rust-asynx").await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("rust-async"), "msg was: {msg}");
    assert!(msg.contains("Closest matches"), "msg was: {msg}");
}

#[tokio::test]
async fn delete_then_list_excludes_item() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.delete("rust-async").await.unwrap();
    let list = mgr.list(true).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn delete_missing_item_returns_not_found() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    let err = mgr.delete("ghost").await.unwrap_err();
    assert!(matches!(err, ResearchError::NotFound(_, _)));
}

#[tokio::test]
async fn archive_marks_status_archived_and_excludes_from_default_list() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.archive("rust-async").await.unwrap();
    let list = mgr.list(false).await.unwrap();
    assert!(list.is_empty(), "archived must be hidden by default");
    let all = mgr.list(true).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].status, ResearchStatus::Archived);
}

#[tokio::test]
async fn search_finds_matching_text_in_research_md() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    let hits = mgr.search("Rust", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "rust-async");
}

#[tokio::test]
async fn search_returns_empty_for_empty_query() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    let hits = mgr.search("", 10).await.unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn save_and_load_state_round_trips() {
    let tmp = TempDir::new().unwrap();
    let manager = ResearchManager::new(tmp.path().join("research"));
    manager
        .create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();

    let mut state = ResearchState::new("async/await");
    state.add_sub_question("q1", "What is tokio?", 10);
    manager.save_state("rust-async", &state).await.unwrap();

    let loaded = manager.load_state("rust-async").await.unwrap();
    assert_eq!(loaded.plan.topic, "async/await");
    assert_eq!(loaded.plan.sub_questions.len(), 1);
}

#[tokio::test]
async fn continue_item_adds_follow_up_sub_question() {
    let tmp = TempDir::new().unwrap();
    let manager = ResearchManager::new(tmp.path().join("research"));
    manager
        .create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();

    let mut state = ResearchState::new("async/await");
    state.add_sub_question("q1", "What is tokio?", 10);
    manager.save_state("rust-async", &state).await.unwrap();

    let continued = manager
        .continue_item("rust-async", Some("focus on async-std"))
        .await
        .unwrap();
    assert!(
        continued
            .plan
            .topic
            .contains("Follow-up: focus on async-std")
    );
    assert!(
        continued
            .plan
            .sub_questions
            .iter()
            .any(|sq| sq.question == "focus on async-std")
    );
}

#[tokio::test]
async fn refresh_index_writes_index_md_with_one_row() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.create("tokio-runtime", "Tokio Runtime", "topic")
        .await
        .unwrap();
    let index_path = ResearchIo::index_path(tmp.path());
    let body = tokio::fs::read_to_string(&index_path).await.unwrap();
    assert!(body.contains("rust-async"));
    assert!(body.contains("tokio-runtime"));
}

// -- Milestone G: search index cache tests -------------------------------

#[tokio::test]
async fn g001_refresh_index_writes_index_json_cache() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();
    let cache_path = ResearchIo::cache_path(tmp.path());
    assert!(
        cache_path.is_file(),
        ".index.json should exist after create"
    );
    let json = tokio::fs::read_to_string(&cache_path).await.unwrap();
    let cache: SearchIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.entries[0].name, "rust-async");
    assert_eq!(cache.entries[0].title, "Rust Async");
    assert_eq!(cache.entries[0].topic, "async/await");
    assert_eq!(cache.entries[0].status, "draft");
    assert!(cache.entries[0].tags.is_empty());
    // search_text should contain the body text (title appears in body).
    assert!(cache.entries[0].search_text.contains("Rust Async"));
}

#[tokio::test]
async fn g001_cache_regenerated_after_delete() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.create("tokio-runtime", "Tokio Runtime", "topic")
        .await
        .unwrap();
    // Delete one item.
    mgr.delete("rust-async").await.unwrap();
    let cache_path = ResearchIo::cache_path(tmp.path());
    let json = tokio::fs::read_to_string(&cache_path).await.unwrap();
    let cache: SearchIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.entries[0].name, "tokio-runtime");
}

#[tokio::test]
async fn g001_cache_regenerated_after_archive() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    mgr.archive("rust-async").await.unwrap();
    let cache_path = ResearchIo::cache_path(tmp.path());
    let json = tokio::fs::read_to_string(&cache_path).await.unwrap();
    let cache: SearchIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.entries[0].status, "archived");
}

#[tokio::test]
async fn g001_cache_contains_one_line_summary() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();
    // Write a document with a summary.
    let mut item = mgr.show("rust-async").await.unwrap();
    item.add_source(Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: String::new(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    });
    let doc = ResearchDocument {
        item,
        summary: "Tokio is the dominant async runtime for Rust.".into(),
        findings: vec!["Finding A".into()],
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        concepts: None,
        contradiction_graph: None,
        loci: None,
        depth_investigation: None,
        evidence_digest: None,
        triple_draft: None,
        cross_locus_reconcile: None,
        source_tensions: None,
        synthesis_audit: None,
        template_body: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        brief: None,
        decomposed_queries: Vec::new(),
        output_format: crate::run_config::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    mgr.write_document(&doc).await.unwrap();

    let cache_path = ResearchIo::cache_path(tmp.path());
    let json = tokio::fs::read_to_string(&cache_path).await.unwrap();
    let cache: SearchIndex = serde_json::from_str(&json).unwrap();
    assert_eq!(
        cache.entries[0].summary,
        "Tokio is the dominant async runtime for Rust."
    );
}

#[tokio::test]
async fn g002_search_uses_cache_and_matches_full_scan() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();
    mgr.create("tokio-runtime", "Tokio Runtime", "async runtime")
        .await
        .unwrap();
    // Search via cache (cache was built during create).
    // "async" appears in the topic/body of both items.
    let hits = mgr.search("async", 10).await.unwrap();
    assert_eq!(hits.len(), 2, "both items should match 'async'");
    // Verify names are present.
    let names: Vec<&str> = hits.iter().map(|h| h.name.as_str()).collect();
    assert!(names.contains(&"rust-async"));
    assert!(names.contains(&"tokio-runtime"));
}

#[tokio::test]
async fn g002_search_falls_back_when_cache_missing() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();
    // Remove the cache file to simulate a missing cache.
    let cache_path = ResearchIo::cache_path(tmp.path());
    tokio::fs::remove_file(&cache_path).await.unwrap();
    // Search should still work via full-scan fallback.
    let hits = mgr.search("Rust", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "rust-async");
}

#[tokio::test]
async fn g003_search_falls_back_when_cache_stale_after_manual_edit() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();
    // Simulate a manual edit by creating a new research directory
    // without going through the manager (so the cache won't know about it).
    let new_dir = tmp.path().join("manual-item");
    tokio::fs::create_dir_all(&new_dir).await.unwrap();
    let research_md = new_dir.join("RESEARCH.md");
    tokio::fs::write(
        &research_md,
        "---\nname: manual-item\ntitle: \"Manual\"\ntopic: \"manual topic\"\nstatus: draft\ncreated: 2024-01-01T00:00:00Z\nmodified: 2024-01-01T00:00:00Z\nsources: 0\n---\n\n# Title: Manual\n\nManual content with keyword Tokio.\n",
    )
    .await
    .unwrap();
    // Search for "Tokio" - the cache doesn't know about "manual-item"
    // so the cache is stale. The search should fall back to full scan
    // and find the match in the manually-created item.
    let hits = mgr.search("Tokio", 10).await.unwrap();
    assert!(
        hits.iter().any(|h| h.name == "manual-item"),
        "full-scan fallback should find manually-created item; got: {:?}",
        hits.iter().map(|h| &h.name).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn g003_search_falls_back_when_cache_has_stale_item_set() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();
    // Delete the item directory directly (bypassing the manager) so the
    // cache still has the entry but the directory is gone.
    let item_dir = tmp.path().join("rust-async");
    tokio::fs::remove_dir_all(&item_dir).await.unwrap();
    // Search should detect the stale cache (item set mismatch) and fall
    // back to full scan, returning no hits.
    let hits = mgr.search("Rust", 10).await.unwrap();
    assert!(hits.is_empty(), "stale cache should fall back to full scan");
}

#[tokio::test]
async fn g003_search_falls_back_when_research_md_modified_after_cache() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    mgr.create("rust-async", "Rust Async", "async/await")
        .await
        .unwrap();
    // Sleep briefly so the manual write mtime is strictly after the cache
    // generation time.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    // Manually append content to RESEARCH.md (simulating a manual edit).
    let research_md =
        ResearchIo::research_md_path(tmp.path(), &ResearchName::new("rust-async").unwrap());
    let original = tokio::fs::read_to_string(&research_md).await.unwrap();
    let modified = format!("{original}\n\nManual edit: keyword XYZXYZ.\n");
    tokio::fs::write(&research_md, &modified).await.unwrap();
    // Search for "XYZXYZ" - the cache doesn't contain this keyword, but
    // the mtime check should detect the stale cache and fall back to full
    // scan, finding the match.
    let hits = mgr.search("XYZXYZ", 10).await.unwrap();
    assert_eq!(
        hits.len(),
        1,
        "mtime-based fallback should find manual edit"
    );
    assert_eq!(hits[0].name, "rust-async");
}

#[test]
fn extract_one_line_summary_returns_empty_when_no_section() {
    let body = "# Title: Test\n\n## Topic\n\nSome topic text.\n";
    assert_eq!(extract_one_line_summary(body), "");
}

#[test]
fn extract_one_line_summary_returns_first_non_placeholder_line() {
    let body = "## Executive Summary\n\nThis is the real summary.\n\n## Findings\n\n...";
    assert_eq!(extract_one_line_summary(body), "This is the real summary.");
}

#[test]
fn extract_one_line_summary_skips_placeholder() {
    let body = "## Executive Summary\n\n(no executive summary recorded yet - run a gathering pass to populate)\n\n## Top 10 Implications\n\n";
    assert_eq!(extract_one_line_summary(body), "");
}

#[test]
fn extract_one_line_summary_truncates_multibyte_safely() {
    // Use a 2-byte character repeated so the 200-byte boundary falls inside
    // a character. The function must not panic and should return a valid
    // truncated string ending with the ellipsis marker.
    let summary = "\u{e9}".repeat(150);
    let body = format!("## Executive Summary\n\n{summary}\n\n## Findings\n\n");
    let got = extract_one_line_summary(&body);
    assert!(!got.is_empty());
    assert!(
        got.ends_with("..."),
        "expected truncated summary to end with ellipsis: {got}"
    );
    assert!(got.chars().count() <= 200);
}
#[test]
fn suggest_closest_picks_shortest_distance() {
    let candidates = vec![
        "rust-async".into(),
        "tokio-runtime".into(),
        "serde-json".into(),
    ];
    let s = suggest_closest_from(&candidates, "rust-asynx");
    // "rust-async" must be the closest by edit distance (1 vs many more).
    assert!(s.starts_with("rust-async"), "got: {s}");
}

#[test]
fn suggest_closest_returns_at_most_three() {
    let candidates = vec![
        "alpha".into(),
        "beta".into(),
        "gamma".into(),
        "delta".into(),
        "epsilon".into(),
    ];
    let s = suggest_closest_from(&candidates, "x");
    assert_eq!(s.matches(',').count() + 1, 3);
}

#[test]
fn suggest_closest_handles_empty_candidate_list() {
    let s = suggest_closest_from(&[], "anything");
    assert!(s.contains("no research items"));
}

#[test]
fn extract_snippet_does_not_panic_on_byte_boundary() {
    let body = "Hello, world!";
    let snippet = extract_snippet(body, 7, 5, 4);
    assert!(snippet.contains("world"));
}

#[test]
fn render_document_for_renders_full_document() {
    let name = sample_name();
    let sources = vec![Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: String::new(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }];
    let doc = render_document_for(&name, "Rust Async", "topic", &sources, "summary", &[]);
    assert!(doc.content.contains("# Title: Rust Async"));
    assert!(
        doc.content
            .contains("| 1 | web | page | - | [https://example.com](https://example.com)")
    );
}

#[test]
fn union_with_existing_sorts_and_dedupes() {
    let referenced = vec!["b".into(), "a".into()];
    let existing = {
        let mut s = HashSet::new();
        s.insert("c".into());
        s.insert("a".into());
        s
    };
    let v = union_with_existing(&referenced, &existing);
    assert_eq!(v, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
}

#[tokio::test]
async fn write_document_persists_body_and_supports_files() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
    let name = ResearchName::new("rust-async").unwrap();
    mgr.create("rust-async", "Rust Async", "topic")
        .await
        .unwrap();
    let mut item = mgr.show("rust-async").await.unwrap();
    item.add_source(Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: String::new(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    });
    let doc = ResearchDocument {
        item,
        summary: "Found one good link".into(),
        findings: vec!["Finding A".into()],
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        concepts: None,
        contradiction_graph: None,
        loci: None,
        depth_investigation: None,
        evidence_digest: None,
        triple_draft: None,
        cross_locus_reconcile: None,
        source_tensions: None,
        synthesis_audit: None,
        template_body: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        brief: None,
        decomposed_queries: Vec::new(),
        output_format: crate::run_config::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    mgr.write_document(&doc).await.unwrap();
    let path = ResearchIo::research_md_path(tmp.path(), &name);
    let body = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(body.contains("Found one good link"));
    assert!(body.contains("Finding A"));
    // Supporting file must exist on disk.
    let supp = ResearchIo::source_body_path(tmp.path(), &name, "web", 1);
    assert!(supp.is_file());
}
