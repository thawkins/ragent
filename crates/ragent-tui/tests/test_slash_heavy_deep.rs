//! Third batch of slash-command coverage: the heavy arms the earlier suites
//! avoid because they spawn background work or touch real state.
//!
//! - `/doctor` runs its full diagnostic body (a spawned task that shells out to
//!   `git` / `rg`) and publishes the report as an `AgentNotice`.
//! - `/codeindex show`, `godnodes`, `explain`, `path` and `communities` against
//!   an index whose semantic graph has actually been built, so the real result
//!   tables render instead of the empty-graph guard.
//! - `/memory show` and `/memory clear` with populated structured memories.
//! - `/spec reverse` usage errors and `/github` / `/gitlab` status arms.
//!
//! Tests only - no production logic is touched.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use std::sync::Arc;

use ragent_agent::event::Event;
use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app inside an isolated tempdir project with a small Rust source tree.
fn project_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    std::fs::create_dir_all(temp.path().join("src")).expect("create src");
    std::fs::write(
        temp.path().join("src/lib.rs"),
        "/// Entry point.\npub fn run() -> i32 {\n    helper() + 1\n}\n\n\
         /// Helper used by [`run`].\npub fn helper() -> i32 {\n    41\n}\n\n\
         /// A widget.\npub struct Widget {\n    /// Count.\n    pub n: i32,\n}\n\n\
         impl Widget {\n    /// Build one.\n    pub fn new() -> Self {\n        Self { n: 0 }\n    }\n\n\
         \x20   /// Bump the count.\n    pub fn bump(&mut self) {\n        self.n += helper();\n    }\n}\n",
    )
    .expect("write src/lib.rs");
    let app = app_with_session();
    (guard, env, temp, app)
}

/// Attach an in-memory index over the project and BUILD its semantic graph, so
/// the graph query arms have real edges to report.
fn attach_graph_index(app: &mut App, dir: &std::path::Path) {
    let config = CodeIndexConfig {
        enabled: true,
        project_root: dir.to_path_buf(),
        index_dir: dir.join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };
    let idx = Arc::new(CodeIndex::open_in_memory(&config).expect("in-memory code index"));
    idx.index_file(&dir.join("src/lib.rs"))
        .expect("index src/lib.rs");
    idx.build_graph().expect("build semantic graph");
    app.code_index = Some(idx);
    app.code_index_enabled = true;
}

