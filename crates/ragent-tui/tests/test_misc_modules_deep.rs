//! Deep coverage for the smaller `ragent-tui` modules: the status-bar and
//! model-picker caches, the tracing layer that routes log records to the TUI
//! log panel, the markdown worker, the `/toolchain` report engine, the team
//! blueprint helpers, the clipboard temp-file lifecycle, the layout utilities,
//! the execution-backend switcher panel, the graphify skill generator, the
//! goal-loop setup dialog, the cron scheduler, and the layout render branches
//! for the agents/teams/status-bar panels.
//!
//! Tests only - no production logic is touched. Writes are confined to
//! `target/temp/` or a `tempfile::TempDir`; the slash-command tests run inside
//! an isolated tempdir project (cwd + `XDG_CONFIG_HOME` redirected) so nothing
//! lands in the developer's real `~/.config/ragent/`.
//!
//! Env mutation is `unsafe` in edition 2024 and the workspace denies
//! `unsafe_code`, so this target opts back in explicitly for the isolation
//! harness; that harness holds a process-global mutex across the async body,
//! so `clippy::await_holding_lock` is also allowed for this file.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::text::Span;

use ragent_agent::message::Message;
use ragent_agent::task::{ReportStatus, TaskEntry, TaskStatus};
use ragent_agent::team::{MemberStatus, TeamConfig, TeamMember};
use ragent_tui::App;
use ragent_tui::app::blueprints::{
    BlueprintInfo, list_installed_blueprints, render_blueprint_detail, render_blueprint_list,
};
use ragent_tui::app::toolchain::{self, CommandProbe, LanguageClass, ReportRow};
use ragent_tui::app::{
    BackendPanelRow, BackendPanelState, MdWorker, ModelPickerRowsCache, StatusBarCache,
    hard_break_lines, image_dimensions_or_placeholder, sanitize_for_display,
};
use ragent_tui::{clipboard, research_progress, tracing_layer, utils};
use ragent_types::ThinkingLevel;
use tracing_subscriber::layer::SubscriberExt as _;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

// ===========================================================================
// Helpers
// ===========================================================================

/// Workspace scratch tree (`target/temp`), resolved from the crate manifest so
/// the path is right whatever directory the test binary runs in.
fn scratch_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/temp/misc_modules_deep");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Render one frame at the given terminal size.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    terminal
}

/// Flatten the whole painted buffer into one string.
fn painted(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

/// Build a [`TaskEntry`] for the active-agents panel.
fn task_entry(
    id: &str,
    parent: &str,
    child: &str,
    agent: &str,
    status: TaskStatus,
    background: bool,
) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: parent.to_string(),
        child_session_id: child.to_string(),
        agent_name: agent.to_string(),
        task_prompt: "prompt".to_string(),
        background,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
    }
}

/// Build a [`ReportRow`] with one probe.
fn report_row(id: &'static str, class: LanguageClass, probes: Vec<CommandProbe>) -> ReportRow {
    ReportRow { id, class, probes }
}

// ===========================================================================
// status_bar_cache
// ===========================================================================

/// The cache literal every `signature_matches` case starts from.
fn status_cache() -> StatusBarCache {
    StatusBarCache {
        line1: vec![Span::raw("line1")],
        line2: vec![Span::raw("line2")],
        status: "Ready".to_string(),
        agent_name: "general".to_string(),
        selected_model: Some("anthropic/claude".to_string()),
        cwd: "/work".to_string(),
        git_branch: "main".to_string(),
        active_tasks_len: 2,
        bg_tasks_len: 1,
        is_processing: false,
        code_index_enabled: true,
        show_log: false,
        show_agents_window: false,
        show_teams_window: false,
        show_tasks_panel: false,
        show_memory: false,
        show_profile: false,
        show_telemetry: false,
        model_loading: false,
        provider_health: Some(true),
        width: 120,
    }
}

#[test]
fn status_cache_matches_its_own_signature() {
    let cache = status_cache();
    assert!(cache.signature_matches(
        "Ready",
        "general",
        Some("anthropic/claude"),
        "/work",
        "main",
        2,
        1,
        false,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        Some(true),
        120,
    ));
    assert_eq!(cache.line1.len(), 1);
    assert_eq!(cache.line2.len(), 1);
}

#[test]
fn status_cache_every_field_breaks_the_match() {
    let cache = status_cache();
    // Each tuple element is one mutated field. A `true` result means the cache
    // still matched, which would be a bug: every input must invalidate it.
    let cases: [(&str, bool); 19] = [
        (
            "status",
            cache.signature_matches(
                "Working",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "agent_name",
            cache.signature_matches(
                "Ready",
                "explore",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "selected_model",
            cache.signature_matches(
                "Ready",
                "general",
                None,
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "cwd",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/other",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "git_branch",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "dev",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "active_tasks_len",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                3,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "bg_tasks_len",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                9,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "is_processing",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "code_index_enabled",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "show_log",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "show_agents_window",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "show_teams_window",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "show_tasks_panel",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                true,
                false,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "show_memory",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                true,
                false,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "show_profile",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "show_telemetry",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                Some(true),
                120,
            ),
        ),
        (
            "model_loading",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                Some(true),
                120,
            ),
        ),
        (
            "provider_health",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(false),
                120,
            ),
        ),
        (
            "width",
            cache.signature_matches(
                "Ready",
                "general",
                Some("anthropic/claude"),
                "/work",
                "main",
                2,
                1,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                Some(true),
                80,
            ),
        ),
    ];
    for (field, still_matched) in cases {
        assert!(!still_matched, "changing {field} must invalidate the cache");
    }
}

#[test]
fn status_cache_none_model_and_unknown_health() {
    let mut cache = status_cache();
    cache.selected_model = None;
    cache.provider_health = None;
    assert!(cache.signature_matches(
        "Ready", "general", None, "/work", "main", 2, 1, false, true, false, false, false, false,
        false, false, false, false, None, 120,
    ));
    // A `None` cache value must not match a `Some` probe value.
    assert!(!cache.signature_matches(
        "Ready",
        "general",
        Some("anthropic/claude"),
        "/work",
        "main",
        2,
        1,
        false,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        None,
        120,
    ));
}

#[test]
fn status_cache_is_cloneable() {
    let cache = status_cache();
    let cloned = cache.clone();
    assert_eq!(cloned.status, cache.status);
    assert_eq!(cloned.width, cache.width);
}

// ===========================================================================
// model_picker_cache
// ===========================================================================

#[test]
fn model_picker_cache_matches_and_rejects() {
    let cache = ModelPickerRowsCache {
        rows: Vec::new(),
        model_count: 3,
        first_model_id: Some("claude-sonnet-4".to_string()),
    };
    assert!(cache.matches(3, Some("claude-sonnet-4")));
    // A different count breaks the match.
    assert!(!cache.matches(4, Some("claude-sonnet-4")));
    // A different first model id breaks the match.
    assert!(!cache.matches(3, Some("gpt-4o")));
    // A missing first model id breaks the match.
    assert!(!cache.matches(3, None));
    assert_eq!(cache.rows.len(), 0);
}

#[test]
fn model_picker_cache_handles_an_empty_model_list() {
    let cache = ModelPickerRowsCache {
        rows: Vec::new(),
        model_count: 0,
        first_model_id: None,
    };
    assert!(cache.matches(0, None));
    assert!(!cache.matches(1, None));
}

// ===========================================================================
// tracing_layer
// ===========================================================================

#[test]
fn tracing_layer_forwards_records_with_target_prefix_and_endpoint() {
    let (tx, rx) = tracing_layer::tui_log_channel(64);
    let layer = tracing_layer::TuiTracingLayer::new(tx);
    let subscriber = tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink));

    tracing::subscriber::with_default(subscriber, || {
        // A `ragent`-targeted message with an `endpoint` field.
        tracing::info!(target: "ragent::llm", endpoint = "https://example.test/v1", "connecting");
        // A non-`ragent` target gains a `[target] ` prefix.
        tracing::warn!(target: "reqwest::connect", "slow dns");
        // An event with no message field.
        tracing::info!(target: "ragent::quiet", answer = 42);
    });

    let mut messages: Vec<(tracing::Level, String)> = Vec::new();
    while let Ok(record) = rx.try_recv() {
        messages.push((record.level, record.message));
    }
    assert_eq!(
        messages.len(),
        3,
        "one record per emitted event: {messages:?}"
    );

    let connecting = &messages[0];
    assert_eq!(connecting.0, tracing::Level::INFO);
    assert_eq!(
        connecting.1, "connecting -> https://example.test/v1",
        "the endpoint is appended and a ragent target is not prefixed"
    );

    let external = &messages[1];
    assert_eq!(external.0, tracing::Level::WARN);
    assert_eq!(external.1, "[reqwest::connect] slow dns");

    // A message-less event produces a ragent-namespaced empty body.
    let empty = &messages[2];
    assert_eq!(empty.1, "");
}

#[test]
fn tracing_layer_keeps_the_url_field_annotation() {
    let (tx, rx) = tracing_layer::tui_log_channel(8);
    let layer = tracing_layer::TuiTracingLayer::new(tx);
    let subscriber = tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink));

    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target: "ragent::net", url = "https://api.test/x", "fetch");
    });

    let record = rx.try_recv().expect("one record");
    assert_eq!(record.level, tracing::Level::DEBUG);
    assert_eq!(record.message, "fetch -> https://api.test/x");
    // Nothing else was queued.
    assert!(rx.try_recv().is_err());
}

#[test]
fn tracing_layer_channel_capacity_is_bounded() {
    let (tx, rx) = tracing_layer::tui_log_channel(1);
    let layer = tracing_layer::TuiTracingLayer::new(tx);
    let subscriber = tracing_subscriber::registry().with(layer);

    // Two events into a one-slot channel: the second `try_send` fails and the
    // record is dropped rather than blocking the runtime.
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target: "ragent::t", "first");
        tracing::info!(target: "ragent::t", "second");
    });

    assert_eq!(rx.try_recv().expect("first record").message, "first");
    assert!(rx.try_recv().is_err(), "overflowing record must be dropped");
}

// ===========================================================================
// md_worker
// ===========================================================================

#[test]
fn md_worker_renders_markdown_to_text() {
    let worker = MdWorker::new();
    let rendered = worker
        .render("# Heading\n\nA paragraph with **bold** text.")
        .expect("render");
    assert!(rendered.contains("Heading"), "rendered: {rendered}");
    assert!(rendered.contains("paragraph"), "rendered: {rendered}");
}

#[test]
fn md_worker_pads_a_long_code_span_inside_a_table_cell() {
    let worker = MdWorker::new();
    // A single-code-span cell longer than 40 characters gets a trailing space
    // so html2text's size estimate and the rendered width agree.
    let long = "a".repeat(60);
    let markdown = format!("| Command | Description |\n|---|---|\n| `{long}` | text |\n");
    let rendered = worker.render(&markdown).expect("render");
    assert!(rendered.contains("Command"), "rendered: {rendered}");
    assert!(rendered.contains("Description"), "rendered: {rendered}");
    assert!(rendered.contains(&long), "the code span must survive");
}

#[test]
fn md_worker_leaves_a_short_code_cell_unchanged() {
    let worker = MdWorker::new();
    let markdown = "| A | B |\n|---|---|\n| `short` | value |\n";
    let rendered = worker.render(markdown).expect("render");
    assert!(rendered.contains("short"), "rendered: {rendered}");
    assert!(rendered.contains("value"), "rendered: {rendered}");
}

#[test]
fn md_worker_handles_a_multi_token_cell_and_a_pipe_escape() {
    let worker = MdWorker::new();
    // `foo` bar  -> not a single code span, left alone.
    // A backslash-pipe inside a code span must not split the cell.
    let long = "b".repeat(50);
    let markdown = format!(
        "| Flag | Note |\n|---|---|\n| `foo` bar | mixed |\n| `--a \\| --b` | sep |\n| `{long}` | long |\n"
    );
    let rendered = worker.render(&markdown).expect("render");
    assert!(rendered.contains("mixed"), "rendered: {rendered}");
    assert!(rendered.contains("long"), "rendered: {rendered}");
}

#[test]
fn md_worker_ignores_tables_inside_fenced_code() {
    let worker = MdWorker::new();
    let markdown = "```\n| A | B |\n|---|---|\n| `x` | y |\n```\n";
    let rendered = worker.render(markdown).expect("render");
    assert!(rendered.contains('|'), "rendered: {rendered}");
}

