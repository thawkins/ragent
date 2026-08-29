//! Manual verification test cases for the Hyperresearch pipeline (T-022).
//!
//! These tests exercise the post-implementation Hyperresearch features in a
//! deterministic, headless way. They are intended to be run manually by a QA
//! engineer or developer before marking the implementation complete, but they
//! also run as ordinary integration tests in CI.
//!
//! Covered scenarios:
//!
//! 1. Full tier includes the complete 16-step pipeline and renders the
//!    contradiction graph, source tensions, and cite-check summary in
//!    `RESEARCH.md`.
//! 2. Light tier skips adversarial/loci/corpus/critic/gap/readability steps.
//! 3. Dissertation tier starts with chapter partitioning.
//! 4. A saved `RunManifest` can be resumed from JSON without losing step state.
//! 5. Web gatherer skips search/fetch when the vault already contains enough
//!    sources for the requested tier.
//! 6. Open-access recovery disclosures appear in `RESEARCH.md` frontmatter and
//!    in the rendered supporting-file block for a recovered source.

use async_trait::async_trait;
use chrono::Utc;
use ragent_research::{
    AssembledDocument, CitationCheckResult, ContradictionClaim, ContradictionEdge, OutputFormat,
    RecoveredOpenAccess, RecoverySource, ResearchDocument, ResearchItem, ResearchName, Source,
    SourceTensions, TensionKind, TensionRecord, Tier, TierRouter, assemble_document,
    contradiction::ContradictionGraph as ContrGraph,
    run_manifest::{RunManifest, RunStep, StepStatus},
    source_vault::{NewVaultSource, SourceVault},
    web_gatherer::{WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool},
};
use std::path::PathBuf;
use std::sync::Arc;

fn sample_name() -> ResearchName {
    ResearchName::new("hyperresearch-manual").expect("name must validate")
}

fn sample_item() -> ResearchItem {
    ResearchItem::new(
        sample_name(),
        "Manual Test",
        "GLP-1 cardiovascular outcomes",
    )
}

