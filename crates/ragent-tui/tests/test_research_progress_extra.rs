//! Coverage tests for `ragent_tui::research_progress`.
//!
//! Exercises the `ResearchProgress` state machine (`apply` / `finish` /
//! `render`), the phase/status round-trips through `encode_progress_event`,
//! `encode_cluster_progress_event`, and `decode_progress_event`, and the
//! web-phase deadline bookkeeping. No production code is touched.

use ragent_research::session::{SessionEvent, SessionPhase};
use ragent_tui::research_progress::{
    ResearchProgress, StepStatus, decode_progress_event, encode_cluster_progress_event,
    encode_progress_event,
};

// ---------------------------------------------------------------------------
// StepStatus icons
// ---------------------------------------------------------------------------

#[test]
fn step_status_icons_are_distinct() {
    assert_eq!(StepStatus::Started.icon(), ">");
    assert_eq!(StepStatus::Done.icon(), "[ok]");
    assert_eq!(StepStatus::Error.icon(), "!");
    assert_eq!(StepStatus::Excluded.icon(), "-");
    assert_eq!(StepStatus::Skipped.icon(), "o");
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_initialises_empty_tracker() {
    let p = ResearchProgress::new("run-1", "topic text");
    assert_eq!(p.name, "run-1");
    assert_eq!(p.topic, "topic text");
    assert!(p.steps.is_empty());
    assert!(p.options.is_none());
    assert!(p.total_sources.is_none());
    assert!(!p.done);
    assert_eq!(p.fetched_count, 0);
    assert_eq!(p.failed_count, 0);
    assert_eq!(p.excluded_live, 0);
    assert_eq!(p.pdf_count, 0);
    assert_eq!(p.youtube_count, 0);
    assert_eq!(p.excluded_count, 0);
    assert!(p.web_phase_deadline.is_none());
}

// ---------------------------------------------------------------------------
// apply() - options snapshot, counters, step completion
// ---------------------------------------------------------------------------

#[test]
fn setup_done_options_line_is_stored_not_a_step() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Setup,
        StepStatus::Done,
        "options in use: mode: tiered",
    );
    assert_eq!(p.options.as_deref(), Some("mode: tiered"));
    assert!(p.steps.is_empty(), "options line must not become a step");
}

#[test]
fn setup_done_options_rendered_at_top() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Setup,
        StepStatus::Done,
        "options in use: depth: standard",
    );
    let rendered = p.render();
    assert!(rendered.contains("Options: depth: standard"));
}

#[test]
fn web_captured_and_failed_and_excluded_accumulate_counts() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "web_phase_start");
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "[EN] captured https://a.example",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "[FR] captured https://b.example",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Error,
        "fetch failed for https://c.example: 500",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Excluded,
        "low relevance drop",
    );
    assert_eq!(p.fetched_count, 2);
    assert_eq!(p.failed_count, 1);
    assert_eq!(p.excluded_live, 1);
}

#[test]
fn totals_line_shown_before_completion() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "web_phase_start");
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "captured https://a.example",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Error,
        "fetch failed for https://b.example",
    );
    p.apply(SessionPhase::Web, StepStatus::Excluded, "drop");
    let rendered = p.render();
    assert!(rendered.contains("[totals] Web fetch totals: 1 fetched, 1 failed, 1 excluded"));
}

#[test]
fn totals_line_omits_excluded_when_zero() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "web_phase_start");
    p.apply(
        SessionPhase::Web,
        StepStatus::Error,
        "fetch failed for https://b",
    );
    let rendered = p.render();
    assert!(rendered.contains("[totals] Web fetch totals: 0 fetched, 1 failed"));
    assert!(!rendered.contains("excluded"));
}

#[test]
fn done_step_updates_last_started_in_place() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "begin web");
    p.apply(SessionPhase::Web, StepStatus::Done, "web finished");
    assert_eq!(p.steps.len(), 1, "started step updated in place");
    assert_eq!(p.steps[0].status, StepStatus::Done);
    assert_eq!(p.steps[0].detail, "web finished");
}

#[test]
fn skipped_step_also_completes_previous_started() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "begin");
    p.apply(SessionPhase::Web, StepStatus::Skipped, "not required");
    assert_eq!(p.steps.len(), 1);
    assert_eq!(p.steps[0].status, StepStatus::Skipped);
}

#[test]
fn deadline_event_clears_deadline_and_rewrites_detail() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Web, StepStatus::Started, "web_phase_start");
    p.apply(SessionPhase::Web, StepStatus::Done, "captured https://a");
    p.apply(
        SessionPhase::Web,
        StepStatus::Skipped,
        "web_deadline reached",
    );
    assert!(p.web_phase_deadline.is_none());
    let rendered = p.render();
    assert!(
        rendered.contains("Web phase deadline reached"),
        "deadline notice rewritten; got {rendered}"
    );
    assert!(rendered.contains("1 source(s) captured"));
}

#[test]
fn web_phase_start_parses_deadline_seconds() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Web,
        StepStatus::Started,
        "web_phase_start deadline: 30s",
    );
    assert!(
        p.web_phase_deadline.is_some(),
        "deadline should be armed from detail"
    );
}

#[test]
fn non_matching_phase_appends_step() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(SessionPhase::Synthesize, StepStatus::Started, "synth begin");
    assert_eq!(p.steps.len(), 1);
    assert_eq!(p.steps[0].phase, "synthesize");
}

// ---------------------------------------------------------------------------
// finish() + render() completion line
// ---------------------------------------------------------------------------

