//! Tests for input history persistence (save / load / debounced flush).
//!
//! Covers `set_history_file`, `load_history`, `save_history`, and the
//! non-blocking `flush_history_if_due` debounce mechanism introduced in
//! Milestone 2 of the TUI compliance plan.

use std::sync::Arc;

use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
    )
}

// =========================================================================
// set_history_file
// =========================================================================

#[test]
fn test_set_history_file_stores_path() {
    let mut app = make_app();
    assert!(app.history_file_path.is_none());

    let path = std::path::PathBuf::from("/tmp/test_history.txt");
    app.set_history_file(path.clone());
    assert_eq!(app.history_file_path, Some(path));
}

// =========================================================================
// save_history — happy path
// =========================================================================

#[test]
fn test_save_history_writes_entries() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());
    app.input_history = vec!["hello".into(), "world".into()];

    app.save_history().unwrap();

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "hello\nworld");
}

#[test]
fn test_save_history_creates_parent_directories() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("a").join("b").join("c").join("history.txt");

    let mut app = make_app();
    app.set_history_file(nested.clone());
    app.input_history = vec!["deep".into()];

    app.save_history().unwrap();

    assert!(nested.exists());
    assert_eq!(std::fs::read_to_string(&nested).unwrap(), "deep");
}

#[test]
fn test_save_history_no_path_is_noop() {
    let app = make_app();
    // No history_file_path set — save should silently succeed.
    assert!(app.save_history().is_ok());
}

#[test]
fn test_save_history_empty_entries() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());
    // input_history is already empty by default

    app.save_history().unwrap();

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.is_empty());
}

#[test]
fn test_save_history_overwrites_previous() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());

    app.input_history = vec!["first".into()];
    app.save_history().unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "first");

    app.input_history = vec!["second".into(), "third".into()];
    app.save_history().unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "second\nthird");
}

// =========================================================================
// save_history — error branches
// =========================================================================

#[cfg(unix)]
#[test]
fn test_save_history_permission_denied() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let readonly_dir = dir.path().join("readonly");
    std::fs::create_dir(&readonly_dir).unwrap();
    std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o444)).unwrap();

    let file = readonly_dir.join("subdir").join("history.txt");

    let mut app = make_app();
    app.set_history_file(file);
    app.input_history = vec!["test".into()];

    let result = app.save_history();
    assert!(result.is_err());

    // Restore permissions so TempDir cleanup succeeds.
    std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
}

// =========================================================================
// load_history — happy path
// =========================================================================

#[test]
fn test_load_history_reads_entries() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");
    std::fs::write(&file, "alpha\nbeta\ngamma").unwrap();

    let mut app = make_app();
    app.set_history_file(file);

    app.load_history().unwrap();

    assert_eq!(app.input_history, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn test_load_history_filters_empty_lines() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");
    std::fs::write(&file, "one\n\n\ntwo\n\nthree\n").unwrap();

    let mut app = make_app();
    app.set_history_file(file);

    app.load_history().unwrap();

    assert_eq!(app.input_history, vec!["one", "two", "three"]);
}

#[test]
fn test_load_history_trims_to_100() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let entries: Vec<String> = (0..150).map(|i| format!("entry_{i}")).collect();
    std::fs::write(&file, entries.join("\n")).unwrap();

    let mut app = make_app();
    app.set_history_file(file);

    app.load_history().unwrap();

    assert_eq!(app.input_history.len(), 100);
    // Should keep the last 100 (entries 50..150).
    assert_eq!(app.input_history[0], "entry_50");
    assert_eq!(app.input_history[99], "entry_149");
}

#[test]
fn test_load_history_missing_file_is_ok() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("nonexistent.txt");

    let mut app = make_app();
    app.set_history_file(file);

    // Should succeed silently — the file simply doesn't exist yet.
    assert!(app.load_history().is_ok());
    assert!(app.input_history.is_empty());
}