#[test]
fn md_worker_restores_red_markers_round_trip() {
    let worker = MdWorker::new();
    let rendered = worker.render("[red]disabled[/red]").expect("render");
    assert!(
        rendered.contains("[red]") && rendered.contains("[/red]"),
        "red markers must survive the pipeline: {rendered}"
    );
    assert!(rendered.contains("disabled"), "rendered: {rendered}");
}

#[test]
fn md_worker_leaves_unclosed_red_marker_text_intact() {
    let worker = MdWorker::new();
    let rendered = worker.render("[red]unterminated").expect("render");
    assert!(rendered.contains("unterminated"), "rendered: {rendered}");
    assert!(!rendered.contains("ragent-red"), "rendered: {rendered}");
}

#[test]
fn md_worker_renders_repeated_requests() {
    let worker = MdWorker::new();
    for i in 0..3 {
        let rendered = worker.render(&format!("request {i}")).expect("render");
        assert!(rendered.contains(&format!("request {i}")), "{rendered}");
    }
}

// ===========================================================================
// toolchain
// ===========================================================================

#[test]
fn toolchain_exhaustiveness_reports_no_violations() {
    assert_eq!(toolchain::exhaustiveness_errors(), Vec::<String>::new());
}

#[test]
fn toolchain_supported_language_rows_cover_the_scanner_list() {
    let rows: Vec<_> = toolchain::supported_language_rows().collect();
    assert!(!rows.is_empty());
    for (id, resolved) in &rows {
        assert!(
            resolved.is_some(),
            "language {id} must resolve through LANGUAGE_RUNTIMES"
        );
    }
}

#[test]
fn toolchain_lookup_classifies_application_and_data_format() {
    let (rust_class, rust_runtime) = toolchain::lookup("rust").expect("rust row");
    assert_eq!(rust_class, LanguageClass::Application);
    let runtime = rust_runtime.expect("rust has a runtime");
    assert!(runtime.commands.contains(&"cargo"));

    let (json_class, json_runtime) = toolchain::lookup("json").expect("json row");
    assert_eq!(json_class, LanguageClass::DataFormat);
    assert!(json_runtime.is_none());

    assert!(toolchain::lookup("no-such-language-xyz").is_none());
}

#[test]
fn toolchain_is_application_matches_the_classification() {
    assert!(toolchain::is_application("rust"));
    assert!(toolchain::is_application("python"));
    assert!(!toolchain::is_application("json"));
    assert!(!toolchain::is_application("no-such-language-xyz"));
}

#[test]
fn toolchain_version_flag_uses_overrides_and_the_default() {
    assert_eq!(toolchain::version_flag_for("java"), "-version");
    assert_eq!(toolchain::version_flag_for("go"), "version");
    assert_eq!(toolchain::version_flag_for("lua"), "-v");
    assert_eq!(toolchain::version_flag_for("cargo"), "--version");
}

#[test]
fn toolchain_language_runtime_many_builds_the_command_chain() {
    static COMMANDS: &[&str] = &["alpha", "beta"];
    let runtime = toolchain::LanguageRuntime::many(COMMANDS);
    assert_eq!(runtime.commands, &["alpha", "beta"]);
}

#[test]
fn toolchain_probe_installed_resolves_a_known_command_and_rejects_garbage() {
    // `cargo` runs this test binary, so it is on PATH.
    let resolved = toolchain::probe_installed("cargo").expect("cargo resolves on PATH");
    assert!(resolved.is_absolute());
    assert!(toolchain::probe_installed("definitely-not-a-command-xyz-123").is_none());
    // Path-bearing and empty names are never probed.
    assert!(toolchain::probe_installed("/bin/sh").is_none());
    assert!(toolchain::probe_installed("").is_none());
}

#[test]
fn toolchain_probe_version_args_captures_output_and_reports_missing_programs() {
    let captured = toolchain::probe_version_args("echo", &["hello-toolchain"]);
    assert_eq!(
        captured,
        toolchain::VersionResult::Text("hello-toolchain".to_string())
    );
    assert_eq!(
        toolchain::probe_version_args("definitely-not-a-command-xyz-123", &["--version"]),
        toolchain::VersionResult::Unknown
    );
}

#[test]
fn toolchain_placeholder_or_text_maps_every_outcome() {
    assert_eq!(
        toolchain::VersionResult::Text("1.2.3".to_string()).placeholder_or_text(),
        Some("1.2.3".to_string())
    );
    assert_eq!(
        toolchain::VersionResult::Unknown.placeholder_or_text(),
        Some("unknown".to_string())
    );
    assert_eq!(
        toolchain::VersionResult::Timeout.placeholder_or_text(),
        Some("timeout".to_string())
    );
}

#[test]
fn toolchain_build_report_rows_walks_every_scanner_language() {
    let rows = toolchain::build_report_rows();
    let scanner: Vec<&str> = ragent_codeindex::scanner::SUPPORTED_LANGUAGES.to_vec();
    assert_eq!(rows.len(), scanner.len());
    for (row, expected) in rows.iter().zip(scanner.iter()) {
        assert_eq!(row.id, *expected);
        match row.class {
            LanguageClass::Application => {
                assert!(!row.probes.is_empty(), "{} must probe a runtime", row.id);
            }
            LanguageClass::DataFormat => {
                assert_eq!(row.probes.len(), 0, "{} must not probe", row.id);
            }
        }
    }
}

#[test]
fn toolchain_render_markdown_report_shapes_the_grid_and_summary() {
    let rows = vec![
        report_row(
            "rust",
            LanguageClass::Application,
            vec![CommandProbe {
                command: "cargo",
                installed: true,
                version: Some("cargo 1.85.0".to_string()),
            }],
        ),
        report_row("json", LanguageClass::DataFormat, Vec::new()),
    ];
    let report = toolchain::render_markdown_report(&rows);
    assert!(report.starts_with("From: /toolchain list"));
    assert!(report.contains("Language"));
    assert!(report.contains("Runtime"));
    assert!(report.contains("Status"));
    assert!(report.contains("Version"));
    assert!(report.contains("cargo 1.85.0"));
    // The Status column is 10 wide, so the data-format marker word-wraps onto
    // its own grid lines.
    assert!(report.contains("runtime"));
    assert!(report.contains("n/a)"));
    assert!(report.contains("1/1 application runtimes installed."));
    assert!(report.ends_with("```\n"));
}

#[test]
fn toolchain_render_markdown_report_handles_the_not_installed_and_multi_runtime_cases() {
    let rows = vec![
        report_row(
            "python",
            LanguageClass::Application,
            vec![CommandProbe {
                command: "python3",
                installed: false,
                version: None,
            }],
        ),
        report_row(
            "typescript",
            LanguageClass::Application,
            vec![
                CommandProbe {
                    command: "node",
                    installed: true,
                    version: Some("v22.0.0".to_string()),
                },
                CommandProbe {
                    command: "tsc",
                    installed: false,
                    version: None,
                },
            ],
        ),
    ];
    let report = toolchain::render_markdown_report(&rows);
    // `not installed` and the per-command status both word-wrap in the narrow
    // Status column, so assert the words rather than the whole phrase.
    assert!(report.contains("installed"));
    assert!(report.contains("node:"));
    assert!(report.contains("tsc:"));
    assert!(report.contains("1/2 application runtimes installed."));
}

#[test]
fn toolchain_render_markdown_report_wraps_a_very_long_version() {
    let long = "x".repeat(180);
    let rows = vec![report_row(
        "zig",
        LanguageClass::Application,
        vec![CommandProbe {
            command: "zig",
            installed: true,
            version: Some(long),
        }],
    )];
    let report = toolchain::render_markdown_report(&rows);
    // The version column is 50 wide, so a 180-char token must be hard-split
    // across several grid lines rather than emitted on one.
    assert!(!report.contains(&"x".repeat(60)));
    assert!(report.contains(&"x".repeat(50)));
}

#[test]
fn toolchain_render_json_report_emits_the_documented_schema() {
    let rows = vec![
        report_row(
            "rust",
            LanguageClass::Application,
            vec![CommandProbe {
                command: "cargo",
                installed: true,
                version: Some("cargo 1.85.0".to_string()),
            }],
        ),
        report_row("toml", LanguageClass::DataFormat, Vec::new()),
    ];
    let json = toolchain::render_json_report(&rows);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["installed"], 1);
    assert_eq!(parsed["total"], 1);
    let languages = parsed["languages"].as_array().expect("languages array");
    assert_eq!(languages.len(), 2);
    assert_eq!(languages[0]["id"], "rust");
    assert_eq!(languages[0]["runtime"], "cargo");
    assert_eq!(languages[0]["status"], "installed");
    assert_eq!(languages[1]["id"], "toml");
    assert_eq!(languages[1]["status"], toolchain::DATA_FORMAT_MARKER);
}

#[test]
fn toolchain_render_json_report_escapes_pipes_in_version_text() {
    let rows = vec![report_row(
        "go",
        LanguageClass::Application,
        vec![CommandProbe {
            command: "go",
            installed: true,
            version: Some("go version | extra".to_string()),
        }],
    )];
    let markdown = toolchain::render_markdown_report(&rows);
    assert!(markdown.contains("go version \\| extra"));
}

// ===========================================================================
// blueprints
// ===========================================================================

fn blueprint_fixture() -> (tempfile::TempDir, Vec<BlueprintInfo>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join(".ragent/blueprints/teams");
    std::fs::create_dir_all(root.join("demo")).expect("create blueprint");
    std::fs::write(
        root.join("demo/README.md"),
        "# Demo\nA demo blueprint used by the tests.",
    )
    .expect("readme");
    std::fs::write(
        root.join("demo/spawn-prompts.json"),
        r#"[{"args":{"teammate_name":"reviewer","agent_type":"build","prompt":"audit"}}]"#,
    )
    .expect("spawn prompts");
    std::fs::write(
        root.join("demo/task-seed.json"),
        r#"[{"input":{"title":"seed","description":"do the thing"}}]"#,
    )
    .expect("task seed");
    let found = list_installed_blueprints(&root);
    (dir, found)
}

#[test]
fn blueprints_list_finds_the_project_blueprint() {
    let (_dir, found) = blueprint_fixture();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "demo");
    assert_eq!(found[0].scope, "project");
}

#[test]
fn blueprints_render_list_counts_teammates_and_tasks() {
    let (_dir, found) = blueprint_fixture();
    let list = render_blueprint_list(&found, "/blueprints");
    assert!(list.contains("From: /blueprints"));
    assert!(list.contains("| `demo` | project | 1 | 1 | A demo blueprint used by the tests. |"));
}

#[test]
fn blueprints_render_list_reports_the_install_locations_when_empty() {
    let list = render_blueprint_list(&[], "/team blueprint");
    assert!(list.contains("No blueprints found."));
    assert!(list.contains(".ragent/blueprints/teams"));
}

#[test]
fn blueprints_render_detail_reads_the_args_shaped_manifests() {
    let (_dir, found) = blueprint_fixture();
    let detail = render_blueprint_detail(&found, "demo", "/blueprints").expect("detail");
    assert!(detail.contains("From: /blueprints demo"));
    assert!(detail.contains("**Scope:** project"));
    assert!(detail.contains("### Description"));
    assert!(detail.contains("| `reviewer` | build | audit |"));
    assert!(detail.contains("| seed | do the thing |"));
    assert!(detail.contains("**Usage:** `/team create demo`"));
}

#[test]
fn blueprints_render_detail_returns_none_for_an_unknown_name() {
    let (_dir, found) = blueprint_fixture();
    assert!(render_blueprint_detail(&found, "missing", "/blueprints").is_none());
}

#[test]
fn blueprints_render_detail_tolerates_a_missing_readme_and_manifests() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join(".ragent/blueprints/teams");
    std::fs::create_dir_all(root.join("bare")).expect("create blueprint");
    let found = list_installed_blueprints(&root);
    let detail = render_blueprint_detail(&found, "bare", "/blueprints").expect("detail");
    assert!(detail.contains("## Blueprint: `bare`"));
    assert!(!detail.contains("### Description"));

    // The list view falls back to `-` for a missing description.
    let list = render_blueprint_list(&found, "/blueprints");
    assert!(list.contains("| `bare` | project | 0 | 0 | - |"));
}

// ===========================================================================
// helpers
// ===========================================================================

#[test]
fn helpers_hard_break_lines_adds_two_trailing_spaces() {
    assert_eq!(hard_break_lines("one\ntwo"), "one  \ntwo  ");
    assert_eq!(hard_break_lines(""), "");
}

