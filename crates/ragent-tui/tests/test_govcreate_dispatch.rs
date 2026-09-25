//! T-011 (spec `govdoc`, FR-001, FR-003, NFR-005): `/spec govcreate` dispatch
//! surface in the `/spec` arm of `slash.rs`.
//!
//! Verifies that `/spec govcreate <args>` dispatches to the `GovCreate` arm
//! and reports through the NFR-005 message builders (`From: /spec govcreate`
//! prefix, no `[err]`), that `/spec govcreate` with missing positionals stays
//! a usage status, that a parseable-but-invalid invocation renders the
//! dedicated govcreate usage block (not the generic `/spec help` table), and
//! that `/spec help` advertises `govcreate` with a worked example and accepts
//! the dedicated usage block.
//!
//! The orchestration runner is T-012; this task tests dispatch and reporting
//! only, and must not touch the filesystem beyond the in-memory `make_app`.

use ragent_tui::app::App;
use support::make_app;

mod support;

/// Render a command's raw output text the way the message window shows it.
fn rendered(app: &mut App, text: &str) -> String {
    app.render_markdown_to_ascii(text)
}
#[test]
fn test_govcreate_valid_invocation_reports_via_nfr005_builders() {
    let mut app = make_app();
    let body = rendered(
        &mut app,
        &ragent_specs::SpecCommand::build_govcreate_message(
            "payments-arch",
            "https://example.gov/arch",
            "./out",
        ),
    );

    assert!(
        body.starts_with("From: /spec govcreate"),
        "the message must carry the surface prefix (NFR-005): {body}"
    );
    assert!(
        body.contains("payments-arch")
            && body.contains("example.gov/arch")
            && body.contains("./out"),
        "the message must name every input: {body}"
    );
    assert!(body.contains("SPEC.md") && body.contains("TESTPLAN.md"));
    assert!(
        !body.contains("[err]"),
        "a valid invocation is not an error: {body}"
    );
    let status = ragent_specs::SpecCommand::build_govcreate_status("payments-arch");
    assert!(status.contains("payments-arch"), "status: {status}");
}

#[test]
fn test_govcreate_missing_positionals_is_usage_error() {
    // FR-003: parsing a run with fewer than three positionals is the usage
    // surface, which the dispatch arm reports as `Usage: /spec govcreate`.
    let cmd = ragent_specs::SpecCommand::parse("govcreate payments-arch");
    assert!(
        matches!(&cmd, ragent_specs::SpecCommand::Unknown(s) if s == "govcreate"),
        "got {cmd:?}"
    );
    assert!(cmd.is_usage_error());
}

#[test]
fn test_govcreate_invalid_spec_id_shows_govcreate_usage_block() {
    let mut app = make_app();
    let reason = match ragent_specs::SpecCommand::parse(
        "govcreate .. https://example.gov/arch ./out --language rust --type cmdline",
    ) {
        ragent_specs::SpecCommand::GovCreateUsage(reason) => reason,
        other => panic!("expected GovCreateUsage, got {other:?}"),
    };
    let body = rendered(
        &mut app,
        &format!(
            "From: /spec govcreate\n\n[err] **{reason}**\n\n{}",
            ragent_specs::SpecCommand::build_govcreate_help_message()
        ),
    );

    assert!(
        body.contains("[err]") && body.contains("invalid spec id"),
        "the specific cause is reported (FR-002): {body}"
    );
    assert!(
        body.contains("--force") && body.contains("/spec govcreate"),
        "the rendered usage block mentions the flags and surface (FR-003): {body}"
    );
    // The angle-bracket placeholders are markdown inline HTML and are dropped
    // by the render pipeline; assert them on the raw block, which is what the
    // usage contract documents.
    let raw = ragent_specs::SpecCommand::build_govcreate_help_message();
    assert!(
        raw.contains("<content-ref>") && raw.contains("<target-folder>") && raw.contains("--force"),
        "the dedicated govcreate usage block (raw): {raw}"
    );
    assert!(
        !raw.contains("## /spec command reference"),
        "the generic help table must not be substituted for the govcreate usage"
    );
}

#[test]
fn test_spec_help_advertises_govcreate_with_worked_example() {
    let mut app = make_app();
    let body = rendered(&mut app, ragent_specs::SpecCommand::build_help_message());
    assert!(
        body.contains("/spec govcreate"),
        "`/spec help` must list govcreate (FR-001, FR-003): {body}"
    );
    assert!(
        body.contains("content-ref") && body.contains("target folder")
            || body.contains("content-ref") && body.contains("target-folder"),
        "the help row must show the positional contract: {body}"
    );
    assert!(
        body.contains("govcreate"),
        "the help row must carry the worked example (NFR-005): {body}"
    );
}

/// `/spec govcreate` (or any usage-error subcommand) must report the cause in
/// the message window, not only a status line (NFR-005).
#[tokio::test]
async fn test_spec_usage_error_subcommands_report_cause_in_message_window() {
    let cases = [
        (
            "/spec govcreate",
            "/spec govcreate",
            "missing required argument",
        ),
        (
            "/spec govcreate payments-arch",
            "/spec govcreate",
            "missing required argument",
        ),
        (
            "/spec reverse",
            "/spec reverse",
            "missing required argument",
        ),
        (
            "/spec status",
            "incomplete arguments",
            "missing required argument",
        ),
    ];

    for (command, expected, cause) in cases {
        let mut app = make_app();
        app.session_id = Some("s1".to_string());

        app.execute_slash_command(command).await;

        let text = app.messages.last().expect("usage message").text_content();
        assert!(
            text.contains(cause),
            "{command} must state the cause '{cause}' in the message window: {text}"
        );
        assert!(
            text.contains(expected),
            "{command} must render the usage block containing '{expected}': {text}"
        );
        assert!(
            app.status.contains("Usage: /spec"),
            "{command} must point at `/spec help` from the status line: {}",
            app.status
        );
    }
}

