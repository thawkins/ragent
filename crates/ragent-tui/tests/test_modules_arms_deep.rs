//! Deep arm coverage for the cron scheduler and the `/loop` and `/bench`
//! surfaces of `ragent-tui`.
//!
//! Drives the reachable branches of the cron fire path (stateful prompt
//! injection, the completion monitor, a zero-duration advance), the `/loop`
//! setup dialog's field accessors, flag parser, checkpoint toggles and start
//! guards, and the `/bench` run guards, staleness watchdog and renderers.
//!
//! Tests only - no production logic is touched. Every write goes to the
//! harness tempdir (cwd + `XDG_CONFIG_HOME`) or `target/temp`.
//!
//! Env mutation is `unsafe` in edition 2024 and the workspace denies
//! `unsafe_code`, so this target opts back in for the isolation harness; that
//! harness holds a process-global mutex across the async body, so
//! `clippy::await_holding_lock` is also allowed for this file.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_tui::App;
use ragent_tui::app::{LoopSetupField, LoopSetupState};
use ragent_types::CronEvent;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

// ===========================================================================
// Helpers
// ===========================================================================

/// A key event with no modifiers.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The whole message window joined into one string.
fn all_text(app: &App) -> String {
    slash_support::all_messages(app)
}

// ===========================================================================
// app::cron - fire path, stateful prompt, completion monitor, advance
// ===========================================================================

/// A repeating event for a known agent fires through the real `AgentManager`
/// spawn path (success branch), which also starts the completion monitor; a
/// stateful event loads (and fails to load) its cross-run loop state; a
/// zero-duration repeating event cannot advance and is disabled.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cron_tick_fires_repeating_events_and_runs_the_completion_monitor() {
    let (_guard, _env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();

    // Install a real manager so `spawn_agent_run` succeeds instead of erroring
    // with "AgentManager not initialized".
    let manager = Arc::new(ragent_agent::task::AgentManager::new(
        app.event_bus.clone(),
        Arc::clone(&app.session_processor),
        8,
        120,
    ));
    assert!(
        app.session_processor
            .agent_manager
            .set(Arc::clone(&manager))
            .is_ok(),
        "the manager slot must be empty on a fresh app"
    );

    let storage = Arc::clone(&app.storage);
    let processor = Arc::clone(&app.session_processor);
    let past = Utc::now() - chrono::Duration::hours(2);

    // 1. Plain repeating event: spawns, advances, and enters the running set.
    storage
        .insert_cron_event(&CronEvent::new(
            "repeat-plain".to_string(),
            "general".to_string(),
            "keep going".to_string(),
            ragent_types::CronSchedule::repeat_from(past, 3600),
            "every 1h".to_string(),
            past,
        ))
        .expect("insert repeating");

    // 2. Stateful repeating event: the state directory is a *directory*, so
    //    `LoopState::load` fails and the handler falls back to a fresh state.
    let mut stateful = CronEvent::new(
        "repeat-stateful".to_string(),
        "general".to_string(),
        "stateful prompt".to_string(),
        ragent_types::CronSchedule::repeat_from(past, 3600),
        "every 1h".to_string(),
        past,
    );
    stateful.stateful = true;
    storage
        .insert_cron_event(&stateful)
        .expect("insert stateful");
    std::fs::create_dir_all(temp.path().join("loop-state/repeat-stateful.txt"))
        .expect("create unreadable state path");

    // 3. Zero-duration repeating event: `advance_next_due` returns `None`, so
    //    the event is disabled rather than left due.
    storage
        .insert_cron_event(&CronEvent::new(
            "repeat-zero".to_string(),
            "general".to_string(),
            "no interval".to_string(),
            ragent_types::CronSchedule::repeat_now(0),
            "every 0s".to_string(),
            past,
        ))
        .expect("insert zero-duration");

    let handle = ragent_tui::app::cron::start_cron_scheduler(
        Arc::clone(&storage),
        Arc::clone(&processor),
        temp.path().to_path_buf(),
    );

    // Wait for the first tick to settle every event.
    let mut settled = false;
    for _ in 0..400 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let plain = storage
            .get_cron_event("repeat-plain")
            .ok()
            .flatten()
            .map(|row| row.next_due);
        let zero = storage.get_cron_event("repeat-zero").ok().flatten();
        if let (Some(next_due), Some(zero)) = (plain, zero) {
            let advanced = chrono::DateTime::parse_from_rfc3339(&next_due)
                .map(|dt| dt.with_timezone(&Utc) > Utc::now())
                .unwrap_or(false);
            if advanced && !zero.enabled {
                settled = true;
                break;
            }
        }
    }
    handle.stop();
    assert!(
        settled,
        "the repeating events must be fired and advanced/disabled"
    );
    // `stop` is idempotent and also runs on drop.
    handle.stop();

    // Every fired event is logged, the success path included.
    let log_dir = temp.path().join("log");
    let log_body: String = std::fs::read_dir(&log_dir)
        .map(|dir| {
            dir.flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("cron-"))
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    assert!(log_body.contains("repeat-plain"), "log: {log_body}");
    assert!(log_body.contains("repeat-stateful"), "log: {log_body}");

    // The monitor polls every 5 s; give it one full cycle so its
    // completion/removal path runs inside this test.
    tokio::time::sleep(Duration::from_millis(6000)).await;
    let snapshot = manager.tasks_snapshot().await;
    assert!(
        snapshot
            .iter()
            .any(|entry| entry.status != ragent_agent::task::TaskStatus::Running),
        "the spawned cron runs must reach a terminal state: {snapshot:?}"
    );
}

