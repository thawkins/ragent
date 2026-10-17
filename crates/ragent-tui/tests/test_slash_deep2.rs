//! Deep slash-command coverage: data-populated branches the existing
//! `/slash` suites never reach.
//!
//! The focused per-command suites (`test_slash_commands.rs`,
//! `test_slash_config_write.rs`, `test_slash_session.rs`, ...) exercise the
//! empty / no-data paths of `/cron`, `/alog`, `/editlog`, `/triggers`,
//! `/bug-report`, `/doctor`, `/github`, `/gitlab`, `/telemetry` and `/i18n`.
//! This target seeds real state (cron rows, an activity-log run, edit-log
//! JSONL, trigger rules) into an isolated tempdir project and drives the
//! populated render branches through the public
//! `App::execute_slash_command` entry point.
//!
//! Every test runs inside its own tempdir with an isolated `XDG_CONFIG_HOME`
//! (via [`slash_support::enter_project`]) so no command can write to the
//! developer's real `~/.config/ragent/`. Env mutation is `unsafe` in edition
//! 2024 and the workspace denies `unsafe_code`, so this target opts back in
//! explicitly; the isolation harness holds a process-global mutex across the
//! async body, so `clippy::await_holding_lock` is also allowed for this file.
#![allow(unsafe_code)]
#![cfg(test)]
#![allow(clippy::await_holding_lock)]

use ragent_agent::trigger::TriggerRuntime;
use ragent_storage::activity_log::ActivityLog;
use ragent_tui::App;
use ragent_types::trigger::TriggerRule;
use ragent_types::{CronEvent, RunId, parse_schedule};

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// A syntactically valid OpenAI-style secret that `redact_secrets` must strip
/// from the bug-report output (>=20 chars after `sk-`).
const REDACT_ME: &str = "sk-abcdefghijklmnopqrstuvwxyz0123456789";

/// An app in a fresh isolated project with a live session.
fn isolated_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();
    (guard, env, temp, app)
}

/// Write `lines` as a single JSONL file at `rel` under `temp` (dirs created).
fn write_jsonl(temp: &tempfile::TempDir, rel: &str, lines: &[&str]) {
    let path = temp.path().join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create jsonl parent");
    }
    let mut body = String::new();
    for line in lines {
        body.push_str(line);
        body.push('\n');
    }
    std::fs::write(&path, body).expect("write jsonl");
}

/// An in-memory cron event with the given id / enabled flag.
fn cron_event(id: &str, enabled: bool) -> CronEvent {
    let parsed = parse_schedule("every 30m", chrono::Utc::now()).expect("valid schedule");
    let mut event = CronEvent::new(
        id.to_string(),
        "general".to_string(),
        "Run the test suite".to_string(),
        parsed.schedule,
        "every 30m".to_string(),
        parsed.next_due,
    );
    event.enabled = enabled;
    event
}

// ---------------------------------------------------------------------------
// /bug-report
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bug_report_writes_redacted_report_file() {
    let (_g, _e, temp, mut app) = isolated_app();
    app.token_usage = (120, 34);
    app.log_entries.push(ragent_tui::app::LogEntry {
        timestamp: chrono::Utc::now(),
        level: ragent_tui::app::LogLevel::Info,
        message: format!("token {REDACT_ME} should be redacted"),
        session_id: None,
        agent_id: None,
        seq: 0,
    });

    app.execute_slash_command("/bug-report").await;

    assert_eq!(app.status, "bug-report: generated");
    let msg = last_message(&app);
    assert!(msg.contains("Bug report generated"), "msg: {msg}");

    let log_dir = temp.path().join("log");
    let found = std::fs::read_dir(&log_dir)
        .expect("log dir exists")
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("bug-report-"))
        .expect("bug-report file written");
    let body = std::fs::read_to_string(found.path()).expect("read report");
    assert!(body.contains("# RAgent Bug Report"), "body: {body}");
    assert!(body.contains("Input tokens"), "body: {body}");
    assert!(
        !body.contains(REDACT_ME),
        "secrets must be redacted: {body}"
    );
}

