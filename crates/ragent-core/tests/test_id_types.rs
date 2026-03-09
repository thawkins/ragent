use ragent_core::id::*;
use std::collections::HashSet;

// ── SessionId ────────────────────────────────────────────────────

#[test]
fn test_session_id_new_unique() {
    let id1 = SessionId::new();
    let id2 = SessionId::new();
    assert_ne!(id1, id2, "Generated SessionIds should be unique");
}

#[test]
fn test_session_id_from_string() {
    let id = SessionId::from("test-session-123".to_string());
    assert_eq!(id.as_str(), "test-session-123");
    assert_eq!(id.0, "test-session-123");
}

#[test]
fn test_session_id_from_str() {
    let id = SessionId::from("test-session");
    assert_eq!(id.as_str(), "test-session");
}

#[test]
fn test_session_id_display() {
    let id = SessionId::from("display-me");
    assert_eq!(format!("{}", id), "display-me");
}

#[test]
fn test_session_id_as_ref() {
    let id = SessionId::from("ref-test");
    let s: &str = id.as_ref();
    assert_eq!(s, "ref-test");
}

#[test]
fn test_session_id_default() {
    let id = SessionId::default();
    assert!(!id.as_str().is_empty(), "Default should generate a UUID");
}

#[test]
fn test_session_id_serde() {
    let id = SessionId::from("serde-test");
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: SessionId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

// ── MessageId ────────────────────────────────────────────────────

#[test]
fn test_message_id_new_unique() {
    let id1 = MessageId::new();
    let id2 = MessageId::new();
    assert_ne!(id1, id2);
}

#[test]
fn test_message_id_from_and_display() {
    let id = MessageId::from("msg-42");
    assert_eq!(id.as_str(), "msg-42");
    assert_eq!(format!("{}", id), "msg-42");
}

// ── ProviderId ───────────────────────────────────────────────────

#[test]
fn test_provider_id_from_and_display() {
    let id = ProviderId::from("anthropic");
    assert_eq!(id.as_str(), "anthropic");
    assert_eq!(format!("{}", id), "anthropic");
}

// ── ToolCallId ───────────────────────────────────────────────────

#[test]
fn test_tool_call_id_from_and_display() {
    let id = ToolCallId::from("call_123");
    assert_eq!(id.as_str(), "call_123");
    assert_eq!(format!("{}", id), "call_123");
}

// ── Hash and Eq for collections ──────────────────────────────────

#[test]
fn test_session_id_in_hashset() {
    let mut set = HashSet::new();
    let id1 = SessionId::from("a");
    let id2 = SessionId::from("b");
    let id1_dup = SessionId::from("a");

    set.insert(id1.clone());
    set.insert(id2.clone());
    set.insert(id1_dup);

    assert_eq!(set.len(), 2, "Duplicate IDs should be deduplicated");
    assert!(set.contains(&SessionId::from("a")));
    assert!(set.contains(&SessionId::from("b")));
}

#[test]
fn test_message_id_equality() {
    let id1 = MessageId::from("same");
    let id2 = MessageId::from("same");
    let id3 = MessageId::from("different");

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

// ── Clone ────────────────────────────────────────────────────────

#[test]
fn test_id_types_clone() {
    let session_id = SessionId::from("clone-test");
    let cloned = session_id.clone();
    assert_eq!(session_id, cloned);

    let msg_id = MessageId::from("clone-msg");
    let cloned = msg_id.clone();
    assert_eq!(msg_id, cloned);
}

// ── All ID types generate valid UUIDs ────────────────────────────

#[test]
fn test_id_types_generate_uuids() {
    let session = SessionId::new();
    let message = MessageId::new();
    let provider = ProviderId::new();
    let tool_call = ToolCallId::new();

    // UUID v4 format: 8-4-4-4-12 hex characters
    for id in &[
        session.as_str(),
        message.as_str(),
        provider.as_str(),
        tool_call.as_str(),
    ] {
        assert_eq!(id.len(), 36, "UUID should be 36 chars: {}", id);
        assert_eq!(
            id.chars().filter(|c| *c == '-').count(),
            4,
            "UUID should have 4 dashes: {}",
            id
        );
    }
}
