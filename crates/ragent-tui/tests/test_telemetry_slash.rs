//! Tests for the `/telemetry` slash-command family.
//!
//! These tests exercise the non-interactive subcommands (`help`, `counters`,
//! and input validation) without launching a full terminal backend, plus the
//! `/telemetry setup` dialog's display order and its save-and-reconfigure
//! path.
//!
//! The save test enters a scratch project directory and redirects the config
//! there with the Rust 2024 `unsafe` `std::env` API; that is the sanctioned
//! test use of `unsafe` in this workspace (see `AGENTS.md`).

#![allow(unsafe_code)]
// The save test drives the async `handle_key_event` while holding the crate's
// `global_env_lock`, so the process-global cwd and env vars cannot change under
// it. `std::sync::MutexGuard` is not `Send`, but nextest runs every test in
// its own process and the future is polled on one thread, so the guard cannot
// cross a thread boundary. This mirrors the other env-mutating test files here.
#![allow(clippy::await_holding_lock)]

#[path = "support/mod.rs"]
mod support;

use ragent_config::OtelProtocol;
use ragent_tui::App;
use ragent_tui::app::ProviderSetupStep;

fn last_message_text(app: &App) -> String {
    app.messages
        .last()
        .map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    ragent_agent::message::MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()
        })
        .unwrap_or_default()
}

/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

#[test]
fn test_telemetry_help_is_added_to_slash_commands() {
    use ragent_tui::app::SLASH_COMMANDS;
    assert!(SLASH_COMMANDS.iter().any(|cmd| cmd.trigger == "telemetry"));
}

#[tokio::test]
async fn test_telemetry_help_appends_help_message() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry help").await;
    let last = last_message_text(&app);
    assert!(
        last.contains("/telemetry on"),
        "help should mention /telemetry on: {last}"
    );
    assert!(
        last.contains("/telemetry setup"),
        "help should mention /telemetry setup: {last}"
    );
    assert!(
        last.contains("/telemetry counters"),
        "help should mention /telemetry counters: {last}"
    );
}

#[tokio::test]
async fn test_telemetry_counters_appends_catalogue() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry counters").await;
    let last = last_message_text(&app);
    assert!(
        last.contains("Usage metrics"),
        "counters should list Usage metrics: {last}"
    );
    assert!(
        last.contains("Performance metrics"),
        "counters should list Performance metrics: {last}"
    );
    assert!(
        last.contains("Cost metrics"),
        "counters should list Cost metrics: {last}"
    );
    assert!(
        last.contains("Effectiveness metrics"),
        "counters should list Effectiveness metrics: {last}"
    );
    assert!(
        last.contains("ragent.llm.requests"),
        "counters should include ragent.llm.requests: {last}"
    );
    assert!(
        last.contains("*Counter*"),
        "counters should include Counter type label: {last}"
    );
    assert!(
        last.contains("*UpDownCounter*"),
        "counters should include UpDownCounter type label: {last}"
    );
    assert!(
        last.contains("*Gauge*"),
        "counters should include Gauge type label: {last}"
    );
    assert!(
        last.contains("*Histogram*"),
        "counters should include Histogram type label: {last}"
    );
    assert!(
        last.contains("Traces"),
        "counters should mention Traces section: {last}"
    );
}

#[tokio::test]
async fn test_telemetry_setup_opens_dialog_with_defaults() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry setup").await;
    let step = app
        .provider_setup
        .as_ref()
        .expect("setup should open provider_setup dialog");
    match step {
        ProviderSetupStep::TelemetrySetup {
            endpoint_field,
            protocol,
            interval_field,
            timeout_field,
            port_field,
            active_field,
            error,
        } => {
            assert_eq!(endpoint_field.text(), "http://localhost:4318");
            assert_eq!(*protocol, OtelProtocol::Http);
            assert_eq!(interval_field.text(), "30");
            assert_eq!(timeout_field.text(), "10");
            assert_eq!(port_field.text(), "");
            assert_eq!(*active_field, 0);
            assert!(error.is_none());
        }
        _ => panic!("expected TelemetrySetup dialog, got {step:?}"),
    }
}

#[tokio::test]
async fn test_telemetry_unknown_subcommand_shows_usage() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry frobnicate").await;
    let last = last_message_text(&app);
    assert!(
        last.contains("Usage: `/telemetry help|on|off|setup|counters`"),
        "unknown subcommand should show usage: {last}"
    );
}

#[tokio::test]
async fn test_telemetry_panel_slash_command_toggles_panel() {
    let mut app = make_app();
    assert!(!app.show_telemetry);
    app.execute_slash_command("/telemetry_panel").await;
    assert!(app.show_telemetry);
    assert!(!app.show_log);
    assert!(!app.show_profile);
    assert!(!app.show_tasks_panel);
    assert!(!app.show_memory);
    assert_eq!(app.status, "telemetry panel visible");

    app.execute_slash_command("/telemetry_panel").await;
    assert!(!app.show_telemetry);
    assert_eq!(app.status, "telemetry panel hidden");
}

#[test]
fn test_telemetry_panel_is_in_slash_commands() {
    use ragent_tui::app::SLASH_COMMANDS;
    assert!(
        SLASH_COMMANDS
            .iter()
            .any(|cmd| cmd.trigger == "telemetry_panel")
    );
}