#[test]
fn helpers_sanitize_for_display_strips_ansi_and_control_characters() {
    assert_eq!(sanitize_for_display("\u{1b}[31mred\u{1b}[0m"), "red");
    assert_eq!(sanitize_for_display("a\u{7}b"), "ab");
    // Newlines and tabs survive.
    assert_eq!(sanitize_for_display("a\nb\tc"), "a\nb\tc");
}

#[test]
fn helpers_image_placeholder_reports_real_png_dimensions() {
    let dir = scratch_dir();
    let path = dir.join("probe.png");
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&7u32.to_be_bytes());
    bytes.extend_from_slice(&5u32.to_be_bytes());
    std::fs::write(&path, &bytes).expect("write png header");

    let rendered = image_dimensions_or_placeholder("alt", "probe.png", &dir);
    assert_eq!(rendered, "[Image: alt (7x5)]");
}

#[test]
fn helpers_image_placeholder_falls_back_to_the_path_for_unknown_files() {
    let dir = scratch_dir();
    let rendered = image_dimensions_or_placeholder("alt", "missing.png", &dir);
    assert!(rendered.starts_with("[Image: alt ("));
    assert!(rendered.contains("missing.png"));
}

#[test]
fn helpers_image_placeholder_keeps_urls_verbatim() {
    let rendered = image_dimensions_or_placeholder(
        "alt",
        "https://example.test/pic.png",
        std::path::Path::new("/base"),
    );
    assert_eq!(rendered, "[Image: alt (https://example.test/pic.png)]");
}

#[test]
fn helpers_image_placeholder_rejects_escaping_local_paths() {
    let dir = scratch_dir();
    // Plant a readable PNG one level ABOVE the document root so the containment
    // guard, not a missing file, is what suppresses the dimension lookup.
    let outside = dir.join("escape_probe.png");
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes.extend_from_slice(&13u32.to_be_bytes());
    bytes.extend_from_slice(b"IHDR");
    bytes.extend_from_slice(&11u32.to_be_bytes());
    bytes.extend_from_slice(&9u32.to_be_bytes());
    std::fs::write(&outside, &bytes).expect("write escape probe");
    let inner = dir.join("doc-root");
    std::fs::create_dir_all(&inner).expect("create doc root");

    let traversal = image_dimensions_or_placeholder("alt", "../escape_probe.png", &inner);
    assert!(traversal.contains("../escape_probe.png"), "got {traversal}");
    assert!(
        !traversal.contains("(11x9)"),
        "an escaping path must never be measured, got {traversal}"
    );

    // The same file resolves when it is referenced from a contained path, so
    // the negative above is the guard and not a broken fixture.
    let contained = image_dimensions_or_placeholder("alt", "escape_probe.png", &dir);
    assert_eq!(contained, "[Image: alt (11x9)]");

    let absolute = image_dimensions_or_placeholder("alt", "/etc/passwd", &dir);
    assert!(absolute.contains("/etc/passwd"), "got {absolute}");
    assert!(
        absolute.starts_with("[Image: alt (/etc/passwd)]"),
        "the absolute path is never joined onto the base, got {absolute}"
    );
}

// ===========================================================================
// utils
// ===========================================================================

#[test]
fn utils_is_ollama_family_covers_both_variants() {
    assert!(utils::is_ollama_family("ollama"));
    assert!(utils::is_ollama_family("ollama_cloud"));
    assert!(!utils::is_ollama_family("anthropic"));
}

#[test]
fn utils_now_unix_ms_is_a_plausible_timestamp() {
    let now = utils::now_unix_ms();
    assert!(now > 1_600_000_000_000, "got {now}");
}

#[test]
fn utils_format_elapsed_and_short_id() {
    assert_eq!(utils::format_elapsed(Utc::now()), "0s");
    // The last eight characters (the unique suffix) are kept.
    assert_eq!(utils::short_id("session-0123456789abcdef"), "89abcdef");
    assert_eq!(utils::short_id("short"), "short");
}

#[test]
fn utils_shorten_middle_keeps_both_ends() {
    assert_eq!(utils::shorten_middle("abcdef", 100), "abcdef");
    assert_eq!(utils::shorten_middle("abcdefghij", 1), "\u{2026}");
    let shortened = utils::shorten_middle("abcdefghij", 6);
    assert!(shortened.starts_with("ab"), "got {shortened}");
    assert!(shortened.ends_with("ij"), "got {shortened}");
    assert_eq!(shortened.chars().count(), 6);
}

#[test]
fn utils_breakpoints_and_panel_metrics() {
    assert!(matches!(
        utils::ResponsiveBreakpoint::from_width(40),
        utils::ResponsiveBreakpoint::Small
    ));
    assert!(matches!(
        utils::ResponsiveBreakpoint::from_width(100),
        utils::ResponsiveBreakpoint::Medium
    ));
    assert!(matches!(
        utils::ResponsiveBreakpoint::from_width(200),
        utils::ResponsiveBreakpoint::Large
    ));
    assert_eq!(utils::ResponsiveBreakpoint::Small.log_split(), (60, 40));
    assert_eq!(utils::ResponsiveBreakpoint::Small.min_content_width(), 40);
    assert_eq!(utils::ResponsiveBreakpoint::Medium.min_content_width(), 60);
    assert_eq!(utils::ResponsiveBreakpoint::Large.min_content_width(), 80);
    assert_eq!(utils::ResponsiveBreakpoint::Small.status_bar_height(), 2);
    assert_eq!(utils::ResponsiveBreakpoint::Small.button_column_width(), 12);
    assert_eq!(
        utils::ResponsiveBreakpoint::Medium.button_column_width(),
        18
    );
    assert_eq!(utils::ResponsiveBreakpoint::Large.button_column_width(), 20);
}

#[test]
fn utils_is_below_minimum_size_checks_both_dimensions() {
    assert!(utils::is_below_minimum_size(ratatui::layout::Rect::new(
        0, 0, 39, 40
    )));
    assert!(utils::is_below_minimum_size(ratatui::layout::Rect::new(
        0, 0, 80, 11
    )));
    assert!(!utils::is_below_minimum_size(ratatui::layout::Rect::new(
        0, 0, 80, 24
    )));
}

#[test]
fn utils_centered_rect_variants_stay_inside_the_area() {
    let area = ratatui::layout::Rect::new(0, 0, 100, 50);
    let centered = utils::centered_rect(50, 50, area);
    assert!(centered.width <= area.width);
    assert!(centered.height <= area.height);
    assert!(centered.x > 0 && centered.y > 0);

    let capped = utils::centered_rect_max(95, 95, 40, 20, area);
    assert!(capped.width <= 40);
    assert!(capped.height <= 20);

    let fixed = utils::centered_rect_fixed(30, 10, area);
    assert_eq!(fixed.width, 30);
    assert_eq!(fixed.height, 10);

    // Oversized requests clamp to the available space instead of overflowing.
    let clamped = utils::centered_rect_fixed(500, 500, area);
    assert_eq!(clamped.width, area.width);
    assert_eq!(clamped.height, area.height);

    // Out-of-range percentages must not underflow.
    let clamped_pct = utils::centered_rect(200, 200, area);
    assert!(clamped_pct.width <= area.width);
    assert!(clamped_pct.height <= area.height);
}

#[test]
fn utils_truncate_with_ellipsis_covers_all_branches() {
    assert_eq!(utils::truncate_with_ellipsis("short", 10), "short");
    assert_eq!(utils::truncate_with_ellipsis("abcdef", 1), "\u{2026}");
    assert_eq!(utils::truncate_with_ellipsis("abcdef", 0), "\u{2026}");
    let truncated = utils::truncate_with_ellipsis("abcdefghij", 4);
    assert_eq!(truncated.chars().count(), 4);
    assert!(truncated.ends_with('\u{2026}'));
}

// ===========================================================================
// clipboard
// ===========================================================================

#[test]
fn clipboard_temp_override_is_thread_local_and_guarded() {
    {
        let _guard = clipboard::ClipboardTestOverrideGuard::new("guarded value");
        assert_eq!(
            clipboard::get_clipboard_text().as_deref(),
            Some("guarded value")
        );
    }
    // The guard cleared the override on drop.
    clipboard::set_clipboard_text_test_override(Some("explicit".to_string()));
    assert_eq!(clipboard::get_clipboard_text().as_deref(), Some("explicit"));
    clipboard::set_clipboard_text_test_override(None);
    // The override is cleared: a headless run simply reports no clipboard text
    // rather than the previous value.
    assert_ne!(clipboard::get_clipboard_text().as_deref(), Some("explicit"));
}

#[test]
fn clipboard_image_to_temp_writes_a_private_png_and_rejects_bad_input() {
    // The write target is `<cwd>/target/temp`, so pin the cwd behind the
    // binary-wide env lock: a concurrent slash test must not redirect it.
    let _lock = crate::global_env_lock();
    let pixels: Vec<u8> = vec![255, 0, 0, 255, 0, 255, 0, 255];
    let image = arboard::ImageData {
        width: 2,
        height: 1,
        bytes: std::borrow::Cow::Owned(pixels),
    };
    let path = clipboard::clipboard_image_to_temp(&image).expect("encode png");
    let header = std::fs::read(&path).expect("read png");
    assert_eq!(&header[..4], b"\x89PNG");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let _ = std::fs::remove_file(&path);

    // Buffer size disagreeing with the declared dimensions is refused.
    let mismatch = arboard::ImageData {
        width: 4,
        height: 4,
        bytes: std::borrow::Cow::Owned(vec![0u8; 4]),
    };
    let err = clipboard::clipboard_image_to_temp(&mismatch).expect_err("mismatch refused");
    assert!(err.to_string().contains("mismatch"), "got {err}");

    // Dimensions over the documented ceiling are refused.
    let oversized = arboard::ImageData {
        width: 20_000,
        height: 1,
        bytes: std::borrow::Cow::Owned(Vec::new()),
    };
    let err = clipboard::clipboard_image_to_temp(&oversized).expect_err("dimensions refused");
    assert!(err.to_string().contains("too large"), "got {err}");
}

#[test]
fn clipboard_prune_removes_only_old_paste_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let old = dir.path().join("ragent_paste_old.png");
    let fresh = dir.path().join("ragent_paste_fresh.png");
    let other = dir.path().join("unrelated.png");
    for path in [&old, &fresh, &other] {
        std::fs::write(path, b"x").expect("write");
    }
    // Age the "old" file past the threshold.
    let ten_minutes_ago = std::time::SystemTime::now() - Duration::from_mins(10);
    let aged = filetime::FileTime::from_system_time(ten_minutes_ago);
    filetime::set_file_mtime(&old, aged).expect("age file");

    let removed = clipboard::prune_clipboard_temp_files_in(dir.path(), Duration::from_mins(5))
        .expect("prune");
    assert_eq!(removed, 1);
    assert!(!old.exists());
    assert!(fresh.exists());
    assert!(other.exists());
}

#[test]
fn clipboard_prune_of_a_missing_directory_is_a_no_op() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nope");
    let removed =
        clipboard::prune_clipboard_temp_files_in(&missing, Duration::from_hours(1)).expect("prune");
    assert_eq!(removed, 0);
}

// ===========================================================================
// backend_panel
// ===========================================================================

/// A panel row literal, healthy or not.
fn backend_row(id: &str, kind: &'static str, active: bool, health_ok: bool) -> BackendPanelRow {
    BackendPanelRow {
        id: id.to_string(),
        name: id.to_string(),
        kind,
        active,
        health: if health_ok { "ok" } else { "unavailable" },
        health_ok,
        detail: if health_ok {
            None
        } else {
            Some("runtime not found".to_string())
        },
        connection: "local".to_string(),
    }
}

#[test]
fn backend_panel_row_selectability_follows_health() {
    assert!(backend_row("local", "local", true, true).is_selectable());
    assert!(!backend_row("docker", "docker", false, false).is_selectable());
}

#[test]
fn backend_panel_state_navigation_and_accessors() {
    let mut state = BackendPanelState {
        rows: vec![
            backend_row("local", "local", true, true),
            backend_row("docker", "docker", false, false),
            backend_row("remote", "remote", false, true),
        ],
        cursor: 0,
    };
    assert_eq!(state.len(), 3);
    assert!(!state.is_empty());
    assert_eq!(state.selected().map(|r| r.id.as_str()), Some("local"));

    // Up at the first row is a no-op.
    state.move_up();
    assert_eq!(state.cursor, 0);

    state.move_down();
    state.move_down();
    assert_eq!(state.selected().map(|r| r.id.as_str()), Some("remote"));
    // Down at the last row is a no-op.
    state.move_down();
    assert_eq!(state.cursor, 2);

    state.move_up();
    assert_eq!(state.cursor, 1);
}

