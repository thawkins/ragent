//! Regression tests for `/tools <switch> on|off`.

use std::sync::{Mutex, OnceLock};

#[path = "support/mod.rs"]
mod support;

struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn enter_temp_config_dir() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    temp
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_toggle_persists_and_updates_hidden_registry() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    app.execute_slash_command("/tools codeindex off").await;

    assert!(!app.tool_visibility.codeindex);
    assert_eq!(app.status, "tools: codeindex off");
    assert!(
        app.messages
            .last()
            .expect("message")
            .text_content()
            .contains("`codeindex` visibility is now **off**")
    );
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    let cfg = ragent_agent::Config::load().expect("load saved config");
    assert!(!cfg.tool_visibility.codeindex);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_codeindex_off_updates_visibility_and_config() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.code_index_enabled = true;
    app.tool_visibility.codeindex = true;

    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    app.execute_slash_command("/codeindex off").await;

    assert!(!app.code_index_enabled);
    assert!(!app.tool_visibility.codeindex);
    assert_eq!(app.status, "codeindex: off");
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    let cfg = ragent_agent::Config::load().expect("load saved config");
    assert!(!cfg.code_index.enabled);
    assert!(!cfg.tool_visibility.codeindex);
}

/// `/tools show` renders a `source` column that names where each tool comes
/// from: `visibility:<switch>` for family-governed tools and `internal` for
/// everything else.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_show_lists_source_column() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools show").await;

    let text = app
        .messages
        .last()
        .expect("message")
        .text_content()
        .to_string();

    // Header carries the new column.
    assert!(text.contains("name"), "header missing name column");
    assert!(
        text.contains("source"),
        "header missing source column: {text}"
    );

    // A family-governed tool is attributed to its visibility switch in the
    // source column (columns 57..81), after the 56-character name column.
    let row = |name: &str| {
        text.lines()
            .find(|l| l.starts_with(&format!("{name} ")))
            .and_then(|l| l.get(57..81))
            .map(str::trim)
            .map(str::to_string)
    };
    assert_eq!(
        row("codeindex_search").as_deref(),
        Some("visibility:codeindex"),
        "codeindex_search not attributed: {text}"
    );

    // A core tool is attributed to internal.
    assert_eq!(
        row("read").as_deref(),
        Some("internal"),
        "read not attributed to internal: {text}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_table_rows_fit_message_width() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = support::make_app();
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools").await;

    let body = app
        .messages
        .last()
        .expect("message")
        .text_content()
        .to_owned();

    // Every row of the visible-tools table must fit inside the default
    // message-window inner width (120 - 2 border columns) so the table is
    // truncated by the window's right edge rather than wrapped by the
    // renderer.  Name and source occupy 56 + 24 columns, leaving at most 34
    // for a description that is pre-truncated, so no row can reach the border.
    let header = body
        .lines()
        .find(|l| l.contains("name") && l.contains("source"))
        .unwrap_or_else(|| panic!("visible-tools header row missing from: {body:?}"));
    assert!(
        header.chars().count() <= 116,
        "header of {} chars exceeds the message inner width: {header:?}",
        header.chars().count()
    );

    let rows: Vec<&str> = body
        .lines()
        .filter(|l| {
            l.starts_with("codeindex_search ")
                || l.starts_with("codeindex_dependencies ")
                || l.starts_with("gitlab_cancel_pipeline ")
        })
        .collect();
    assert_eq!(rows.len(), 3, "tool rows missing from: {body}");
    for row in rows {
        assert!(
            row.chars().count() <= 116,
            "row of {} chars exceeds the message inner width: {row:?}",
            row.chars().count()
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_slash_tools_lists_disabled_tools_separately() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_agent::ToolVisibilityConfig::default();

    app.execute_slash_command("/tools codeindex off").await;
    app.execute_slash_command("/tools").await;

    let body = app
        .messages
        .last()
        .expect("message")
        .text_content()
        .to_owned();

    // The disabled family is listed under its own heading, in its own section,
    // with the same three-column layout as the visible listing.
    assert!(
        body.contains("Disabled by visibility"),
        "missing disabled heading in: {body}"
    );
    // Red was removed: the separate section carries the meaning, so no row may
    // be wrapped in the TUI's `[red]…[/red]` span markers.
    assert!(
        !body.contains("[red]") && !body.contains("[/red]"),
        "disabled rows should no longer be wrapped in red: {body}"
    );

    // The disabled row must keep the shared column layout. Find the section and
    // check the `codeindex_search` row still carries its `visibility:codeindex`
    // source column, at the same column offset the visible rows use.
    let section = body
        .split_once("Disabled by visibility")
        .map(|(_, rest)| rest)
        .unwrap_or_else(|| panic!("missing disabled section in: {body}"));
    let row = section
        .lines()
        .find(|l| l.contains("codeindex_search"))
        .unwrap_or_else(|| panic!("codeindex_search missing from disabled section: {body}"));
    let name_at = row.find("codeindex_search").expect("name column");
    let source_at = row
        .find("visibility:codeindex")
        .unwrap_or_else(|| panic!("disabled row lost its source column: {row:?}"));
    assert_eq!(name_at, 0, "disabled row is not column-aligned: {row:?}");
    assert_eq!(
        source_at, 57,
        "disabled row source column is not aligned with the header: {row:?}"
    );
}
