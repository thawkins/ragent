//! Tests for the `/spec govcreate` usage, status/log strings, and the FR-018
//! invocation-frontmatter builder (spec `govdoc` T-004, FR-003, FR-018,
//! NFR-005).

use ragent_specs::SpecCommand;
use ragent_tools_extended::project_scaffold::{ScaffoldRequest, parse_flags};

/// Build a validated scaffold request from `/new` flag tokens.
fn scaffold(flags: &[&str]) -> ScaffoldRequest {
    parse_flags(flags).expect("test fixture flags must be valid")
}

/// Assert a string is ASCII-only (NFR-005).
fn assert_ascii(label: &str, text: &str) {
    assert!(
        text.is_ascii(),
        "{label} must be ASCII-only; found non-ASCII: {text:?}"
    );
}

// ---------------------------------------------------------------------------
// build_govcreate_help_message (FR-003)
// ---------------------------------------------------------------------------

#[test]
fn test_govcreate_help_documents_positionals_and_flags() {
    let help = SpecCommand::build_govcreate_help_message();
    assert_ascii("govcreate help", &help);
    for needle in [
        "From: /spec govcreate",
        "<specid>",
        "<content-ref>",
        "<target-folder>",
        "--language",
        "--type",
        "--stack",
        "--github",
        "--gitlab",
        "--force",
    ] {
        assert!(
            help.contains(needle),
            "help should mention {needle}: {help}"
        );
    }
}

#[test]
fn test_govcreate_help_documents_both_content_reference_forms() {
    let help = SpecCommand::build_govcreate_help_message();
    assert!(
        help.contains("http://") && help.contains("https://"),
        "help should document the URL form: {help}"
    );
    assert!(
        help.contains("local\n") || help.contains("local file or folder"),
        "help should document the local file/folder form: {help}"
    );
    assert!(
        help.contains("quoted"),
        "help should note that paths with spaces must be quoted: {help}"
    );
}

#[test]
fn test_govcreate_help_derives_language_and_type_lists() {
    let help = SpecCommand::build_govcreate_help_message();
    let languages = ragent_tools_extended::project_scaffold::language_value_list();
    let app_types = ragent_tools_extended::project_scaffold::app_type_value_list();
    assert!(
        help.contains(&languages),
        "help should embed the language value list: {help}"
    );
    assert!(
        help.contains(&app_types),
        "help should embed the app-type value list: {help}"
    );
}

#[test]
fn test_govcreate_help_has_no_err_marker() {
    let help = SpecCommand::build_govcreate_help_message();
    assert!(
        !help.contains("[err]"),
        "the usage block is not an error report: {help}"
    );
}

// ---------------------------------------------------------------------------
// status / message / log (NFR-005)
// ---------------------------------------------------------------------------

#[test]
fn test_govcreate_status_names_spec_id() {
    let status = SpecCommand::build_govcreate_status("payments-arch");
    assert_ascii("govcreate status", &status);
    assert!(status.contains("payments-arch"), "status: {status}");
}

#[test]
fn test_govcreate_message_names_all_inputs() {
    let message = SpecCommand::build_govcreate_message(
        "payments-arch",
        "https://docs.example.gov/arch",
        "./payments-svc",
    );
    assert_ascii("govcreate message", &message);
    assert!(
        message.starts_with("From: /spec govcreate"),
        "message should carry the surface prefix: {message}"
    );
    for needle in [
        "payments-arch",
        "https://docs.example.gov/arch",
        "./payments-svc",
        "specs/payments-arch/SPEC.md",
        "PLAN.md",
        "TESTPLAN.md",
    ] {
        assert!(
            message.contains(needle),
            "message should mention {needle}: {message}"
        );
    }
}

#[test]
fn test_govcreate_log_records_inputs_and_force() {
    let log = SpecCommand::build_govcreate_log(
        "payments-arch",
        "https://docs.example.gov/arch",
        "./payments-svc",
        false,
    );
    assert_ascii("govcreate log", &log);
    assert!(log.contains("payments-arch"), "log: {log}");
    assert!(log.contains("https://docs.example.gov/arch"), "log: {log}");
    assert!(log.contains("./payments-svc"), "log: {log}");
    assert!(!log.contains("--force"), "log without force: {log}");

    let forced = SpecCommand::build_govcreate_log("a", "b", "c", true);
    assert!(forced.contains("--force"), "log with force: {forced}");
}