#[test]
fn finish_records_counts_and_clears_deadline() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Web,
        StepStatus::Started,
        "web_phase_start deadline: 30s",
    );
    p.finish(7, 2, 1, 3);
    assert_eq!(p.total_sources, Some(7));
    assert_eq!(p.pdf_count, 2);
    assert_eq!(p.youtube_count, 1);
    assert_eq!(p.excluded_count, 3);
    assert!(p.done);
    assert!(p.web_phase_deadline.is_none());
}

#[test]
fn render_completion_line_includes_extras() {
    let mut p = ResearchProgress::new("my-run", "a topic");
    p.finish(5, 2, 2, 1);
    let rendered = p.render();
    assert!(rendered.contains("Complete - 5 source(s)"));
    assert!(rendered.contains("2 PDFs"));
    assert!(rendered.contains("2 YouTube videos"));
    assert!(rendered.contains("1 excluded"));
    assert!(rendered.contains("`/research open my-run`"));
}

#[test]
fn render_completion_line_singular_extras() {
    let mut p = ResearchProgress::new("r", "t");
    p.finish(1, 1, 1, 0);
    let rendered = p.render();
    // Singular forms drop the plural "s" and the extras are joined with " and ".
    assert!(rendered.contains("1 PDF and 1 YouTube video"));
    assert!(!rendered.contains("excluded"));
}

#[test]
fn render_completion_without_extras() {
    let mut p = ResearchProgress::new("r", "t");
    p.finish(4, 0, 0, 0);
    let rendered = p.render();
    assert!(rendered.contains("[ok] Complete - 4 source(s)"));
    assert!(!rendered.contains("including"));
}

#[test]
fn render_multiline_detail_is_continuation_indented() {
    let mut p = ResearchProgress::new("r", "t");
    p.apply(
        SessionPhase::Local,
        StepStatus::Done,
        "first line\nsecond line\nthird",
    );
    let rendered = p.render();
    assert!(rendered.contains("first line\n"));
    assert!(rendered.contains("second line"));
    assert!(rendered.contains("third"));
}

#[test]
fn render_empty_detail_line_renders_prefix_only() {
    let mut p = ResearchProgress::new("r", "t");
    // A synthetic step with an empty detail renders a bare prefix line.
    p.steps.push(ragent_tui::research_progress::ResearchStep {
        phase: "local",
        detail: String::new(),
        status: StepStatus::Started,
    });
    let rendered = p.render();
    assert!(rendered.contains("[research] Research Progress - `r`"));
    assert!(rendered.contains("local"));
}

// ---------------------------------------------------------------------------
// encode / decode
// ---------------------------------------------------------------------------

#[test]
fn encode_progress_event_phase_roundtrip() {
    let encoded = encode_progress_event(
        "n",
        "tp",
        &SessionEvent::Phase {
            phase: SessionPhase::Synthesize,
        },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.name, "n");
    assert_eq!(d.topic, "tp");
    assert_eq!(d.phase, SessionPhase::Synthesize);
    assert_eq!(d.status, StepStatus::Started);
}

#[test]
fn encode_progress_event_empty_queries_renders_no_decomposition() {
    let encoded = encode_progress_event(
        "n",
        "tp",
        &SessionEvent::QueriesDecomposed { queries: vec![] },
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert!(d.detail.contains("no decomposition"));
}

#[test]
fn decode_rejects_non_sentinel_and_malformed() {
    assert!(decode_progress_event("plain text").is_none());
    assert!(decode_progress_event("__research_progress__not json").is_none());
}

#[test]
fn decode_rejects_unknown_phase_and_status() {
    let bad_phase = r#"__research_progress__{"name":"n","topic":"t","phase":"nope","status":"done","detail":"d","total_sources":null,"pdf_count":0,"youtube_count":0,"excluded_count":0}"#;
    assert!(decode_progress_event(bad_phase).is_none());

    let bad_status = r#"__research_progress__{"name":"n","topic":"t","phase":"web","status":"bogus","detail":"d","total_sources":null,"pdf_count":0,"youtube_count":0,"excluded_count":0}"#;
    assert!(decode_progress_event(bad_status).is_none());
}

#[test]
fn decode_status_token_mapping() {
    let cases: &[(&str, StepStatus)] = &[
        ("started", StepStatus::Started),
        ("in_progress", StepStatus::Started),
        ("pending", StepStatus::Started),
        ("completed", StepStatus::Done),
        ("captured", StepStatus::Done),
        ("queries", StepStatus::Done),
        ("preview", StepStatus::Done),
        ("config", StepStatus::Done),
        ("event", StepStatus::Done),
        ("tier done", StepStatus::Done),
        ("failed", StepStatus::Error),
        ("failed_url", StepStatus::Error),
        ("error", StepStatus::Error),
        ("excluded_url", StepStatus::Excluded),
        ("skipped", StepStatus::Skipped),
    ];
    for (token, expected) in cases {
        let encoded = encode_cluster_progress_event("n", "t", SessionPhase::Web, token, "d");
        let d = decode_progress_event(&encoded)
            .unwrap_or_else(|| panic!("token {token} should decode"));
        assert_eq!(d.status, *expected, "token {token}");
    }
}

#[test]
fn encode_cluster_progress_event_roundtrip_fields() {
    let encoded = encode_cluster_progress_event(
        "cluster-run",
        "cluster topic",
        SessionPhase::Assemble,
        "done",
        "assembled",
    );
    let d = decode_progress_event(&encoded).expect("decode");
    assert_eq!(d.name, "cluster-run");
    assert_eq!(d.topic, "cluster topic");
    assert_eq!(d.phase, SessionPhase::Assemble);
    assert_eq!(d.status, StepStatus::Done);
    assert_eq!(d.detail, "assembled");
    assert_eq!(d.pdf_count, 0);
}

#[test]
fn phase_token_roundtrip_all_variants() {
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
        assert_eq!(d.phase, phase, "phase {:?}", phase);
    }
}