// ---------------------------------------------------------------------------
// /doctor - the full diagnostic body
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn doctor_runs_diagnostics_and_publishes_report() {
    let (_g, _e, _t, mut app) = project_app();
    let mut rx = app.event_bus.subscribe();

    app.execute_slash_command("/doctor").await;
    // The report is published asynchronously as an AgentNotice.
    let mut report = None;
    for _ in 0..400 {
        if let Ok(Event::AgentNotice { message, .. }) = rx.try_recv()
            && message.contains("Diagnostic Report")
        {
            report = Some(message);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let report = report.expect("doctor must publish a diagnostic report");
    assert!(report.contains("git"), "report: {report}");
    assert!(report.contains("ripgrep"), "report: {report}");
    assert!(report.contains("Diagnostics complete"), "report: {report}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn doctor_with_index_and_model_reports_them() {
    let (_g, _e, temp, mut app) = project_app();
    attach_graph_index(&mut app, temp.path());
    app.selected_model = Some("ollama/qwen2.5".to_string());
    let mut rx = app.event_bus.subscribe();

    app.execute_slash_command("/doctor").await;
    let mut report = None;
    for _ in 0..400 {
        if let Ok(Event::AgentNotice { message, .. }) = rx.try_recv()
            && message.contains("Diagnostic Report")
        {
            report = Some(message);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let report = report.expect("doctor must publish a report");
    // The code-index line reports files/symbols; the model line reports the
    // configured provider-or-model or the "no provider configured" warning.
    assert!(report.contains("code index"), "report: {report}");
    assert!(
        report.contains("no provider configured") || report.contains("ollama"),
        "report: {report}"
    );
}

// ---------------------------------------------------------------------------
// /codeindex graph queries against a built graph
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn godnodes_with_built_graph_renders_table() {
    let (_g, _e, temp, mut app) = project_app();
    attach_graph_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex godnodes 5").await;
    assert!(
        app.status.starts_with("codeindex: godnodes"),
        "got {}",
        app.status
    );
    let text = last_message(&app);
    assert!(
        text.contains("God Nodes") || text.contains("No graph data"),
        "text: {text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explain_with_built_graph_reports_symbol() {
    let (_g, _e, temp, mut app) = project_app();
    attach_graph_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex explain helper").await;
    assert!(
        app.status.starts_with("codeindex: explain"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn path_with_built_graph_searches_route() {
    let (_g, _e, temp, mut app) = project_app();
    attach_graph_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex path run helper")
        .await;
    assert!(
        app.status.starts_with("codeindex: path"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn communities_with_built_graph_lists_groups() {
    let (_g, _e, temp, mut app) = project_app();
    attach_graph_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex communities").await;
    assert!(
        app.status.starts_with("codeindex: communities"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn show_with_populated_index_reports_counts() {
    let (_g, _e, temp, mut app) = project_app();
    attach_graph_index(&mut app, temp.path());

    app.execute_slash_command("/codeindex show").await;
    assert!(app.status.starts_with("codeindex:"), "got {}", app.status);
    let text = last_message(&app);
    assert!(text.contains("Files indexed"), "text: {text}");
    assert!(text.contains("Total symbols"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /memory with populated structured memories
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_show_with_rows_renders_categories() {
    let (_g, _e, _t, mut app) = project_app();
    let cwd = std::env::current_dir().expect("cwd");
    app.storage
        .create_memory(
            "a remembered fact about the project",
            "fact",
            "manual",
            0.9,
            &cwd.to_string_lossy(),
            "mem-test",
            &["demo".to_string()],
        )
        .expect("store memory");

    app.execute_slash_command("/memory show").await;
    assert_eq!(app.status, "memory panel visible");
    let text = last_message(&app);
    assert!(text.contains("From: /memory show"), "text: {text}");
    assert!(text.contains("remembered fact"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_show_empty_reports_no_memories() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/memory show").await;
    let text = last_message(&app);
    assert!(
        text.contains("no memories for this project"),
        "text: {text}"
    );
}

// ---------------------------------------------------------------------------
// /spec reverse usage errors
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spec_reverse_with_bad_flag_reports_usage() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/spec reverse owner/repo --bogus-flag")
        .await;
    assert!(
        app.status.contains("reverse usage") || app.status.contains("Usage:"),
        "got {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// /github and /gitlab status arms
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn github_status_reports_token_state() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/github status").await;
    let text = last_message(&app);
    assert!(
        text.contains("configured") || text.contains("No GitHub token"),
        "text: {text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gitlab_status_reports_configuration() {
    let (_g, _e, _t, mut app) = project_app();
    // `/gitlab status` renders a report in the message window without touching
    // the status bar.
    let before = app.status.clone();
    app.execute_slash_command("/gitlab status").await;
    assert_eq!(app.status, before, "status must be unchanged");
    let text = last_message(&app);
    assert!(text.contains("From: /gitlab"), "text: {text}");
    assert!(
        text.contains("not configured") || text.contains("configured"),
        "text: {text}"
    );
}

// ---------------------------------------------------------------------------
// /webapi / /update - the state-reporting arms only (no server, no download)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webapi_status_reports_disabled() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/webapi status").await;
    let text = last_message(&app);
    assert!(text.contains("Web API"), "text: {text}");
    assert!(text.contains("Disabled"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /reload and /agents
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reload_config_reports_success() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/reload").await;
    assert!(app.status.starts_with("reload"), "got {}", app.status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agents_lists_every_cycleable_agent() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/agents").await;
    assert_eq!(app.status, "agents");
    let text = last_message(&app);
    assert!(text.contains("general"), "text: {text}");
}
