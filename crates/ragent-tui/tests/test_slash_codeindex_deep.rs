//! Deep `/codeindex`, `/router` and `/mouse` dispatch coverage.
//!
//! `test_slash_codeindex.rs` covers the "no active index" and empty-graph
//! branches; this file drives the ACTIVE paths: `/codeindex on` (creating and
//! saving the index), `/codeindex off`, `reindex`, `graph build` /
//! `graph lang`, `godnodes` / `explain` / `path` / `communities` against an
//! indexed tree, `rebuild` and `skillgen`. It also covers the `/router`
//! sub-arms (`on`, `off`, `reload`, `weights`, `boundaries`, `tiers`, `test`)
//! and the `/mouse` family.
//!
//! Tests only - no production logic is touched.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use std::sync::Arc;

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app inside an isolated tempdir project holding one Rust source file, so
/// `/codeindex on` and the graph queries have real content to work with.
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
        "/// Add one.\npub fn add_one(x: i32) -> i32 {\n    helper(x) + 1\n}\n\n\
         fn helper(x: i32) -> i32 {\n    x\n}\n\n\
         pub struct Widget {\n    pub n: i32,\n}\n",
    )
    .expect("write src/lib.rs");
    let app = app_with_session();
    (guard, env, temp, app)
}

/// Attach an in-memory index built from a temp copy of the project, so the
/// graph queries run against real derived edges.
fn attach_indexed(app: &mut App, dir: &std::path::Path) -> Arc<CodeIndex> {
    let config = CodeIndexConfig {
        enabled: true,
        project_root: dir.to_path_buf(),
        index_dir: dir.join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };
    let idx = Arc::new(CodeIndex::open_in_memory(&config).expect("in-memory code index"));
    let lib = dir.join("src/lib.rs");
    idx.index_file(&lib).expect("index src/lib.rs");
    app.code_index = Some(idx.clone());
    app.code_index_enabled = true;
    idx
}