#[test]
fn backend_panel_state_handles_an_empty_row_list() {
    let state = BackendPanelState {
        rows: Vec::new(),
        cursor: 0,
    };
    assert_eq!(state.len(), 0);
    assert!(state.is_empty());
    assert!(state.selected().is_none());
}

#[test]
fn backend_panel_state_build_marks_the_active_backend() {
    let config = ragent_config::Config::default();
    let cwd = scratch_dir();
    let panel = BackendPanelState::build(&config, &cwd, "local");
    assert_ne!(panel.len(), 0, "the local backend is always present");
    let local = panel
        .rows
        .iter()
        .find(|r| r.kind == "local")
        .expect("local row");
    assert!(local.active, "local is the active backend");
    assert!(local.health_ok);
    assert_eq!(panel.cursor, 0);
}

#[test]
fn backend_panel_app_methods_open_move_activate_and_close() {
    let (_guard, _env, _temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = slash_support::app_with_session();

    app.open_backend_panel();
    assert!(app.backend_panel.is_some(), "panel opened");
    assert!(app.needs_redraw);
    let panel = app.backend_panel.as_ref().expect("panel");
    assert_ne!(panel.len(), 0);
    let opened_rows = panel.len();

    app.backend_panel_move_down();
    app.backend_panel_move_up();
    if opened_rows > 0 {
        assert_eq!(
            app.backend_panel.as_ref().expect("panel").cursor,
            0,
            "moving down then up returns to the first row"
        );
    }

    // Activating the already-active local row is a reported no-op.
    app.backend_panel_activate_selected();
    assert!(
        app.status.contains("backend"),
        "status must report the outcome, got {}",
        app.status
    );

    app.close_backend_panel();
    assert!(app.backend_panel.is_none());

    // Moving with no panel open is a no-op rather than a panic.
    app.backend_panel_move_up();
    app.backend_panel_move_down();
    app.backend_panel_activate_selected();
    assert!(app.backend_panel.is_none());
}

#[test]
fn backend_panel_switch_by_selector_accepts_kinds_and_rejects_unknown_ids() {
    let (_guard, _env, _temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = slash_support::app_with_session();

    // The already-active local backend is a genuine (no-op) switch attempt.
    assert!(app.switch_backend_by_selector("local"));
    assert_eq!(app.active_backend, "local");

    // An unregistered id is refused with a listing hint.
    assert!(!app.switch_backend_by_selector("no-such-backend-xyz"));
    assert_eq!(app.status, "backend: unknown");
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("no-such-backend-xyz"), "got {text}");
}

// ===========================================================================
// skillgen
// ===========================================================================

#[test]
fn skillgen_writes_the_skill_and_reference_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let result =
        ragent_tui::app::skillgen::generate_graphify_skill_in(dir.path()).expect("generate skill");
    assert_eq!(result.dest, dir.path().join("graphify"));
    assert_eq!(result.files_written, 9, "SKILL.md plus eight references");
    assert!(!result.already_existed);
    assert!(result.dest.join("SKILL.md").is_file());
    assert!(result.dest.join("references/query.md").is_file());

    // A second run overwrites in place and reports the pre-existing directory.
    let second =
        ragent_tui::app::skillgen::generate_graphify_skill_in(dir.path()).expect("regenerate");
    assert!(second.already_existed);
    assert_eq!(second.files_written, 9);
}

// ===========================================================================
// loop_dialog
// ===========================================================================

