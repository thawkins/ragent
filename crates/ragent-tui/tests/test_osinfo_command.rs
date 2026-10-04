//! T-024 (spec `osinfo`, FR-013..FR-016, FR-018): `/osinfo` dispatch tests.
//!
//! Drives the `handle_osinfo_command` family through the public
//! `App::execute_slash_command` entry point:
//!
//! - `/osinfo` (bare), `/osinfo help`, `/osinfo --help`, `/osinfo -h` render
//!   the same help page, prefixed `From: /osinfo help`, status `osinfo: help`
//!   (FR-015, TC-009/TC-010);
//! - `/osinfo show` renders the host report prefixed `From: /osinfo show`,
//!   status `osinfo: show` (FR-016, TC-011);
//! - `/osinfo show` renders exactly the `os_info` tool's text output, proving
//!   FR-018's single source of truth (TC-015);
//! - `/osinfo show` and `/osinfo show --no-probe` each match the tool's
//!   same-mode output, so the `--no-probe` flag selects probe=false (FR-014,
//!   FR-016, FR-020);
//! - an unknown subcommand renders a correction listing the valid
//!   subcommands, status `osinfo: usage` (FR-014, TC-013).

use std::collections::BTreeSet;

use support::make_app;

mod support;

/// Text of the most recent assistant message, or `""` when there is none.
fn last_message_text(app: &ragent_tui::App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

/// Normalise a markdown report for comparison: trim each line, drop blanks,
/// and fold the `-` list bullet to the `*` form `render_markdown_to_ascii`
/// emits, so the tool's raw markdown and the slash command's rendered markdown
/// can be compared label-for-label.
fn normalise(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|line| line.trim().replace("- **", "* **"))
        .filter(|line| !line.is_empty())
        .collect()
}

/// True for report lines whose value legitimately drifts between two
/// `os_info` collections in the same process (TC-015: uptime and available
/// memory; CPU frequency is also re-sampled).
fn is_volatile(line: &str) -> bool {
    line.contains("Uptime") || line.contains("Available Memory") || line.contains("Frequency")
}

// ---------------------------------------------------------------------------
// Help page (FR-015, TC-009/TC-010)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_osinfo_help_page_renders_and_sets_status() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());

    app.execute_slash_command("/osinfo help").await;

    let text = last_message_text(&app);
    assert!(
        text.starts_with("From: /osinfo help"),
        "help page must be prefixed `From: /osinfo help`, got: {text}"
    );
    // The subcommand table documents both `show` and `help`.
    assert!(
        text.contains("Subcommand") && text.contains("Description"),
        "help page must render the subcommand table, got: {text}"
    );
    assert!(
        text.contains("/osinfo show") && text.contains("/osinfo help"),
        "help table must list the show and help subcommands, got: {text}"
    );
    // The read-only guarantee and the probe modes are stated, plus at least
    // one worked example. The markdown renderer wraps long paragraphs, so a
    // multi-word phrase can straddle a line break; compare against
    // whitespace-collapsed text.
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        collapsed.contains("read-only") && collapsed.contains("no network request"),
        "help page must state the read-only guarantee, got: {text}"
    );
    assert!(
        collapsed.contains("--no-probe"),
        "help page must document the --no-probe flag, got: {text}"
    );
    assert!(
        text.contains("Examples"),
        "help page must include a worked-example section, got: {text}"
    );
    assert_eq!(app.status, "osinfo: help", "bare/help status bar");
}