#[test]
fn test_load_history_no_path_is_noop() {
    let mut app = make_app();
    // No history_file_path set.
    assert!(app.load_history().is_ok());
}

#[test]
fn test_load_history_clears_previous_entries() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");
    std::fs::write(&file, "fresh").unwrap();

    let mut app = make_app();
    app.input_history = vec!["stale1".into(), "stale2".into()];
    app.set_history_file(file);

    app.load_history().unwrap();

    assert_eq!(app.input_history, vec!["fresh"]);
}

// =========================================================================
// load_history — error branches
// =========================================================================

#[cfg(unix)]
#[test]
fn test_load_history_permission_denied() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");
    std::fs::write(&file, "secret").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();

    let mut app = make_app();
    app.set_history_file(file.clone());

    let result = app.load_history();
    assert!(result.is_err());

    // Restore permissions so TempDir cleanup succeeds.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
}

// =========================================================================
// Round-trip: save then load
// =========================================================================

#[test]
fn test_history_round_trip() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());
    app.input_history = vec![
        "first command".into(),
        "/opt cot".into(),
        "multi word input".into(),
    ];

    app.save_history().unwrap();

    // Simulate fresh app load.
    app.input_history.clear();
    app.load_history().unwrap();

    assert_eq!(
        app.input_history,
        vec!["first command", "/opt cot", "multi word input"]
    );
}

// =========================================================================
// Debounce fields: history_dirty / history_save_deadline
// =========================================================================

#[test]
fn test_dirty_flag_starts_false() {
    let app = make_app();
    assert!(!app.history_dirty);
    assert!(app.history_save_deadline.is_none());
}

#[test]
fn test_flush_history_if_due_noop_when_clean() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());
    app.input_history = vec!["hello".into()];
    // history_dirty is false — flush should be a no-op.
    app.flush_history_if_due();

    // File should not exist because no flush happened.
    assert!(!file.exists());
}

#[tokio::test]
async fn test_flush_history_if_due_saves_after_deadline() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());
    app.input_history = vec!["debounced".into()];

    // Simulate dirty state with an already-expired deadline.
    app.history_dirty = true;
    app.history_save_deadline = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(1))
            .unwrap(),
    );

    app.flush_history_if_due();

    // Dirty flag should be cleared.
    assert!(!app.history_dirty);
    assert!(app.history_save_deadline.is_none());

    // Give the spawn_blocking task a moment to complete.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(content, "debounced");
}

#[tokio::test]
async fn test_flush_history_if_due_skips_before_deadline() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());
    app.input_history = vec!["too_early".into()];

    // Dirty but deadline is far in the future.
    app.history_dirty = true;
    app.history_save_deadline =
        Some(std::time::Instant::now() + std::time::Duration::from_secs(60));

    app.flush_history_if_due();

    // Dirty flag should still be set — flush was deferred.
    assert!(app.history_dirty);
    assert!(app.history_save_deadline.is_some());

    // File should not exist.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!file.exists());
}

#[tokio::test]
async fn test_flush_history_if_due_no_path_set() {
    let mut app = make_app();
    app.history_dirty = true;
    app.history_save_deadline = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(1))
            .unwrap(),
    );

    // Should not panic even though dirty + deadline expired but no path set.
    app.flush_history_if_due();

    // Dirty flag stays true because no file could be written.
    assert!(app.history_dirty);
}

// =========================================================================
// Unicode / special characters
// =========================================================================

#[test]
fn test_history_round_trip_unicode() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("history.txt");

    let mut app = make_app();
    app.set_history_file(file.clone());
    app.input_history = vec![
        "こんにちは世界".into(),
        "émojis 🚀🦀".into(),
        "中文测试".into(),
    ];

    app.save_history().unwrap();
    app.input_history.clear();
    app.load_history().unwrap();

    assert_eq!(
        app.input_history,
        vec!["こんにちは世界", "émojis 🚀🦀", "中文测试"]
    );
}
