//! Inline tests for `manager.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use tempfile::TempDir;

#[tokio::test]
async fn complete_gathering_preserves_sources_count_and_queries() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path());
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
    item.set_queries(vec!["Rust async".into(), "Tokio runtime".into()]);
    let doc = ResearchDocument {
        item,
        summary: "summary".into(),
        findings: Vec::new(),
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
        decomposed_queries: vec!["Rust async".into(), "Tokio runtime".into()],
        brief: None,
        output_format: crate::run_config::OutputFormat::Report,
        comparison_table: None,
        evaluation_scorecard: None,
        provider_stats: None,
    };
    mgr.write_document(&doc).await.unwrap();
    mgr.complete_gathering("rust-async").await.unwrap();

    let path = ResearchIo::research_md_path(tmp.path(), &ResearchName::new("rust-async").unwrap());
    let content = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(
        content.contains("sources: 1 # see sources/ subdirectory"),
        "frontmatter sources count should be preserved after complete_gathering; got:\n{content}"
    );
    assert!(
        content.contains("queries:\n  - \"Rust async\"\n  - \"Tokio runtime\""),
        "frontmatter queries list should be preserved after complete_gathering; got:\n{content}"
    );
    assert!(
        content.contains("status: complete"),
        "status should be updated to complete; got:\n{content}"
    );
}