// ===========================================================================
// app::loop_dialog - field accessors, navigation, limits, start guards
// ===========================================================================

#[test]
fn loop_dialog_field_accessors_and_navigation_cover_every_field() {
    let mut state = LoopSetupState::new(
        vec![
            ("general".to_string(), "general".to_string()),
            ("explore".to_string(), "explore".to_string()),
        ],
        0,
    );

    // Every text field is reachable through both accessors; the two
    // non-text fields return `None`.
    for field in [
        LoopSetupField::Goal,
        LoopSetupField::VerifyCmd,
        LoopSetupField::Scope,
        LoopSetupField::ReadOnly,
        LoopSetupField::ToolSet,
        LoopSetupField::MaxSteps,
        LoopSetupField::CostLimit,
    ] {
        assert!(state.field_ref(field).is_some(), "{field:?} must be text");
        assert!(
            state.field_mut(field).is_some(),
            "{field:?} must be mutable text"
        );
    }
    for field in [LoopSetupField::Agent, LoopSetupField::Checkpoints] {
        assert!(state.field_ref(field).is_none(), "{field:?} is not text");
        assert!(
            state.field_mut(field).is_none(),
            "{field:?} is not mutable text"
        );
    }

    // `next`/`previous` are inverses over the whole ring.
    let mut field = LoopSetupField::Agent;
    for _ in 0..9 {
        field = field.next();
    }
    assert_eq!(field, LoopSetupField::Agent);
    let mut field = LoopSetupField::Agent;
    for _ in 0..9 {
        field = field.previous();
    }
    assert_eq!(field, LoopSetupField::Agent);
}