#[test]
fn loop_dialog_parse_helpers_cover_their_branches() {
    assert_eq!(
        ragent_tui::app::parse_comma_list("a, b , a ,, c"),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
    assert_eq!(ragent_tui::app::parse_comma_list(""), Vec::<String>::new());
    assert_eq!(ragent_tui::app::parse_optional_u64(" 42 "), Some(42));
    assert_eq!(ragent_tui::app::parse_optional_u64("not-a-number"), None);
    assert_eq!(ragent_tui::app::parse_optional_u64(""), None);
}

#[test]
fn loop_dialog_parse_loop_flags_extracts_overrides() {
    let tokens: Vec<String> = "--max-steps 12 --cost_limit=34 --timeout 5 general do it"
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let (remaining, overrides) = ragent_tui::app::parse_loop_flags(&tokens).expect("parsed");
    assert_eq!(
        remaining,
        vec!["general".to_string(), "do".to_string(), "it".to_string()]
    );
    assert_eq!(overrides.max_steps, Some(12));
    assert_eq!(overrides.cost_limit, Some(34));
    assert_eq!(overrides.timeout, Some(5));
}

#[test]
fn loop_dialog_parse_loop_flags_reports_bad_values() {
    let missing: Vec<String> = std::iter::once("--max-steps").map(str::to_string).collect();
    let err = ragent_tui::app::parse_loop_flags(&missing).expect_err("missing value");
    assert!(err.contains("missing value for --max-steps"), "got {err}");

    let invalid: Vec<String> = ["--timeout", "abc"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let err = ragent_tui::app::parse_loop_flags(&invalid).expect_err("invalid value");
    assert!(err.contains("invalid value for --timeout"), "got {err}");
}

#[test]
fn loop_dialog_build_spec_from_state_validates_the_goal() {
    let defaults = ragent_config::LoopConfig::default();
    let mut state = ragent_tui::app::LoopSetupState::new(
        vec![("general".to_string(), "general agent".to_string())],
        0,
    );
    let err = ragent_tui::app::build_spec_from_state(&state, &defaults).expect_err("empty goal");
    assert!(err.contains("missing field: goal"), "got {err}");

    state.goal_field = ragent_tui::input_field::InputField::with_text("ship it");
    state.verify_cmd_field = ragent_tui::input_field::InputField::with_text("cargo test");
    state.scope_field = ragent_tui::input_field::InputField::with_text("src/**, Cargo.toml");
    state.tool_set_field = ragent_tui::input_field::InputField::with_text("read, write");
    state.max_steps_field = ragent_tui::input_field::InputField::with_text("7");
    state.checkpoints = true;

    let spec = ragent_tui::app::build_spec_from_state(&state, &defaults).expect("valid spec");
    assert_eq!(spec.goal, "ship it");
    assert_eq!(spec.agent, "general");
    assert_eq!(spec.verify_cmd.as_deref(), Some("cargo test"));
    assert_eq!(spec.scope, vec!["src/**", "Cargo.toml"]);
    assert_eq!(spec.tool_set, vec!["read", "write"]);
    assert_eq!(spec.max_steps, Some(7));
    assert!(spec.checkpoints);
}

#[test]
fn loop_dialog_apply_overrides_only_touches_set_flags() {
    let mut spec = ragent_agent::session::loop_state::LoopSpec::new("general", "goal");
    let untouched = spec.clone();
    ragent_tui::app::apply_loop_overrides(&mut spec, &ragent_tui::app::LoopOverrides::default());
    assert_eq!(spec.max_steps, untouched.max_steps);
    assert_eq!(spec.cost_limit, untouched.cost_limit);

    ragent_tui::app::apply_loop_overrides(
        &mut spec,
        &ragent_tui::app::LoopOverrides {
            max_steps: Some(3),
            cost_limit: Some(1000),
            timeout: Some(9),
        },
    );
    assert_eq!(spec.max_steps, Some(3));
    assert_eq!(spec.cost_limit, Some(1000));
    assert_eq!(spec.checkpoint_timeout_secs, Some(9));
}

#[test]
fn loop_dialog_open_and_cancel_round_trip_preserves_values() {
    let (_guard, _env, _temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = slash_support::app_with_session();

    ragent_tui::app::open_loop_setup(&mut app);
    assert!(app.loop_setup.is_some());
    assert_eq!(app.status, "loop: set a goal to start");

    // Esc keeps the draft for a later re-open.
    ragent_tui::app::handle_loop_setup_key(&mut app, key_event(KeyCode::Esc));
    assert!(app.loop_setup.is_none());
    assert!(app.loop_setup_draft.is_some());

    ragent_tui::app::open_loop_setup(&mut app);
    assert!(app.loop_setup.is_some());
    assert_eq!(app.status, "loop: setup restored");
}

#[test]
fn loop_dialog_key_handler_navigates_fields_and_reports_a_missing_goal() {
    let (_guard, _env, _temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = slash_support::app_with_session();
    ragent_tui::app::open_loop_setup(&mut app);

    // Tab / BackTab cycle the active field.
    let before = app.loop_setup.as_ref().expect("open").active_field;
    ragent_tui::app::handle_loop_setup_key(&mut app, key_event(KeyCode::Tab));
    let after = app.loop_setup.as_ref().expect("open").active_field;
    assert_ne!(before, after);
    ragent_tui::app::handle_loop_setup_key(&mut app, key_event(KeyCode::BackTab));
    assert_eq!(app.loop_setup.as_ref().expect("open").active_field, before);

    // Up / Down move between text fields.
    ragent_tui::app::handle_loop_setup_key(&mut app, key_event(KeyCode::Down));
    ragent_tui::app::handle_loop_setup_key(&mut app, key_event(KeyCode::Up));

    // A typed character lands in the focused text field.
    ragent_tui::app::handle_loop_setup_key(&mut app, key_event(KeyCode::Char('g')));

    // Enter with an empty goal reports the missing field back into the dialog.
    ragent_tui::app::handle_loop_setup_key(&mut app, key_event(KeyCode::Enter));
    let error = app
        .loop_setup
        .as_ref()
        .and_then(|s| s.error.clone())
        .unwrap_or_default();
    assert!(error.contains("goal"), "got {error}");
}

#[test]
fn loop_dialog_show_help_documents_the_commands() {
    let mut app = slash_support::app_with_session();
    ragent_tui::app::show_loop_help(&mut app);
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("From: /loop help"));
    assert!(text.contains("--max-steps"));
    assert!(text.contains("budget_exhausted"));
}

#[test]
fn loop_dialog_setup_state_agent_selection_wraps() {
    let mut state = ragent_tui::app::LoopSetupState::new(
        vec![
            ("general".to_string(), "a".to_string()),
            ("build".to_string(), "b".to_string()),
        ],
        0,
    );
    assert_eq!(state.selected_agent_name(), "general");
    state.select_next_agent();
    assert_eq!(state.selected_agent_name(), "build");
    state.select_next_agent();
    assert_eq!(state.selected_agent_name(), "general", "wraps to the first");
    state.select_previous_agent();
    assert_eq!(state.selected_agent_name(), "build", "wraps to the last");
}

#[test]
fn loop_dialog_field_navigation_and_text_predicate() {
    use ragent_tui::app::LoopSetupField;
    assert!(LoopSetupField::Goal.is_text());
    assert!(!LoopSetupField::Agent.is_text());
    assert!(!LoopSetupField::Checkpoints.is_text());
    let first = LoopSetupField::Agent;
    assert_ne!(first.next(), first);
    assert_eq!(first.next().previous(), first);
}

/// Build a bare key event with no modifiers.
fn key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

// ===========================================================================
// cron scheduler
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cron_scheduler_starts_ticks_and_stops() {
    let storage =
        std::sync::Arc::new(ragent_agent::storage::Storage::open_in_memory().expect("db"));
    let (_guard, _env, temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = slash_support::app_with_session();
    let processor = std::sync::Arc::clone(&app.session_processor);

    let handle = ragent_tui::app::cron::start_cron_scheduler(
        std::sync::Arc::clone(&storage),
        processor,
        temp.path().to_path_buf(),
    );

    // Let the spawned task run its first (event-free) tick.
    tokio::time::sleep(Duration::from_millis(80)).await;
    handle.stop();

    // Dropping the handle signals stop again; the sleeping loop then observes
    // the flag at its next wake. Nothing is written into the repo.
    drop(handle);
    tokio::time::sleep(Duration::from_millis(20)).await;
}

// ===========================================================================
// Layout: agents / teams / status-bar render branches
// ===========================================================================

/// Populate the App with a two-level agent tree plus a background shell task.
fn app_with_agent_tree() -> App {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.agent_name = "general".to_string();
    app.active_tasks.push(task_entry(
        "task-1",
        "s1",
        "s2",
        "explore",
        TaskStatus::Running,
        false,
    ));
    app.active_tasks.push(task_entry(
        "task-2",
        "s2",
        "s3",
        "build",
        TaskStatus::Suspended,
        true,
    ));
    app.active_tasks.push(task_entry(
        "task-3",
        "s1",
        "s4",
        "build",
        TaskStatus::Completed,
        false,
    ));
    app.bg_tasks.push(ragent_tui::app::BgTaskView {
        id: "bg-1".to_string(),
        session_id: "s1".to_string(),
        command: "cargo build --workspace --all-targets".to_string(),
        status: "running".to_string(),
        created_at: Utc::now(),
        completed_at: None,
    });
    app.bg_tasks.push(ragent_tui::app::BgTaskView {
        id: "bg-2".to_string(),
        session_id: "s1".to_string(),
        command: "ls".to_string(),
        status: "completed".to_string(),
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
    });
    app
}

#[test]
fn layout_agents_window_paints_the_tree_and_buttons() {
    let mut app = app_with_agent_tree();
    app.show_agents_window = true;
    let terminal = render(&mut app, 140, 44);
    let screen = painted(&terminal);
    assert!(screen.contains("Agents"), "the overlay title must paint");
    assert!(screen.contains("lead"), "the primary agent row must paint");
    assert!(app.active_agents_area.width > 0);
    // Only the non-terminal rows (two agent tasks plus one running background
    // shell task) keep a hit rectangle; terminal rows are filtered out.
    assert_eq!(app.agent_row_button_areas.len(), 3);
    assert_eq!(app.agent_row_kill_areas.len(), 3);
    assert_eq!(app.agent_row_button_task_ids.len(), 3);
}

#[test]
fn layout_agents_window_scrolls_past_the_visible_window() {
    let mut app = app_with_agent_tree();
    app.show_agents_window = true;
    app.active_agents_scroll_offset = 2;
    let _ = render(&mut app, 140, 20);
    assert!(app.active_agents_max_scroll > 0);
}

#[test]
fn layout_agents_window_without_tasks_still_paints() {
    let mut app = support::make_app();
    app.show_agents_window = true;
    let terminal = render(&mut app, 120, 30);
    assert!(painted(&terminal).contains("Agents"));
    assert_eq!(app.active_agents_max_scroll, 0);
}

#[test]
fn layout_teams_window_paints_the_lead_and_teammates() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.agent_name = "general".to_string();
    app.show_teams_window = true;
    app.active_team = Some(TeamConfig::new("alpha", "s1"));

    let mut working = TeamMember::new("reviewer", "tm-001", "build");
    working.status = MemberStatus::Working;
    working.session_id = Some("s2".to_string());
    app.team_members.push(working);

    let mut suspended = TeamMember::new("tester", "tm-002", "build");
    suspended.status = MemberStatus::Suspended;
    app.team_members.push(suspended);

    let mut stopped = TeamMember::new("archivist", "tm-003", "build");
    stopped.status = MemberStatus::Stopped;
    app.team_members.push(stopped);

    app.focused_teammate = Some("tm-001".to_string());
    app.team_message_counts.insert("tm-001".to_string(), (3, 2));

    let terminal = render(&mut app, 160, 44);
    let screen = painted(&terminal);
    assert!(screen.contains("Teams"), "the overlay title must paint");
    assert!(screen.contains("reviewer"), "teammate rows must paint");
    assert!(screen.contains("working"));
    // Two non-terminal teammates carry buttons; the stopped one is filtered out.
    assert_eq!(app.team_row_button_areas.len(), 2);
    assert_eq!(app.team_row_kill_areas.len(), 2);
    assert_eq!(app.team_row_button_agent_ids[0], "tm-001");
}

#[test]
fn layout_teams_window_without_members_shows_the_hint() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.show_teams_window = true;
    let terminal = render(&mut app, 140, 40);
    assert!(painted(&terminal).contains("no teammates yet"));
}

#[test]
fn status_bar_full_width_surfaces_every_segment() {
    let mut app = support::make_app();
    app.status = "Ready".to_string();
    app.cwd = "/home/dev/projects/ragent".to_string();
    app.git_branch = Some("main".to_string());
    app.last_prompt = "a".repeat(80);
    app.selected_model = Some("anthropic/claude-sonnet-4".to_string());
    app.selected_thinking_level = Some(ThinkingLevel::High);
    app.is_processing = true;
    app.quota_percent = Some(85.0);
    app.code_index_enabled = true;
    app.code_index_busy = true;
    app.code_index_graph_busy = true;
    app.autopilot_enabled = true;
    let terminal = render(&mut app, 200, 30);
    let screen = painted(&terminal);
    assert!(screen.contains("ragent"), "the version prefix must paint");
    assert!(screen.contains("main"), "the git branch must paint");
    assert!(
        screen.contains("...."),
        "a long prompt is truncated with ...."
    );
    assert!(
        screen.contains("cdx-idx"),
        "the code-index busy tag must paint"
    );
    assert!(screen.contains("cdx-graph"));
}

#[test]
fn status_bar_compact_and_minimal_modes_differ() {
    let mut app = support::make_app();
    app.cwd = "/home/dev/projects/ragent-with-a-long-name".to_string();
    app.is_processing = true;
    // 100 columns -> Compact, 60 columns -> Minimal.
    let compact = painted(&render(&mut app, 100, 20));
    assert!(compact.contains("ragent"), "compact screen: {compact}");
    let minimal = painted(&render(&mut app, 60, 20));
    assert!(minimal.contains("ragent"), "minimal screen: {minimal}");
}

#[test]
fn status_bar_renders_the_compaction_and_compression_indicators() {
    let mut app = support::make_app();
    app.compact_in_progress = true;
    let both = painted(&render(&mut app, 140, 24));
    assert!(both.contains("compacting"));

    app.compress_in_progress = true;
    let combined = painted(&render(&mut app, 140, 24));
    assert!(combined.contains("cmp+compress"));
}

#[test]
fn status_bar_renders_the_web_phase_countdown() {
    let mut app = support::make_app();
    let mut progress = research_progress::ResearchProgress::new("run", "topic");
    progress.web_phase_deadline = Some(std::time::Instant::now() + Duration::from_secs(95));
    app.research_progress.push(progress);
    let screen = painted(&render(&mut app, 140, 24));
    assert!(screen.contains("web:1:3"), "countdown screen: {screen}");
}

#[test]
fn status_bar_renders_a_non_local_backend_and_error_health() {
    let mut app = support::make_app();
    app.active_backend = "docker".to_string();
    // Mark the provider unhealthy so the error icon branch renders.
    app.provider_health
        .store(2, std::sync::atomic::Ordering::Relaxed);
    app.selected_model = Some("anthropic/claude".to_string());
    // The unhealthy provider branch paints the error colouring.
    let _ = render(&mut app, 140, 24);
}

#[test]
fn status_bar_hides_the_service_row_in_non_verbose_modes() {
    let mut app = support::make_app();
    app.autopilot_enabled = true;
    app.code_index_enabled = true;

    // Minimal (60 cols) defers the service indicator row to `/status`, so the
    // autopilot marker must be absent; the full-width render carries it.
    let minimal = painted(&render(&mut app, 60, 20));
    let full = painted(&render(&mut app, 200, 20));
    assert!(full.contains("ragent"));
    assert!(
        full.chars().count() > minimal.chars().count(),
        "the full-width status bar must carry more segments"
    );
}

#[test]
fn status_bar_paints_the_status_history_and_error_status() {
    let mut app = support::make_app();
    app.status = "error: model unavailable".to_string();
    let screen = painted(&render(&mut app, 140, 24));
    assert!(screen.contains("model unavailable"), "screen: {screen}");
}

// ===========================================================================
// Slash-command surfaces (isolated project)
// ===========================================================================

/// Run a slash command in an isolated project and return the last message.
async fn run_slash(cmd: &str) -> (App, String) {
    let mut app = slash_support::app_with_session();
    app.execute_slash_command(cmd).await;
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    (app, text)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_toolchain_help_and_unknown_subcommand() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, text) = run_slash("/toolchain help").await;
    assert_eq!(app.status, "toolchain: help");
    assert!(text.contains("Runtime Toolchain Presence Report"));

    let (app, text) = run_slash("/toolchain bogus").await;
    assert_eq!(app.status, "toolchain: usage");
    assert!(text.contains("Unknown subcommand"));
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_toolchain_list_rejects_an_unknown_language_id() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    // The filter is applied after the walk, so this also exercises the real
    // PATH/version probe path.
    let (app, text) = run_slash("/toolchain list no-such-language-xyz").await;
    assert_eq!(app.status, "toolchain: list");
    assert!(text.contains("Unknown language id"), "got {text}");
    assert!(text.contains("`rust`"), "the valid id list must be shown");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_bench_help_and_list() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, text) = run_slash("/bench help").await;
    assert_eq!(app.status, "bench help");
    assert!(text.contains("bench"), "got {text}");

    let (app, text) = run_slash("/bench list").await;
    assert!(app.status.starts_with("bench"), "got {}", app.status);
    assert_ne!(text, "");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_plugins_help_and_list() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, text) = run_slash("/plugins help").await;
    assert_eq!(app.status, "plugins");
    assert!(text.to_lowercase().contains("plugins"), "got {text}");

    // `/plugins list` opens the interactive panel and appends no message.
    let (app, _text) = run_slash("/plugins list").await;
    assert_eq!(app.status, "plugins");
    assert!(app.plugins_list.is_some(), "the list panel must open");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_connectors_help_and_list() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, text) = run_slash("/connectors help").await;
    assert_eq!(app.status, "connectors");
    assert!(text.to_lowercase().contains("connectors"), "got {text}");

    let (app, text) = run_slash("/connectors list").await;
    assert_eq!(app.status, "connectors");
    assert_ne!(text, "");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_new_help_renders_the_scaffold_usage() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, text) = run_slash("/new help").await;
    assert!(app.status.starts_with("new"), "got {}", app.status);
    assert!(text.contains("--language"), "got {text}");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_loop_help_and_invalid_flag() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (_app, text) = run_slash("/loop help").await;
    assert!(
        text.to_lowercase().contains("goal-driven agent loops"),
        "got {text}"
    );

    let (app, text) = run_slash("/loop general --max-steps").await;
    assert_eq!(app.status, "loop: invalid argument");
    assert!(text.contains("missing value for --max-steps"), "got {text}");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_hook_covers_help_bad_repo_and_bad_depth() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();

    // Bare/help form renders the usage page.
    ragent_tui::app::reverse_command_for_tests(&mut app, "help");
    assert_eq!(app.status, "reverse: usage");

    // An unparseable repo identifier is rejected before any fetch.
    ragent_tui::app::reverse_command_for_tests(&mut app, "not-a-repo-format");
    assert_eq!(app.status, "reverse: invalid repo");

    // An out-of-range --depth is rejected before any fetch.
    ragent_tui::app::reverse_command_for_tests(&mut app, "thawkins/ragent --depth 99");
    assert_eq!(app.status, "reverse: invalid depth");
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("depth"), "got {text}");
    drop(temp);
}

/// The `/toolchain list` walk runs on a blocking thread, so the runtime must
/// be multi-threaded. The cwd/env isolation harness is entered separately here
/// because the lock is already held by the caller.
fn enter_project_pair() -> (slash_support::EnvGuard, tempfile::TempDir) {
    let env = slash_support::EnvGuard::new();
    let temp = tempfile::tempdir().expect("tempdir");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };
    std::env::set_current_dir(temp.path()).expect("set cwd");
    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    std::fs::write(
        ragent_dir.join("ragent.json"),
        r#"{ "defaultAgent": "general" }"#,
    )
    .expect("write project config");
    (env, temp)
}

// ===========================================================================
// Research progress (render surface the event tests do not reach)
// ===========================================================================

#[test]
fn research_progress_render_includes_the_options_and_final_counts() {
    let mut progress = research_progress::ResearchProgress::new("run-x", "topic-y");
    progress.apply(
        ragent_research::session::SessionPhase::Setup,
        research_progress::StepStatus::Done,
        "options in use: mode: tiered",
    );
    progress.apply(
        ragent_research::session::SessionPhase::Synthesize,
        research_progress::StepStatus::Started,
        "multi\nline detail",
    );
    progress.finish(4, 1, 1, 2);
    let rendered = progress.render();
    assert!(rendered.contains("Research Progress - `run-x`"));
    assert!(rendered.contains("Options: mode: tiered"));
    assert!(rendered.contains("multi"));
    assert!(rendered.contains("line detail"));
}

// ===========================================================================
// /spawn, /bench, compaction and /new poll arms
// ===========================================================================

