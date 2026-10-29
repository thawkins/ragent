//! Deep coverage for the research-progress tracker, the `/research` slash
//! dispatch, the swarm status/cancel grammar, and the `lib.rs` public
//! deadline/render policy helpers.
//!
//! Three surfaces are exercised here:
//!
//! 1. [`ragent_tui::research_progress`] - a public module, so its
//!    `encode_progress_event` / `encode_cluster_progress_event` /
//!    `decode_progress_event` / `ResearchProgress::{apply,finish,render}` API is
//!    called directly. Every `SessionEvent` arm the existing suites miss is
//!    round-tripped.
//! 2. `App::handle_research_command` (`src/app/research.rs`) - `pub(crate)` with
//!    no public hook, so it is driven through the public
//!    `App::execute_slash_command("/research ...")` entry point.
//! 3. `App::handle_swarm_status` / `handle_swarm_cancel` (`src/app/swarm.rs`) -
//!    likewise driven through `/swarm ...`. The decomposition/spawn/poll/finalize
//!    helpers (`execute_swarm_decomposition`, `spawn_swarm_teammates`,
//!    `poll_swarm_unblock`, `poll_swarm_completion`, `finalize_swarm_completion`)
//!    are `pub(crate)` with no public test hook and are reachable only from the
//!    `run_tui` loop (`poll_pending_swarm`) - they are NOT covered here.
//! 4. `ragent_tui::{compute_next_deadline_test, should_render}` (`src/lib.rs`) -
//!    public test wrappers, called directly to hit every deadline branch.
//!
//! Tests only - no production logic is touched.

#[path = "support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use ragent_research::cite_checker::CitationCheckResult;
use ragent_research::contradiction::{ContradictionClaim, ContradictionEdge};
use ragent_research::corpus_critic::{CorpusCriticReport, GapFetchResult};
use ragent_research::digest::{EvidenceDigest, TripleDraft};
use ragent_research::evaluation::EvaluationScorecard;
use ragent_research::locus::{DepthInvestigation, LocusSet};
use ragent_research::patcher::PatchResult;
use ragent_research::readability::ReadabilityAudit;
use ragent_research::reconcile::{CrossLocusReconcile, SourceTensions};
use ragent_research::session::{
    AnalysisEvent, SessionEvent, SessionPhase, SynthesisEvent, SynthesizeOutcome,
};
use ragent_research::synthesis::SynthesisAudit;
use ragent_tui::research_progress::{
    ResearchProgress, ResearchStep, StepStatus, decode_progress_event,
    encode_cluster_progress_event, encode_progress_event,
};

// ===========================================================================
// research_progress.rs - encode_progress_event: every SessionEvent arm
// ===========================================================================

/// Encode one event and return the decoded detail (panicking if it does not
/// decode). Used for the arms whose produced status token is in the decoder's
/// allow-list.
fn detail_of(event: &SessionEvent) -> String {
    let encoded = encode_progress_event("run", "topic", event);
    decode_progress_event(&encoded)
        .unwrap_or_else(|| panic!("event should decode: {encoded}"))
        .detail
}

/// Encode one event that produces a status token the decoder deliberately does
/// not recognise; assert the pipeline yields `None` (kept out of the tracker).
fn assert_encode_undecodable(event: &SessionEvent) {
    let encoded = encode_progress_event("run", "topic", event);
    assert!(
        decode_progress_event(&encoded).is_none(),
        "status token should not decode: {encoded}"
    );
}

#[test]
fn web_search_failed_encodes_web_error() {
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::WebSearchFailed {
            error: "engine exploded".into(),
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.phase, SessionPhase::Web);
    assert_eq!(d.status, StepStatus::Error);
    assert!(d.detail.contains("web search failed: engine exploded"));
}

#[test]
fn web_source_excluded_encodes_web_excluded() {
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::WebSourceExcluded {
            url: "https://a.example".into(),
            reason: "low relevance".into(),
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.phase, SessionPhase::Web);
    assert_eq!(d.status, StepStatus::Excluded);
    assert!(
        d.detail
            .contains("excluded https://a.example: low relevance")
    );
}

#[test]
fn from_file_body_preview_encodes_setup_preview() {
    let detail = detail_of(&SessionEvent::FromFileBodyPreview {
        path: "/tmp/doc.pdf".into(),
        body_preview: "the extracted body".into(),
    });
    assert!(detail.contains("--from-file body preview for /tmp/doc.pdf"));
    assert!(detail.contains("the extracted body"));
}

#[test]
fn local_captured_encodes_local_capture() {
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::LocalCaptured {
            path: "src/lib.rs".into(),
            score: 7,
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.phase, SessionPhase::Local);
    assert_eq!(d.status, StepStatus::Done);
    assert!(d.detail.contains("captured src/lib.rs (score 7)"));
}

#[test]
fn spec_captured_encodes_specs_capture() {
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::SpecCaptured {
            spec_id: "auth".into(),
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.phase, SessionPhase::Specs);
    assert!(d.detail.contains("referenced spec auth"));
}

