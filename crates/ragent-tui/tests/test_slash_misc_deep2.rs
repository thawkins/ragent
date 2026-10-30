//! Second batch of miscellaneous slash-command dispatch coverage.
//!
//! Complements `test_slash_misc.rs` / `test_slash_misc_deep.rs` with the arms
//! that those files leave unvisited: `/alog config`, `/editlog` on/off paths,
//! `/osinfo`, `/telemetry`, `/gcf`, `/security`, `/i18n`, `/prompt`,
//! `/toolchain` unknown, `/skill`, `/skills`, `/blueprints`, `/template`,
//! `/goal`, `/actionloop`, `/mcp` connect/disconnect usage, `/tools`,
//! `/system`, `/cost`, `/clip`, `/name`, `/undo`, `/yolo`, `/compact`,
//! `/compress`, `/startup`, `/llmstats`, `/resume`, `/reload`
//! and `/help`.
//!
//! Tests only - no production logic is touched.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app in an isolated tempdir project with a live session.
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

// ---------------------------------------------------------------------------
// /alog
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_config_and_on_off_toggle() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/alog config").await;
    assert_eq!(app.status, "alog: config");
    assert!(last_message(&app).contains("From: /alog config"));

    app.execute_slash_command("/alog on").await;
    assert_eq!(app.status, "alog: on");

    app.execute_slash_command("/alog off").await;
    assert_eq!(app.status, "alog: off");

    app.execute_slash_command("/alog frobnicate").await;
    assert_eq!(app.status, "alog: usage");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_export_without_run_id_is_a_usage_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog export").await;
    assert_eq!(app.status, "alog: error");
    assert!(last_message(&app).contains("--yes"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_delete_without_run_id_is_a_usage_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog delete").await;
    assert_eq!(app.status, "alog: error");
}

// ---------------------------------------------------------------------------
// /editlog
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn editlog_on_off_and_clear() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/editlog on").await;
    assert_eq!(app.status, "editlog: enabled");

    app.execute_slash_command("/editlog off").await;
    assert_eq!(app.status, "editlog: disabled");

    app.execute_slash_command("/editlog clear").await;
    assert_eq!(app.status, "editlog: cleared");

    app.execute_slash_command("/editlog frobnicate").await;
    assert_eq!(app.status, "editlog: usage");
}

// ---------------------------------------------------------------------------
// /osinfo
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn osinfo_command_family() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/osinfo help").await;
    assert_eq!(app.status, "osinfo: help");
    assert!(last_message(&app).contains("From: /osinfo help"));

    // A bare `/osinfo` is the help page; only an unknown subcommand is usage.
    app.execute_slash_command("/osinfo").await;
    assert_eq!(app.status, "osinfo: help");

    app.execute_slash_command("/osinfo frobnicate").await;
    assert_eq!(app.status, "osinfo: usage");
}

// ---------------------------------------------------------------------------
// /telemetry
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn telemetry_family_without_configuration() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/telemetry help").await;
    assert_eq!(app.status, "telemetry: help");

    app.execute_slash_command("/telemetry counters").await;
    assert_eq!(app.status, "telemetry: counters");

    app.execute_slash_command("/telemetry frobnicate").await;
    assert_eq!(app.status, "telemetry: usage");
}

// ---------------------------------------------------------------------------
// /gcf, /security, /i18n
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gcf_command_family() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/gcf help").await;
    assert_eq!(app.status, "gcf: help");

    app.execute_slash_command("/gcf show").await;
    assert_eq!(app.status, "gcf: show");

    app.execute_slash_command("/gcf frobnicate").await;
    assert_eq!(app.status, "gcf: usage");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn security_command_family() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/security help").await;
    assert_eq!(app.status, "security: help");

    app.execute_slash_command("/security status").await;
    assert_eq!(app.status, "security: status");

    app.execute_slash_command("/security frobnicate").await;
    assert_eq!(app.status, "security: usage");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn i18n_command_family() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/i18n help").await;
    assert_eq!(app.status, "i18n: help");

    app.execute_slash_command("/i18n status").await;
    assert_eq!(app.status, "i18n: status");

    app.execute_slash_command("/i18n list").await;
    assert!(app.status.starts_with("i18n: "), "got {}", app.status);

    app.execute_slash_command("/i18n frobnicate").await;
    assert!(app.status.starts_with("i18n: "), "got {}", app.status);
}