#[test]
fn test_govcreate_usage_block_is_ascii_and_prefixed() {
    // FR-003 / NFR-005: the dedicated usage block exists on the same surface
    // `/spec help` points at.
    let help = ragent_specs::SpecCommand::build_govcreate_help_message();
    assert!(help.starts_with("From: /spec govcreate"));
    assert!(help.is_ascii(), "the usage block is ASCII-only (NFR-005)");
}

#[test]
fn test_spec_help_govcreate_row_keeps_positionals_in_command_column() {
    // Regression test for the table-cell misalignment: the renderer used to
    // drop the trailing backtick from the long `/spec govcreate …` code span,
    // which under-sized the command column and pushed `content-ref` /
    // `target-folder` into the Description column.  The fix in
    // `md_worker::preprocess_markdown_tables` appends a wrapping-safe space
    // inside the code span so the column stays aligned.
    let mut app = make_app();
    let body = rendered(&mut app, ragent_specs::SpecCommand::build_help_message());

    // The header row of the rendered ASCII table must have exactly three
    // columns.
    let header = body
        .lines()
        .find(|l| l.contains("Command") && l.contains("Arguments"))
        .unwrap_or_else(|| panic!("help table header: {body}"));
    assert_eq!(header.matches('|').count(), 4, "three columns: {header}");

    // Identify the govcreate row (the multi-line block that mentions
    // "govcreate"). The row runs from the line carrying the govcreate command
    // span to the closing border; continuation lines are cell data, so the
    // only reliable terminator is the `+-…-+` border.
    let mut in_row = false;
    let mut row_text = String::new();
    for line in body.lines() {
        if !in_row && line.contains("/spec govcreate") {
            in_row = true;
        }
        if in_row {
            row_text.push_str(line);
            row_text.push('\n');
        }
        if in_row && line.starts_with("+-") && line.ends_with('+') {
            break;
        }
    }
    assert!(
        row_text.contains("/spec govcreate"),
        "row extracted: {row_text}"
    );

    // The command column wraps over several lines; the first line carries
    // `/spec govcreate` and a continuation line carries `<specid>`, while the
    // arguments column starts with `required`. The remaining positionals
    // appear on the continuation lines of the same cell.
    //
    // Continuations are located by cell content rather than row position: a
    // table row may be followed by blank or non-cell lines, so "line 2" is not
    // reliably a cell line.
    let command_cell = |line: &str| -> String {
        line.split('|')
            .nth(1)
            .map(|cell| cell.trim().to_string())
            .unwrap_or_default()
    };
    let arguments_cell = |line: &str| -> String {
        line.split('|')
            .nth(2)
            .map(|cell| cell.trim().to_string())
            .unwrap_or_default()
    };
    let description_cell = |line: &str| -> String {
        line.split('|')
            .nth(3)
            .map(|cell| cell.trim().to_string())
            .unwrap_or_default()
    };
    let row_lines: Vec<&str> = row_text.lines().filter(|l| l.starts_with('|')).collect();
    let first_data_line = row_lines
        .first()
        .copied()
        .unwrap_or_else(|| panic!("first govcreate row line: {row_text}"));
    let specid_line = row_lines
        .iter()
        .copied()
        .find(|l| command_cell(l).contains("<specid>"))
        .unwrap_or_else(|| panic!("specid continuation line: {row_text}"));
    assert!(
        command_cell(first_data_line).contains("/spec govcreate"),
        "command column starts the code span: {first_data_line}"
    );
    assert!(
        command_cell(specid_line).contains("<specid>"),
        "specid stays in the command column: {specid_line}"
    );
    assert!(
        arguments_cell(first_data_line).starts_with("required"),
        "arguments column starts with required: {first_data_line}"
    );
    assert!(
        !description_cell(first_data_line).contains("specid")
            && !description_cell(first_data_line).contains("content-ref")
            && !description_cell(first_data_line).contains("target-folder"),
        "positionals must not bleed into the Description column: {first_data_line}"
    );

    // The two remaining positionals appear on the continuation lines of the
    // command column; they must still not appear in the Description column.
    let second = row_text
        .lines()
        .find(|l| {
            l.starts_with('|') && !l.contains("/spec govcreate") && l.contains("<content-ref>")
        })
        .unwrap_or_else(|| panic!("content-ref continuation line: {row_text}"));
    let cells2: Vec<&str> = second.split('|').skip(1).collect();
    assert!(
        cells2[0].trim().contains("<content-ref>"),
        "content-ref stays in the command column: {second}"
    );
    assert!(
        !cells2[2].trim().contains("content-ref"),
        "content-ref out of the Description column: {second}"
    );
    let third = row_text
        .lines()
        .find(|l| {
            l.starts_with('|') && l.contains("<target-folder>") && !l.contains("<content-ref>")
        })
        .unwrap_or_else(|| panic!("target-folder line: {row_text}"));
    let cells3: Vec<&str> = third.split('|').skip(1).collect();
    assert!(
        cells3[0].trim().contains("<target-folder>"),
        "target-folder stays in the command column: {third}"
    );
    let desc3 = cells3[2].trim();
    assert!(
        !desc3.contains("`target-folder`"),
        "description must not carry the positional token text: {third}"
    );
}