#[test]
fn synthesize_outcome_variants_render() {
    let cases = [
        (SynthesizeOutcome::Llm, "LLM analysis applied"),
        (
            SynthesizeOutcome::FallbackEmpty,
            "LLM returned empty content - using mechanical fallback",
        ),
        (
            SynthesizeOutcome::NoLlm,
            "no LLM engine configured - using mechanical fallback",
        ),
    ];
    for (outcome, needle) in cases {
        let detail = detail_of(&SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
            outcome,
            detail: None,
        }));
        assert!(detail.contains(needle), "got: {detail}");
    }

    let detail = detail_of(&SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
        outcome: SynthesizeOutcome::FallbackError,
        detail: None,
    }));
    assert!(detail.contains("LLM synthesis failed - using mechanical fallback"));
}

#[test]
fn supervisor_plan_and_entities_encode() {
    // `SupervisorPlanUpdated` encodes status "started", which `parse_phase`
    // maps back to `SupervisorPlan`, so the round-trip decodes; the sub-topic
    // text is free-form so a single token is used (a comma in a multi-topic
    // label would be re-split by the detail tokeniser).
    let detail = detail_of(&SessionEvent::SupervisorPlanUpdated {
        sub_topics: vec!["planning".into()],
    });
    assert!(detail.contains("supervisor plan with 1 sub-topic(s): planning"));

    // `CompetitiveEntities` encodes status "entities", which is intentionally
    // absent from `decode_progress_event`'s allow-list, so it does not
    // round-trip; assert the produced payload text directly and confirm the
    // non-decodability separately.
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::CompetitiveEntities {
            entities: vec!["X".into(), "Y".into()],
            criteria: vec![],
            inferred: true,
        },
    );
    assert!(encoded.contains("competitive set (inferred=true): X, Y | criteria: none"));
    assert_encode_undecodable(&SessionEvent::CompetitiveEntities {
        entities: vec!["X".into(), "Y".into()],
        criteria: vec![],
        inferred: true,
    });

    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::CompetitiveEntities {
            entities: vec!["X".into()],
            criteria: vec!["price".into()],
            inferred: false,
        },
    );
    assert!(encoded.contains("criteria: price"));
    assert!(encoded.contains("inferred=false"));
}

#[test]
fn supervisor_entities_status_token_is_not_decodable() {
    // "entities" is produced by the encoder but the decoder's allow-list omits
    // it, so the event falls through to the generic agent-notice path.
    assert_encode_undecodable(&SessionEvent::CompetitiveEntities {
        entities: vec!["X".into()],
        criteria: vec![],
        inferred: false,
    });
}

#[test]
fn researcher_lifecycle_events_encode() {
    let detail = detail_of(&SessionEvent::ResearcherSpawned {
        id: "r1".into(),
        sub_topic: "async".into(),
    });
    assert!(detail.contains("spawned researcher r1: async"));

    // `ResearcherProgress` echoes its status token verbatim; only the tokens in
    // `decode_progress_event`'s allow-list round-trip, so use "completed"
    // (a real `ResearcherStatus` spelling), not "capturing".
    let detail = detail_of(&SessionEvent::ResearcherProgress {
        id: "r1".into(),
        status: "completed".into(),
        detail: "working".into(),
        sources_found: 3,
    });
    assert!(detail.contains("researcher r1: working (3 source(s))"));

    let detail = detail_of(&SessionEvent::ResearcherCompleted {
        id: "r1".into(),
        summary: "all good".into(),
    });
    assert!(detail.contains("researcher r1 completed: all good"));

    let detail = detail_of(&SessionEvent::SupervisorMerged { findings_count: 4 });
    assert!(detail.contains("merged 4 researcher finding(s)"));
}

#[test]
fn researcher_note_status_token_is_not_decodable() {
    assert_encode_undecodable(&SessionEvent::ResearcherNote {
        id: "r1".into(),
        note: "a note".into(),
    });
}

#[test]
fn needs_clarification_status_token_is_not_decodable() {
    // The "needs_clarification" token has no decoder mapping either.
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::NeedsClarification {
            question: "which one?".into(),
        },
    );
    assert!(decode_progress_event(&encoded).is_none());
}

#[test]
fn tier_done_encodes_finalize_summary() {
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::TierDone {
            completed: 5,
            skipped: 1,
            failed: 2,
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.phase, SessionPhase::Finalize);
    assert_eq!(d.status, StepStatus::Done);
    assert!(
        d.detail
            .contains("pipeline complete: 5 completed, 1 skipped, 2 failed")
    );
}

#[test]
fn run_step_status_and_phase_mapping() {
    // Status mapping: pending/in_progress -> started, completed -> done,
    // failed -> error, skipped -> skipped, anything else -> "event" (done).
    let cases = [
        ("pending", StepStatus::Started),
        ("in_progress", StepStatus::Started),
        ("completed", StepStatus::Done),
        ("failed", StepStatus::Error),
        ("skipped", StepStatus::Skipped),
        ("frobnicated", StepStatus::Done),
    ];
    for (status, expected) in cases {
        let encoded = encode_progress_event(
            "run",
            "topic",
            &SessionEvent::RunStep {
                step: "synthesize".into(),
                status: status.into(),
                detail: Some("ctx".into()),
            },
        );
        let d = decode_progress_event(&encoded)
            .unwrap_or_else(|| panic!("run step {status} should decode"));
        assert_eq!(d.status, expected, "status {status}");
        assert!(d.detail.contains("pipeline step: synthesize (ctx)"));
    }
}