// ---------------------------------------------------------------------------
// /prompt
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prompt_family_reads_only() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/prompt help").await;
    assert_eq!(app.status, "prompt: help");

    app.execute_slash_command("/prompt list").await;
    assert!(app.status.starts_with("prompt: list"), "got {}", app.status);

    // A prompt render is read-only and must not call the model.
    app.execute_slash_command("/prompt primary").await;
    assert!(
        app.status.starts_with("[wait] prompt") || app.status.starts_with("prompt:"),
        "got {}",
        app.status
    );

    app.execute_slash_command("/prompt frobnicate").await;
    assert!(
        app.status.contains("unknown subcommand"),
        "got {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// /toolchain, /skill, /skills, /blueprints
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn toolchain_unknown_subcommand() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/toolchain frobnicate").await;
    assert_eq!(app.status, "toolchain: usage");
    assert!(last_message(&app).contains("Unknown subcommand"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_family_lists_and_reports_unknown_names() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/skill").await;
    assert!(app.status.starts_with("skill"), "got {}", app.status);

    app.execute_slash_command("/skills").await;
    assert!(app.status.starts_with("skill"), "got {}", app.status);

    app.execute_slash_command("/skill no-such-skill-xyz").await;
    assert!(
        app.status.contains("not found") || app.status.contains("unknown"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blueprints_lists_with_no_installed_blueprints() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/blueprints").await;
    assert!(app.status.starts_with("blueprints"), "got {}", app.status);
    assert!(last_message(&app).contains("From: /blueprints"));
}

// ---------------------------------------------------------------------------
// /template, /goal, /actionloop
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn template_family() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/template help").await;
    assert_eq!(app.status, "template: help");

    app.execute_slash_command("/template").await;
    assert_eq!(app.status, "template: listed");
    assert!(last_message(&app).contains("Available Templates"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn goal_family() {
    let (_g, _e, _t, mut app) = isolated_app();

    // `/goal help` is not a subcommand: the unknown arm names it.
    app.execute_slash_command("/goal help").await;
    assert_eq!(app.status, "goal: unknown 'help'");

    app.execute_slash_command("/goal show").await;
    assert_eq!(app.status, "goal: none");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actionloop_report_without_samples() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/actionloop").await;
    // No samples recorded, so the report is optional and may be empty; the
    // command must still answer with a status.
    assert!(
        app.status.starts_with("actionloop") || app.status == "ready",
        "got {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// /mcp connect / disconnect usage
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_connect_without_id_is_a_usage_error() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/mcp connect").await;
    assert_eq!(app.status, "Usage: /mcp connect <id>");

    app.execute_slash_command("/mcp disconnect").await;
    assert_eq!(app.status, "Usage: /mcp disconnect <id>");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_discover_and_list_render() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/mcp discover").await;
    assert!(app.mcp_discover.is_some(), "discover must open the dialog");

    app.mcp_discover = None;
    app.execute_slash_command("/mcp").await;
    assert_eq!(app.status, "mcp");
    assert!(last_message(&app).contains("Subcommands: /mcp discover"));
}

// ---------------------------------------------------------------------------
// /system, /tools, /cost, /clip, /name, /undo, /yolo
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn system_prompt_append_and_show() {
    let (_g, _e, _t, mut app) = isolated_app();

    // A bare `/system` shows the current prompt, or reports none set.
    app.execute_slash_command("/system").await;
    assert!(
        app.status == "No system prompt set" || last_message(&app).contains("From: /system"),
        "got {}",
        app.status
    );

    app.execute_slash_command("/system Be terse.").await;
    assert_eq!(app.status, "system prompt updated");
    assert!(
        app.agent_info
            .prompt
            .as_deref()
            .is_some_and(|p| p.contains("Be terse.")),
        "the prompt override must be applied"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tools_list_and_toggle_unknown() {
    let (_g, _e, _t, mut app) = isolated_app();

    // A bare `/tools` is the help page.
    app.execute_slash_command("/tools").await;
    assert_eq!(app.status, "tools help");

    app.execute_slash_command("/tools list").await;
    assert_eq!(app.status, "tools");
    assert!(last_message(&app).contains("Tool"));

    // An unknown switch is a reported error, not a silent no-op.
    app.execute_slash_command("/tools frobnicate").await;
    assert_eq!(app.status, "tools error");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cost_summary_without_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/cost").await;
    assert!(app.status.starts_with("cost"), "got {}", app.status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clip_and_name_and_undo() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/clip").await;
    assert_eq!(app.status, "clip: nothing to copy");

    app.execute_slash_command("/name my-session").await;
    assert_eq!(app.status, "Session name set to 'my-session'");

    // No user message in the window yet, so undo reports the miss.
    app.execute_slash_command("/undo").await;
    assert_eq!(app.status, "[warn] No user message found to undo");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn yolo_toggle() {
    let (_g, _e, _t, mut app) = isolated_app();
    // `/yolo on` sets the state rather than flipping it: repeating it must not
    // turn YOLO back off (the documented "Enable" contract).
    app.execute_slash_command("/yolo on").await;
    assert!(
        app.status.contains("YOLO mode ENABLED"),
        "got {}",
        app.status
    );
    app.execute_slash_command("/yolo on").await;
    assert!(
        app.status.contains("YOLO mode ENABLED"),
        "repeating `/yolo on` must keep YOLO on, got {}",
        app.status
    );
    // `/yolo off` likewise sets, and repeating it stays off.
    app.execute_slash_command("/yolo off").await;
    assert!(
        app.status.contains("YOLO mode disabled"),
        "got {}",
        app.status
    );
    app.execute_slash_command("/yolo off").await;
    assert!(
        app.status.contains("YOLO mode disabled"),
        "repeating `/yolo off` must keep YOLO off, got {}",
        app.status
    );
    // `/yolo status` reports without changing the state.
    app.execute_slash_command("/yolo on").await;
    app.execute_slash_command("/yolo status").await;
    assert_eq!(app.status, "YOLO mode ENABLED [warn]");
    assert!(last_message(&app).contains("YOLO mode: **enabled**"));
    // A bare `/yolo` still toggles.
    app.execute_slash_command("/yolo").await;
    assert!(
        app.status.contains("YOLO mode disabled"),
        "got {}",
        app.status
    );
    // An unknown subcommand is refused without touching the state.
    app.execute_slash_command("/yolo frobnicate").await;
    assert_eq!(app.status, "yolo: usage");
    assert!(last_message(&app).contains("Unknown subcommand"));
}

// ---------------------------------------------------------------------------
// /compact, /compress, /startup, /llmstats
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compact_and_compress_without_history() {
    let (_g, _e, _t, mut app) = isolated_app();

    // With no messages there is nothing to compact: both commands report it.
    app.execute_slash_command("/compact").await;
    assert!(
        app.status.contains("No messages to compact") || app.status.contains("No active session"),
        "got {}",
        app.status
    );

    app.execute_slash_command("/compress").await;
    assert!(
        app.status.contains("No messages to compact") || app.status.contains("No active session"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_and_llmstats() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/startup").await;
    assert!(last_message(&app).contains("From: /startup"));

    app.execute_slash_command("/llmstats").await;
    assert!(last_message(&app).contains("From: /llmstats"));
}

// ---------------------------------------------------------------------------
// /resume, /reload, /help
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_without_id_opens_the_picker_or_reports_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/resume").await;
    assert!(
        app.history_picker.is_some() || app.status.contains("Nothing to resume"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reload_unknown_subcommand_is_reported() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/reload frobnicate").await;
    assert!(app.status.starts_with("reload"), "got {}", app.status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn help_renders_the_command_reference() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/help").await;
    assert_eq!(app.status, "help");
    let text = last_message(&app);
    assert!(text.contains("/help"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn help_lists_plugin_commands_when_present() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.plugin_commands
        .push(ragent_agent::plugin::PluginCommand {
            plugin_id: "demo".to_string(),
            name: "demo-command".to_string(),
            trigger: "demo-command".to_string(),
            description: String::new(),
            prompt: Some("do the thing".to_string()),
        });

    app.execute_slash_command("/help").await;
    let text = last_message(&app);
    assert!(text.contains("Plugin commands:"), "text: {text}");
    assert!(text.contains("/demo-command"), "text: {text}");
    assert!(text.contains("(plugin demo)"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /quit / /exit
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quit_sets_running_false() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/quit").await;
    assert!(!app.is_running, "quit must stop the run loop");
}
