//! `/codeindex` TUI dispatch tests.
//!
//! Exercises every `/codeindex` sub-arm through the public
//! `App::execute_slash_command` entry point, both with no active index (the
//! `codeindex: not active` branches) and with an in-memory [`CodeIndex`]
//! attached (the empty-graph guard and query branches). No real project is
//! indexed, so no filesystem crawl runs.
#![cfg(test)]

use std::sync::Arc;

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// Build an app with a live session.
fn app() -> App {
    let mut app = support::make_app();
    app.session_id = Some("slash-codeindex".to_string());
    app
}

/// Attach a fresh in-memory code index (empty: no symbols, no graph) to the
/// app, mirroring how `/codeindex on` attaches a live index.
fn attach_empty_index(app: &mut App) -> Arc<CodeIndex> {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let config = CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir: dir.path().join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };
    let idx = Arc::new(CodeIndex::open_in_memory(&config).expect("in-memory code index"));
    app.code_index = Some(idx.clone());
    idx
}

/// Text of the most recent message in the message window.
fn last_message(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// help / unknown / languages
// ---------------------------------------------------------------------------

#[tokio::test]
async fn help_lists_all_subcommands() {
    let mut app = app();
    app.execute_slash_command("/codeindex help").await;
    assert_eq!(app.status, "codeindex: help");
    let text = last_message(&app);
    for needle in [
        "/codeindex on",
        "/codeindex show",
        "/codeindex lang",
        "/codeindex graph build",
        "/codeindex explain",
        "/codeindex path",
        "/codeindex communities",
        "/codeindex godnodes",
        "/codeindex skillgen",
    ] {
        assert!(text.contains(needle), "help missing {needle}: {text}");
    }
}

#[tokio::test]
async fn unknown_subcommand_prints_usage() {
    let mut app = app();
    app.execute_slash_command("/codeindex nonsense-xyz").await;
    assert!(last_message(&app).contains("Usage: `/codeindex"));
}

#[tokio::test]
async fn lang_lists_supported_languages() {
    let mut app = app();
    app.execute_slash_command("/codeindex lang").await;
    assert!(
        app.status.starts_with("codeindex:") && app.status.contains("languages"),
        "got {}",
        app.status
    );
    assert!(last_message(&app).contains("Supported Languages"));
}

// ---------------------------------------------------------------------------
// show / status (active and inactive)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn show_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.code_index_enabled = false;
    app.execute_slash_command("/codeindex show").await;
    assert_eq!(app.status, "codeindex: disabled");
    assert!(last_message(&app).contains("not currently active"));
}

#[tokio::test]
async fn status_alias_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.code_index_enabled = false;
    app.execute_slash_command("/codeindex status").await;
    assert_eq!(app.status, "codeindex: disabled");
}

#[tokio::test]
async fn bare_codeindex_shows_status() {
    let mut app = app();
    app.code_index = None;
    app.code_index_enabled = false;
    app.execute_slash_command("/codeindex").await;
    assert!(app.status.starts_with("codeindex:"));
}

#[tokio::test]
async fn show_with_active_empty_index_renders_status() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.code_index_enabled = true;
    app.execute_slash_command("/codeindex show").await;
    assert!(
        app.status.starts_with("codeindex:") && app.status.contains("files"),
        "got {}",
        app.status
    );
    let text = last_message(&app);
    assert!(text.contains("Code Index Status"), "status body: {text}");
    assert!(text.contains("Graph Dataset"), "graph section: {text}");
}

// ---------------------------------------------------------------------------
// graph query guards (empty graph)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn godnodes_empty_graph_guard() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex godnodes").await;
    assert_eq!(app.status, "codeindex: godnodes (empty graph)");
    assert!(last_message(&app).contains("/codeindex graph build"));
}

#[tokio::test]
async fn godnodes_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex godnodes").await;
    assert_eq!(app.status, "codeindex: not active");
}

#[tokio::test]
async fn explain_without_symbol_shows_usage() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex explain").await;
    assert_eq!(app.status, "codeindex: explain (usage)");
}

#[tokio::test]
async fn explain_empty_graph_guard() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex explain some_symbol")
        .await;
    assert_eq!(app.status, "codeindex: explain (empty graph)");
}

#[tokio::test]
async fn explain_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex explain some_symbol")
        .await;
    assert_eq!(app.status, "codeindex: not active");
}

#[tokio::test]
async fn path_without_two_symbols_shows_usage() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex path only_one").await;
    assert_eq!(app.status, "codeindex: path (usage)");
}

#[tokio::test]
async fn path_empty_graph_guard() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex path alpha beta")
        .await;
    assert_eq!(app.status, "codeindex: path (empty graph)");
}

#[tokio::test]
async fn path_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex path alpha beta")
        .await;
    assert_eq!(app.status, "codeindex: not active");
}

#[tokio::test]
async fn communities_empty_graph_guard() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex communities").await;
    assert_eq!(app.status, "codeindex: communities (empty graph)");
}

#[tokio::test]
async fn communities_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex communities").await;
    assert_eq!(app.status, "codeindex: not active");
}

// ---------------------------------------------------------------------------
// graph build / lang
// ---------------------------------------------------------------------------

#[tokio::test]
async fn graph_build_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex graph build").await;
    assert_eq!(app.status, "codeindex: not active");
}

#[tokio::test]
async fn graph_lang_without_language_shows_usage() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex graph lang").await;
    assert_eq!(app.status, "codeindex: graph lang (usage)");
}

#[tokio::test]
async fn graph_lang_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex graph lang rust")
        .await;
    assert_eq!(app.status, "codeindex: not active");
}

#[tokio::test]
async fn graph_unknown_subcommand_prints_usage() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex graph frobnicate")
        .await;
    assert!(last_message(&app).contains("/codeindex graph"));
}

// ---------------------------------------------------------------------------
// reindex / rebuild / skillgen
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reindex_without_index_reports_inactive() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex reindex").await;
    assert_eq!(app.status, "codeindex: not active");
}

#[tokio::test]
async fn rebuild_without_index_reports_message() {
    let mut app = app();
    app.code_index = None;
    app.execute_slash_command("/codeindex rebuild").await;
    assert!(last_message(&app).contains("not active"));
}

#[tokio::test]
async fn rebuild_with_empty_index_indexes_zero_documents() {
    let mut app = app();
    attach_empty_index(&mut app);
    app.execute_slash_command("/codeindex rebuild").await;
    assert!(
        app.status.starts_with("codeindex: FTS rebuilt"),
        "got {}",
        app.status
    );
}