// ---------------------------------------------------------------------------
// build_govcreate_frontmatter (FR-018)
// ---------------------------------------------------------------------------

#[test]
fn test_govcreate_frontmatter_records_invocation() {
    let request = scaffold(&["--language", "rust", "--type", "cmdline", "--stack", "axum"]);
    let fm = SpecCommand::build_govcreate_frontmatter(
        "payments-arch",
        "https://docs.example.gov/arch",
        "./payments-svc",
        &request,
    );
    assert_ascii("govcreate frontmatter", &fm);
    assert!(
        fm.starts_with("---\n"),
        "frontmatter must open a YAML block: {fm}"
    );
    assert!(
        fm.trim_end().ends_with("---"),
        "frontmatter must close: {fm}"
    );
    assert!(fm.contains("status: draft\n"), "frontmatter: {fm}");
    assert!(fm.contains("id: payments-arch\n"), "frontmatter: {fm}");
    assert!(fm.contains("invocation:\n"), "frontmatter: {fm}");
    assert!(
        fm.contains("command: \"/spec govcreate\"\n"),
        "frontmatter: {fm}"
    );
    assert!(
        fm.contains("content_ref: \"https://docs.example.gov/arch\"\n"),
        "frontmatter: {fm}"
    );
    assert!(
        fm.contains("target_folder: \"./payments-svc\"\n"),
        "frontmatter: {fm}"
    );
    assert!(fm.contains("language: rust\n"), "frontmatter: {fm}");
    assert!(fm.contains("type: cmdline\n"), "frontmatter: {fm}");
    assert!(fm.contains("stack: \"axum\"\n"), "frontmatter: {fm}");
    assert!(fm.contains("hosting: none\n"), "frontmatter: {fm}");
}

#[test]
fn test_govcreate_frontmatter_records_absent_stack_and_hosting() {
    let request = scaffold(&["--language", "python", "--type", "library"]);
    let fm = SpecCommand::build_govcreate_frontmatter("legacy", "./docs", "./out", &request);
    assert!(
        fm.contains("stack: null\n"),
        "absent stack should be null: {fm}"
    );
    assert!(fm.contains("hosting: none\n"), "absent hosting: {fm}");
}

#[test]
fn test_govcreate_frontmatter_records_hosting_target() {
    let github = scaffold(&["--language", "rust", "--type", "cmdline", "--github"]);
    let fm = SpecCommand::build_govcreate_frontmatter("a", "b", "c", &github);
    assert!(fm.contains("hosting: github\n"), "frontmatter: {fm}");

    let gitlab = scaffold(&["--language", "rust", "--type", "cmdline", "--gitlab"]);
    let fm = SpecCommand::build_govcreate_frontmatter("a", "b", "c", &gitlab);
    assert!(fm.contains("hosting: gitlab\n"), "frontmatter: {fm}");
}

#[test]
fn test_govcreate_frontmatter_escapes_special_characters() {
    let request = scaffold(&["--language", "rust", "--type", "cmdline"]);
    let fm = SpecCommand::build_govcreate_frontmatter(
        "esc",
        "C:\\docs\\arch \"quoted\"",
        "./out",
        &request,
    );
    assert!(
        fm.contains("content_ref: \"C:\\\\docs\\\\arch \\\"quoted\\\"\"\n"),
        "backslashes and quotes must be escaped: {fm}"
    );
    // The escaping keeps the block parseable: exactly two `---` fence lines.
    assert_eq!(
        fm.lines().filter(|l| l.trim() == "---").count(),
        2,
        "frontmatter block should have exactly one open/close fence: {fm}"
    );
}

#[test]
fn test_govcreate_frontmatter_keeps_one_fence_with_special_values() {
    // A colon-bearing content reference (query string) must stay on its own
    // value line and must not introduce a spurious `---` fence or unquoted
    // colon that would break the block.
    let request = scaffold(&["--language", "rust", "--type", "cmdline", "--stack", "axum"]);
    let fm = SpecCommand::build_govcreate_frontmatter(
        "payments-arch",
        "https://docs.example.gov/arch?x=1&y=2",
        "./payments-svc",
        &request,
    );
    assert_eq!(
        fm.lines().filter(|l| l.trim() == "---").count(),
        2,
        "block should have exactly one open/close fence pair: {fm}"
    );
    assert!(
        fm.contains("content_ref: \"https://docs.example.gov/arch?x=1&y=2\"\n"),
        "the whole URL must stay inside one quoted scalar: {fm}"
    );
}