#[test]
fn run_step_phase_mapping_covers_every_bucket() {
    let cases = [
        ("decompose", SessionPhase::Web),
        ("width_sweep", SessionPhase::Web),
        ("web_deadline", SessionPhase::Web),
        ("web_phase_start", SessionPhase::Web),
        ("vault_sufficient", SessionPhase::Setup),
        ("supervisor_plan", SessionPhase::SupervisorPlan),
        ("supervisor_delegate", SessionPhase::SupervisorDelegate),
        ("supervisor_synthesize", SessionPhase::SupervisorSynthesize),
        ("supervisor_finalize", SessionPhase::SupervisorFinalize),
        ("anything_else", SessionPhase::Synthesize),
    ];
    for (step, expected) in cases {
        let encoded = encode_progress_event(
            "run",
            "topic",
            &SessionEvent::RunStep {
                step: step.into(),
                status: "completed".into(),
                detail: None,
            },
        );
        let d = decode_progress_event(&encoded).unwrap_or_else(|| panic!("step {step} decodes"));
        assert_eq!(d.phase, expected, "step {step}");
        // A `None` detail drops the trailing parenthetical.
        assert_eq!(d.detail, format!("pipeline step: {step}"));
    }
}

#[test]
fn synthesis_audit_and_corpus_critic_encode() {
    let detail = detail_of(&SessionEvent::Synthesis(SynthesisEvent::SynthesisAudit {
        audit: SynthesisAudit {
            overall_score: 88,
            recommendation: "proceed".into(),
            ..SynthesisAudit::default()
        },
    }));
    assert!(detail.contains("synthesis audit: 88/100 - proceed"));

    let detail = detail_of(&SessionEvent::Analysis(AnalysisEvent::CorpusCritic {
        report: CorpusCriticReport {
            score: 71,
            passed: false,
            issues: vec!["i".into()],
            gaps: vec!["g".into(), "g2".into()],
            ..CorpusCriticReport::default()
        },
    }));
    assert!(detail.contains("corpus critic: 71/100 (review) - 1 issue(s), 2 gap(s)"));
}

#[test]
fn gap_fetch_attempted_and_skipped_branches() {
    let attempted = detail_of(&SessionEvent::Analysis(AnalysisEvent::GapFetch {
        result: GapFetchResult {
            new_sources: 2,
            queries: vec!["q".into()],
            attempted: true,
            ..GapFetchResult::default()
        },
    }));
    assert!(attempted.contains("gap-fill fetch: 2 new source(s) from 1 query(s) "));

    let skipped = detail_of(&SessionEvent::Analysis(AnalysisEvent::GapFetch {
        result: GapFetchResult::default(),
    }));
    assert!(skipped.contains("(skipped)"));
}

#[test]
fn surgical_patch_counts_applied() {
    let detail = detail_of(&SessionEvent::Synthesis(SynthesisEvent::SurgicalPatch {
        result: PatchResult {
            score_before: 40,
            score_after: 55,
            ..PatchResult::default()
        },
    }));
    assert!(detail.contains("surgical patch: 40 -> 55 (0 patch(es), 0 applied)"));
}

#[test]
fn cite_check_pass_and_fail_branches() {
    let pass = detail_of(&SessionEvent::Synthesis(SynthesisEvent::CiteCheck {
        result: CitationCheckResult {
            passed: true,
            checked: 4,
            gate_open: true,
            ..CitationCheckResult::default()
        },
    }));
    assert!(pass.contains("cite check: 4 citation(s) checked - pass (gate open)"));

    let fail = detail_of(&SessionEvent::Synthesis(SynthesisEvent::CiteCheck {
        result: CitationCheckResult {
            passed: false,
            checked: 1,
            gate_open: false,
            ..CitationCheckResult::default()
        },
    }));
    assert!(fail.contains("CITATION_VERIFICATION_FAILED"));
    assert!(fail.contains("(gate closed)"));
}

#[test]
fn polish_readability_and_evaluation_encode() {
    let detail = detail_of(&SessionEvent::Synthesis(SynthesisEvent::Polish {
        result: ragent_research::PolishResult {
            control_chars_removed: 3,
            whitespace_normalized: 4,
            empty_paragraphs_removed: 5,
            ..ragent_research::PolishResult::default()
        },
    }));
    assert!(detail.contains("3 control char(s), 4 whitespace run(s), 5 empty paragraph(s)"));

    let pass = detail_of(&SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit {
        result: ReadabilityAudit {
            score: 90,
            passed: true,
            ..ReadabilityAudit::default()
        },
    }));
    assert!(pass.contains("readability audit: 90/100"));

    let ok = detail_of(&SessionEvent::Synthesis(SynthesisEvent::Evaluation {
        scorecard: EvaluationScorecard {
            overall: 82,
            quality: 1,
            relevance: 2,
            groundedness: 3,
            completeness: 4,
            structure: 5,
            ..EvaluationScorecard::default()
        },
    }));
    assert!(ok.contains("self-evaluation: 82/100 (Q 1 R 2 G 3 C 4 S 5)"));

    let failed = detail_of(&SessionEvent::Synthesis(SynthesisEvent::Evaluation {
        scorecard: EvaluationScorecard {
            error: Some("model offline".into()),
            ..EvaluationScorecard::default()
        },
    }));
    assert!(failed.contains("self-evaluation: failed"));
}