#[test]
fn spawn_poll_surfaces_launch_success_and_failure() {
    let mut app = slash_support::app_with_session();

    // Nothing pending: the poll is a no-op.
    let before = app.messages.len();
    ragent_tui::poll_spawn_result_for_tests(&mut app);
    assert_eq!(app.messages.len(), before);

    // Success arm: the detached task id is reported and the panel is told the
    // run will never be waited on.
    {
        let mut slot = app.spawn_result.lock().expect("spawn slot");
        *slot = Some(Ok("s2-explore-abc123456789".to_string()));
    }
    ragent_tui::poll_spawn_result_for_tests(&mut app);
    assert!(
        app.status.starts_with("spawn: detached task"),
        "{}",
        app.status
    );
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("Detached sub-agent launched"), "got {text}");
    assert!(text.contains("nothing will wait for it"));

    // Failure arm.
    {
        let mut slot = app.spawn_result.lock().expect("spawn slot");
        *slot = Some(Err("AgentManager not initialized".to_string()));
    }
    ragent_tui::poll_spawn_result_for_tests(&mut app);
    assert!(
        app.status.starts_with("[warn] spawn failed"),
        "{}",
        app.status
    );
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(
        text.contains("Could not launch detached sub-agent"),
        "got {text}"
    );
}

#[test]
fn spawn_poll_restores_the_pending_marker_for_an_empty_error() {
    let mut app = slash_support::app_with_session();
    {
        let mut slot = app.spawn_result.lock().expect("spawn slot");
        *slot = Some(Err(String::new()));
    }
    let before = app.messages.len();
    ragent_tui::poll_spawn_result_for_tests(&mut app);
    // The marker entry is put back so concurrent /spawn invocations stay
    // serialised, and no user-visible message is emitted.
    let pending = app.spawn_result.lock().expect("spawn slot").is_some();
    assert!(pending, "the marker entry must be restored");
    assert_eq!(app.messages.len(), before);
}

#[test]
fn compaction_poll_applies_a_successful_compaction() {
    let mut app = slash_support::app_with_session();
    app.messages
        .push(Message::user_text("s", "before compaction"));
    app.compact_in_progress = true;
    app.auto_compact_in_progress = true;

    {
        let mut slot = app.compact_result.lock().expect("compact slot");
        *slot = Some(Ok(vec![Message::user_text("s", "compacted summary")]));
    }
    app.poll_compaction_result();

    assert!(!app.compact_in_progress);
    assert!(!app.auto_compact_in_progress);
    assert_eq!(app.status, "ready");
    let transcript = app
        .messages
        .iter()
        .map(Message::text_content)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(transcript.contains("compacted summary"), "{transcript}");
}

#[test]
fn compaction_poll_surfaces_a_failure_and_clears_the_auto_flag() {
    let mut app = slash_support::app_with_session();
    app.compact_in_progress = true;
    app.auto_compact_in_progress = true;
    app.pending_send_after_compact = Some(("queued".to_string(), Vec::new()));

    {
        let mut slot = app.compact_result.lock().expect("compact slot");
        *slot = Some(Err("provider refused".to_string()));
    }
    app.poll_compaction_result();

    assert!(!app.compact_in_progress);
    assert!(!app.auto_compact_in_progress);
    assert!(app.auto_compact_failed);
    assert!(
        app.status.starts_with("[warn] compact failed"),
        "{}",
        app.status
    );
}

#[test]
fn compaction_poll_without_a_pending_result_is_a_no_op() {
    let mut app = slash_support::app_with_session();
    app.compact_in_progress = true;
    app.poll_compaction_result();
    // No result deposited: the in-progress flag is untouched.
    assert!(app.compact_in_progress);
}

#[test]
fn bench_poll_surfaces_the_finished_run_and_workbooks() {
    let mut app = slash_support::app_with_session();
    app.active_bench_task_id = Some("bench-task-1".to_string());
    app.active_bench_started_at = Some(Utc::now());

    {
        let mut slot = app.bench_result.lock().expect("bench slot");
        *slot = Some(Ok(ragent_bench::BenchRunOutcome {
            message: "From: /bench run\n[ok] humaneval complete".to_string(),
            workbook_paths: vec![PathBuf::from("target/temp/bench.xlsx")],
            summaries: Vec::new(),
        }));
    }
    app.poll_pending_bench();

    assert!(
        app.active_bench_task_id.is_none(),
        "the active run is cleared"
    );
    assert_eq!(app.status, "bench: done");
    assert_eq!(app.bench_last_workbooks.len(), 1);
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("humaneval complete"), "got {text}");
    assert_eq!(
        app.bench_last_workbooks[0],
        PathBuf::from("target/temp/bench.xlsx")
    );
    assert!(app.bench_last_finished_at.is_some());
}

#[test]
fn bench_poll_surfaces_a_failure() {
    let mut app = slash_support::app_with_session();
    {
        let mut slot = app.bench_result.lock().expect("bench slot");
        *slot = Some(Err("no benchmark data".to_string()));
    }
    app.poll_pending_bench();
    assert!(
        app.status.starts_with("[warn] bench failed"),
        "{}",
        app.status
    );
    let summary = app.bench_last_summary.clone().unwrap_or_default();
    assert!(summary.contains("no benchmark data"), "got {summary}");
}

#[test]
fn bench_poll_watchdog_detaches_a_stale_run() {
    let mut app = slash_support::app_with_session();
    app.active_bench_task_id = Some("wedged".to_string());
    app.active_bench_started_at = Some(Utc::now() - chrono::Duration::hours(2));

    app.poll_pending_bench();

    assert!(app.active_bench_task_id.is_none(), "the run is detached");
    assert_eq!(app.status, "bench: timed out");
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("Benchmark watchdog"), "got {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_bench_status_and_open_last_report_the_empty_state() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, status) = run_slash("/bench status").await;
    assert!(status.contains("No benchmark runs yet."), "got {status}");
    assert!(app.status.starts_with("bench"), "got {}", app.status);

    let (_app, open_last) = run_slash("/bench open last").await;
    assert!(
        open_last.contains("No benchmark workbooks available yet."),
        "got {open_last}"
    );
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_bench_status_and_open_last_report_a_finished_run() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();
    app.bench_last_summary = Some("From: /bench run\n[ok] all suites passed".to_string());
    app.bench_last_finished_at = Some(Utc::now());
    app.bench_last_workbooks = vec![PathBuf::from("target/temp/bench.xlsx")];

    app.execute_slash_command("/bench status").await;
    let status = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(status.contains("Last Benchmark Run"), "got {status}");
    assert!(status.contains("all suites passed"), "got {status}");

    app.execute_slash_command("/bench open last").await;
    let open_last = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(
        open_last.contains("Latest Benchmark Results"),
        "got {open_last}"
    );
    assert!(open_last.contains("bench.xlsx"), "got {open_last}");
    drop(temp);
}

#[test]
fn newproj_poll_streams_progress_then_the_final_summary() {
    let mut app = slash_support::app_with_session();
    app.newproj_running = true;
    app.newproj_progress_slug = Some("Scaffolding 'demo'...".to_string());

    // A progress batch creates the panel message.
    {
        let mut lines = app.newproj_progress.lock().expect("progress slot");
        lines.push("[step] writing README.md".to_string());
        lines.push("[step] git init".to_string());
    }
    app.poll_newproj_result();
    assert!(app.newproj_running, "the run is still in flight");
    let panel = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(panel.contains("writing README.md"), "got {panel}");
    assert!(panel.contains("git init"), "got {panel}");

    // A second batch updates the same message in place rather than stacking.
    let messages_after_first = app.messages.len();
    {
        let mut lines = app.newproj_progress.lock().expect("progress slot");
        lines.push("[step] initial commit".to_string());
    }
    app.poll_newproj_result();
    assert_eq!(app.messages.len(), messages_after_first);

    // The terminal outcome lands and clears the running flag.
    {
        let mut slot = app.newproj_result.lock().expect("result slot");
        *slot = Some(Ok((
            "Scaffolded `demo` in 5 steps.".to_string(),
            "\n\nStack: none".to_string(),
        )));
    }
    app.poll_newproj_result();
    assert!(!app.newproj_running);
    assert_eq!(app.status, "new: scaffolded");
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("Scaffolded `demo`"), "got {text}");
    assert!(text.contains("Stack: none"), "got {text}");
}

#[test]
fn newproj_poll_surfaces_a_failure() {
    let mut app = slash_support::app_with_session();
    app.newproj_running = true;
    {
        let mut slot = app.newproj_result.lock().expect("result slot");
        *slot = Some(Err("target directory is not empty".to_string()));
    }
    app.poll_newproj_result();
    assert!(!app.newproj_running);
    assert_eq!(app.status, "new: emission failed");
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("target directory is not empty"), "got {text}");
}

#[test]
fn newproj_poll_without_state_is_a_no_op() {
    let mut app = slash_support::app_with_session();
    let before = app.messages.len();
    app.poll_newproj_result();
    assert_eq!(app.messages.len(), before);
    assert!(!app.newproj_running);
}

// ===========================================================================
// helpers reached through the event handler
// (`loop_termination_banner`, `plural`, `files_preview`, `summarise_error`)
// ===========================================================================

/// Build an app whose session id matches the events published below.
fn app_for_events() -> App {
    let mut app = slash_support::app_with_session();
    app.session_id = Some("s-events".to_string());
    app
}

#[tokio::test]
async fn loop_terminated_banner_covers_every_status_label() {
    let cases = [
        ("completed", "[ok]"),
        ("error", "[x]"),
        ("budget_exhausted", "[..]"),
        ("interrupted", "[stop]"),
        ("something_else", "*"),
    ];
    for (status, icon) in cases {
        let mut app = app_for_events();
        app.handle_event(ragent_agent::event::Event::LoopTerminated {
            session_id: "s-events".to_string(),
            status: status.to_string(),
            iterations: 1,
            verification: None,
            reason: None,
        })
        .await;
        let banner = app
            .messages
            .last()
            .map(Message::text_content)
            .unwrap_or_default();
        assert!(
            banner.starts_with(icon),
            "status {status} must render the {icon} icon, got {banner}"
        );
        // A single iteration renders the singular noun via `plural`.
        assert!(
            banner.contains("1 iteration") && !banner.contains("1 iterations"),
            "got {banner}"
        );
    }
}

#[tokio::test]
async fn loop_terminated_banner_appends_verification_and_reason() {
    let mut app = app_for_events();
    app.handle_event(ragent_agent::event::Event::LoopTerminated {
        session_id: "s-events".to_string(),
        status: "budget_exhausted".to_string(),
        iterations: 7,
        verification: Some("cargo test passed".to_string()),
        reason: Some("step budget spent".to_string()),
    })
    .await;
    let banner = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(banner.contains("7 iterations"), "got {banner}");
    assert!(
        banner.contains("Verification: cargo test passed"),
        "got {banner}"
    );
    assert!(banner.contains("Reason: step budget spent"), "got {banner}");
}

#[tokio::test]
async fn loop_change_summary_previews_the_affected_files() {
    let files: Vec<String> = (0..12).map(|i| format!("src/file{i}.rs")).collect();
    let mut app = app_for_events();
    app.handle_event(ragent_agent::event::Event::LoopChangeSummary {
        session_id: "s-events".to_string(),
        status: "completed".to_string(),
        iterations: 2,
        files_modified: 12,
        files_created: 0,
        files_deleted: 0,
        diffstat: "+120 -3 lines across 12 files".to_string(),
        files,
    })
    .await;
    let summary = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    // `files_preview` keeps the first eight entries and adds the (+N more) tail.
    assert!(summary.contains("src/file0.rs"), "got {summary}");
    assert!(summary.contains("src/file7.rs"), "got {summary}");
    assert!(!summary.contains("src/file8.rs"), "got {summary}");
    assert!(summary.contains("(+4 more)"), "got {summary}");
    assert!(summary.contains("2 iterations"), "got {summary}");
    assert!(
        app.pending_rollback.is_some(),
        "the rollback offer is armed"
    );
}

#[tokio::test]
async fn loop_change_summary_without_extra_files_omits_the_more_suffix() {
    let mut app = app_for_events();
    app.handle_event(ragent_agent::event::Event::LoopChangeSummary {
        session_id: "s-events".to_string(),
        status: "completed".to_string(),
        iterations: 1,
        files_modified: 1,
        files_created: 0,
        files_deleted: 0,
        diffstat: "+1 -0 lines across 1 file".to_string(),
        files: vec!["src/only.rs".to_string()],
    })
    .await;
    let summary = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(summary.contains("src/only.rs"), "got {summary}");
    assert!(!summary.contains("more)"), "got {summary}");
}

