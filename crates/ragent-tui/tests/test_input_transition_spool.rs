//! Coverage for the durable input-transition spool.
//!
//! The TUI's only live trace of input handling is the log panel, which is
//! rate-limited and lost when the process exits, so a reported "characters
//! land in the wrong place" could not be diagnosed after the fact. Every
//! transition whose buffer or cursor actually changed is now mirrored into
//! `log/logwindow/input-transitions.log`.
//!
//! These tests pin the two properties that make that spool usable as
//! evidence: a real transition is recorded with the handler that caused it,
//! and a keypress that moves nothing writes no line at all (so an idle
//! session does not bury the interesting rows).
//!
//! The spool honours `helpers::log_root()`, and `support::make_app` redirects
//! that to the git-ignored `target/temp/log` tree, so a test run never
//! dirties the checked-in `log/` directory.

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Path of the transition spool under the scratch log root.
fn spool_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/temp/log/logwindow/input-transitions.log")
}

/// Read the spool, returning an empty string when it does not exist yet.
fn read_spool() -> String {
    std::fs::read_to_string(spool_path()).unwrap_or_default()
}

/// Rows of the spool that record the given resulting buffer text.
///
/// The spool file is process-wide, so several tests in this binary append to
/// it concurrently (nextest runs each test in its own process). Every row
/// records the text the buffer ended up holding, which is unique per test
/// here, so matching on it isolates this test's rows without needing a lock
/// or a dedicated file.
fn rows_ending_with(text: &str) -> Vec<String> {
    let needle = format!("after=\"{text}\"");
    read_spool()
        .lines()
        .filter(|row| row.ends_with(&needle))
        .map(str::to_string)
        .collect()
}

/// A plain key event with no modifiers.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[tokio::test]
async fn test_input_transition_spool_records_a_typed_character() {
    let mut app = support::make_app();
    // The spool accumulates across runs, so this test asserts on the growth
    // its own keystroke caused rather than on an absolute row count.
    let before = rows_ending_with("q").len();

    app.handle_key_event(key(KeyCode::Char('q'))).await;

    let rows = rows_ending_with("q");
    assert_eq!(
        rows.len(),
        before + 1,
        "typing a character must append exactly one transition row to {}",
        spool_path().display()
    );
    let row = rows
        .last()
        .unwrap_or_else(|| unreachable!("row just added"));
    assert!(row.contains("source="), "row was: {row}");
    assert!(row.contains("before_chars=0"), "row was: {row}");
    assert!(row.contains("after_chars=1"), "row was: {row}");
    assert!(row.contains("after_cursor=1"), "row was: {row}");
}

#[tokio::test]
async fn test_input_transition_spool_is_silent_when_nothing_moves() {
    let mut app = support::make_app();
    let before = read_spool().lines().count();

    // A key that changes neither the buffer nor the cursor: with an empty
    // transcript this scrolls nothing, so it must add no row at all.
    app.handle_key_event(key(KeyCode::PageDown)).await;

    let after = read_spool().lines().count();
    assert_eq!(
        after, before,
        "a no-op keypress must not write a transition row (spool grew {before} -> {after})"
    );
}

#[tokio::test]
async fn test_input_transition_spool_records_backspace_cursor_delta() {
    let mut app = support::make_app();

    app.handle_key_event(key(KeyCode::Char('z'))).await;
    app.handle_key_event(key(KeyCode::Char('y'))).await;
    app.handle_key_event(key(KeyCode::Backspace)).await;

    // The typing row before the deletion ended at "zy"; the backspace row is
    // the one that returned the buffer to "z".
    let rows = rows_ending_with("z");
    let row = rows
        .iter()
        .find(|row| row.contains("cursor_delta=-1"))
        .unwrap_or_else(|| panic!("expected a backspace row ending at \"z\": {rows:?}"));
    assert!(
        row.contains("after_chars=1"),
        "backspace should leave one character, row was: {row}"
    );
    // The cursor must retreat with the deletion; a delta of 0 here would be
    // exactly the "text right, cursor wrong" shape this spool exists to catch.
    assert!(
        row.contains("before=\"zy\""),
        "backspace should have deleted from \"zy\", row was: {row}"
    );
}