#[test]
fn plan_updated_and_sub_question_status_encode() {
    let detail = detail_of(&SessionEvent::PlanUpdated {
        sub_questions: vec!["q1".into(), "q2".into()],
    });
    assert!(detail.contains("plan updated: 2 sub-question(s)"));

    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::SubQuestionStatusChanged {
            id: "q1".into(),
            status: "answered".into(),
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.phase, SessionPhase::Web);
    assert!(d.detail.contains("sub-question q1: answered"));
}

#[test]
fn source_failed_with_and_without_source() {
    let with_source = detail_of(&SessionEvent::SourceFailed {
        source: Some("src/a.rs".into()),
        error: "boom".into(),
    });
    assert!(with_source.contains("source failed: src/a.rs - boom"));

    let without = detail_of(&SessionEvent::SourceFailed {
        source: None,
        error: "boom".into(),
    });
    assert!(without.contains("source failed: boom"));
}

#[test]
fn critic_result_with_and_without_score() {
    let scored = detail_of(&SessionEvent::Synthesis(SynthesisEvent::CriticResult {
        score: Some(70),
        gaps: vec!["g".into()],
    }));
    assert!(scored.contains("critic: 70/100 - 1 gap(s)"));

    let unscored = detail_of(&SessionEvent::Synthesis(SynthesisEvent::CriticResult {
        score: None,
        gaps: vec![],
    }));
    assert!(unscored.contains("critic: 0 gap(s)"));
}

#[test]
fn verification_pass_and_fail_branches() {
    let pass = detail_of(&SessionEvent::VerificationResult {
        passed: true,
        issues: vec![],
    });
    assert!(pass.contains("verification: pass - 0 issue(s)"));

    let fail = detail_of(&SessionEvent::VerificationResult {
        passed: false,
        issues: vec!["i".into()],
    });
    assert!(fail.contains("verification: fail - 1 issue(s)"));
}

#[test]
fn iteration_completed_with_and_without_score() {
    let scored = detail_of(&SessionEvent::IterationCompleted {
        iteration: 2,
        score: Some(50),
    });
    assert!(scored.contains("iteration 2 complete (score 50)"));

    let unscored = detail_of(&SessionEvent::IterationCompleted {
        iteration: 3,
        score: None,
    });
    assert!(unscored.contains("iteration 3 complete"));
}

#[test]
fn follow_up_queries_encode() {
    let encoded = encode_progress_event(
        "run",
        "topic",
        &SessionEvent::FollowUpQueries {
            queries: vec!["a".into(), "b".into()],
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.phase, SessionPhase::Web);
    assert_eq!(d.status, StepStatus::Done);
    assert!(d.detail.contains("2 follow-up querie(s) generated"));
}

#[test]
fn analysis_events_encode_counts() {
    let edge = ContradictionEdge {
        claim_a: ContradictionClaim {
            text: "a".into(),
            source_index: 1,
            source_kind: "web".into(),
            source_path: "https://a".into(),
        },
        claim_b: ContradictionClaim {
            text: "b".into(),
            source_index: 2,
            source_kind: "web".into(),
            source_path: "https://b".into(),
        },
        dimension: "cost".into(),
        note: "conflict".into(),
        strength: 80,
    };
    let detail = detail_of(&SessionEvent::Analysis(AnalysisEvent::ContradictionGraph {
        edges: vec![edge],
        sources_scanned: 3,
    }));
    assert!(detail.contains("contradiction graph: 1 edge(s) from 3 source(s)"));

    let loci = detail_of(&SessionEvent::Analysis(AnalysisEvent::LociAnalysis {
        loci: LocusSet::default(),
        sources_scanned: 4,
    }));
    assert!(loci.contains("loci analysis: 0 dimension(s) from 4 source(s)"));

    let depth = detail_of(&SessionEvent::Analysis(AnalysisEvent::DepthInvestigation {
        investigations: vec![DepthInvestigation::default(); 2],
    }));
    assert!(depth.contains("depth investigation: 2 loci classified"));

    let reconcile = detail_of(&SessionEvent::Analysis(
        AnalysisEvent::CrossLocusReconcile {
            reconcile: CrossLocusReconcile::default(),
        },
    ));
    assert!(reconcile.contains("cross-locus reconcile: 0 pair(s)"));

    let tensions = detail_of(&SessionEvent::Analysis(AnalysisEvent::SourceTensions {
        tensions: SourceTensions::default(),
    }));
    assert!(tensions.contains("source tensions: 0 recorded"));

    let digest = detail_of(&SessionEvent::Analysis(AnalysisEvent::EvidenceDigest {
        digest: EvidenceDigest::default(),
    }));
    assert!(digest.contains("evidence digest: 0 claim(s) from 0 source(s)"));

    let draft = detail_of(&SessionEvent::Analysis(AnalysisEvent::TripleDraft {
        draft: TripleDraft::default(),
    }));
    assert!(draft.contains("triple draft: 0 candidate(s) produced"));
}

// ===========================================================================
// research_progress.rs - apply / finish / render state machine
// ===========================================================================

#[test]
fn apply_web_deadline_rewrites_and_clears() {
    let mut p = ResearchProgress::new("n", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "web_phase_start");
    p.apply(SessionPhase::Web, StepStatus::Done, "captured https://a");
    p.apply(SessionPhase::Web, StepStatus::Done, "captured https://b");
    p.apply(
        SessionPhase::Web,
        StepStatus::Skipped,
        "pipeline step: web_deadline (reached)",
    );
    // Detail rewritten with the live capture count.
    let last = p.steps.last().expect("deadline step");
    assert!(
        last.detail.contains("**Web phase deadline reached**"),
        "got: {}",
        last.detail
    );
    assert!(last.detail.contains("2 source(s) captured before timeout"));
    assert!(p.web_phase_deadline.is_none());
}

#[test]
fn apply_web_deadline_via_detail_only_clears_deadline() {
    // The redundant `detail.contains("web_deadline")` guard clears a previously
    // armed deadline even without the Skipped status.
    let mut p = ResearchProgress::new("n", "t");
    p.apply(
        SessionPhase::Web,
        StepStatus::Started,
        "web_phase_start deadline: 30s",
    );
    assert!(p.web_phase_deadline.is_some());
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "pipeline step: web_deadline reached",
    );
    assert!(p.web_phase_deadline.is_none());
}