fn base_document() -> ResearchDocument {
    ResearchDocument {
        item: sample_item(),
        summary: "Summary text.".into(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: Vec::new(),
        open_questions: Vec::new(),
        contradiction_graph: None,
        loci: None,
        depth_investigation: None,
        evidence_digest: None,
        triple_draft: None,
        cross_locus_reconcile: None,
        source_tensions: None,
        synthesis_audit: None,
        corpus_critic: None,
        gap_fetch: None,
        surgical_patch: None,
        cite_check: None,
        polish: None,
        readability_audit: None,
        template_body: None,
        decomposed_queries: Vec::new(),
        output_format: OutputFormat::Report,
    }
}

fn web_source(index: usize, body: &str) -> Source {
    Source::Web {
        url: format!("https://example.test/{index}"),
        title: format!("Source {index}"),
        captured_at: Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: body.into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: "manual".into(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

#[test]
fn manual_case_1_full_tier_renders_graph_tensions_and_cite_summary() {
    let sources = vec![
        web_source(1, "GLP-1 drugs improve cardiovascular outcomes."),
        web_source(2, "GLP-1 drugs have no effect on cardiovascular outcomes."),
    ];

    let mut graph = ContrGraph::empty();
    graph.add_edge(ContradictionEdge {
        claim_a: ContradictionClaim::from_source("improves", 1, &sources[0]),
        claim_b: ContradictionClaim::from_source("no effect", 2, &sources[1]),
        dimension: "cardiovascular outcomes".into(),
        note: "opposing cardiovascular claims".into(),
        strength: 60,
    });

    let mut doc = base_document();
    doc.item.sources = sources;
    doc.contradiction_graph = Some(graph);
    doc.source_tensions = Some(SourceTensions {
        tensions: vec![TensionRecord {
            kind: TensionKind::Contradiction,
            label: "cardiovascular outcomes".into(),
            source_indices: vec![1, 2],
            note: "direct contradiction between sources".into(),
        }],
        sources_scanned: 2,
    });
    doc.cite_check = Some(CitationCheckResult {
        passed: true,
        checked: 2,
        failed_claims: Vec::new(),
        issues: Vec::new(),
        gate_open: true,
    });

    let AssembledDocument { body, corpa, .. } = assemble_document(&doc);

    // The QA render sections live in the CORPA.md companion payload; the
    // RESEARCH.md body keeps only the citation-check summary.
    assert!(
        corpa.contains("## Contradiction Graph"),
        "CORPA.md must contain a Contradiction Graph section"
    );
    assert!(
        corpa.contains("opposing cardiovascular claims"),
        "graph edge note must be rendered in CORPA.md"
    );
    assert!(
        corpa.contains("## Source Tensions"),
        "CORPA.md must contain a Source Tensions section"
    );
    assert!(
        corpa.contains("direct contradiction between sources"),
        "tension note must be rendered in CORPA.md"
    );
    assert!(
        !body.contains("## Contradiction Graph"),
        "RESEARCH.md must no longer carry the Contradiction Graph section"
    );
    assert!(
        !body.contains("## Source Tensions"),
        "RESEARCH.md must no longer contain a Source Tensions section"
    );
    assert!(
        body.contains("## Citation Check"),
        "RESEARCH.md must contain a Citation Check section"
    );
    assert!(
        body.contains("**Summary:** 2 citation(s) checked, 2 passed, 0 failed; gate open."),
        "cite-check summary must be rendered"
    );
}

#[test]
fn manual_case_2_light_tier_skips_adversarial_steps() {
    let router = TierRouter::new("run-light", "glp1", "GLP-1 outcomes", Tier::Light);
    let steps: Vec<RunStep> = router.manifest().steps.iter().map(|s| s.step).collect();

    assert!(steps.contains(&RunStep::Decompose));
    assert!(steps.contains(&RunStep::WidthSweep));
    assert!(steps.contains(&RunStep::Polish));
    assert!(
        !steps.contains(&RunStep::ContradictionGraph),
        "light tier must skip ContradictionGraph"
    );
    assert!(
        !steps.contains(&RunStep::LociAnalysis),
        "light tier must skip LociAnalysis"
    );
    assert!(
        !steps.contains(&RunStep::ReadabilityAudit),
        "light tier must skip ReadabilityAudit"
    );
    assert!(
        !steps.contains(&RunStep::Critics),
        "light tier must skip Critics"
    );
    assert!(
        !steps.contains(&RunStep::GapFetch),
        "light tier must skip GapFetch"
    );
}

#[test]
fn manual_case_3_dissertation_tier_starts_with_chapter_partition() {
    let router = TierRouter::new("run-diss", "glp1", "GLP-1 outcomes", Tier::Dissertation);
    let steps: Vec<RunStep> = router.manifest().steps.iter().map(|s| s.step).collect();

    assert_eq!(steps[0], RunStep::ChapterPartition);
    assert!(steps.contains(&RunStep::ContradictionGraph));
    assert!(steps.contains(&RunStep::Synthesize));
    assert!(steps.contains(&RunStep::ReadabilityAudit));
}

#[test]
fn manual_case_4_manifest_resumes_without_losing_state() {
    let mut router = TierRouter::new("run-resume", "glp1", "GLP-1 outcomes", Tier::Full);
    let observer = ragent_research::tier_router::NoopTierRouterObserver;
    router.start_step(RunStep::Decompose, &observer);
    router.finish_step(RunStep::Decompose, &observer);
    router.start_step(RunStep::WidthSweep, &observer);

    let json = router.manifest_json().expect("manifest serializes");
    let manifest = RunManifest::from_json(&json).expect("manifest deserializes");
    let resumed = TierRouter::from_manifest(manifest);

    assert_eq!(resumed.manifest().steps[0].status, StepStatus::Completed);
    assert_eq!(resumed.manifest().steps[1].status, StepStatus::InProgress);
    assert_eq!(
        resumed.next_step(),
        Some(RunStep::WidthSweep),
        "resume must continue at the in-progress step"
    );
}

struct PanicSearch;

#[async_trait]
impl WebSearchTool for PanicSearch {
    async fn search(&self, _query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        panic!("search must not be called when the vault is sufficient")
    }
}

struct PanicFetch;

#[async_trait]
impl WebFetchTool for PanicFetch {
    async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
        panic!("fetch must not be called when the vault is sufficient")
    }
}

#[tokio::test]
async fn manual_case_5_sufficient_vault_sources_skip_new_fetches() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault_root = tmp.path().join("vault");
    let vault = SourceVault::open_with_root(&vault_root, "run-sufficient").unwrap();

    for i in 0..5 {
        vault
            .store(&NewVaultSource {
                url: format!("https://manual.test/page-{i}"),
                title: format!("Manual Page {i}"),
                fetch_timestamp: Some(Utc::now()),
                search_tool: "mf_search".into(),
                search_engine: "duckduckgo".into(),
                media_type: "page".into(),
                content_type: None,
                body_text: format!("content about GLP-1 and cardiovascular outcomes {i}"),
            })
            .unwrap();
    }

    let gatherer = WebGatherer::new(Arc::new(PanicSearch), Arc::new(PanicFetch))
        .with_vault(Arc::new(vault))
        .with_sufficient_sources(5);

    let result = gatherer
        .gather_with_observer("GLP-1 cardiovascular outcomes", 10, None)
        .await
        .expect("gather succeeds using vault sources");

    assert_eq!(
        result.sources.len(),
        5,
        "all sufficient vaulted sources must be returned"
    );
}

#[test]
fn manual_case_6_open_access_recovery_is_disclosed() {
    let mut item = sample_item();
    item.open_access_recovery = true;
    item.add_source(Source::Web {
        url: "https://doi.org/10.1234/manual".into(),
        title: "Manual OA Paper".into(),
        captured_at: Utc::now(),
        published_at: None,
        body_path: PathBuf::from("sources/web-01.md"),
        body: "full text recovered".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: Some(Box::new(RecoveredOpenAccess {
            url: "https://pmc.ncbi.nlm.nih.gov/articles/PMC123456/".into(),
            source: RecoverySource::EuropePmc,
            license: Some("CC-BY-4.0".into()),
            version: Some("publishedVersion".into()),
        })),
        author: None,
    });

    let doc = ResearchDocument {
        item,
        ..base_document()
    };
    let AssembledDocument {
        content,
        frontmatter,
        body,
        corpa: _,
    } = assemble_document(&doc);

    assert!(
        frontmatter.contains("open_access_recovery: true"),
        "frontmatter must disclose OA recovery; frontmatter:\n{frontmatter}"
    );
    let rendered =
        ragent_research::render_supporting_file(&doc.item.sources[0]).expect("web source renders");
    assert!(
        rendered.contains("https://pmc.ncbi.nlm.nih.gov/articles/PMC123456/"),
        "supporting file must link to recovered OA URL: {rendered}"
    );
    assert!(
        rendered.contains("europepmc"),
        "supporting file must name recovery source: {rendered}"
    );
    assert!(
        rendered.contains("CC-BY-4.0"),
        "supporting file must disclose license: {rendered}"
    );

    // Ensure we exercise the assembled document fields (frontmatter/body come
    // from content), avoiding unused warnings for content.
    assert!(!content.is_empty());
    assert!(!body.is_empty());
}
