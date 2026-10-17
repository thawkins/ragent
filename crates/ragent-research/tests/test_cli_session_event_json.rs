//! Tests for `cli::session_event_json` / `render_session_event_json`, exercising
//! every top-level `SessionEvent` arm (including the nested `Analysis` and
//! `Synthesis` sub-enums). Pure logic - builds events by hand and checks the
//! rendered JSON `kind`/`payload` shape.

use ragent_research::analysis::AnalysisResult;
use ragent_research::cli::{render_session_event_json, session_event_json};
use ragent_research::contradiction::{ContradictionClaim, ContradictionEdge};
use ragent_research::corpus_critic::{CorpusCriticReport, GapFetchResult};
use ragent_research::digest::{EvidenceDigest, TripleDraft};
use ragent_research::locus::{DepthInvestigation, LocusSet};
use ragent_research::open_access::{RecoveredOpenAccess, RecoverySource};
use ragent_research::patcher::PatchResult;
use ragent_research::readability::{PolishResult, ReadabilityAudit};
use ragent_research::reconcile::{CrossLocusReconcile, SourceTensions};
use ragent_research::session::{
    AnalysisEvent, SessionEvent, SessionPhase, SynthesisEvent, SynthesizeOutcome,
};

fn kind_of(event: &SessionEvent) -> String {
    let json: serde_json::Value =
        serde_json::from_str(&session_event_json(event)).expect("valid json");
    json["kind"].as_str().expect("kind string").to_string()
}

fn claim(index: usize) -> ContradictionClaim {
    ContradictionClaim {
        text: "x".to_string(),
        source_index: index,
        source_kind: "web".to_string(),
        source_path: "https://a".to_string(),
    }
}