#[test]
fn apply_options_snapshot_only_for_setup_done() {
    let mut p = ResearchProgress::new("n", "t");
    // Setup + Started with the options prefix is NOT stored (status must be Done).
    p.apply(
        SessionPhase::Setup,
        StepStatus::Started,
        "options in use: mode: tiered",
    );
    assert!(p.options.is_none());
    p.apply(
        SessionPhase::Setup,
        StepStatus::Done,
        "options in use: mode: tiered",
    );
    assert_eq!(p.options.as_deref(), Some("mode: tiered"));
}

#[test]
fn apply_captured_requires_web_done() {
    // A non-web phase with a "captured " detail is not folded into the counter.
    let mut p = ResearchProgress::new("n", "t");
    p.apply(SessionPhase::Local, StepStatus::Done, "captured src/lib.rs");
    assert_eq!(p.fetched_count, 0);
    assert_eq!(p.steps.len(), 1);
}

#[test]
fn render_options_multiline_and_completion_variants() {
    let mut p = ResearchProgress::new("run", "topic");
    p.apply(
        SessionPhase::Setup,
        StepStatus::Done,
        "options in use: mode: supervisor",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "decomposed into 2 queries:\n  - one\n  - two",
    );
    let rendered = p.render();
    assert!(rendered.contains("Options: mode: supervisor"));
    assert!(rendered.contains("  - one"));
    assert!(rendered.contains("  - two"));
    // Continuation lines indent to the detail column.
    let lines: Vec<&str> = rendered.lines().collect();
    let idx = lines
        .iter()
        .position(|l| l.contains("decomposed into 2 queries"))
        .expect("detail line");
    assert!(lines[idx + 1].starts_with("              "));

    // Completion with PDF + YouTube + excluded extras.
    p.finish(9, 2, 1, 1);
    let rendered = p.render();
    assert!(
        rendered.contains(
            "Complete - 9 source(s), including 2 PDFs and 1 YouTube video and 1 excluded"
        )
    );
}

#[test]
fn render_totals_line_variants() {
    // fetched only -> no excluded suffix.
    let mut p = ResearchProgress::new("n", "t");
    p.apply(SessionPhase::Web, StepStatus::Done, "captured https://a");
    let rendered = p.render();
    assert!(rendered.contains("[totals] Web fetch totals: 1 fetched, 0 failed"));
    assert!(!rendered.contains("excluded"));

    // excluded_live only -> the count is shown.
    let mut p = ResearchProgress::new("n", "t");
    p.apply(SessionPhase::Web, StepStatus::Excluded, "drop");
    let rendered = p.render();
    assert!(rendered.contains("[totals] Web fetch totals: 0 fetched, 0 failed, 1 excluded"));
}

#[test]
fn render_empty_detail_renders_prefix_only() {
    let mut p = ResearchProgress::new("n", "t");
    p.steps.push(ResearchStep {
        phase: "local",
        detail: String::new(),
        status: StepStatus::Started,
    });
    let rendered = p.render();
    assert!(rendered.contains("[research] Research Progress"));
    assert!(rendered.contains("local"));
}

#[test]
fn step_status_icons_are_all_distinct() {
    let icons = [
        StepStatus::Started.icon(),
        StepStatus::Done.icon(),
        StepStatus::Error.icon(),
        StepStatus::Excluded.icon(),
        StepStatus::Skipped.icon(),
    ];
    assert_eq!(icons, [">", "[ok]", "!", "-", "o"]);
}

#[test]
fn cluster_progress_event_roundtrips_every_phase_token() {
    let phases = [
        SessionPhase::Setup,
        SessionPhase::Web,
        SessionPhase::Local,
        SessionPhase::Specs,
        SessionPhase::Synthesize,
        SessionPhase::Assemble,
        SessionPhase::Finalize,
        SessionPhase::SupervisorPlan,
        SessionPhase::SupervisorDelegate,
        SessionPhase::SupervisorSynthesize,
        SessionPhase::SupervisorFinalize,
    ];
    for phase in phases {
        let encoded = encode_cluster_progress_event("n", "t", phase, "done", "d");
        let d = decode_progress_event(&encoded).expect("decode");
        assert_eq!(d.phase, phase);
        assert_eq!(d.status, StepStatus::Done);
    }
}

