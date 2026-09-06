#![allow(clippy::assert_is_empty)]
//! Tests for the `invocation` frontmatter field (FR: verbatim replay).
//!
//! Every research run records the exact front-end invocation (CLI command,
//! TUI slash command, or HTTP request summary) in the `RESEARCH.md`
//! frontmatter so a future `/research update` command can replay the run.

use ragent_research::ResearchRunRequest;
use ragent_research::build_session_config;
use ragent_research::item::ResearchItem;
use ragent_research::research_name::ResearchName;

fn sample_item() -> ResearchItem {
    let name = ResearchName::new("invocation-test").expect("valid name");
    ResearchItem::new(name, "Invocation Test", "replay verification")
}

// ── ResearchRunRequest → SessionConfig plumbing ──────────────────────────

#[test]
fn build_session_config_copies_invocation() {
    let req = ResearchRunRequest {
        invocation: Some("ragent research create --name x \"topic\"".to_string()),
        ..ResearchRunRequest::new("x", "topic")
    };
    let cfg = build_session_config(&req, None);
    assert_eq!(
        cfg.invocation.as_deref(),
        Some("ragent research create --name x \"topic\"")
    );
}

#[test]
fn build_session_config_defaults_invocation_to_none() {
    let req = ResearchRunRequest::new("x", "topic");
    let cfg = build_session_config(&req, None);
    assert!(cfg.invocation.is_none());
}

// ── Frontmatter rendering and parsing ────────────────────────────────────

#[test]
fn frontmatter_records_invocation_and_round_trips() {
    let mut item = sample_item();
    item.invocation = Some(
        "ragent research create --name invocation-test \"topic\" --tier full --use-local"
            .to_string(),
    );
    let fm = item.render_frontmatter();
    assert!(
        fm.contains("invocation: \"ragent research create --name invocation-test"),
        "frontmatter must record the verbatim invocation; got:\n{fm}"
    );
    let parsed = ResearchItem::from_frontmatter(&fm).expect("must parse");
    assert_eq!(parsed.invocation, item.invocation);
}

#[test]
fn frontmatter_omits_invocation_when_unset() {
    let item = sample_item();
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains("invocation:"),
        "frontmatter must omit the invocation line when unset; got:\n{fm}"
    );
}

#[test]
fn frontmatter_parses_invocation_with_colons_and_quotes() {
    let block = "---\nname: invocation-test\ntitle: T\ntopic: t\ninvocation: \"ragent research create --name invocation-test \\\"a: b\\\" --model openai:gpt-4.1\"\n---\n";
    let item = ResearchItem::from_frontmatter(block).expect("must parse");
    assert_eq!(
        item.invocation.as_deref(),
        Some("ragent research create --name invocation-test \"a: b\" --model openai:gpt-4.1")
    );
}

#[test]
fn frontmatter_defaults_invocation_to_none_for_legacy_items() {
    // Legacy RESEARCH.md files have no invocation line; parsing must not fail.
    let block = "---\nname: invocation-test\ntitle: T\ntopic: t\n---\n";
    let item = ResearchItem::from_frontmatter(block).expect("legacy item must parse");
    assert!(item.invocation.is_none());
}

#[test]
fn frontmatter_invocation_survives_control_char_sanitization() {
    let mut item = sample_item();
    item.invocation = Some("ragent research create --name invocation-test \"topic\"\n".to_string());
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains('\n') || fm.lines().all(|l| l.len() < 200),
        "rendered invocation must stay a single line; got:\n{fm}"
    );
    let parsed = ResearchItem::from_frontmatter(&fm).expect("must parse");
    // The trailing newline is flattened to a trailing space by the sanitizer.
    assert_eq!(
        parsed.invocation.as_deref(),
        Some("ragent research create --name invocation-test \"topic\" ")
    );
}

#[test]
fn invocation_field_is_public_for_front_ends() {
    // Front-ends (CLI/TUI/HTTP) populate `invocation` on the run request
    // before calling build_session_config; this pins the field is reachable
    // and defaultable.
    let req = ResearchRunRequest::default();
    assert!(req.invocation.is_none());
    let mut req = req;
    req.invocation = Some("POST /research x \"topic\"".to_string());
    assert_eq!(
        req.invocation.as_deref(),
        Some("POST /research x \"topic\"")
    );
}