#[test]
fn top_level_events_render_expected_kinds() {
    let events = vec![
        (
            SessionEvent::Phase {
                phase: SessionPhase::Web,
            },
            "phase",
        ),
        (
            SessionEvent::QueriesDecomposed {
                queries: vec!["q".to_string()],
            },
            "queries",
        ),
        (
            SessionEvent::WebCaptured {
                url: "https://a".to_string(),
                title: "t".to_string(),
                search_tool: "mf_search".to_string(),
                search_engine: "wikipedia".to_string(),
                body_preview: "p".to_string(),
                language: "English".to_string(),
                oa_recovery: Some(Box::new(RecoveredOpenAccess {
                    url: "https://oa".to_string(),
                    source: RecoverySource::Unpaywall,
                    license: None,
                    version: None,
                })),
                media_type: "page".to_string(),
            },
            "web_captured",
        ),
        (
            SessionEvent::FromUrlBodyPreview {
                url: "https://a".to_string(),
                body_preview: "p".to_string(),
            },
            "from_url_body_preview",
        ),
        (
            SessionEvent::FromFileBodyPreview {
                path: "/p".to_string(),
                body_preview: "p".to_string(),
            },
            "from_file_body_preview",
        ),
        (
            SessionEvent::LocalCaptured {
                path: "/p".to_string(),
                score: 3,
            },
            "local_captured",
        ),
        (
            SessionEvent::SpecCaptured {
                spec_id: "s".to_string(),
            },
            "spec_captured",
        ),
        (
            SessionEvent::WebSearchFailed {
                error: "e".to_string(),
            },
            "web_search_failed",
        ),
        (
            SessionEvent::WebFetchFailed {
                url: "https://a".to_string(),
                error: "e".to_string(),
            },
            "web_fetch_failed",
        ),
        (
            SessionEvent::WebSourceExcluded {
                url: "https://a".to_string(),
                reason: "low".to_string(),
            },
            "web_source_excluded",
        ),
        (
            SessionEvent::SourceFailed {
                source: Some("s".to_string()),
                error: "e".to_string(),
            },
            "source_failed",
        ),
        (
            SessionEvent::NeedsClarification {
                question: "q".to_string(),
            },
            "needs_clarification",
        ),
        (
            SessionEvent::PlanUpdated {
                sub_questions: vec!["q".to_string()],
            },
            "plan_updated",
        ),
        (
            SessionEvent::SubQuestionStatusChanged {
                id: "1".to_string(),
                status: "done".to_string(),
            },
            "sub_question_status_changed",
        ),
        (
            SessionEvent::VerificationResult {
                passed: true,
                issues: vec!["i".to_string()],
            },
            "verification_result",
        ),
        (
            SessionEvent::IterationCompleted {
                iteration: 2,
                score: Some(80),
            },
            "iteration_completed",
        ),
        (
            SessionEvent::FollowUpQueries {
                queries: vec!["q".to_string()],
            },
            "follow_up_queries",
        ),
        (
            SessionEvent::Done {
                total_sources: 3,
                pdf_count: 1,
                youtube_count: 0,
                excluded_count: 2,
            },
            "done",
        ),
        (
            SessionEvent::RunStep {
                step: "web".to_string(),
                status: "ok".to_string(),
                detail: None,
            },
            "run_step",
        ),
        (
            SessionEvent::TierDone {
                completed: 1,
                skipped: 0,
                failed: 0,
            },
            "tier_done",
        ),
        (
            SessionEvent::SupervisorPlanUpdated {
                sub_topics: vec!["t".to_string()],
            },
            "supervisor_plan_updated",
        ),
        (
            SessionEvent::CompetitiveEntities {
                entities: vec!["a".to_string()],
                criteria: vec!["c".to_string()],
                inferred: true,
            },
            "competitive_entities",
        ),
        (
            SessionEvent::ResearcherSpawned {
                id: "r1".to_string(),
                sub_topic: "st".to_string(),
            },
            "researcher_spawned",
        ),
        (
            SessionEvent::ResearcherProgress {
                id: "r1".to_string(),
                status: "capturing".to_string(),
                detail: "d".to_string(),
                sources_found: 4,
            },
            "researcher_progress",
        ),
        (
            SessionEvent::ResearcherNote {
                id: "r1".to_string(),
                note: "n".to_string(),
            },
            "researcher_note",
        ),
        (
            SessionEvent::ResearcherCompleted {
                id: "r1".to_string(),
                summary: "s".to_string(),
            },
            "researcher_completed",
        ),
        (
            SessionEvent::SupervisorMerged { findings_count: 5 },
            "supervisor_merged",
        ),
        (
            SessionEvent::ConfigSnapshot {
                mode: "tiered".to_string(),
                output_format: "report".to_string(),
                depth: Some("deep".to_string()),
                iterations: Some(2),
                tier: Some("full".to_string()),
                from_urls: vec!["https://a".to_string()],
                from_files: vec!["/f".to_string()],
            },
            "config",
        ),
    ];
    for (event, expected) in events {
        assert_eq!(kind_of(&event), expected);
    }
}

#[test]
fn analysis_events_render_expected_kinds() {
    let edge = ContradictionEdge {
        claim_a: claim(1),
        claim_b: claim(2),
        dimension: "safety".to_string(),
        note: "n".to_string(),
        strength: 50,
    };
    let events = vec![
        (
            AnalysisEvent::ContradictionGraph {
                edges: vec![edge],
                sources_scanned: 2,
            },
            "contradiction_graph",
        ),
        (
            AnalysisEvent::LociAnalysis {
                loci: LocusSet::empty(),
                sources_scanned: 3,
            },
            "loci_analysis",
        ),
        (
            AnalysisEvent::DepthInvestigation {
                investigations: vec![DepthInvestigation::default()],
            },
            "depth_investigation",
        ),
        (
            AnalysisEvent::CrossLocusReconcile {
                reconcile: CrossLocusReconcile::empty(),
            },
            "cross_locus_reconcile",
        ),
        (
            AnalysisEvent::SourceTensions {
                tensions: SourceTensions::empty(),
            },
            "source_tensions",
        ),
        (
            AnalysisEvent::EvidenceDigest {
                digest: EvidenceDigest::empty(),
            },
            "evidence_digest",
        ),
        (
            AnalysisEvent::CorpusCritic {
                report: CorpusCriticReport::empty(),
            },
            "corpus_critic",
        ),
        (
            AnalysisEvent::GapFetch {
                result: GapFetchResult::default(),
            },
            "gap_fetch",
        ),
        (
            AnalysisEvent::TripleDraft {
                draft: TripleDraft::empty(),
            },
            "triple_draft",
        ),
    ];
    for (event, expected) in events {
        assert_eq!(kind_of(&SessionEvent::Analysis(event)), expected);
    }
}

