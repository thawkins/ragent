//! Tests for the per-message render-cache staleness contract (FR-003, FR-006).
//!
//! The TUI render cache keys staleness on `Message::edit_seq` (bumped by
//! `Message::touch()`) instead of a global version counter, so mutating one
//! message never invalidates the cached renders of the others. These tests
//! pin the type-level contract that the layout reconcile logic depends on.

use ragent_types::message::{Message, MessagePart, Role};

#[test]
fn test_new_message_starts_with_zero_edit_seq() {
    let msg = Message::user_text("s1", "hello");
    assert_eq!(msg.edit_seq, 0);
}

#[test]
fn test_touch_bumps_edit_seq_and_updated_at() {
    let mut msg = Message::user_text("s1", "hello");
    let before = msg.updated_at;
    std::thread::sleep(std::time::Duration::from_millis(2));
    msg.touch();
    assert_eq!(msg.edit_seq, 1, "touch must bump edit_seq");
    assert!(msg.updated_at >= before, "touch must refresh updated_at");
    msg.touch();
    assert_eq!(msg.edit_seq, 2, "edit_seq must increase monotonically");
}

#[test]
fn test_edit_seq_default_zero_when_deserialising_old_payload() {
    // A payload serialised before `edit_seq` existed must still load (the
    // field is `#[serde(default)]`); old sessions resume without errors.
    let json = r#"{
        "id": "m-old",
        "session_id": "s1",
        "role": "user",
        "parts": [{"type": "text", "text": "legacy"}],
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z"
    }"#;
    let msg: Message =
        serde_json::from_str(json).expect("legacy payload without edit_seq must deserialise");
    assert_eq!(msg.edit_seq, 0, "missing edit_seq defaults to 0");
}

#[test]
fn test_edit_seq_survives_roundtrip() {
    let mut msg = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "body".into(),
        }],
    );
    msg.touch();
    msg.touch();
    let json = serde_json::to_string(&msg).expect("serialise");
    let back: Message = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(
        back.edit_seq, 2,
        "edit_seq must survive a serialise roundtrip"
    );
}

#[test]
fn test_touched_message_has_distinct_seq_from_untouched_sibling() {
    // The cache contract: two messages in the same session can carry
    // different edit_seq values, letting the renderer re-render only the
    // touched one.
    let mut a = Message::user_text("s1", "a");
    let b = Message::user_text("s1", "b");
    a.touch();
    assert_ne!(a.edit_seq, b.edit_seq);
}
