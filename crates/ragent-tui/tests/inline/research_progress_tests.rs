//! Inline tests for `research_progress.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_encode_decode_roundtrip_queries_decomposed() {
    let queries = vec![
        "async rust ecosystem overview".into(),
        "tokio vs async-std comparison".into(),
    ];
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::QueriesDecomposed {
            queries: queries.clone(),
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Web);
    assert_eq!(decoded.status, StepStatus::Done);
    assert!(
        decoded.detail.contains("decomposed into 2 queries"),
        "detail should report actual count: {}",
        decoded.detail
    );
    for q in &queries {
        assert!(
            decoded.detail.contains(q),
            "detail should list each query: {}",
            decoded.detail
        );
    }
}

#[test]
fn test_encode_decode_roundtrip_from_url_body_preview() {
    let encoded = encode_progress_event(
        "rust-async",
        "async rust",
        &SessionEvent::FromUrlBodyPreview {
            url: "https://example.com/guide".into(),
            body_preview: "Long-form article about Rust async/await idioms.".into(),
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Setup);
    assert_eq!(decoded.status, StepStatus::Done);
    assert!(
        decoded.detail.contains("https://example.com/guide"),
        "detail should mention the URL: {}",
        decoded.detail
    );
    assert!(
        decoded
            .detail
            .contains("Long-form article about Rust async/await"),
        "detail should include the body preview: {}",
        decoded.detail
    );
}

#[test]
fn test_encode_decode_roundtrip_phase() {
    let encoded = encode_progress_event(
        "rust-async",
        "async rust",
        &SessionEvent::Phase {
            phase: SessionPhase::Web,
        },
    );
    assert!(encoded.starts_with(PROGRESS_SENTINEL));
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.name, "rust-async");
    assert_eq!(decoded.topic, "async rust");
    assert_eq!(decoded.phase, SessionPhase::Web);
    assert_eq!(decoded.status, StepStatus::Started);
    assert_eq!(decoded.detail, "searching the web");
}

#[test]
fn test_encode_decode_roundtrip_done() {
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::Done {
            total_sources: 7,
            pdf_count: 2,
            youtube_count: 1,
            excluded_count: 0,
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Finalize);
    assert_eq!(decoded.status, StepStatus::Done);
    assert_eq!(decoded.total_sources, Some(7));
    assert_eq!(decoded.pdf_count, 2);
    assert_eq!(decoded.youtube_count, 1);
}

#[test]
fn test_encode_decode_roundtrip_synthesize() {
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::Phase {
            phase: SessionPhase::Synthesize,
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Synthesize);
}

#[test]
fn test_synthesize_result_llm_outcome_renders_cleanly() {
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
            outcome: SynthesizeOutcome::Llm,
            detail: None,
        }),
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Synthesize);
    assert_eq!(decoded.status, StepStatus::Done);
    assert!(decoded.detail.contains("LLM analysis applied"));
}

#[test]
fn test_synthesize_result_fallback_error_includes_detail() {
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
            outcome: SynthesizeOutcome::FallbackError,
            detail: Some("provider returned 401".into()),
        }),
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert!(decoded.detail.contains("provider returned 401"));
    assert!(decoded.detail.contains("mechanical fallback"));
}

#[test]
fn test_synthesize_result_no_llm_renders_cleanly() {
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
            outcome: SynthesizeOutcome::NoLlm,
            detail: None,
        }),
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert!(decoded.detail.contains("no LLM engine configured"));
}