#[tokio::test]
async fn bug_report_with_unknown_session_reports_not_found() {
    let (_g, _e, temp, mut app) = isolated_app();
    // A non-empty session id `ensure_session` leaves untouched; storage has no
    // such row, so the report's Ok(None) branch renders.
    app.session_id = Some("ghost-session".to_string());

    app.execute_slash_command("/bug-report").await;

    assert_eq!(app.status, "bug-report: generated");
    let log_dir = temp.path().join("log");
    let found = std::fs::read_dir(&log_dir)
        .expect("log dir exists")
        .flatten()
        .find(|e| e.file_name().to_string_lossy().starts_with("bug-report-"))
        .expect("bug-report file written");
    let body = std::fs::read_to_string(found.path()).expect("read report");
    assert!(
        body.contains("Not found in storage"),
        "unknown-session branch: {body}"
    );
}

// ---------------------------------------------------------------------------
// /editlog show + /editlog analyse (populated)
// ---------------------------------------------------------------------------

const EDITLOG_SUCCESS: &str = r#"{"timestamp":"2025-01-01T00:00:00Z","tool":"edit","file_path":"src/a.rs","old_str":"plain","new_str":"x","outcome":"success","dry_run":false}"#;
const EDITLOG_TAB_FAILURE: &str = "{\"timestamp\":\"2025-01-01T00:00:01Z\",\"tool\":\"edit\",\"file_path\":\"src/b.rs\",\"old_str\":\"\\tindented line\",\"new_str\":\"x\",\"outcome\":\"no match found in src/b.rs\",\"dry_run\":false}";
const EDITLOG_BLANK_FAILURE: &str = "{\"timestamp\":\"2025-01-01T00:00:02Z\",\"tool\":\"multi_edit\",\"file_path\":\"src/c.rs\",\"old_str\":\"first\\n\\nsecond\",\"new_str\":\"y\",\"outcome\":\"whitespace mismatch\",\"dry_run\":false}";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn editlog_show_with_entries_renders_stats_table() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_jsonl(
        &temp,
        "log/editlog/edits-20250101.jsonl",
        &[EDITLOG_SUCCESS, EDITLOG_TAB_FAILURE, EDITLOG_BLANK_FAILURE],
    );

    app.execute_slash_command("/editlog show").await;

    assert_eq!(app.status, "editlog: shown");
    let msg = last_message(&app);
    assert!(msg.contains("Success %"), "stats table: {msg}");
    assert!(msg.contains("Failure reasons"), "failure reasons: {msg}");
    assert!(msg.contains("edit"), "tool column: {msg}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn editlog_show_empty_log_reports_empty() {
    let (_g, _e, temp, mut app) = isolated_app();
    // Directory exists but holds no entries.
    std::fs::create_dir_all(temp.path().join("log/editlog")).expect("mkdir editlog");

    app.execute_slash_command("/editlog show").await;

    assert_eq!(app.status, "editlog: empty");
    assert!(last_message(&app).contains("No edit-log entries"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn editlog_analyse_with_failures_renders_risk_tables() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_jsonl(
        &temp,
        "log/editlog/edits-20250102.jsonl",
        &[EDITLOG_SUCCESS, EDITLOG_TAB_FAILURE, EDITLOG_BLANK_FAILURE],
    );

    app.execute_slash_command("/editlog analyse").await;

    assert_eq!(app.status, "editlog: analysed");
    let msg = last_message(&app);
    assert!(msg.contains("Analysed 2 failed edit"), "summary: {msg}");
    assert!(
        msg.contains("Success vs failure by tool"),
        "per-tool table: {msg}"
    );
    assert!(msg.contains("Fail/Success ratio"), "ratio column: {msg}");
    assert!(msg.contains("Common combinations"), "combos: {msg}");
    assert!(msg.contains("Examples"), "examples: {msg}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn editlog_analyse_no_failures_reports_clean() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_jsonl(
        &temp,
        "log/editlog/edits-20250103.jsonl",
        &[EDITLOG_SUCCESS],
    );

    app.execute_slash_command("/editlog analyse").await;

    assert_eq!(app.status, "editlog: no failures");
    assert!(last_message(&app).contains("No failed edit-log entries"));
}

// ---------------------------------------------------------------------------
// /cron list + /cron log
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cron_list_with_events_renders_table() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.storage
        .insert_cron_event(&cron_event("nightly", true))
        .expect("insert event");
    app.storage
        .insert_cron_event(&cron_event("paused", false))
        .expect("insert event");

    app.execute_slash_command("/cron list").await;

    assert_eq!(app.status, "cron: list");
    let msg = last_message(&app);
    assert!(msg.contains("Scheduled Events"), "header: {msg}");
    assert!(msg.contains("nightly"), "id column: {msg}");
    assert!(msg.contains("paused"), "second row: {msg}");
}

#[tokio::test]
async fn cron_list_empty_reports_empty() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/cron list").await;
    assert_eq!(app.status, "cron: list empty");
    assert!(last_message(&app).contains("No scheduled events"));
}

