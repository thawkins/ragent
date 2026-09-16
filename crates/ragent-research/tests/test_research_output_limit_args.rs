//! Integration tests for the `--max-concepts` / `--max-findings` research
//! output-limit arguments (spec `researchmax`; FR-006, FR-007, FR-012, FR-013,
//! FR-019, NFR-003).
//!
//! Three layers are covered here:
//!
//! - the shared hand parser (`ResearchCliCommand::parse`) accepts both flags,
//!   preserves `0` as the "unbounded" sentinel, and rejects malformed values;
//! - the `/research help` text advertises both flags with their defaults;
//! - `ResearchRunRequest::from_invocation` round-trips the flags recorded by
//!   each front-end grammar (CLI argv, TUI slash, HTTP summary).

use ragent_research::ResearchRunRequest;
use ragent_research::cli::ResearchCliCommand;

// ── Hand parser ───────────────────────────────────────────────────────────

#[test]
fn parse_create_accepts_output_limit_flags() {
    let cmd = ResearchCliCommand::parse("create limits topic --max-concepts 3 --max-findings 9");
    match cmd {
        ResearchCliCommand::Create {
            name,
            topic,
            max_concepts,
            max_findings,
            ..
        } => {
            assert_eq!(name, "limits");
            assert_eq!(topic, "topic");
            assert_eq!(max_concepts, Some(3));
            assert_eq!(max_findings, Some(9));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn parse_create_defaults_output_limits_to_none() {
    // FR-006 / FR-007: omitting the flags leaves the resolution to
    // `research.max_*` config or the built-in defaults.
    match ResearchCliCommand::parse("create defaults topic") {
        ResearchCliCommand::Create {
            max_concepts,
            max_findings,
            ..
        } => {
            assert_eq!(max_concepts, None);
            assert_eq!(max_findings, None);
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn parse_create_preserves_zero_output_limit() {
    // FR-016: `0` means unbounded and must survive parsing as `Some(0)`.
    match ResearchCliCommand::parse("create unbounded topic --max-concepts 0 --max-findings 0") {
        ResearchCliCommand::Create {
            max_concepts,
            max_findings,
            ..
        } => {
            assert_eq!(max_concepts, Some(0));
            assert_eq!(max_findings, Some(0));
        }
        other => panic!("expected Create, got {other:?}"),
    }
}

#[test]
fn parse_create_rejects_non_numeric_output_limit() {
    // FR-019: a malformed value is rejected rather than silently ignored.
    for input in [
        "create bad topic --max-concepts abc",
        "create bad topic --max-findings abc",
        "create bad topic --max-concepts -1",
        "create bad topic --max-findings -1",
    ] {
        match ResearchCliCommand::parse(input) {
            ResearchCliCommand::Invalid(msg) => {
                assert!(
                    msg.contains("expected an integer"),
                    "rejection should explain the expected type: {msg}"
                );
            }
            other => panic!("expected Invalid for `{input}`, got {other:?}"),
        }
    }
}

// ── Help text (NFR-003) ───────────────────────────────────────────────────

#[test]
fn help_message_lists_output_limit_flags_with_defaults() {
    let help = ResearchCliCommand::build_help_message();
    assert!(
        help.contains("--max-concepts N"),
        "help must document --max-concepts; got:\n{help}"
    );
    assert!(
        help.contains("--max-findings N"),
        "help must document --max-findings; got:\n{help}"
    );
    assert!(
        help.contains("default 5"),
        "help must state the concept default; got:\n{help}"
    );
    assert!(
        help.contains("default 20"),
        "help must state the finding default; got:\n{help}"
    );
}

// ── Invocation round-trip (FR-012, FR-013) ────────────────────────────────

#[test]
fn from_invocation_round_trips_output_limits_cli_argv_form() {
    let recorded =
        "ragent research create cli-limits \"a topic\" --max-concepts 4 --max-findings 11";
    let req = ResearchRunRequest::from_invocation(recorded).expect("CLI argv form must parse");
    assert_eq!(req.max_concepts, Some(4));
    assert_eq!(req.max_findings, Some(11));

    // Replaying the recorded command must be stable.
    let replayed = ResearchRunRequest::from_invocation(&req.invocation.clone().unwrap())
        .expect("replay must parse");
    assert_eq!(replayed, req);
}

#[test]
fn from_invocation_round_trips_output_limits_tui_slash_form() {
    let recorded = "/research create tui-limits \"a topic\" --max-concepts 2 --max-findings 3";
    let req = ResearchRunRequest::from_invocation(recorded).expect("TUI slash form must parse");
    assert_eq!(req.max_concepts, Some(2));
    assert_eq!(req.max_findings, Some(3));
}

#[test]
fn from_invocation_round_trips_output_limits_http_summary_form() {
    let recorded = "POST /research http-limits \"a topic\" --max-concepts 6 --max-findings 12";
    let req = ResearchRunRequest::from_invocation(recorded).expect("HTTP summary form must parse");
    assert_eq!(req.max_concepts, Some(6));
    assert_eq!(req.max_findings, Some(12));
}

#[test]
fn from_invocation_leaves_output_limits_unset_when_absent() {
    let req = ResearchRunRequest::from_invocation("ragent research create plain \"a topic\"")
        .expect("plain form must parse");
    assert_eq!(req.max_concepts, None);
    assert_eq!(req.max_findings, None);
}
