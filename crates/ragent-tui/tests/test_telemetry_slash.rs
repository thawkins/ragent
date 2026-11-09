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

/// Enter an empty scratch project with no global config, for as long as the
/// returned guard lives.
///
/// `/telemetry setup` prefills its fields from `Config::load()`, which reads the
/// operator's user-global `ragent.json`. Asserting the *defaults* therefore
/// asserted whatever the developer had configured - the test passed on a CI box
/// with no config and failed on a workstation whose `telemetry.otel.logs.level`
/// was `all`. Redirecting `XDG_CONFIG_HOME` into a scratch tree and the cwd into
/// an empty project makes the loaded config the built-in default.
///
/// Both mutations are process-global, so this returns the crate-wide
/// [`global_env_lock`](crate::global_env_lock) guard; the caller binds it for the
/// length of the test and must not take the lock again (it is not reentrant).
/// The cwd and env are restored on drop.
fn enter_isolated_project(tag: &str) -> (std::sync::MutexGuard<'static, ()>, EnvRestore) {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target/temp")
        .join(tag);
    let _ = std::fs::remove_dir_all(&scratch);
    let project = scratch.join("project");
    std::fs::create_dir_all(&project).expect("create scratch project");

    let guard = crate::global_env_lock();
    let restore = EnvRestore::new();
    std::env::set_current_dir(&project).expect("enter scratch project");
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::remove_var("RAGENT_CONFIG_CONTENT");
        std::env::set_var("XDG_CONFIG_HOME", scratch.join("xdg"));
    }
    (guard, restore)
}