#[test]
fn test_encode_decode_roundtrip_polish_and_readability_audit() {
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::Synthesis(SynthesisEvent::Polish {
            result: ragent_research::PolishResult {
                changes: vec![ragent_research::PolishChange {
                    field: "summary".into(),
                    description: "normalized whitespace".into(),
                }],
                control_chars_removed: 1,
                whitespace_normalized: 2,
                empty_paragraphs_removed: 3,
                note: "Polished".into(),
            },
        }),
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Synthesize);
    assert_eq!(decoded.status, StepStatus::Done);
    assert!(decoded.detail.contains("polish"));
    assert!(decoded.detail.contains("1 control char"));

    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit {
            result: ragent_research::ReadabilityAudit {
                score: 85,
                passed: true,
                issues: vec!["issue".into()],
                recommendations: vec!["rec".into()],
                avg_finding_length: 400,
                missing_label_count: 0,
                long_paragraph_count: 0,
            },
        }),
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Synthesize);
    assert_eq!(decoded.status, StepStatus::Done);
    assert!(decoded.detail.contains("readability audit"));
    assert!(decoded.detail.contains("85/100"));
}

#[test]
fn test_decode_rejects_non_sentinel() {
    assert!(decode_progress_event("ragent-research: {...}").is_none());
    assert!(decode_progress_event("plain text").is_none());
}

#[test]
fn test_decode_rejects_malformed_payload() {
    assert!(decode_progress_event(&format!("{PROGRESS_SENTINEL}not json")).is_none());
    assert!(decode_progress_event(&format!("{PROGRESS_SENTINEL}{{}}")).is_none());
}

#[test]
fn test_encode_cluster_progress_event_roundtrip() {
    let encoded = encode_cluster_progress_event(
        "rust-async",
        "async rust",
        SessionPhase::Synthesize,
        "started",
        "sending concept-extraction prompt to gemini/gemini-2.0-flash...",
    );
    assert!(encoded.starts_with(PROGRESS_SENTINEL));
    let decoded = decode_progress_event(&encoded).expect("decode cluster progress");
    assert_eq!(decoded.name, "rust-async");
    assert_eq!(decoded.topic, "async rust");
    assert_eq!(decoded.phase, SessionPhase::Synthesize);
    assert_eq!(decoded.status, StepStatus::Started);
    assert!(decoded.detail.contains("concept-extraction prompt"));
    assert!(decoded.total_sources.is_none());
}

#[test]
fn test_progress_apply_appends_then_completes() {
    let mut p = ResearchProgress::new("n", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    assert_eq!(p.steps.len(), 1);
    assert_eq!(p.steps[0].status, StepStatus::Started);
    p.apply(SessionPhase::Web, StepStatus::Done, "3 source(s) captured");
    assert_eq!(p.steps.len(), 1, "done updates in place");
    assert_eq!(p.steps[0].status, StepStatus::Done);
    assert_eq!(p.steps[0].detail, "3 source(s) captured");
}

#[test]
fn test_progress_render_shows_log_list() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.apply(
        SessionPhase::Setup,
        StepStatus::Started,
        "creating research item",
    );
    p.apply(
        SessionPhase::Setup,
        StepStatus::Done,
        "creating research item",
    );
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    p.apply(SessionPhase::Web, StepStatus::Done, "3 source(s) captured");
    p.finish(3, 0, 0, 0);
    let rendered = p.render();
    assert!(rendered.contains("[research] Research Progress"));
    assert!(rendered.contains("[ok] setup"));
    assert!(rendered.contains("[ok] web"));
    assert!(rendered.contains("[ok] Complete - 3 source(s)"));
    assert!(rendered.contains("/research open rust-async"));
}

#[test]
fn test_progress_render_indents_multiline_detail() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "decomposed into 3 queries:\n  - query one\n  - query two\n  - query three",
    );
    let rendered = p.render();
    assert!(rendered.contains("decomposed into 3 queries:"));
    assert!(rendered.contains("  - query one"));
    assert!(rendered.contains("  - query two"));
    assert!(rendered.contains("  - query three"));
    // Continuation lines should be indented to align with the first detail column.
    let lines: Vec<&str> = rendered.lines().collect();
    let first_idx = lines
        .iter()
        .position(|l| l.contains("decomposed into 3 queries"))
        .expect("first detail line");
    assert!(
        lines[first_idx + 1].starts_with("              "),
        "continuation line should be indented: {}",
        lines[first_idx + 1]
    );
}