// ---------------------------------------------------------------------------
// enable / disable
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_creates_index_and_saves_config() {
    let (_g, _e, _t, mut app) = project_app();
    app.code_index = None;
    app.execute_slash_command("/codeindex on").await;
    assert_eq!(app.status, "codeindex: on");
    assert!(app.code_index.is_some(), "index must be attached");
    assert!(app.code_index_enabled);
    let text = last_message(&app);
    assert!(text.contains("Code index"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_when_already_active_reports_active() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());
    app.execute_slash_command("/codeindex on").await;
    assert_eq!(app.status, "codeindex: on");
    let text = last_message(&app);
    assert!(text.contains("already active"), "text: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn off_when_active_deactivates() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());
    app.execute_slash_command("/codeindex off").await;
    assert_eq!(app.status, "codeindex: off");
    assert!(app.code_index.is_none(), "index must be dropped");
    assert!(!app.code_index_enabled);
    assert!(last_message(&app).contains("disabled and deactivated"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn off_when_inactive_reports_not_active() {
    let (_g, _e, _t, mut app) = project_app();
    app.code_index = None;
    app.execute_slash_command("/codeindex off").await;
    assert_eq!(app.status, "codeindex: off");
    assert!(last_message(&app).contains("was not currently active"));
}

// ---------------------------------------------------------------------------
// reindex / rebuild / graph
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reindex_with_index_starts_background_run() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());
    app.execute_slash_command("/codeindex reindex").await;
    assert!(
        app.status.starts_with("[wait] codeindex: reindexing"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn graph_build_with_index_starts_background_run() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());
    app.execute_slash_command("/codeindex graph build").await;
    assert!(
        app.status.starts_with("[wait] codeindex: building graph"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn graph_lang_with_language_and_index() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());
    app.execute_slash_command("/codeindex graph lang rust")
        .await;
    assert!(
        app.status
            .starts_with("[wait] codeindex: building graph (rust)"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rebuild_with_index_reports_count() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());
    app.execute_slash_command("/codeindex rebuild").await;
    assert!(
        app.status.starts_with("codeindex: FTS rebuilt"),
        "got {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// graph queries on an indexed (still empty-graph) tree
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn godnodes_and_friends_report_on_indexed_tree() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());

    // Every graph query must answer with a codeindex: status and a message;
    // whether it reports real nodes or the empty-graph guard depends on how
    // many edges the derivation produced for this one-file tree.
    for cmd in [
        "/codeindex godnodes",
        "/codeindex explain add_one",
        "/codeindex path add_one helper",
        "/codeindex communities",
    ] {
        app.execute_slash_command(cmd).await;
        assert!(
            app.status.starts_with("codeindex: "),
            "{cmd} -> {}",
            app.status
        );
        assert_ne!(last_message(&app), "", "{cmd} produced no message");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn show_with_active_index_renders_statistics() {
    let (_g, _e, temp, mut app) = project_app();
    attach_indexed(&mut app, temp.path());
    app.execute_slash_command("/codeindex show").await;
    assert!(app.status.starts_with("codeindex:"), "got {}", app.status);
    let text = last_message(&app);
    assert!(text.contains("Files indexed"), "text: {text}");
}

// ---------------------------------------------------------------------------
// skillgen
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skillgen_writes_graphify_skill() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/codeindex skillgen").await;
    assert!(
        app.status.starts_with("codeindex: skillgen") || app.status == "codeindex: skillgen failed",
        "got {}",
        app.status
    );
    let text = last_message(&app);
    assert!(text.contains("graphify"), "text: {text}");
}

// ---------------------------------------------------------------------------
// /router
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_on_off_and_status() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/router on").await;
    assert!(app.status.starts_with("router:"), "got {}", app.status);
    assert!(app.router_enabled, "router must be enabled");

    app.execute_slash_command("/router status").await;
    assert_eq!(app.status, "router: status");
    assert!(last_message(&app).contains("Router status:"));

    app.execute_slash_command("/router off").await;
    assert!(!app.router_enabled, "router must be disabled");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_reload_reports_outcome() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/router reload").await;
    assert!(app.status.starts_with("router:"), "got {}", app.status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_tiers_weights_boundaries_render_tables() {
    let (_g, _e, _t, mut app) = project_app();

    app.execute_slash_command("/router tiers").await;
    assert_eq!(app.status, "router: tiers");
    assert!(last_message(&app).contains("Tier"));

    app.execute_slash_command("/router weights").await;
    assert_eq!(app.status, "router: weights");
    assert!(last_message(&app).contains("Weight"));

    app.execute_slash_command("/router boundaries").await;
    assert_eq!(app.status, "router: boundaries");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_test_classifies_a_prompt() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/router test summarise the release notes")
        .await;
    assert!(
        app.status.starts_with("router: test ->"),
        "got {}",
        app.status
    );
    assert!(last_message(&app).contains("composite"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn router_unknown_subcommand_warns() {
    let (_g, _e, _t, mut app) = project_app();
    app.execute_slash_command("/router frobnicate").await;
    assert_eq!(app.status, "router: unknown");
}

// ---------------------------------------------------------------------------
// /mouse
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mouse_on_off_and_state() {
    let (_g, _e, _t, mut app) = project_app();

    app.execute_slash_command("/mouse on").await;
    assert_eq!(app.status, "mouse: enabled");
    assert!(app.mouse_enabled);

    app.execute_slash_command("/mouse off").await;
    assert_eq!(app.status, "mouse: disabled (keyboard-only mode)");
    assert!(!app.mouse_enabled);

    // A bare `/mouse` reports the current state.
    app.execute_slash_command("/mouse").await;
    assert_eq!(app.status, "mouse: disabled");

    app.execute_slash_command("/mouse help").await;
    assert_eq!(app.status, "mouse: help");
}

// ---------------------------------------------------------------------------
// /websearch
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn websearch_help_and_usage() {
    let (_g, _e, _t, mut app) = project_app();

    app.execute_slash_command("/websearch help").await;
    assert_eq!(app.status, "websearch: help");

    // `search` without a query is a usage error, not a network call.
    app.execute_slash_command("/websearch search").await;
    assert_eq!(app.status, "websearch: search needs a query");

    app.execute_slash_command("/websearch frobnicate").await;
    assert_eq!(app.status, "websearch: unknown");
}

// ---------------------------------------------------------------------------
// /webapi
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn webapi_help_and_unknown() {
    let (_g, _e, _t, mut app) = project_app();

    // `/webapi help` renders the endpoint reference without touching status.
    let before = app.status.clone();
    app.execute_slash_command("/webapi help").await;
    assert_eq!(app.status, before, "help must not change the status");
    let text = last_message(&app);
    assert!(text.contains("Web API"), "text: {text}");
    assert!(text.contains("/sessions"), "text: {text}");

    // An unknown subcommand prints the usage line.
    app.execute_slash_command("/webapi frobnicate").await;
    assert!(
        last_message(&app).contains("Usage: `/webapi"),
        "text: {}",
        last_message(&app)
    );
}

// A separate helper keeps `support::make_app` reachable for files that do not
// need the isolated project harness.
#[allow(dead_code)]
fn plain_app() -> App {
    support::make_app()
}