#[tokio::test]
async fn test_osinfo_help_aliases_render_identically() {
    // The bare command and all three help aliases produce the same page.
    let mut baseline: Option<String> = None;
    for input in ["/osinfo", "/osinfo help", "/osinfo --help", "/osinfo -h"] {
        let mut app = make_app();
        app.session_id = Some("s".to_string());
        app.execute_slash_command(input).await;

        let text = last_message_text(&app);
        assert_eq!(
            app.status, "osinfo: help",
            "help form '{input}' must set the status bar to `osinfo: help`"
        );
        assert!(
            text.starts_with("From: /osinfo help"),
            "help form '{input}' must carry the help prefix, got: {text}"
        );
        match &baseline {
            None => baseline = Some(text),
            Some(first) => assert_eq!(
                &text, first,
                "help form '{input}' must render identically to `/osinfo help`"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Show report (FR-016, TC-011)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_osinfo_show_renders_report_and_sets_status() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());

    app.execute_slash_command("/osinfo show").await;

    let text = last_message_text(&app);
    assert!(
        text.starts_with("From: /osinfo show"),
        "report must be prefixed `From: /osinfo show`, got: {text}"
    );
    assert_eq!(app.status, "osinfo: show", "`/osinfo show` status bar");
    // The five report sections are present (FR-016).
    for section in ["Operating System", "CPU", "GPU", "Memory", "Process"] {
        assert!(
            text.contains(section),
            "report must contain the `{section}` section, got: {text}"
        );
    }
    // Representative fields across every section.
    for field in [
        "Family",
        "Kernel",
        "Distribution",
        "Architecture",
        "Logical Cores",
        "Total Memory",
        "Uptime",
        "PID",
        "Working Directory",
        "Shell",
        "Username",
    ] {
        assert!(
            text.contains(field),
            "report must contain the `{field}` field, got: {text}"
        );
    }
    // Human-readable text form (FR-016): bytes rendered as GiB.
    assert!(
        text.contains("GiB"),
        "text report must render memory in GiB, got: {text}"
    );
}

// ---------------------------------------------------------------------------
// Single source of truth (FR-018, TC-015)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_osinfo_show_matches_tool_renderer() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    app.execute_slash_command("/osinfo show").await;

    let slash = last_message_text(&app);
    let body = slash
        .strip_prefix("From: /osinfo show")
        .expect("slash report must be prefixed `From: /osinfo show`");

    // FR-018: the slash command reuses the tool's own collector and renderer,
    // so every non-volatile line matches the tool's text output exactly. The
    // default `/osinfo show` and the default tool call both probe (FR-020).
    let tool_info = ragent_agent::tool::os_info::OsInfo::collect();
    let tool_report = ragent_agent::tool::os_info::render_text(&tool_info);

    let slash_lines: BTreeSet<String> = normalise(body)
        .into_iter()
        .filter(|line| !is_volatile(line))
        .collect();
    let tool_lines: BTreeSet<String> = normalise(&tool_report)
        .into_iter()
        .filter(|line| !is_volatile(line))
        .collect();

    assert_eq!(
        slash_lines, tool_lines,
        "`/osinfo show` must render exactly the `os_info` text output (FR-018)"
    );
}

#[tokio::test]
async fn test_osinfo_show_no_probe_matches_process_free_tool_renderer() {
    // FR-014/FR-020: the `--no-probe` flag selects the process-free collection
    // path, so the slash report matches the tool's own `probe: false` output
    // rather than the probed default.
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    app.execute_slash_command("/osinfo show --no-probe").await;

    let slash = last_message_text(&app);
    assert_eq!(app.status, "osinfo: show", "no-probe status bar");
    let body = slash
        .strip_prefix("From: /osinfo show")
        .expect("slash report must be prefixed `From: /osinfo show`");

    let tool_info = ragent_agent::tool::os_info::OsInfo::collect_with_probe(false);
    let tool_report = ragent_agent::tool::os_info::render_text(&tool_info);

    let slash_lines: BTreeSet<String> = normalise(body)
        .into_iter()
        .filter(|line| !is_volatile(line))
        .collect();
    let tool_lines: BTreeSet<String> = normalise(&tool_report)
        .into_iter()
        .filter(|line| !is_volatile(line))
        .collect();

    assert_eq!(
        slash_lines, tool_lines,
        "`/osinfo show --no-probe` must render the process-free `os_info` output (FR-018)"
    );
}

// ---------------------------------------------------------------------------
// Unknown subcommand (FR-014, TC-013)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_osinfo_unknown_subcommand_lists_valid_and_sets_usage() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());

    app.execute_slash_command("/osinfo frobnicate").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("Unknown subcommand"),
        "unknown subcommand must be rejected, got: {text}"
    );
    // FR-014: the correction lists the valid subcommands.
    assert!(
        text.contains("/osinfo show") && text.contains("help"),
        "correction must list the valid `help`/`show` subcommands, got: {text}"
    );
    assert_eq!(app.status, "osinfo: usage", "unknown subcommand status bar");
    // No report is rendered on the error path.
    assert!(
        !text.contains("Operating System"),
        "no report must render for an unknown subcommand, got: {text}"
    );
}