#[test]
fn synthesis_events_render_expected_kinds() {
    let events = vec![
        (
            SynthesisEvent::SynthesizeResult {
                outcome: SynthesizeOutcome::FallbackEmpty,
                detail: Some("d".to_string()),
            },
            "synthesize_result",
        ),
        (
            SynthesisEvent::SynthesisAudit {
                audit: ragent_research::synthesis::SynthesisAudit::empty(),
            },
            "synthesis_audit",
        ),
        (
            SynthesisEvent::CriticResult {
                score: Some(70),
                gaps: vec!["g".to_string()],
            },
            "critic_result",
        ),
        (
            SynthesisEvent::SurgicalPatch {
                result: PatchResult::empty(),
            },
            "surgical_patch",
        ),
        (
            SynthesisEvent::CiteCheck {
                result: ragent_research::cite_checker::CitationCheckResult::empty(),
            },
            "citation_check",
        ),
        (
            SynthesisEvent::Polish {
                result: PolishResult::empty(),
            },
            "polish",
        ),
        (
            SynthesisEvent::ReadabilityAudit {
                result: ReadabilityAudit::empty(),
            },
            "readability_audit",
        ),
        (
            SynthesisEvent::Evaluation {
                scorecard: ragent_research::evaluation::EvaluationScorecard::default(),
            },
            "evaluation_scorecard",
        ),
    ];
    for (event, expected) in events {
        assert_eq!(kind_of(&SessionEvent::Synthesis(event)), expected);
    }
}

#[test]
fn session_phase_as_str_covers_all_variants() {
    for (phase, label) in [
        (SessionPhase::Setup, "setup"),
        (SessionPhase::Web, "web"),
        (SessionPhase::Local, "local"),
        (SessionPhase::Specs, "specs"),
        (SessionPhase::Synthesize, "synthesize"),
        (SessionPhase::Assemble, "assemble"),
        (SessionPhase::Finalize, "finalize"),
        (SessionPhase::SupervisorPlan, "supervisor_plan"),
        (SessionPhase::SupervisorDelegate, "supervisor_delegate"),
        (SessionPhase::SupervisorSynthesize, "supervisor_synthesize"),
        (SessionPhase::SupervisorFinalize, "supervisor_finalize"),
    ] {
        assert_eq!(phase.as_str(), label);
    }
}

#[test]
fn synthesize_outcome_as_str_covers_all_variants() {
    assert_eq!(SynthesizeOutcome::Llm.as_str(), "llm");
    assert_eq!(SynthesizeOutcome::FallbackEmpty.as_str(), "fallback-empty");
    assert_eq!(SynthesizeOutcome::FallbackError.as_str(), "fallback-error");
    assert_eq!(SynthesizeOutcome::NoLlm.as_str(), "no-llm");
}

#[test]
fn render_session_event_json_prefixes_kind() {
    let event = SessionEvent::Analysis(AnalysisEvent::CorpusCritic {
        report: CorpusCriticReport::empty(),
    });
    let rendered = render_session_event_json(&event);
    assert!(rendered.starts_with("ragent-research: "));
    assert!(rendered.contains("\"kind\":\"corpus_critic\""));
    // `AnalysisResult` default keeps the analysis import meaningful.
    assert_eq!(
        AnalysisResult::default().findings,
        [] as [std::string::String; 0]
    );
}
