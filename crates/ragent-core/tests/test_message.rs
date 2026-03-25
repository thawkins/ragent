//! Tests for test_message.rs

use ragent_core::message::{Message, MessagePart, Role};

#[test]
fn test_message_user_text_content() {
    let msg = Message::user_text("session-1", "Hello, world!");
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.session_id, "session-1");
    assert_eq!(msg.text_content(), "Hello, world!");
    assert_eq!(msg.parts.len(), 1);
    match &msg.parts[0] {
        MessagePart::Text { text } => assert_eq!(text, "Hello, world!"),
        other => panic!("expected Text part, got: {other:?}"),
    }
}

#[test]
fn test_message_serialization_roundtrip() {
    let msg = Message::user_text("s1", "round-trip test");
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, msg.id);
    assert_eq!(deserialized.session_id, msg.session_id);
    assert_eq!(deserialized.role, msg.role);
    assert_eq!(deserialized.text_content(), "round-trip test");
}
