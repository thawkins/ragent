//! Inline tests for `research.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::CreateResearchRequest;

#[test]
fn to_run_request_maps_new_mode_and_summarization_and_evaluate() {
    let req = CreateResearchRequest {
        topic: "Compare A and B".into(),
        format: Some("comparison-table".into()),
        mode: Some("competitive".into()),
        summarization_model: Some("ollama:phi4".into()),
        tier: Some("light".into()),
        research_model: Some("anthropic:claude-sonnet-4".into()),
        max_concurrent_research_units: Some(3),
        evaluate: true,
        ..minimal_request("compete", false)
    };

    let run = req.to_run_request();
    assert_eq!(run.mode, Some("competitive".into()));
    assert_eq!(run.output_format, Some("comparison-table".into()));
    assert_eq!(run.summarization_model, Some("ollama:phi4".into()));
    assert_eq!(run.evaluate, Some(true));
    assert_eq!(run.tier, Some("light".into()));
    assert_eq!(run.research_model, Some("anthropic:claude-sonnet-4".into()));
    assert_eq!(run.max_concurrent_research_units, Some(3));
}

#[test]
fn to_run_request_preserves_defaults_when_optional_fields_omitted() {
    let req = CreateResearchRequest {
        topic: "Rust".into(),
        ..minimal_request("plain", false)
    };

    let run = req.to_run_request();
    assert!(run.mode.is_none());
    assert!(run.summarization_model.is_none());
    assert_eq!(run.evaluate, Some(false));
}

/// Build a minimal request with every optional field defaulted, for the
/// scholarly-exclusion invocation tests (FR-009).
fn minimal_request(name: &str, no_scholarly: bool) -> CreateResearchRequest {
    CreateResearchRequest {
        name: name.into(),
        topic: "Rust async".into(),
        title: None,
        sources_dir: None,
        template: None,
        from_urls: Vec::new(),
        from_files: Vec::new(),
        use_local: false,
        use_specs: false,
        use_low_relevance: false,
        no_scholarly,
        use_pdf: false,
        oa_recovery: None,
        fetch_concurrency: None,
        fetch_timeout_secs: None,
        local_concurrency: None,
        depth: None,
        iterations: None,
        format: None,
        mode: None,
        summarization_model: None,
        tier: None,
        web_phase_timeout_secs: None,
        local_phase_timeout_secs: None,
        search_max_retries: None,
        search_retry_base_delay_ms: None,
        max_web_results: None,
        max_search_calls: None,
        max_local_sources: None,
        max_synthesis_sources: None,
        max_concepts: None,
        max_findings: None,
        brief: None,
        research_model: None,
        compression_model: None,
        final_report_model: None,
        max_concurrent_research_units: None,
        evaluate: false,
        url_cloak: false,
    }
}

#[test]
fn invocation_summary_emits_canonical_no_papers_spelling() {
    // FR-005/FR-009: the server must emit the canonical `--no-papers`
    // spelling so the recorded invocation replays on every front-end.
    let req = minimal_request("excl-on", true);
    let summary = req.invocation_summary();
    assert!(
        summary.contains("--no-papers"),
        "summary should carry the canonical flag: {summary}"
    );
    assert!(
        !summary.contains("--no-scholarly"),
        "summary should not emit the legacy alias: {summary}"
    );
    // The recorded invocation must round-trip through the shared parser.
    let replayed = ragent_research::ResearchRunRequest::from_invocation(&summary)
        .expect("canonical summary must replay");
    assert!(replayed.no_scholarly);
}

#[test]
fn invocation_summary_omits_flag_when_exclusion_off() {
    let req = minimal_request("excl-off", false);
    let summary = req.invocation_summary();
    assert!(
        !summary.contains("--no-papers"),
        "summary should not carry the flag when disabled: {summary}"
    );
}

#[test]
fn to_run_request_forwards_scholarly_exclusion() {
    let req = minimal_request("forward", true);
    let run = req.to_run_request();
    assert!(run.no_scholarly);
}

#[test]
fn to_run_request_forwards_concept_and_finding_limits() {
    // FR-012: the HTTP limits must reach the shared run request.
    let req = CreateResearchRequest {
        max_concepts: Some(2),
        max_findings: Some(3),
        ..minimal_request("limits", false)
    };
    let run = req.to_run_request();
    assert_eq!(run.max_concepts, Some(2));
    assert_eq!(run.max_findings, Some(3));
}

#[test]
fn invocation_summary_round_trips_concept_and_finding_limits() {
    // FR-013: the summary emits the flags only when set, and the recorded
    // invocation replays through the hand parser.
    let req = CreateResearchRequest {
        max_concepts: Some(2),
        max_findings: Some(3),
        ..minimal_request("limits-rt", false)
    };
    let summary = req.invocation_summary();
    assert!(
        summary.contains("--max-concepts 2"),
        "summary missing concept limit: {summary}"
    );
    assert!(
        summary.contains("--max-findings 3"),
        "summary missing finding limit: {summary}"
    );
    let replayed = ragent_research::ResearchRunRequest::from_invocation(&summary)
        .expect("summary with limits must replay");
    assert_eq!(replayed.max_concepts, Some(2));
    assert_eq!(replayed.max_findings, Some(3));
}

#[test]
fn invocation_summary_omits_limits_when_unset() {
    let summary = minimal_request("limits-off", false).invocation_summary();
    assert!(
        !summary.contains("--max-concepts") && !summary.contains("--max-findings"),
        "summary must omit the limits when unset: {summary}"
    );
}
