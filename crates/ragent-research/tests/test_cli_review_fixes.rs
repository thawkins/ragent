//! Tests for the `/research` review fixes in `crates/ragent-research/src/cli.rs`:
//! strict `create` parsing, missing-target usage errors, balanced error
//! quoting, the shared `show` source rows, the per-surface help prefix, and
//! lenient replay of recorded invocations.

use std::path::PathBuf;

use ragent_research::ResearchRunRequest;
use ragent_research::cli::{ResearchCliCommand, render_show_output};
use ragent_research::show_source_rows;
use ragent_research::source::{LocalSourceKind, Source};

fn invalid_message(input: &str) -> String {
    match ResearchCliCommand::parse(input) {
        ResearchCliCommand::Invalid(message) => message,
        other => panic!("expected Invalid for `{input}`, got {other:?}"),
    }
}

#[test]
fn test_create_unknown_flag_is_rejected() {
    let message = invalid_message("create my-item some topic --max-concept 3");
    assert!(
        message.contains("unknown flag `--max-concept`"),
        "{message}"
    );
}

#[test]
fn test_create_flag_before_name_is_not_the_name() {
    let message = invalid_message("create --foo my-item topic");
    assert!(message.contains("unknown flag `--foo`"), "{message}");
}

#[test]
fn test_create_value_flag_without_value_is_rejected() {
    let message = invalid_message("create my-item topic --depth");
    assert_eq!(message, "`--depth` requires a value");
}

#[test]
fn test_create_without_name_names_the_problem() {
    let message = invalid_message("create");
    assert_eq!(message, "`create` requires a research item name");
}

#[test]
fn test_invalid_value_message_has_balanced_backticks() {
    let message = invalid_message("create my-item topic --max-concepts xyz");
    assert_eq!(
        message,
        "invalid value for `--max-concepts xyz`: expected an integer"
    );
    assert_eq!(message.matches('`').count() % 2, 0);
}

#[test]
fn test_bare_research_shows_help() {
    assert_eq!(ResearchCliCommand::parse(""), ResearchCliCommand::Help);
}

#[test]
fn test_verbs_without_a_name_are_usage_errors() {
    for verb in [
        "show", "open", "delete", "archive", "resume", "continue", "update", "cluster", "deck",
        "export",
    ] {
        let message = invalid_message(verb);
        assert_eq!(message, format!("`{verb}` requires a research item name"));
    }
    assert_eq!(
        invalid_message("search"),
        "`search` requires a search query"
    );
    assert_eq!(
        invalid_message("import"),
        "`import` requires a path to import"
    );
}

#[test]
fn test_search_keeps_words_starting_with_dash() {
    assert_eq!(
        ResearchCliCommand::parse("search rust -nightly --json"),
        ResearchCliCommand::Search {
            query: "rust -nightly".to_string(),
            json: true,
        }
    );
}

#[test]
fn test_unterminated_quote_is_rejected() {
    let message = invalid_message("create my-item \"open quote");
    assert!(message.contains("unterminated"), "{message}");
}

#[test]
fn test_recorded_invocation_tolerates_unknown_flags() {
    // A CLI-recorded argv can carry global flags; replay must still work.
    let req = ResearchRunRequest::from_invocation(
        "ragent research create my-item rust async --log-level debug",
    )
    .expect("recorded invocation should replay");
    assert_eq!(req.name, "my-item");
}

#[test]
fn test_show_source_rows_label_each_source_by_its_own_kind() {
    let sources = vec![Source::Local {
        path: "src/lib.rs".into(),
        kind: LocalSourceKind::InProject,
        captured_at: chrono::Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "high".into(),
        body: String::new(),
    }];
    let rows = show_source_rows(&sources);
    assert_eq!(rows.len(), 1);
    let (id, url, _title, kind, _preview) = &rows[0];
    assert_eq!(id, "1");
    assert_eq!(url, "src/lib.rs");
    assert_eq!(kind, "local");
    let out = render_show_output("n", "t", "topic", "draft", "c", "m", &rows);
    assert!(out.contains("  [local] 1: "), "{out}");
    assert!(!out.contains("[web]"), "{out}");
}

#[test]
fn test_help_uses_the_surface_prefix() {
    let tui = ResearchCliCommand::build_help_message_for("/research");
    assert!(tui.starts_with("Available `/research` commands:"), "{tui}");
    let cli = ResearchCliCommand::build_help_message();
    assert!(
        cli.starts_with("Available `ragent research` commands:"),
        "{cli}"
    );
    assert!(cli.contains("(not yet implemented)"));
}

#[test]
fn test_continue_trailing_words_are_the_follow_up() {
    assert_eq!(
        ResearchCliCommand::parse("continue rust-async focus on io_uring integration"),
        ResearchCliCommand::Continue {
            name: "rust-async".to_string(),
            message: Some("focus on io_uring integration".to_string()),
        }
    );
    assert_eq!(
        ResearchCliCommand::parse("continue rust-async --message \"only this\" ignored"),
        ResearchCliCommand::Continue {
            name: "rust-async".to_string(),
            message: Some("only this".to_string()),
        }
    );
}