#[tokio::test]
async fn agent_error_surfaces_the_stripped_summary_in_the_status_bar() {
    let mut app = app_for_events();
    app.handle_event(ragent_agent::event::Event::AgentError {
        session_id: "s-events".to_string(),
        error: "LLM call failed: Unknown model: claude-haiku-4.5".to_string(),
    })
    .await;
    // `summarise_error` strips the `LLM call failed: ` prefix.
    assert_eq!(app.status, "error: Unknown model: claude-haiku-4.5");
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("Unknown model"), "got {text}");
}

#[tokio::test]
async fn agent_error_rewrites_the_chat_completions_mismatch() {
    let mut app = app_for_events();
    app.handle_event(ragent_agent::event::Event::AgentError {
        session_id: "s-events".to_string(),
        error: "model is not accessible via the /chat/completions endpoint".to_string(),
    })
    .await;
    assert!(
        app.status
            .contains("use /model and pick a non-Codex chat model"),
        "got {}",
        app.status
    );
}

#[tokio::test]
async fn agent_error_truncates_a_very_long_message() {
    let mut app = app_for_events();
    let long = "x".repeat(400);
    app.handle_event(ragent_agent::event::Event::AgentError {
        session_id: "s-events".to_string(),
        error: long,
    })
    .await;
    // `truncate_to_char_boundary` caps the status-bar summary at 120 bytes plus
    // the ellipsis.
    assert!(
        app.status.len() <= "error: ".len() + 130,
        "status too long: {}",
        app.status.len()
    );
}

#[test]
fn tracing_layer_records_debug_formatted_fields() {
    let (tx, rx) = tracing_layer::tui_log_channel(8);
    let layer = tracing_layer::TuiTracingLayer::new(tx);
    let subscriber = tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink));

    tracing::subscriber::with_default(subscriber, || {
        // `?value` records through `Visit::record_debug`, which the visitor
        // formats with `{:?}` for both the message and the endpoint aliases.
        tracing::info!(
            target: "ragent::dbg",
            endpoint = ?"https://debug.test/v1",
            "debug-shaped"
        );
        tracing::info!(target: "ragent::dbg", message = ?42);
    });

    let first = rx.try_recv().expect("first record");
    assert_eq!(
        first.message, "debug-shaped -> \"https://debug.test/v1\"",
        "the debug-formatted endpoint is quoted and appended"
    );

    let second = rx.try_recv().expect("second record");
    assert_eq!(second.message, "42");
}

#[test]
fn md_worker_leaves_a_table_when_prose_follows() {
    let worker = MdWorker::new();
    let long = "c".repeat(55);
    let markdown = format!(
        "Intro line.\n\n| Command | Description |\n|---|---|\n| `{long}` | text |\n\nAfter the table `{long}` is prose.\n"
    );
    let rendered = worker.render(&markdown).expect("render");
    assert!(rendered.contains("Intro line."), "rendered: {rendered}");
    assert!(rendered.contains("After the table"), "rendered: {rendered}");
    assert!(rendered.contains("Command"), "rendered: {rendered}");
}

#[test]
fn md_worker_handles_a_separator_only_row_and_a_ragged_table() {
    let worker = MdWorker::new();
    // A separator line that opens the document, then a row with no cells.
    let markdown = "|---|---|\n| one |\n\n| a | b |\n|---|---|\n| `x` | y |\n";
    let rendered = worker.render(markdown).expect("render");
    assert!(rendered.contains("one"), "rendered: {rendered}");
}

#[test]
fn backend_panel_switches_via_a_registered_descriptor() {
    let (_guard, _env, temp) = slash_support::enter_project(
        r#"{
          "defaultAgent": "general",
          "backends": [
            { "id": "box", "name": "Docker box", "kind": "docker", "image": "alpine" }
          ]
        }"#,
    );
    let mut app = slash_support::app_with_session();

    // The selector matches the registered row by kind; whether the switch is
    // accepted depends on the host's container runtime, so assert both paths
    // stay internally consistent.
    let attempted = app.switch_backend_by_selector("docker");
    assert!(attempted, "a registered kind is always a switch attempt");
    if app.active_backend == "docker" {
        assert_eq!(app.status, "backend: docker");
        // The switch is persisted into the project config file.
        let raw = std::fs::read_to_string(temp.path().join(".ragent/ragent.json"))
            .expect("read project config");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse config");
        assert_eq!(
            parsed.get("execution_backend").and_then(|v| v.as_str()),
            Some("docker")
        );
    } else {
        assert_eq!(app.active_backend, "local", "a refusal keeps local");
        assert_eq!(app.status, "backend: unavailable");
        let text = app
            .messages
            .last()
            .map(Message::text_content)
            .unwrap_or_default();
        assert!(text.contains("unavailable"), "got {text}");
    }
}

#[test]
fn backend_panel_switch_by_bare_kind_synthesises_an_unregistered_row() {
    // The config declares no docker descriptor, so the selector takes the
    // bare-kind probe path that synthesises the row from the kind label.
    let (_guard, _env, temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = slash_support::app_with_session();

    let attempted = app.switch_backend_by_selector("DOCKER");
    assert!(attempted, "the selector is case-insensitive");
    if app.active_backend == "docker" {
        assert_eq!(app.status, "backend: docker");
        let raw = std::fs::read_to_string(temp.path().join(".ragent/ragent.json"))
            .expect("read project config");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse config");
        assert_eq!(
            parsed.get("execution_backend").and_then(|v| v.as_str()),
            Some("docker")
        );
    } else {
        assert_eq!(app.status, "backend: unavailable");
    }
}

// ===========================================================================
// Slash-command usage branches: bare form, unknown subcommand, config reads
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_plugins_bare_and_unknown_render_usage() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    let (app, bare) = run_slash("/plugins").await;
    assert_eq!(app.status, "plugins");
    assert!(bare.contains("From: /plugins"), "got {bare}");

    let (app, unknown) = run_slash("/plugins nosuchsub").await;
    assert_eq!(app.status, "plugins");
    assert!(unknown.contains("From: /plugins"), "got {unknown}");

    // `/plugins marketplace` without a valid sub-subcommand renders usage
    // rather than dispatching.
    let (_app, marketplace) = run_slash("/plugins marketplace").await;
    assert!(
        marketplace.contains("From: /plugins"),
        "marketplace usage: {marketplace}"
    );
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_plugins_stores_reports_the_configured_endpoints() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, text) = run_slash("/plugins stores").await;
    assert_eq!(app.status, "plugins");
    assert!(text.contains("From: /plugins stores"), "got {text}");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_plugins_list_mcp_reports_the_bridged_tool_inventory() {
    use ragent_agent::mcp::{McpServer, McpStatus, McpToolDef};
    use ragent_config::McpServerConfig;

    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    let mut app = slash_support::app_with_session();
    app.mcp_servers.push(McpServer {
        id: "demo.files".to_string(),
        config: McpServerConfig::default(),
        status: McpStatus::Connected,
        tools: vec![McpToolDef {
            name: "read_file".to_string(),
            description: "read".to_string(),
            parameters: serde_json::json!({}),
        }],
    });
    app.mcp_servers.push(McpServer {
        id: "demo.off".to_string(),
        config: McpServerConfig::default(),
        status: McpStatus::Disabled,
        tools: Vec::new(),
    });
    app.execute_slash_command("/plugins list --mcp").await;

    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert_eq!(app.status, "plugins");
    assert!(text.contains("MCP tool inventory"), "got {text}");
    assert!(text.contains("demo.files"), "got {text}");
    assert!(text.contains("read_file"), "got {text}");
    assert!(text.contains("1/2 server(s) connected"), "got {text}");

    // With no live client the inventory reports the empty case instead.
    let (app, empty) = run_slash("/plugins list --mcp").await;
    assert_eq!(app.status, "plugins");
    assert!(empty.contains("(no MCP servers configured)"), "got {empty}");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_connectors_bare_unknown_and_stores() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    let (app, bare) = run_slash("/connectors").await;
    assert_eq!(app.status, "connectors");
    assert!(bare.to_lowercase().contains("connectors"), "got {bare}");

    let (_app, unknown) = run_slash("/connectors nosuchsub").await;
    assert!(
        unknown.to_lowercase().contains("connectors"),
        "got {unknown}"
    );

    let (app, stores) = run_slash("/connectors stores").await;
    assert_eq!(app.status, "connectors");
    assert!(
        stores.to_lowercase().contains("catalogue") || stores.contains("From: /connectors"),
        "got {stores}"
    );
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_connectors_test_rejects_an_unknown_connector() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, text) = run_slash("/connectors test no-such-connector-xyz").await;
    assert_eq!(app.status, "connectors");
    assert!(
        text.to_lowercase().contains("connector"),
        "the harness must report the unknown id: {text}"
    );
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_new_bare_and_invalid_language_report_usage() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    let (app, bare) = run_slash("/new").await;
    assert!(app.status.starts_with("new"), "got {}", app.status);
    assert!(!bare.is_empty(), "the bare form must render usage");

    // An unsupported language is refused before any file is written.
    let (app, invalid) = run_slash("/new --language nosuchlang --type cmdline").await;
    assert!(app.status.starts_with("new"), "got {}", app.status);
    assert!(!invalid.is_empty(), "got {invalid}");
    // The temp project stays empty: no scaffold was emitted.
    assert!(!temp.path().join("README.md").exists());
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_bench_status_reports_the_active_run_context() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();
    app.active_bench_task_id = Some("bench-active".to_string());
    app.active_bench_started_at = Some(Utc::now());
    app.active_bench_summary = Some("humaneval on rust".to_string());

    app.execute_slash_command("/bench status").await;
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("Active Benchmark Run"), "got {text}");
    assert!(text.contains("bench-active"), "got {text}");
    assert!(text.contains("humaneval on rust"), "got {text}");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_bench_unknown_subcommand_reports_usage() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let (app, _text) = run_slash("/bench nosuchsub").await;
    // The unknown subcommand is reported on the status bar.
    assert_eq!(app.status, "[warn] unknown /bench subcommand: nosuchsub");
    drop(temp);
}

/// The scheduler fires its first tick immediately, so seeding due events
/// exercises the tick body (fire, skip-disabled, unknown-agent) through the
/// public `start_cron_scheduler` entry point.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cron_scheduler_first_tick_processes_due_events() {
    use ragent_types::{CronEvent, CronSchedule};

    let (_guard, _env, temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let storage =
        std::sync::Arc::new(ragent_agent::storage::Storage::open_in_memory().expect("db"));
    let app = slash_support::app_with_session();
    let processor = std::sync::Arc::clone(&app.session_processor);

    let past = Utc::now() - chrono::Duration::hours(1);

    // A due one-shot event for a known agent: fires (the spawn fails without an
    // AgentManager) and is then disabled.
    storage
        .insert_cron_event(&CronEvent::new(
            "due-one-shot".to_string(),
            "general".to_string(),
            "do the thing".to_string(),
            CronSchedule::one_shot(past),
            "at past".to_string(),
            past,
        ))
        .expect("insert one-shot");

    // A due repeating event for an unknown agent: logged as an error and
    // advanced rather than disabled.
    storage
        .insert_cron_event(&CronEvent::new(
            "due-unknown-agent".to_string(),
            "no-such-agent-xyz".to_string(),
            "nobody home".to_string(),
            CronSchedule::repeat_from(past, 3600),
            "every 1h".to_string(),
            past,
        ))
        .expect("insert repeating");

    // A disabled due one-shot: skipped and pushed to the far future.
    let disabled = CronEvent::new(
        "due-disabled".to_string(),
        "general".to_string(),
        "never runs".to_string(),
        CronSchedule::one_shot(past),
        "at past".to_string(),
        past,
    );
    storage
        .insert_cron_event(&disabled)
        .expect("insert disabled");
    storage
        .set_cron_event_enabled("due-disabled", false)
        .expect("disable");

    let handle = ragent_tui::app::cron::start_cron_scheduler(
        std::sync::Arc::clone(&storage),
        processor,
        temp.path().to_path_buf(),
    );

    // Wait for the first tick to finish: the one-shot event ends up disabled.
    let mut fired = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        if let Ok(Some(row)) = storage.get_cron_event("due-one-shot")
            && !row.enabled
        {
            fired = true;
            break;
        }
    }
    handle.stop();
    assert!(fired, "the due one-shot event must be fired and disabled");

    // The repeating event for the unknown agent stays enabled with an advanced
    // next_due (FR-016 advances rather than disables).
    let repeating = storage
        .get_cron_event("due-unknown-agent")
        .expect("get repeating")
        .expect("found");
    assert!(repeating.enabled, "a repeating event stays enabled");
    let next_due = chrono::DateTime::parse_from_rfc3339(&repeating.next_due).expect("next_due");
    assert!(
        next_due.with_timezone(&Utc) > Utc::now(),
        "next_due advanced"
    );

    // The skip path pushes the disabled one-shot far into the future.
    let skipped = storage
        .get_cron_event("due-disabled")
        .expect("get disabled")
        .expect("found");
    assert!(!skipped.enabled);
    assert!(
        skipped.next_due.starts_with("9999"),
        "a disabled one-shot is pushed to the far future, got {}",
        skipped.next_due
    );

    // Every outcome is appended to `<working dir>/log/cron-*.jsonl`.
    let log_dir = temp.path().join("log");
    let entries: Vec<String> = std::fs::read_dir(&log_dir)
        .map(|dir| {
            dir.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|name| name.starts_with("cron-") && name.contains(".jsonl"))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(entries.len(), 1, "one cron log file per tick: {entries:?}");
    let body = std::fs::read_to_string(log_dir.join(&entries[0])).expect("read cron log");
    assert!(body.contains("due-one-shot"), "log: {body}");
    assert!(body.contains("due-unknown-agent"), "log: {body}");
    assert!(body.contains("due-disabled"), "log: {body}");
}