#[tokio::test]
async fn cron_log_with_entries_renders_log_table() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_jsonl(
        &temp,
        "log/cron-20250101-000000.jsonl",
        &[
            r#"{"timestamp":"2025-01-01T00:00:00Z","event_id":"evt1","agent_type":"general","prompt":"run tests","schedule":"every 30m","outcome":"success","error":null,"run_id":null}"#,
            r#"{"timestamp":"2025-01-01T00:01:00Z","event_id":"evt2","agent_type":"general","prompt":"broken","schedule":"every 30m","outcome":"error","error":"boom","run_id":null}"#,
            r#"{"timestamp":"2025-01-01T00:02:00Z","event_id":"evt3","agent_type":"general","prompt":"busy","schedule":"every 30m","outcome":"skipped","error":null,"run_id":null}"#,
            r#"{"timestamp":"2025-01-01T00:03:00Z","event_id":"evt4","agent_type":"general","prompt":"odd","schedule":"every 30m","outcome":"weird","error":null,"run_id":null}"#,
        ],
    );

    app.execute_slash_command("/cron log").await;

    assert_eq!(app.status, "cron: log");
    let msg = last_message(&app);
    assert!(msg.contains("Execution Log"), "header: {msg}");
    assert!(msg.contains("evt1") && msg.contains("evt4"), "rows: {msg}");
}

#[tokio::test]
async fn cron_log_filtered_by_event_id() {
    let (_g, _e, temp, mut app) = isolated_app();
    write_jsonl(
        &temp,
        "log/cron-20250102-000000.jsonl",
        &[
            r#"{"timestamp":"2025-01-02T00:00:00Z","event_id":"keep","agent_type":"general","prompt":"a","schedule":"every 1h","outcome":"success","error":null,"run_id":null}"#,
            r#"{"timestamp":"2025-01-02T00:01:00Z","event_id":"drop","agent_type":"general","prompt":"b","schedule":"every 1h","outcome":"success","error":null,"run_id":null}"#,
        ],
    );

    app.execute_slash_command("/cron log keep").await;

    assert_eq!(app.status, "cron: log");
    let msg = last_message(&app);
    assert!(msg.contains("keep"), "kept event: {msg}");
    assert!(
        !msg.contains("drop"),
        "filtered event must be absent: {msg}"
    );
}

#[tokio::test]
async fn cron_log_empty_reports_empty() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/cron log").await;
    assert_eq!(app.status, "cron: log empty");
    assert!(last_message(&app).contains("No execution log entries"));
}

// ---------------------------------------------------------------------------
// /alog list / status / export / delete (populated activity log)
// ---------------------------------------------------------------------------

/// Path the `/alog` handlers resolve from the empty `db_path` of `make_app`.
fn alog_path() -> std::path::PathBuf {
    ActivityLog::default_path(std::path::Path::new(""))
}