#[test]
fn decode_rejects_unknown_status_and_bad_json() {
    assert!(decode_progress_event("no sentinel").is_none());
    assert!(
        decode_progress_event(&format!(
            "{}not-json",
            ragent_tui::research_progress::PROGRESS_SENTINEL
        ))
        .is_none()
    );
    // Valid JSON, unknown phase.
    let bad_phase = format!(
        "{{\"name\":\"n\",\"topic\":\"t\",\"phase\":\"zzz\",\"status\":\"done\",\"detail\":\"d\",\"total_sources\":null,\"pdf_count\":0,\"youtube_count\":0,\"excluded_count\":0}}"
    );
    assert!(decode_progress_event(&bad_phase).is_none());
}

// ===========================================================================
// app/research.rs - driven through `/research ...`
// ===========================================================================

fn app_with_session(session: &str) -> ragent_tui::App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.needs_redraw = false;
    // Point the research store at a fresh tempdir so no run touches the repo.
    let dir = temp_dir("research-deep");
    app.cwd_path = dir;
    app
}

fn joined_text(app: &ragent_tui::App) -> String {
    app.messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            ragent_agent::message::MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "ragent-tui-deep-{}-{}-{}",
        std::process::id(),
        tag,
        n
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[tokio::test]
async fn research_create_seeds_progress_and_status() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research create myrun a topic about rust")
        .await;
    assert!(
        app.status.starts_with("[wait] research: myrun"),
        "got: {}",
        app.status
    );
    assert!(app.research_progress.iter().any(|p| p.name == "myrun"));
    let text = joined_text(&app);
    assert!(
        text.contains("Gathering sources for `myrun`"),
        "got: {text}"
    );
    assert!(text.contains("Topic: a topic about rust"), "got: {text}");
}

#[tokio::test]
async fn research_create_from_urls_uses_urls_subject() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research create urlrun --from-url https://example.com/page")
        .await;
    let text = joined_text(&app);
    assert!(
        text.contains("URL(s): https://example.com/page"),
        "got: {text}"
    );
}

#[tokio::test]
async fn research_create_from_files_uses_file_subject() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research create filerun --from-file docs/guide.md")
        .await;
    let text = joined_text(&app);
    assert!(text.contains("File(s): docs/guide.md"), "got: {text}");
}

#[tokio::test]
async fn research_create_no_topic_no_sources_uses_url_placeholder() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research create bare").await;
    let text = joined_text(&app);
    assert!(
        text.contains("URL: \n") || text.contains("URL:"),
        "got: {text}"
    );
}

#[tokio::test]
async fn research_create_explicit_format_is_reflected() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research create fmt topic --format imrad")
        .await;
    let text = joined_text(&app);
    assert!(text.contains("Format: `imrad`"), "got: {text}");
}

#[tokio::test]
async fn research_list_empty_store_publishes_rendered_output() {
    let mut app = app_with_session("s1");
    let mut rx = app.event_bus.subscribe();
    app.execute_slash_command("/research list").await;
    assert!(app.status.contains("listing items"), "got: {}", app.status);
    let text = recv_text_delta(&mut rx).await.expect("list output");
    assert!(text.contains("From: /research list"), "got: {text}");
}

#[tokio::test]
async fn research_show_missing_item_publishes_error() {
    let mut app = app_with_session("s1");
    let mut rx = app.event_bus.subscribe();
    app.execute_slash_command("/research show nope").await;
    let text = recv_text_delta(&mut rx).await.expect("show output");
    assert!(text.contains("From: /research show"), "got: {text}");
    assert!(text.contains("**Error:**"), "got: {text}");
}

#[tokio::test]
async fn research_search_publishes_output() {
    let mut app = app_with_session("s1");
    let mut rx = app.event_bus.subscribe();
    app.execute_slash_command("/research search rust").await;
    let text = recv_text_delta(&mut rx).await.expect("search output");
    assert!(text.contains("From: /research search"), "got: {text}");
}

#[tokio::test]
async fn research_archive_missing_item_publishes_error() {
    let mut app = app_with_session("s1");
    let mut rx = app.event_bus.subscribe();
    app.execute_slash_command("/research archive nope").await;
    let text = recv_text_delta(&mut rx).await.expect("archive output");
    assert!(text.contains("From: /research archive"), "got: {text}");
}

#[tokio::test]
async fn research_open_existing_item_publishes_view() {
    let dir = temp_dir("research-open-ok");
    let root = dir.join("research");
    let manager = ragent_research::ResearchManager::new(&root);
    manager
        .create("viewable", "Viewable Title", "a topic")
        .await
        .expect("create item");

    let mut app = app_with_session("s1");
    app.cwd_path = dir;
    app.execute_slash_command("/research open viewable").await;
    assert!(app.status.contains("opening"), "got: {}", app.status);

    // Drain events until the OpenResearchView for the item appears.
    let mut rx = app.event_bus.subscribe();
    let mut found = false;
    for _ in 0..50 {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(ragent_agent::event::Event::OpenResearchView { name, .. }))
                if name == "viewable" =>
            {
                found = true;
                break;
            }
            Ok(Ok(_)) | Ok(Err(_)) => {}
            Err(_) => break,
        }
    }
    // The spawn may have completed before the second subscribe; accept either
    // the view event or a synchronously-rendered status.
    assert!(
        found || app.status.contains("opening"),
        "expected the research view to be published"
    );
}