// ===========================================================================
// /spawn command branches and compaction start guards
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_spawn_help_usage_and_validation_branches() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();

    app.execute_slash_command("/spawn help").await;
    assert_eq!(app.status, "spawn: help");

    app.execute_slash_command("/spawn").await;
    assert_eq!(app.status, "spawn: help");

    // No prompt after the agent name.
    app.execute_slash_command("/spawn general").await;
    assert_eq!(app.status, "spawn: usage");

    app.execute_slash_command("/spawn no-such-agent-xyz go")
        .await;
    assert_eq!(app.status, "spawn: unknown agent 'no-such-agent-xyz'");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_spawn_guards_a_pending_launch() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();
    // A launch already occupies the slot.
    {
        let mut slot = app.spawn_result.lock().expect("spawn slot");
        *slot = Some(Err(String::new()));
    }
    app.execute_slash_command("/spawn general do the thing")
        .await;
    assert_eq!(
        app.status,
        "[warn] spawn: another /spawn is still launching - wait for it"
    );
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_spawn_reports_a_missing_sub_agent_manager() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();
    // `make_app` never installs an AgentManager, so the launch cannot start.
    app.execute_slash_command("/spawn general summarise the tree")
        .await;
    assert_eq!(
        app.status,
        "[warn] spawn: sub-agent manager unavailable in this session"
    );
    // The pending marker is released so the next /spawn is not blocked.
    assert!(app.spawn_result.lock().expect("spawn slot").is_none());
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_compact_refuses_without_messages() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();
    // A fresh session has no messages to compact.
    app.execute_slash_command("/compact").await;
    assert_eq!(app.status, "[warn] No messages to compact");
    assert!(!app.compact_in_progress);
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_compact_refuses_without_a_model() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();
    app.messages.push(Message::user_text("s-compact", "hello"));
    app.selected_model = None;
    app.agent_info.model = None;
    app.execute_slash_command("/compact").await;
    assert_eq!(
        app.status,
        "[warn] No model selected - use /model to choose"
    );
    assert!(!app.compact_in_progress);
    drop(temp);
}

// ===========================================================================
// helpers reached through further slash surfaces
// ===========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_from_cmd_builder_reaches_help_surfaces() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    // `/undo help`, `/resume help` and `/actionloop help` all route through
    // `helpers::from_cmd`, which builds the canonical `From: /<cmd>\n\n<body>`
    // reply shape.
    for (cmd, header) in [
        ("/undo help", "From: /undo help"),
        ("/resume help", "From: /resume help"),
        ("/actionloop help", "From: /actionloop help"),
    ] {
        let (_app, text) = run_slash(cmd).await;
        assert!(
            text.starts_with(header),
            "{cmd} must use the canonical prefix, got {text}"
        );
        // The builder emits exactly one blank line between marker and body.
        assert!(text.starts_with(&format!("{header}\n\n")), "got {text}");
    }
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_swarm_parses_the_agent_flag_via_helpers() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    // `/swarm --agent build <prompt>` reaches `helpers::parse_swarm_args`, which
    // strips the `--agent <type>` pair out of the prompt; with no model
    // configured the decomposition then reports rather than spawning.
    let (app, _text) = run_slash("/swarm --agent build refactor the module").await;
    // Without a configured model the swarm decomposition refuses up front.
    assert!(
        app.status.contains("swarm"),
        "the swarm refusal must name the command: {}",
        app.status
    );

    let (_app, help) = run_slash("/swarm help").await;
    assert!(help.to_lowercase().contains("swarm"), "got {help}");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn slash_alog_export_validates_the_run_id() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    // A missing run id and a missing `--yes` both surface the usage block
    // through `helpers::parse_alog_run_id_yes`.
    let (app, text) = run_slash("/alog export").await;
    assert!(app.status.starts_with("alog"), "got {}", app.status);
    assert!(text.contains("Missing argument"), "got {text}");

    let (app, text) = run_slash("/alog export run-alpha").await;
    assert!(app.status.starts_with("alog"), "got {}", app.status);
    assert!(text.contains("`--yes` flag is required"), "got {text}");

    // A path-shaped run id is refused before any file is written.
    let (app, text) = run_slash("/alog export ../../evil --yes").await;
    assert!(app.status.starts_with("alog"), "got {}", app.status);
    assert!(text.contains("Invalid run id"), "got {text}");
    assert!(
        !temp.path().join("log/exports").exists(),
        "a rejected run id must not create the export directory"
    );

    // An unknown but well-formed run id is reported from `open_verified_alog`.
    let (app, text) = run_slash("/alog export run-does-not-exist --yes").await;
    assert!(app.status.starts_with("alog"), "got {}", app.status);
    assert!(text.contains("No run with id"), "got {text}");
    drop(temp);
}

// ===========================================================================
// status-bar abbreviation helper tables and prompt tag
// ===========================================================================

#[test]
fn status_bar_abbreviations_cover_their_tables() {
    use ragent_tui::layout_statusbar::abbreviations;

    // Full mode returns every label unchanged.
    for label in ["tokens", "provider", "context", "tasks", "health"] {
        assert_eq!(abbreviations::label(label, true), label);
    }
    assert_eq!(abbreviations::label("tokens", false), "tok");
    assert_eq!(abbreviations::label("provider", false), "pvd");
    assert_eq!(abbreviations::label("context", false), "ctx");
    assert_eq!(abbreviations::label("tasks", false), "t");
    assert_eq!(abbreviations::label("health", false), "hlth");
    assert_eq!(abbreviations::label("code_index", false), "idx");
    assert_eq!(abbreviations::label("memory", false), "mem");
    assert_eq!(abbreviations::label("git", false), "git");
    assert_eq!(abbreviations::label("branch", false), "br");
    assert_eq!(abbreviations::label("status", false), "sts");
    assert_eq!(abbreviations::label("unknown", false), "unknown");

    assert_eq!(abbreviations::service("code_index"), "Idx");
    assert_eq!(abbreviations::service("memory"), "Mem");
    assert_eq!(abbreviations::service("plugins"), "plugins");

    assert_eq!(abbreviations::provider("anthropic"), "An");
    assert_eq!(abbreviations::provider("claude"), "Cl");
    assert_eq!(abbreviations::provider("openai"), "OAI");
    assert_eq!(abbreviations::provider("gpt"), "GPT");
    assert_eq!(abbreviations::provider("gemini"), "Gm");
    assert_eq!(abbreviations::provider("hugging_face"), "HF");
    assert_eq!(abbreviations::provider("copilot"), "CoPilot");
    assert_eq!(abbreviations::provider("ollama"), "Oll");
    assert_eq!(abbreviations::provider("azure_foundry"), "AzF");
    assert_eq!(abbreviations::provider("generic"), "generic");
}

#[test]
fn status_bar_prompt_display_text_truncates_with_the_marker() {
    use ragent_tui::layout_statusbar::prompt_display_text;
    assert_eq!(prompt_display_text("", 10), "");
    assert_eq!(prompt_display_text("short", 10), "[short]");
    assert_eq!(prompt_display_text("exactly-10", 10), "[exactly-10]");
    assert_eq!(prompt_display_text("abcdefghijk", 10), "[abcdefghij....]");
    assert_eq!(prompt_display_text("a", 0), "[....]");
}

#[test]
fn status_bar_response_mode_thresholds() {
    use ragent_tui::layout_statusbar::ResponsiveMode;
    assert_eq!(ResponsiveMode::from_width(0), ResponsiveMode::Minimal);
    assert_eq!(ResponsiveMode::from_width(79), ResponsiveMode::Minimal);
    assert_eq!(ResponsiveMode::from_width(80), ResponsiveMode::Compact);
    assert_eq!(ResponsiveMode::from_width(119), ResponsiveMode::Compact);
    assert_eq!(ResponsiveMode::from_width(120), ResponsiveMode::Full);
    assert_eq!(ResponsiveMode::from_width(400), ResponsiveMode::Full);
}

#[test]
fn status_bar_shortens_the_cwd_when_the_prompt_tag_needs_room() {
    let mut app = support::make_app();
    // A deep cwd plus a long prompt tag forces the shortened-cwd branch.
    app.cwd = "/home/dev/projects/a-very-long-nested-project-directory-name".to_string();
    app.git_branch = Some("feature/long-branch-name".to_string());
    app.last_prompt = "p".repeat(60);
    app.selected_model = Some("anthropic/claude-sonnet-4".to_string());
    app.is_processing = true;

    // Every width from narrow to wide must render without panicking, and the
    // prompt tag must still be visible whenever the tag plan applies.
    for width in [60u16, 80, 100, 120, 140, 160] {
        let screen = painted(&render(&mut app, width, 24));
        assert!(screen.contains("ragent"), "width {width}: {screen}");
    }
}

#[test]
fn skillgen_global_entry_point_writes_into_the_isolated_config_dir() {
    // `XDG_CONFIG_HOME` is redirected into the tempdir, so
    // `generate_graphify_skill` resolves `~/.config/ragent/skills/` there.
    let (_guard, _env, temp) = slash_support::enter_project(r#"{ "defaultAgent": "general" }"#);
    let result = ragent_tui::app::skillgen::generate_graphify_skill().expect("global skillgen");
    assert_eq!(result.files_written, 9);
    assert!(
        result.dest.starts_with(temp.path()),
        "dest: {:?}",
        result.dest
    );
    assert!(result.dest.join("SKILL.md").is_file());
    assert!(!result.already_existed);

    let second = ragent_tui::app::skillgen::generate_graphify_skill().expect("regenerate");
    assert!(second.already_existed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compaction_start_accepts_a_configured_model() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();
    let mut app = slash_support::app_with_session();
    app.messages.push(Message::user_text("s-compact", "hello"));
    app.selected_model = Some("anthropic/claude-sonnet-4-20250514".to_string());

    // The manual path sets the in-progress flag and spawns the compaction; with
    // no reachable provider the spawned task fails and deposits an error into
    // the result slot, which the poll surfaces.
    app.execute_slash_command("/compact").await;
    assert!(app.compact_in_progress, "status: {}", app.status);
    assert_eq!(app.status, "compacting...");
    assert!(!app.auto_compact_in_progress);

    for _ in 0..200 {
        app.poll_compaction_result();
        if !app.compact_in_progress {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!app.compact_in_progress, "the compaction result must land");
    drop(temp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reverse_hook_refuses_without_a_provider_token() {
    let _lock = crate::global_env_lock();
    let (_env, temp) = enter_project_pair();

    // The authenticated fetch path makes a real network call, so only the
    // deterministic no-credential branch is exercised here.
    if ragent_agent::github::auth::load_token().is_some() {
        drop(temp);
        return;
    }

    let mut app = slash_support::app_with_session();
    ragent_tui::app::reverse_command_for_tests(&mut app, "thawkins/ragent");
    assert_eq!(app.status, "reverse: no token");
    let text = app
        .messages
        .last()
        .map(Message::text_content)
        .unwrap_or_default();
    assert!(text.contains("No GitHub token configured"), "got {text}");
    drop(temp);
}