/// Walk the rendered telemetry dialog and return, in screen order, the field
/// index whose row currently carries the `>` marker.
///
/// The dialog paints the active field with a leading `"> "` gutter and every
/// inactive field with `"  "`, one `label` line above each value line. This
/// helper maps the marker back to the field index so a test can assert that
/// display order matches the `Tab` order.
fn marked_field_order(app: &mut App) -> Vec<(String, u8)> {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let backend = TestBackend::new(90, 34);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");
    let buffer = terminal.backend().buffer().clone();

    // Field index by the label line that precedes its value line.
    let labels: [(&str, u8); 5] = [
        ("OTLP endpoint:", 0),
        ("Transport protocol:", 1),
        ("Export interval (s):", 2),
        ("Export timeout (s):", 3),
        ("Internal Prometheus port:", 4),
    ];

    let mut rows: Vec<String> = Vec::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        rows.push(line.trim_end().to_string());
    }

    let mut order = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        // The value row is painted as `<border> <gutter> <value>`, where the
        // gutter is `"> "` for the active field and `"  "` otherwise.
        if !row.contains("> ") {
            continue;
        }
        // The marker can only be on a value row; walk back to the nearest
        // label row above it to name the field.
        // The dialog is centred inside a bordered box, so a label row is
        // `... <border> <label> ... <border> ...`; match on containment.
        let label = rows[..index].iter().rev().find_map(|candidate| {
            labels
                .iter()
                .find(|(text, _)| candidate.contains(text))
                .map(|(text, idx)| ((*text).to_string(), *idx))
        });
        if let Some(entry) = label {
            order.push(entry);
        }
    }
    order
}

/// Bug fix regression: the telemetry dialog used to render the protocol row
/// last while `Tab` cycled it at index 1, so the highlight jumped forwards to
/// the bottom of the form and then backwards. Display order must now match
/// `Tab` order, i.e. consecutive presses move the marker exactly one row down.
#[tokio::test]
async fn test_telemetry_setup_display_order_matches_tab_order() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = make_app();
    app.execute_slash_command("/telemetry setup").await;

    let mut previous: Option<u8> = None;
    for expected in 0u8..5 {
        let order = marked_field_order(&mut app);
        let (label, index) = order
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("step {expected}: no marked row found, order={order:?}"));
        assert_eq!(
            index, expected,
            "field {expected} should be marked at step {expected}; got {label} ({index}) - \
             display order must follow the Tab order 0,1,2,3,4"
        );
        if let Some(prev) = previous {
            assert_eq!(
                index,
                prev + 1,
                "Tab must move the marker one row down, {prev} -> {index}"
            );
        }
        previous = Some(index);
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .await;
    }

    // One more Tab wraps back to the first field, still one row from "end".
    let order = marked_field_order(&mut app);
    assert_eq!(
        order.first().map(|(_, idx)| *idx),
        Some(0),
        "Tab from the last field must wrap to the first"
    );
}

/// Bug fix regression: `/telemetry setup` saved the new `otel` block and then
/// stopped, leaving an already-running subsystem exporting with the old
/// endpoint until the next restart. Saving must reconfigure in place, so the
/// persisted config and the live subsystem agree immediately.
#[tokio::test]
async fn test_telemetry_setup_save_reconfigures_live_subsystem() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    // Redirect the persisted config into the git-ignored scratch tree so this
    // test never writes into the operator's real `ragent.json`. The dialog
    // writes to the *project* config (`<cwd>/.ragent/ragent.json`) when one is
    // loaded, so the test seeds that file and runs from its directory.
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target/temp/test_telemetry_setup_save_reconfigures");
    let _ = std::fs::remove_dir_all(&scratch);
    let project_dir = scratch.join("project");
    std::fs::create_dir_all(project_dir.join(".ragent")).expect("create scratch dir");
    let config_path = project_dir.join(".ragent").join("ragent.json");
    std::fs::write(&config_path, "{}\n").expect("seed config");

    let guard = crate::global_env_lock();
    let previous_dir = std::env::current_dir().expect("read cwd");
    // `set_current_dir` and the env vars are process-global, so the crate-wide
    // lock is held for the whole test (see `AGENTS.md`).
    std::env::set_current_dir(&project_dir).expect("enter scratch project");
    // A blank `RAGENT_CONFIG_CONTENT` keeps the loader off any operator config
    // while still resolving the project file from the new working directory.
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::set_var("XDG_CONFIG_HOME", scratch.join("xdg"));
    }

    let mut app = make_app();
    app.execute_slash_command("/telemetry setup").await;

    // Point the OTLP endpoint somewhere unmistakable, then save.
    let endpoint = "http://127.0.0.1:4399";
    for _ in 0..endpoint.len() {
        app.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
            .await;
    }
    for ch in endpoint.chars() {
        app.handle_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
            .await;
    }
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;

    assert!(
        app.provider_setup.is_none(),
        "a successful save should close the dialog"
    );

    let persisted: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).expect("read saved config"))
            .expect("saved config is valid JSON");
    assert_eq!(
        persisted["telemetry"]["otel"]["endpoint"], endpoint,
        "the endpoint typed into the form must be persisted"
    );
    assert_eq!(
        persisted["telemetry"]["otel"]["enabled"], true,
        "saving the setup form enables telemetry"
    );

    // The live subsystem must reflect the saved block, not the pre-edit one.
    let live = app.session_processor.telemetry.config();
    assert_eq!(
        live.endpoint, endpoint,
        "saving the form must reconfigure the live subsystem, not just the file"
    );

    let last = last_message_text(&app);
    assert!(
        !last.contains("[err]"),
        "a clean reconfigure must not report an error: {last}"
    );

    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    std::env::set_current_dir(&previous_dir).expect("restore cwd");
    drop(guard);
}