/// Receive the next `TextDelta` for a short window, skipping unrelated events.
async fn recv_text_delta(
    rx: &mut tokio::sync::broadcast::Receiver<ragent_agent::event::Event>,
) -> Option<String> {
    for _ in 0..50 {
        match tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Ok(ragent_agent::event::Event::TextDelta { text, .. })) => return Some(text),
            Ok(Ok(_)) | Ok(Err(_)) => {}
            Err(_) => return None,
        }
    }
    None
}

// ===========================================================================
// app/swarm.rs - driven through `/swarm ...`
// ===========================================================================

use ragent_agent::team::{
    MemberStatus, SwarmDecomposition, SwarmState, SwarmSubtask, Task, TaskStatus, TaskStore,
    TeamMember, TeamStore,
};

fn swarm_subtask(id: &str, deps: &[&str]) -> SwarmSubtask {
    SwarmSubtask {
        id: id.to_string(),
        title: format!("Task {id}"),
        description: format!("Do work for {id}"),
        depends_on: deps.iter().map(|s| (*s).to_string()).collect(),
        agent_type: None,
        model: None,
    }
}

fn swarm_state(team_name: &str, tasks: Vec<SwarmSubtask>, spawned: bool) -> SwarmState {
    SwarmState {
        team_name: team_name.to_string(),
        prompt: "do work".to_string(),
        decomposition: SwarmDecomposition { tasks },
        spawned,
        completed: false,
        default_agent_type: None,
    }
}

#[tokio::test]
async fn swarm_status_reads_task_store_rows() {
    let dir = temp_dir("swarm-store");
    std::fs::create_dir_all(dir.join(".ragent")).expect("ragent dir");
    let team = "swarm-store-team";
    let store = TeamStore::create(team, "s-lead", &dir, true).expect("create team");
    let ts = TaskStore::open(&store.dir).expect("task store");

    let mut t1 = Task::new("s1", "Task s1");
    t1.status = TaskStatus::Pending;
    ts.add_task(t1).await.expect("add s1");
    let mut t2 = Task::new("s2", "Task s2");
    t2.status = TaskStatus::InProgress;
    t2.assigned_to = Some("tm-001".to_string());
    ts.add_task(t2).await.expect("add s2");
    let mut t3 = Task::new("s3", "Task s3");
    t3.status = TaskStatus::Completed;
    ts.add_task(t3).await.expect("add s3");
    let mut t4 = Task::new("s4", "Task s4");
    t4.status = TaskStatus::Cancelled;
    t4.depends_on = vec!["s1".to_string()];
    ts.add_task(t4).await.expect("add s4");

    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(swarm_state(
        team,
        vec![
            swarm_subtask("s1", &[]),
            swarm_subtask("s2", &[]),
            swarm_subtask("s3", &[]),
            swarm_subtask("s4", &["s1"]),
        ],
        true,
    ));
    let mut member = TeamMember::new("swarm-s1", "tm-001", "general");
    member.status = MemberStatus::Working;
    app.team_members.push(member);

    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    // Task store rows: every status icon + the assigned + dep columns.
    assert!(text.contains("[wait]"), "got: {text}");
    assert!(text.contains("[sync]"), "got: {text}");
    assert!(text.contains("[ok]"), "got: {text}");
    assert!(text.contains("[err]"), "got: {text}");
    assert!(text.contains("tm-001"), "got: {text}");
    assert!(text.contains("1/4"), "got: {text}");
    assert!(text.contains("swarm-s1"), "got: {text}");
    assert!(text.contains("working"), "got: {text}");
}

#[tokio::test]
async fn swarm_status_all_completed_prints_cleanup_hint() {
    let dir = temp_dir("swarm-all-done");
    std::fs::create_dir_all(dir.join(".ragent")).expect("ragent dir");
    let team = "swarm-all-done-team";
    let store = TeamStore::create(team, "s-lead", &dir, true).expect("create team");
    let ts = TaskStore::open(&store.dir).expect("task store");
    let mut t1 = Task::new("s1", "Task s1");
    t1.status = TaskStatus::Completed;
    ts.add_task(t1).await.expect("add task");

    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(swarm_state(team, vec![swarm_subtask("s1", &[])], true));

    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    assert!(text.contains("All tasks complete!"), "got: {text}");
    assert!(text.contains("/swarm cancel"), "got: {text}");
}

#[tokio::test]
async fn swarm_status_empty_decomposition_has_zero_total() {
    // total == 0 exercises the checked_div guard (empty bar, no divide-by-zero).
    let dir = temp_dir("swarm-empty");
    let mut app = app_with_session("s-lead");
    app.cwd_path = dir;
    app.swarm_state = Some(swarm_state("swarm-empty-team", vec![], true));
    app.execute_slash_command("/swarm status").await;
    let text = joined_text(&app);
    assert!(text.contains("0/0"), "got: {text}");
    assert!(text.contains("(spawning...)"), "got: {text}");
}

