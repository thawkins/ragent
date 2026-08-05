//! Tests for the shared clipboard helpers.
//!
//! Covers text round-trip, repeated writes, and empty-string handling using
//! the deterministic synchronous writer.  Image temp-file tests remain in
//! `test_clipboard_tempfile.rs`.
//!
//! Note: text tests require a working system clipboard.  On headless Linux
//! CI without a display server they gracefully skip.

use std::sync::Mutex;

use ragent_tui::clipboard::{get_clipboard_text_sync, set_clipboard_text_sync};

/// Serialises clipboard tests because they all share the singleton system
/// clipboard.
static CLIPBOARD_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Returns `true` if a round-trip can be performed on this host.
fn clipboard_available() -> bool {
    set_clipboard_text_sync("availability_probe").is_ok()
        && get_clipboard_text_sync().as_deref() == Some("availability_probe")
}

#[test]
fn test_clipboard_text_roundtrip() {
    let _guard = CLIPBOARD_TEST_LOCK.lock().unwrap();
    if !clipboard_available() {
        return;
    }

    let original = "ragent clipboard round-trip: café 🦀";
    set_clipboard_text_sync(original).expect("write should succeed");
    let read = get_clipboard_text_sync().expect("clipboard should contain the written text");
    assert_eq!(read, original);
}

#[test]
fn test_clipboard_repeated_writes() {
    let _guard = CLIPBOARD_TEST_LOCK.lock().unwrap();
    if !clipboard_available() {
        return;
    }

    let values = ["first", "second", "third", "first"];
    for value in values {
        set_clipboard_text_sync(value).expect("write should succeed");
        let read = get_clipboard_text_sync().expect("clipboard read should succeed after a set");
        assert_eq!(
            read, value,
            "clipboard should reflect the most recent write"
        );
    }
}

#[test]
fn test_clipboard_empty_text() {
    let _guard = CLIPBOARD_TEST_LOCK.lock().unwrap();
    if !clipboard_available() {
        return;
    }

    // Prime the clipboard with non-empty content first so we can verify the
    // empty write actually replaces it.
    set_clipboard_text_sync("non-empty").expect("write should succeed");
    assert_eq!(get_clipboard_text_sync().as_deref(), Some("non-empty"));

    set_clipboard_text_sync("").expect("write should succeed");
    let read = get_clipboard_text_sync().expect("empty clipboard should still be readable");
    assert_eq!(read, "");
}
