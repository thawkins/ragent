#![allow(clippy::assert_is_empty)]
//! Tests for `/research update` invocation replay.
//!
//! The recorded frontmatter `invocation` string is written in three
//! front-end grammars (CLI argv, TUI slash command, HTTP summary).
//! [`ragent_research::ResearchRunRequest::from_invocation`] must rebuild a
//! replayable request from each of them, and the shared
//! [`ragent_research::cli::ResearchCliCommand`] parser must accept the new
//! `update <name>` verb.

use ragent_research::cli::ResearchCliCommand;
use ragent_research::{InvocationParseError, ResearchRunRequest};

// ── CLI argv form ─────────────────────────────────────────────────────────

#[test]
fn from_invocation_parses_cli_argv_form() {
    let recorded = r#"target/debug/ragent research create rust-async "Rust async patterns" --tier full --use-local --format imrad"#;
    let req = ResearchRunRequest::from_invocation(recorded).expect("CLI form must parse");
    assert_eq!(req.name, "rust-async");
    assert_eq!(req.topic, "Rust async patterns");
    assert_eq!(req.tier.as_deref(), Some("full"));
    assert_eq!(req.output_format.as_deref(), Some("imrad"));
    assert!(req.use_local);
    // The replayed request keeps the recorded command so the re-run re-stamps
    // the original invocation rather than the update command.
    assert_eq!(req.invocation.as_deref(), Some(recorded));
}

#[test]
fn from_invocation_parses_cli_argv_with_plural_clap_flags() {
    // clap derives `--from-urls`/`--from-files` from the field names; the
    // shared parser must accept both spellings.
    let recorded =
        "ragent research create seeded --from-urls https://example.com/a --from-files docs/x.md";
    let req = ResearchRunRequest::from_invocation(recorded).expect("plural flags must parse");
    assert_eq!(req.name, "seeded");
    assert_eq!(req.from_urls, vec!["https://example.com/a"]);
    assert_eq!(req.from_files, vec!["docs/x.md"]);
}

#[test]
fn from_invocation_preserves_model_and_concurrency_flags() {
    let recorded = "ragent research create deep-dive topic --mode supervisor \
                    --max-concurrent-research-units 4 --summarization-model openai:gpt-4.1-mini \
                    --clarify --no-papers --evaluate";
    let req = ResearchRunRequest::from_invocation(recorded).expect("flags must parse");
    assert_eq!(req.mode.as_deref(), Some("supervisor"));
    assert_eq!(req.max_concurrent_research_units, Some(4));
    assert_eq!(
        req.summarization_model.as_deref(),
        Some("openai:gpt-4.1-mini")
    );
    assert_eq!(req.clarify, Some(true));
    assert!(req.no_scholarly);
    assert_eq!(req.evaluate, Some(true));
}

// ── TUI slash form ────────────────────────────────────────────────────────

#[test]
fn from_invocation_parses_tui_slash_form() {
    let recorded = r#"/research create rust-async "Rust async patterns" --tier full"#;
    let req = ResearchRunRequest::from_invocation(recorded).expect("TUI form must parse");
    assert_eq!(req.name, "rust-async");
    assert_eq!(req.topic, "Rust async patterns");
    assert_eq!(req.tier.as_deref(), Some("full"));
    assert_eq!(req.invocation.as_deref(), Some(recorded));
}

// ── HTTP summary form ─────────────────────────────────────────────────────

#[test]
fn from_invocation_parses_http_summary_form() {
    let recorded = r#"POST /research api-topic "Rust lifetimes" --tier full --use-local --brief "focus on drop rules""#;
    let req = ResearchRunRequest::from_invocation(recorded).expect("HTTP form must parse");
    assert_eq!(req.name, "api-topic");
    assert_eq!(req.topic, "Rust lifetimes");
    assert_eq!(req.tier.as_deref(), Some("full"));
    assert!(req.use_local);
    assert_eq!(req.brief.as_deref(), Some("focus on drop rules"));
    assert_eq!(req.invocation.as_deref(), Some(recorded));
}

#[test]
fn from_invocation_parses_http_summary_with_flags() {
    let recorded = "POST /research http-run \"topic\" --mode competitive --format comparison-table \
                    --from-url https://example.com --iterations 2";
    let req = ResearchRunRequest::from_invocation(recorded).expect("HTTP flags must parse");
    assert_eq!(req.name, "http-run");
    assert_eq!(req.mode.as_deref(), Some("competitive"));
    assert_eq!(req.output_format.as_deref(), Some("comparison-table"));
    assert_eq!(req.from_urls, vec!["https://example.com"]);
    assert_eq!(req.iterations, Some(2));
}

// ── Round trip: create → invocation → replay ──────────────────────────────

#[test]
fn replayed_request_builds_the_same_session_config() {
    use ragent_research::build_session_config;

    let recorded = r#"ragent research create roundtrip "some topic" --tier full --use-local --mode supervisor"#;
    let original = ResearchRunRequest::from_invocation(recorded).expect("must parse");
    let replayed = ResearchRunRequest::from_invocation(&original.invocation.clone().unwrap())
        .expect("replay must parse");
    assert_eq!(original, replayed, "replay of the replay must be stable");
    // Both map to equivalent session configs.
    let a = build_session_config(&original, None);
    let b = build_session_config(&replayed, None);
    assert_eq!(a.engine.mode, b.engine.mode);
    assert_eq!(a.engine.tier, b.engine.tier);
    assert_eq!(a.local.disable_local, b.local.disable_local);
}

// ── Error cases ───────────────────────────────────────────────────────────

#[test]
fn from_invocation_rejects_empty_string() {
    let err = ResearchRunRequest::from_invocation("   ").expect_err("empty must fail");
    assert!(matches!(err, InvocationParseError::Empty));
}

#[test]
fn from_invocation_rejects_non_create_verbs() {
    let err = ResearchRunRequest::from_invocation("ragent research list --all")
        .expect_err("non-create must fail");
    assert!(matches!(err, InvocationParseError::NotCreate(_)));
    let err = ResearchRunRequest::from_invocation("/research delete doomed")
        .expect_err("delete must fail");
    assert!(matches!(err, InvocationParseError::NotCreate(_)));
}

#[test]
fn from_invocation_rejects_create_without_name() {
    let err = ResearchRunRequest::from_invocation("ragent research create")
        .expect_err("missing name must fail");
    assert!(matches!(err, InvocationParseError::MissingName));
}

// ── `update` verb in the shared parser ────────────────────────────────────

#[test]
fn parse_update_takes_first_positional_name() {
    match ResearchCliCommand::parse("update my-research") {
        ResearchCliCommand::Update { name } => assert_eq!(name, "my-research"),
        other => panic!("expected Update, got {other:?}"),
    }
}

#[test]
fn parse_update_tolerates_extra_arguments() {
    // Recorded CLI/TUI/HTTP update invocations may carry stray arguments;
    // they are tolerated and ignored so replay stays simple.
    match ResearchCliCommand::parse("update my-research --flag value") {
        ResearchCliCommand::Update { name } => assert_eq!(name, "my-research"),
        other => panic!("expected Update, got {other:?}"),
    }
}

#[test]
fn parse_update_without_name_defaults_to_empty() {
    match ResearchCliCommand::parse("update") {
        ResearchCliCommand::Update { name } => assert_eq!(name, ""),
        other => panic!("expected Update, got {other:?}"),
    }
}

#[test]
fn help_message_lists_update() {
    let help = ResearchCliCommand::build_help_message();
    assert!(
        help.contains("update <name>"),
        "help table must document the update verb; got:\n{help}"
    );
}