#[test]
fn loop_dialog_parse_loop_flags_reports_u32_overflow_and_equals_forms() {
    let overflow_steps: Vec<String> = ["--max-steps=4294967296"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let err = ragent_tui::app::parse_loop_flags(&overflow_steps).expect_err("overflow");
    assert!(err.contains("exceeds u32 range"), "got {err}");

    let overflow_timeout: Vec<String> = ["--timeout", "4294967296"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let err = ragent_tui::app::parse_loop_flags(&overflow_timeout).expect_err("overflow");
    assert!(err.contains("exceeds u32 seconds"), "got {err}");

    // A missing value for a `=form`-less flag names the flag.
    let missing_timeout: Vec<String> = ["--timeout"].iter().map(|s| (*s).to_string()).collect();
    let err = ragent_tui::app::parse_loop_flags(&missing_timeout).expect_err("missing value");
    assert!(err.contains("missing value for --timeout"), "got {err}");
}

#[test]
fn loop_dialog_checkpoints_toggle_and_cost_limit_round_trip() {
    let (_guard, _env, _temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = app_with_session();
    ragent_tui::app::open_loop_setup(&mut app);

    // Walk the dialog onto the checkpoints switch and toggle it with both the
    // Up/Down keys and Space.
    for _ in 0..8 {
        ragent_tui::app::handle_loop_setup_key(&mut app, key(KeyCode::Tab));
    }
    let state = app.loop_setup.as_ref().expect("dialog open");
    assert_eq!(state.active_field, LoopSetupField::Checkpoints);

    let before = app.loop_setup.as_ref().expect("open").checkpoints;
    ragent_tui::app::handle_loop_setup_key(&mut app, key(KeyCode::Char(' ')));
    assert_ne!(
        app.loop_setup.as_ref().expect("open").checkpoints,
        before,
        "space toggles the checkpoint switch"
    );
    ragent_tui::app::handle_loop_setup_key(&mut app, key(KeyCode::Up));
    assert_eq!(
        app.loop_setup.as_ref().expect("open").checkpoints,
        before,
        "up toggles the checkpoint switch back"
    );
    ragent_tui::app::handle_loop_setup_key(&mut app, key(KeyCode::Down));
    assert_ne!(app.loop_setup.as_ref().expect("open").checkpoints, before);

    // A cost limit entered in the dialog lands on the spec.
    let defaults = ragent_config::LoopConfig::default();
    let mut state = LoopSetupState::new(vec![("general".to_string(), "g".to_string())], 0);
    state.goal_field = ragent_tui::input_field::InputField::with_text("ship it");
    state.cost_limit_field = ragent_tui::input_field::InputField::with_text("2500");
    state.max_steps_field = ragent_tui::input_field::InputField::with_text("");
    let spec = ragent_tui::app::build_spec_from_state(&state, &defaults).expect("valid");
    assert_eq!(spec.cost_limit, Some(2500));
    assert_eq!(spec.max_steps, Some(defaults.max_steps));
}

#[test]
fn loop_dialog_start_guards_report_the_missing_configuration() {
    let (_guard, _env, _temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = app_with_session();
    app.configured_provider = None;
    app.loop_setup = Some(LoopSetupState::new(Vec::new(), 0));

    // No provider: the dialog refuses and stays open.
    let mut state = LoopSetupState::new(vec![("general".to_string(), "g".to_string())], 0);
    state.goal_field = ragent_tui::input_field::InputField::with_text("ship it");
    app.loop_setup = Some(state);
    ragent_tui::app::handle_loop_setup_key(&mut app, key(KeyCode::Enter));
    assert_eq!(
        app.status,
        "loop: no provider configured - use /provider first"
    );

    // A provider but no model: the next guard reports it.
    app.configured_provider = Some(ragent_tui::app::ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic".to_string(),
        source: ragent_tui::app::ProviderSource::Database,
    });
    app.selected_model = None;
    app.agent_info.model = None;
    let mut state = LoopSetupState::new(vec![("general".to_string(), "g".to_string())], 0);
    state.goal_field = ragent_tui::input_field::InputField::with_text("ship it");
    app.loop_setup = Some(state);
    ragent_tui::app::handle_loop_setup_key(&mut app, key(KeyCode::Enter));
    assert_eq!(app.status, "loop: no model selected - use /model first");
}

// ===========================================================================
// app::bench - staleness watchdog, renderers, run guards
// ===========================================================================

#[test]
fn bench_watchdog_detaches_a_stale_run() {
    let mut app = support::make_app();
    // `append_assistant_text` only records a message when a session is set.
    app.session_id = Some("bench-watchdog".to_string());
    app.active_bench_task_id = Some("bench-stale".to_string());
    app.active_bench_started_at = Some(Utc::now() - chrono::Duration::minutes(45));
    app.active_bench_cancel = Some(Arc::new(AtomicBool::new(false)));

    app.poll_pending_bench();

    assert_eq!(app.status, "bench: timed out");
    assert!(app.active_bench_task_id.is_none(), "the run is detached");
    assert!(app.active_bench_started_at.is_none());
    let text = all_text(&app);
    assert!(
        text.contains("did not finish within the watchdog window"),
        "the watchdog message is appended: {text}"
    );
}

#[test]
fn bench_show_status_and_open_last_render_the_recorded_state() {
    let mut app = support::make_app();
    app.selected_model = Some("anthropic/claude-sonnet-4".to_string());

    // `show` with no workbooks yet.
    let show = app_execute(&mut app, "/bench show");
    assert!(show.contains("(none)"), "show: {show}");
    assert!(show.contains("anthropic/claude-sonnet-4"), "show: {show}");

    // `status` with an active run, a progress snapshot, and cancellation.
    app.active_bench_task_id = Some("bench-1".to_string());
    app.active_bench_summary = Some("`humaneval` on `anthropic/m`".to_string());
    app.active_bench_started_at = Some(Utc::now());
    let cancel = Arc::new(AtomicBool::new(true));
    app.active_bench_cancel = Some(cancel);
    let progress = ragent_bench::BenchProgressHandle::default();
    progress.set(ragent_bench::BenchRunProgress {
        suite_id: "humaneval".to_string(),
        suite_index: 1,
        total_suites: 2,
        completed_cases: 3,
        total_cases: 9,
    });
    app.active_bench_progress = Some(progress);
    let status = app_execute(&mut app, "/bench status");
    assert!(status.contains("Active Benchmark Run"), "status: {status}");
    assert!(status.contains("Cancellation"), "status: {status}");
    assert!(status.contains("3/9"), "status: {status}");

    // `open last` with recorded workbooks renders each path plus the summary.
    app.active_bench_task_id = None;
    app.bench_last_workbooks = vec![
        std::path::PathBuf::from("target/temp/bench/a.xlsx"),
        std::path::PathBuf::from("target/temp/bench/b.xlsx"),
    ];
    app.bench_last_summary = Some("## Benchmark Run\n\n- done".to_string());
    let opened = app_execute(&mut app, "/bench open last");
    assert!(opened.contains("a.xlsx"), "open: {opened}");
    assert!(opened.contains("b.xlsx"), "open: {opened}");
    assert!(opened.contains("## Benchmark Run"), "open: {opened}");

    // `show` now lists both workbooks.
    let show = app_execute(&mut app, "/bench show");
    assert!(show.contains("a.xlsx"), "show: {show}");
}

#[test]
fn bench_run_guards_refuse_before_spawning() {
    let (_guard, _env, _temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let mut app = app_with_session();
    app.selected_model = Some("anthropic/claude-sonnet-4".to_string());

    // Guard 1: a run is already active.
    app.active_bench_task_id = Some("bench-active".to_string());
    let _ = app_execute(&mut app, "/bench run humaneval");
    assert_eq!(app.status, "[warn] A benchmark run is already active.");

    // Guard 2: no model configured.
    app.active_bench_task_id = None;
    app.selected_model = None;
    let _ = app_execute(&mut app, "/bench run humaneval");
    assert_eq!(
        app.status,
        "[warn] /bench run requires a configured model - use /model"
    );

    // Guard 3: an unusable model selection.
    app.selected_model = Some("no-such-provider-no-such-model".to_string());
    let _ = app_execute(&mut app, "/bench run humaneval");
    assert!(
        app.status.starts_with("[warn] Invalid model selection:"),
        "status: {}",
        app.status
    );
    assert!(app.active_bench_task_id.is_none());
    assert_eq!(app.active_tasks.len(), 0, "no bench task is registered");
}

/// Run a slash command on a scratch runtime and return the appended text.
fn app_execute(app: &mut App, command: &str) -> String {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    rt.block_on(async { app.execute_slash_command(command).await });
    last_message(app)
}
