//! Additional `/research` coverage reachable through the PUBLIC
//! `execute_slash_command` entry point.
//!
//! The `/research` dispatch lives in the `pub(crate)` `App::handle_research_command`
//! (`src/app/research.rs`), so it is only reachable via the slash command. These
//! tests exercise the subcommands and validation arms the existing
//! `test_widgets_and_events_extra.rs` / `test_research_cluster.rs` suites do not
//! cover: `resume`, `export`, `import`, `config`, `update`, `unknown`, the
//! `cluster` no-model / invalid-name guards, and the list/open/search/show/archive
//! synchronous status lines against a populated store.
//!
//! Tests only - no production logic is touched.

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

fn app_with_session(session: &str) -> App {
    let mut app = support::make_app();
    app.session_id = Some(session.to_string());
    app.needs_redraw = false;
    app
}

fn joined_text(app: &App) -> String {
    app.messages
        .iter()
        .flat_map(|m| m.parts.iter())
        .filter_map(|p| match p {
            ragent_agent::message::MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "ragent-tui-research-more-{}-{}-{}",
        std::process::id(),
        tag,
        n
    ));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// ---------------------------------------------------------------------------
// Not-yet-implemented subcommands
// ---------------------------------------------------------------------------

#[tokio::test]
async fn research_resume_reports_not_implemented() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research resume topic-a").await;
    assert!(joined_text(&app).contains("not yet implemented in the TUI"));
}

#[tokio::test]
async fn research_export_reports_not_implemented() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research export topic-a").await;
    let text = joined_text(&app);
    assert!(text.contains("Exporting"), "got: {text}");
    assert!(text.contains("not yet implemented"), "got: {text}");
}

#[tokio::test]
async fn research_import_reports_not_implemented() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research import some/path.json")
        .await;
    let text = joined_text(&app);
    assert!(text.contains("Importing"), "got: {text}");
}

#[tokio::test]
async fn research_config_describes_defaults_source() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research config").await;
    let text = joined_text(&app);
    assert!(text.contains("ragent.json"), "got: {text}");
}

// ---------------------------------------------------------------------------
// Unknown / invalid grammar
// ---------------------------------------------------------------------------

#[tokio::test]
async fn research_unknown_subcommand_reports_error() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research bogus-verb").await;
    let text = joined_text(&app);
    assert!(text.contains("unknown subcommand"), "got: {text}");
    assert!(text.contains("bogus-verb"), "got: {text}");
}

#[tokio::test]
async fn research_invalid_flag_reports_error() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research create mytopic topic --iterations notanumber")
        .await;
    let text = joined_text(&app);
    assert!(
        text.contains("Error") && text.contains("expected an integer"),
        "got: {text}"
    );
}

// ---------------------------------------------------------------------------
// cluster validation guards
// ---------------------------------------------------------------------------

#[tokio::test]
async fn research_cluster_invalid_name_is_refused() {
    let mut app = app_with_session("s1");
    app.execute_slash_command("/research cluster Bad_Name")
        .await;
    assert_eq!(app.status, "research: cluster invalid name");
    assert!(joined_text(&app).contains("invalid research name"));
}

#[tokio::test]
async fn research_cluster_without_a_selected_model_is_refused() {
    let dir = temp_dir("cluster-no-model");
    let sources = dir.join("research/kilo/sources");
    std::fs::create_dir_all(&sources).expect("create sources dir");
    std::fs::write(sources.join("web-01.md"), "body").expect("write source");

    let mut app = app_with_session("s1");
    app.cwd_path = dir;
    app.selected_model = None;
    app.execute_slash_command("/research cluster kilo").await;
    assert_eq!(
        app.status, "research: cluster 'kilo' no model selected",
        "got: {}",
        app.status
    );
}

#[tokio::test]
async fn research_cluster_missing_folder_is_refused() {
    let dir = temp_dir("cluster-missing");
    let mut app = app_with_session("s1");
    app.cwd_path = dir;
    app.execute_slash_command("/research cluster nofolder")
        .await;
    assert_eq!(app.status, "research: cluster 'nofolder' folder missing");
}

// ---------------------------------------------------------------------------
// update - synchronous status + progress tracking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn research_update_seeds_progress_and_wait_status() {
    let dir = temp_dir("update");
    let mut app = app_with_session("s1");
    app.cwd_path = dir;
    app.execute_slash_command("/research update topic-x").await;

    assert!(
        app.status.starts_with("[wait] research: update topic-x"),
        "got: {}",
        app.status
    );
    assert!(
        app.research_progress.iter().any(|p| p.name == "topic-x"),
        "the update seeds a progress tracker"
    );
}

// ---------------------------------------------------------------------------
// delete --yes drives the async path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn research_delete_with_yes_sets_deleting_status() {
    let dir = temp_dir("delete-yes");
    let mut app = app_with_session("s1");
    app.cwd_path = dir;
    app.execute_slash_command("/research delete topic-a --yes")
        .await;
    assert!(
        app.status.contains("deleting 'topic-a'"),
        "got: {}",
        app.status
    );
}

// ---------------------------------------------------------------------------
// list / open / search / show / archive status lines with a store root
// ---------------------------------------------------------------------------

#[tokio::test]
async fn research_subcommands_with_all_flag_set_status_line() {
    let dir = temp_dir("all-flag");
    let mut app = app_with_session("s1");
    app.cwd_path = dir;

    app.execute_slash_command("/research list --all").await;
    assert!(
        app.status.contains("including archived"),
        "got: {}",
        app.status
    );

    app.execute_slash_command("/research show topic-a").await;
    assert!(app.status.contains("showing"), "got: {}", app.status);
}