/// Seed an activity-log run with two model-message events at the `/alog` path.
fn seed_alog_run(run: &str) {
    let log = ActivityLog::open(&alog_path()).expect("open activity log");
    let run_id = RunId::from(run);
    log.record_model_message(&run_id, "user", "hello", None)
        .expect("record event");
    log.record_model_message(&run_id, "assistant", "hi", Some("m1".into()))
        .expect("record event");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_list_with_run_renders_run_table() {
    let (_g, _e, _t, mut app) = isolated_app();
    seed_alog_run("run-alpha");

    app.execute_slash_command("/alog list").await;

    assert_eq!(app.status, "alog: list");
    let msg = last_message(&app);
    assert!(msg.contains("Activity Log - Runs"), "header: {msg}");
    assert!(msg.contains("Model"), "columns: {msg}");
    assert!(msg.contains("alpha"), "run row: {msg}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_list_empty_reports_empty() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog list").await;
    assert_eq!(app.status, "alog: list");
    assert!(last_message(&app).contains("empty (no runs)"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_status_with_run_renders_health_summary() {
    let (_g, _e, _t, mut app) = isolated_app();
    seed_alog_run("run-beta");

    app.execute_slash_command("/alog status").await;

    assert_eq!(app.status, "alog: status");
    let msg = last_message(&app);
    assert!(msg.contains("Activity Log - Status"), "header: {msg}");
    assert!(msg.contains("Health"), "health row: {msg}");
    assert!(msg.contains("Runs by status"), "status breakdown: {msg}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_export_with_yes_writes_jsonl() {
    let (_g, _e, temp, mut app) = isolated_app();
    seed_alog_run("run-export");

    app.execute_slash_command("/alog export run-export --yes")
        .await;

    assert_eq!(app.status, "alog: exported");
    let msg = last_message(&app);
    assert!(msg.contains("Exported run"), "confirmation: {msg}");
    let export = temp.path().join("log/exports/export-run-export.jsonl");
    assert!(export.exists(), "export file must exist at {export:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_delete_with_yes_removes_run() {
    let (_g, _e, _t, mut app) = isolated_app();
    seed_alog_run("run-del");

    app.execute_slash_command("/alog delete run-del --yes")
        .await;
    assert_eq!(app.status, "alog: deleted");
    assert!(last_message(&app).contains("Deleted run"));

    // The run is gone: a follow-up list reports the empty state.
    app.execute_slash_command("/alog list").await;
    assert!(last_message(&app).contains("empty (no runs)"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_delete_without_yes_is_refused() {
    let (_g, _e, _t, mut app) = isolated_app();
    seed_alog_run("run-guard");

    app.execute_slash_command("/alog delete run-guard").await;

    assert_eq!(app.status, "alog: error");
    assert!(last_message(&app).contains("--yes` flag is required"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_export_rejects_path_traversal_run_id() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog export ../../evil --yes")
        .await;
    assert_eq!(app.status, "alog: error");
    assert!(last_message(&app).contains("Invalid run id"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_delete_unknown_run_reports_not_found() {
    let (_g, _e, _t, mut app) = isolated_app();
    // Open the log so the file exists, but with no matching run.
    ActivityLog::open(&alog_path()).expect("open activity log");

    app.execute_slash_command("/alog delete ghost --yes").await;

    assert_eq!(app.status, "alog: error");
    assert!(last_message(&app).contains("No run with id"));
}

// ---------------------------------------------------------------------------
// /triggers list / enable / disable / status / remove (populated)
// ---------------------------------------------------------------------------

/// Install a runtime carrying one rule, returning the rule id.
fn install_rule(app: &mut App) -> String {
    let runtime = TriggerRuntime::default();
    let id = runtime.add_rule(TriggerRule::new("when build.done exists", "run cargo test"));
    app.trigger_runtime = Some(runtime);
    id.as_str().to_string()
}

#[tokio::test]
async fn triggers_list_with_rules_renders_table() {
    let (_g, _e, _t, mut app) = isolated_app();
    let id = install_rule(&mut app);

    app.execute_slash_command("/triggers list").await;

    assert_eq!(app.status, "triggers: list");
    let msg = last_message(&app);
    assert!(msg.contains("Trigger Rules"), "header: {msg}");
    assert!(msg.contains(&id[..id.len().min(8)]), "rule row: {msg}");
}

#[tokio::test]
async fn triggers_status_with_rules_reports_counts() {
    let (_g, _e, _t, mut app) = isolated_app();
    install_rule(&mut app);

    app.execute_slash_command("/triggers status").await;

    assert_eq!(app.status, "triggers: status");
    let msg = last_message(&app);
    assert!(msg.contains("Trigger Runtime Status"), "header: {msg}");
    assert!(msg.contains("Total rules"), "rule-count label: {msg}");
    assert!(msg.contains("| 1"), "rule count value: {msg}");
}

#[tokio::test]
async fn triggers_disable_enable_remove_cycle() {
    let (_g, _e, _t, mut app) = isolated_app();
    let id = install_rule(&mut app);

    app.execute_slash_command(&format!("/triggers disable {id}"))
        .await;
    assert_eq!(app.status, "triggers: disabled");
    assert!(last_message(&app).contains("disabled"));

    app.execute_slash_command(&format!("/triggers enable {id}"))
        .await;
    assert_eq!(app.status, "triggers: enabled");

    app.execute_slash_command(&format!("/triggers remove {id}"))
        .await;
    assert_eq!(app.status, "triggers: removed");

    app.execute_slash_command("/triggers list").await;
    assert_eq!(app.status, "triggers: list empty");
}

#[tokio::test]
async fn triggers_set_enabled_unknown_rule_reports_not_found() {
    let (_g, _e, _t, mut app) = isolated_app();
    install_rule(&mut app);

    app.execute_slash_command("/triggers disable no-such-rule")
        .await;
    assert_eq!(app.status, "triggers: not found");
}

// ---------------------------------------------------------------------------
// /doctor help
// ---------------------------------------------------------------------------

#[tokio::test]
async fn doctor_help_renders_usage_without_diagnostics() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/doctor help").await;
    assert_eq!(app.status, "doctor: help");
    assert!(last_message(&app).contains("/doctor"));
}

// ---------------------------------------------------------------------------
// /github logout  +  /gitlab setup / logout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn github_logout_removes_stored_token() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/github logout").await;
    assert!(last_message(&app).contains("GitHub token removed"));
}

#[tokio::test]
async fn gitlab_setup_opens_setup_dialog() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/gitlab setup").await;
    assert!(app.provider_setup.is_some(), "setup dialog should be open");
    assert!(matches!(
        app.provider_setup,
        Some(ragent_tui::app::ProviderSetupStep::GitLabSetup { .. })
    ));
}

#[tokio::test]
async fn gitlab_logout_clears_config_and_token() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/gitlab logout").await;
    assert!(last_message(&app).contains("removed"));
}

// ---------------------------------------------------------------------------
// /telemetry off  +  /i18n on  +  /alog on|off
// ---------------------------------------------------------------------------

#[tokio::test]
async fn telemetry_off_disables_and_persists() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/telemetry off").await;
    assert_eq!(app.status, "telemetry: disabled");
    assert!(last_message(&app).contains("Telemetry disabled"));
}

#[tokio::test]
async fn i18n_on_enables_translation() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/i18n on").await;
    assert_eq!(app.status, "i18n: on");
}

#[tokio::test]
async fn alog_on_off_persist_state() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog on").await;
    assert_eq!(app.status, "alog: on");

    app.execute_slash_command("/alog off").await;
    assert_eq!(app.status, "alog: off");
}

// ---------------------------------------------------------------------------
// /mode variants
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mode_all_role_variants_and_normal_alias() {
    let (_g, _e, _t, mut app) = isolated_app();
    for (cmd, needle) in [
        ("/mode coder", "coder"),
        ("/mode reviewer", "reviewer"),
        ("/mode debugger", "debugger"),
        ("/mode tester", "tester"),
    ] {
        app.execute_slash_command(cmd).await;
        assert!(
            app.status.contains(needle),
            "{cmd} status was {}",
            app.status
        );
        assert!(app.role_mode.is_some(), "{cmd} should set a role mode");
    }

    // `normal` is an alias for `off`.
    app.execute_slash_command("/mode normal").await;
    assert_eq!(app.status, "mode: normal");
    assert!(app.role_mode.is_none(), "normal must clear the role mode");
}