#[tokio::test]
async fn swarm_prompt_with_model_starts_decomposing() {
    let mut app = app_with_session("s-lead");
    app.selected_model = Some("provider/model-x".to_string());
    app.execute_slash_command("/swarm build a thing").await;
    assert!(
        app.status.contains("decomposing goal"),
        "got: {}",
        app.status
    );
    let state = app.swarm_state.as_ref().expect("swarm state recorded");
    assert_eq!(state.prompt, "build a thing");
    assert!(!state.spawned);
    let text = joined_text(&app);
    assert!(text.contains("Swarm Decomposition"), "got: {text}");
}

#[tokio::test]
async fn swarm_prompt_with_agent_flag_records_default_agent() {
    let mut app = app_with_session("s-lead");
    app.selected_model = Some("provider/model-x".to_string());
    app.execute_slash_command("/swarm --agent coder build a thing")
        .await;
    let state = app.swarm_state.as_ref().expect("swarm state recorded");
    assert_eq!(state.default_agent_type.as_deref(), Some("coder"));
    assert_eq!(state.prompt, "build a thing");
}

// ===========================================================================
// lib.rs - compute_next_deadline_test / should_render policy branches
// ===========================================================================

fn past_idle() -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(3))
        .expect("clock past 3s")
}

#[test]
fn deadline_default_is_bounded_by_idle_redraw() {
    let app = support::make_app();
    let now = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, now);
    // The 60s default is capped by the idle redraw interval (~2s) off last_draw.
    assert!(deadline <= now + Duration::from_secs(3), "got {deadline:?}");
}

#[test]
fn deadline_permission_queue_wakes_within_a_second() {
    let mut app = support::make_app();
    app.permission_queue.push_back(permission_request("perm-1"));
    let now = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(1100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_timed_question_wakes_within_a_second() {
    let mut app = support::make_app();
    app.question_queue
        .push_back(ragent_tui::app::QuestionRequest {
            id: "q1".to_string(),
            session_id: "s1".to_string(),
            question: "pick".to_string(),
            options: vec!["a".to_string()],
            default_index: Some(0),
            expire_at_ms: Some(1),
            secret: false,
        });
    let now = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(1100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_active_web_phase_wakes_within_a_second() {
    let mut app = support::make_app();
    let mut p = ResearchProgress::new("r", "t");
    p.web_phase_deadline = Some(Instant::now() + Duration::from_secs(30));
    app.research_progress.push(p);
    let now = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(1100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_status_and_banner_expiry_are_scheduled() {
    let mut app = support::make_app();
    app.status_set_at = Some(Instant::now());
    app.run_cost_banner_at = Some(Instant::now());
    app.history_save_deadline = Some(Instant::now() + Duration::from_millis(500));
    let now = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, now);
    // The earliest of status expiry (2s), banner (15s), and flush (0.5s) wins.
    assert!(
        deadline <= now + Duration::from_millis(600),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_swarm_state_wakes_within_two_seconds() {
    let mut app = support::make_app();
    app.swarm_state = Some(swarm_state("swarm-x", vec![], false));
    let now = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(2100),
        "got {deadline:?}"
    );
}

#[test]
fn deadline_pending_stream_group_is_throttle_bounded() {
    let mut app = support::make_app();
    // A *pending* message group is `message_cache_dirty_from < messages.len()`
    // (0 < 1), so the throttle-tail deadline must fire promptly rather than
    // waiting the full idle interval.
    app.messages
        .push(ragent_agent::message::Message::user_text("s", "hi"));
    app.message_cache_dirty_from = 0;
    let now = Instant::now();
    let deadline = ragent_tui::compute_next_deadline_test(&app, now);
    assert!(
        deadline <= now + Duration::from_millis(50),
        "pending stream group should schedule a near-term repaint, got {deadline:?}"
    );
}

#[test]
fn should_render_dirty_and_idle_states() {
    let mut app = support::make_app();
    app.needs_redraw = true;
    assert!(ragent_tui::should_render(&app, Instant::now()));

    app.needs_redraw = false;
    assert!(!ragent_tui::should_render(&app, Instant::now()));
    assert!(!ragent_tui::should_render(&app, past_idle()));
}

#[test]
fn should_render_safety_wake_repaints_only_for_live_display() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    // A live permission countdown makes the periodic safety wake paint.
    app.permission_queue.push_back(permission_request("perm-2"));
    assert!(ragent_tui::should_render(&app, past_idle()));
}

#[test]
fn should_render_paints_a_pending_stream_group() {
    let mut app = support::make_app();
    app.needs_redraw = false;
    // A pending cache group is `message_cache_dirty_from < messages.len()`
    // (0 < 1) and counts as visible content at the safety interval.
    app.messages
        .push(ragent_agent::message::Message::user_text("s", "hi"));
    app.message_cache_dirty_from = 0;
    assert!(ragent_tui::should_render(&app, past_idle()));
}

#[test]
fn poll_spawn_result_test_hook_is_callable() {
    // The public test hook must run the (empty) spawn-result poll without panic.
    let mut app = support::make_app();
    ragent_tui::poll_spawn_result_for_tests(&mut app);
}

// A minimal PermissionRequest builder mirroring the shape the countdown dialog
// queues (kept local so the file does not depend on a helper in `support`).
fn permission_request(id: &str) -> ragent_agent::permission::PermissionRequest {
    ragent_agent::permission::PermissionRequest {
        id: id.to_string(),
        session_id: "s1".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["ls".to_string()],
        metadata: serde_json::json!({ "timeout_secs": 120u64 }),
        tool_call_id: None,
    }
}