/// Restores the process-global cwd and config env vars on drop.
struct EnvRestore {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl EnvRestore {
    fn new() -> Self {
        Self {
            keys: vec![
                ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
                ("RAGENT_CONFIG", std::env::var("RAGENT_CONFIG").ok()),
                (
                    "RAGENT_CONFIG_CONTENT",
                    std::env::var("RAGENT_CONFIG_CONTENT").ok(),
                ),
            ],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in &self.keys {
            match value {
                Some(v) => unsafe { std::env::set_var(key, v) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        std::env::set_current_dir(&self.cwd).ok();
    }
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
    // The dialog prefills from `Config::load()`, so the operator's real
    // `ragent.json` would decide what "the default" is. Pin an empty config.
    let (_guard, _restore) = enter_isolated_project("test_telemetry_setup_defaults");
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
            logs_field,
            active_field,
            error,
        } => {
            assert_eq!(endpoint_field.text(), "http://localhost:4318");
            assert_eq!(*protocol, OtelProtocol::Http);
            assert_eq!(interval_field.text(), "30");
            assert_eq!(timeout_field.text(), "10");
            assert_eq!(port_field.text(), "");
            // The logs field carries the export level (default `info`); log
            // export itself follows `/telemetry on|off`, so an empty field is
            // no longer how the signal is turned off.
            assert_eq!(logs_field.text(), "info");
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
        last.contains("Usage: `/telemetry help|on|off|setup|counters|logs"),
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
    let labels: [(&str, u8); 6] = [
        ("OTLP endpoint:", 0),
        ("Transport protocol:", 1),
        ("Export interval (s):", 2),
        ("Export timeout (s):", 3),
        ("Internal Prometheus port:", 4),
        // Spec `otel` FR-036: the logs-signal level is the last field.
        ("Logs level:", 5),
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
    for expected in 0u8..6 {
        let order = marked_field_order(&mut app);
        let (label, index) = order
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("step {expected}: no marked row found, order={order:?}"));
        assert_eq!(
            index, expected,
            "field {expected} should be marked at step {expected}; got {label} ({index}) - \
             display order must follow the Tab order 0,1,2,3,4,5"
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
    // test never writes into the operator's real `ragent.json`. Telemetry is
    // written to the *user-global* config (`$XDG_CONFIG_HOME/ragent/ragent.json`):
    // a project `.ragent/ragent.json` has its `telemetry` block stripped by the
    // repo-content guard, so it cannot hold this configuration.
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target/temp/test_telemetry_setup_save_reconfigures");
    let _ = std::fs::remove_dir_all(&scratch);
    let project_dir = scratch.join("project");
    std::fs::create_dir_all(&project_dir).expect("create scratch dir");
    let xdg = scratch.join("xdg");
    let config_path = xdg.join("ragent").join("ragent.json");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create global config dir");
    std::fs::write(&config_path, "{}\n").expect("seed config");

    let guard = crate::global_env_lock();
    let previous_dir = std::env::current_dir().expect("read cwd");
    // `set_current_dir` and the env vars are process-global, so the crate-wide
    // lock is held for the whole test (see `AGENTS.md`).
    std::env::set_current_dir(&project_dir).expect("enter scratch project");
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::set_var("XDG_CONFIG_HOME", &xdg);
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

/// `/telemetry on` is one switch for both signals: the block it persists must
/// carry `telemetry.otel.logs.enabled: true` beside `telemetry.otel.enabled`,
/// and `/telemetry off` must clear both. There is no separate logs on/off, so
/// `/telemetry logs on` and `/telemetry logs off` must not move the flag.
///
/// The destination is the user-global `ragent.json`: the repo-content guard in
/// `Config::merge_project` discards the whole `telemetry` block of a project
/// `.ragent/ragent.json` (SEC-ragent-config-001/002/003), so that file can
/// neither hold nor apply these settings.
///
/// The assertion is on the persisted block rather than on the live subsystem:
/// this binary is built without the `telemetry` feature, so `reconfigure()` on
/// an enabled config fails and reports telemetry as disabled by design.
#[tokio::test]
async fn test_telemetry_on_off_switches_metrics_and_logs_together() {
    let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target/temp/test_telemetry_on_off_both_signals");
    let _ = std::fs::remove_dir_all(&scratch);
    let project_dir = scratch.join("project");
    std::fs::create_dir_all(&project_dir).expect("create scratch dir");
    let xdg = scratch.join("xdg");
    let config_path = xdg.join("ragent").join("ragent.json");
    std::fs::create_dir_all(config_path.parent().expect("config parent"))
        .expect("create global config dir");
    std::fs::write(&config_path, "{}\n").expect("seed config");

    let guard = crate::global_env_lock();
    let previous_dir = std::env::current_dir().expect("read cwd");
    std::env::set_current_dir(&project_dir).expect("enter scratch project");
    unsafe {
        std::env::remove_var("RAGENT_CONFIG");
        std::env::set_var("XDG_CONFIG_HOME", &xdg);
    }

    let read = || -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(&config_path).expect("read config"))
            .expect("config is valid JSON")
    };

    let mut app = make_app();
    app.execute_slash_command("/telemetry on").await;
    let saved = read();
    assert_eq!(
        saved["telemetry"]["otel"]["enabled"], true,
        "`/telemetry on` must enable telemetry"
    );
    assert_eq!(
        saved["telemetry"]["otel"]["logs"]["enabled"], true,
        "`/telemetry on` must enable log export too - there is no separate switch"
    );
    assert!(
        !saved["telemetry"]["otel"]["export_interval_seconds"].is_null(),
        "the whole otel block must be written, not just the flags: {saved}"
    );

    // `logs off` is no longer a switch: it must explain that the level is all
    // this command changes, and name `/telemetry on|off` as the real switch.
    app.execute_slash_command("/telemetry logs off").await;
    let reply = last_message_text(&app);
    assert!(
        reply.contains("/telemetry on|off"),
        "`/telemetry logs off` must point at the real switch: {reply}"
    );

    // A level change persists the level.
    app.execute_slash_command("/telemetry logs warn").await;
    assert_eq!(read()["telemetry"]["otel"]["logs"]["level"], "warn");

    // `warning` is the config file's alias for `warn`; it is stored canonically.
    app.execute_slash_command("/telemetry logs warning").await;
    assert_eq!(read()["telemetry"]["otel"]["logs"]["level"], "warn");

    // `all` is an accepted level: it exports every severity the layer observes.
    app.execute_slash_command("/telemetry logs all").await;
    let reply = last_message_text(&app);
    assert!(
        reply.contains("set to `all`"),
        "`/telemetry logs all` must be accepted: {reply}"
    );
    assert_eq!(read()["telemetry"]["otel"]["logs"]["level"], "all");

    // Leave the configured level at `warn` so the retention assertions below
    // keep checking the earlier choice.
    app.execute_slash_command("/telemetry logs warn").await;
    assert_eq!(read()["telemetry"]["otel"]["logs"]["level"], "warn");

    // `on` again must keep the level the operator chose. It must also keep the
    // endpoint: a one-flag toggle must not reset the rest of the block.
    app.execute_slash_command("/telemetry on").await;
    let saved = read();
    assert_eq!(
        saved["telemetry"]["otel"]["logs"]["level"], "warn",
        "`/telemetry on` must keep the configured log level: {saved}"
    );
    assert_eq!(
        saved["telemetry"]["otel"]["endpoint"], "http://localhost:4318",
        "`/telemetry on` must keep the configured endpoint: {saved}"
    );

    // `/telemetry off` stops log export as well as the metrics.
    app.execute_slash_command("/telemetry off").await;
    let saved = read();
    assert_eq!(
        saved["telemetry"]["otel"]["enabled"], false,
        "`/telemetry off` must disable telemetry"
    );
    assert_eq!(
        saved["telemetry"]["otel"]["logs"]["enabled"], false,
        "`/telemetry off` must stop log export too"
    );

    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    std::env::set_current_dir(&previous_dir).expect("restore cwd");
    drop(guard);
}

/// Seed the isolated user-global `ragent.json` with `body` and return its path.
fn seed_global_config(tag: &str, body: &serde_json::Value) -> std::path::PathBuf {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp")
        .join(tag)
        .join("xdg/ragent/ragent.json");
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("create config dir");
    std::fs::write(&path, body.to_string()).expect("seed config");
    path
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read config"))
        .expect("config is valid JSON")
}

/// Regression: saving `/telemetry setup` built a fresh block and reset every
/// setting the dialog has no row for (service name, resource attributes,
/// per-metric toggles, cardinality limit) to its default.
#[tokio::test]
async fn test_telemetry_setup_save_preserves_unlisted_settings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let tag = "test_telemetry_setup_preserves_unlisted";
    let (_guard, _restore) = enter_isolated_project(tag);
    let path = seed_global_config(
        tag,
        &serde_json::json!({"telemetry": {"otel": {
            "service_name": "my-agent",
            "resource_attributes": {"team": "infra"},
            "metrics": {"ragent.llm.requests": false},
            "cardinality_limit": 42
        }}}),
    );

    let mut app = make_app();
    app.execute_slash_command("/telemetry setup").await;
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert!(app.provider_setup.is_none(), "save should close the dialog");

    let otel = &read_json(&path)["telemetry"]["otel"];
    assert_eq!(otel["service_name"], "my-agent", "{otel}");
    assert_eq!(otel["resource_attributes"]["team"], "infra", "{otel}");
    assert_eq!(otel["metrics"]["ragent.llm.requests"], false, "{otel}");
    assert_eq!(otel["cardinality_limit"], 42, "{otel}");
    assert_eq!(otel["enabled"], true, "saving the form enables telemetry");
}

/// Regression: `/telemetry on` printed `[ok] Telemetry enabled` even when the
/// stored block was invalid and nothing could export. It must refuse, say why,
/// and leave the file untouched.
#[tokio::test]
async fn test_telemetry_on_with_invalid_config_reports_error() {
    let tag = "test_telemetry_on_invalid_config";
    let (_guard, _restore) = enter_isolated_project(tag);
    let path = seed_global_config(
        tag,
        &serde_json::json!({"telemetry": {"otel": {"endpoint": "not a url"}}}),
    );

    let mut app = make_app();
    app.execute_slash_command("/telemetry on").await;
    let reply = last_message_text(&app);
    assert!(
        reply.contains("[err]"),
        "invalid config must be reported: {reply}"
    );
    assert!(!reply.contains("[ok]"), "must not claim success: {reply}");
    assert_eq!(app.status, "telemetry: enable failed");
    assert_ne!(
        read_json(&path)["telemetry"]["otel"]["enabled"],
        true,
        "an invalid block must not be persisted as enabled"
    );
}

/// Regression: the deprecated `experimental.open_telemetry` alias is applied
/// after the `telemetry` block on load, so leaving it in place re-enabled
/// telemetry on the next start after `/telemetry off`.
#[tokio::test]
async fn test_telemetry_off_clears_legacy_open_telemetry_flag() {
    let tag = "test_telemetry_off_clears_legacy_flag";
    let (_guard, _restore) = enter_isolated_project(tag);
    let path = seed_global_config(
        tag,
        &serde_json::json!({"experimental": {"open_telemetry": true}}),
    );

    let mut app = make_app();
    app.execute_slash_command("/telemetry off").await;
    let saved = read_json(&path);
    assert!(
        saved["experimental"]["open_telemetry"].is_null(),
        "the legacy alias must be removed: {saved}"
    );
    assert_eq!(saved["telemetry"]["otel"]["enabled"], false);

    let loaded = ragent_agent::Config::load().expect("load config");
    assert!(
        !loaded.telemetry.otel.enabled,
        "`/telemetry off` must survive a reload"
    );
}
