//! Tests for session import persistence (TASK-008).
//!
//! Verifies that exported messages can be imported into a new session,
//! preserving content, roles, and ordering while assigning new IDs.

use ragent_core::{
    message::{Message, MessagePart, Role},
    storage::Storage,
};

/// Create an in-memory storage instance.
fn mem_storage() -> Storage {
    Storage::open_in_memory().expect("in-memory storage")
}

#[test]
fn test_import_round_trip_preserves_content() {
    let storage = mem_storage();

    // Create original session and messages
    storage.create_session("sess-orig", "/tmp/project").unwrap();
    let m1 = Message::user_text("sess-orig", "Hello world");
    let m2 = Message::new(
        "sess-orig",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "Hi there!".to_string(),
        }],
    );
    storage.create_message(&m1).unwrap();
    storage.create_message(&m2).unwrap();

    // Export
    let exported = storage.get_messages("sess-orig").unwrap();
    let json = serde_json::to_string(&exported).unwrap();

    // Import: deserialise and re-parent into a new session
    let imported_msgs: Vec<Message> = serde_json::from_str(&json).unwrap();
    storage.create_session("sess-import", "/tmp/project").unwrap();

    for msg in &imported_msgs {
        let new_msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "sess-import".to_string(),
            role: msg.role.clone(),
            parts: msg.parts.clone(),
            created_at: msg.created_at,
            updated_at: msg.updated_at,
        };
        storage.create_message(&new_msg).unwrap();
    }

    // Verify
    let loaded = storage.get_messages("sess-import").unwrap();
    assert_eq!(loaded.len(), 2, "should have 2 imported messages");
    assert_eq!(loaded[0].text_content(), "Hello world");
    assert_eq!(loaded[0].role, Role::User);
    assert_eq!(loaded[1].text_content(), "Hi there!");
    assert_eq!(loaded[1].role, Role::Assistant);
}

#[test]
fn test_import_assigns_new_ids() {
    let storage = mem_storage();

    storage.create_session("sess-a", "/tmp/a").unwrap();
    let m1 = Message::user_text("sess-a", "Original");
    let orig_id = m1.id.clone();
    storage.create_message(&m1).unwrap();

    // Export and import
    let exported = storage.get_messages("sess-a").unwrap();
    let json = serde_json::to_string(&exported).unwrap();
    let imported_msgs: Vec<Message> = serde_json::from_str(&json).unwrap();

    storage.create_session("sess-b", "/tmp/b").unwrap();
    for msg in &imported_msgs {
        let new_msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "sess-b".to_string(),
            role: msg.role.clone(),
            parts: msg.parts.clone(),
            created_at: msg.created_at,
            updated_at: msg.updated_at,
        };
        storage.create_message(&new_msg).unwrap();
    }

    let loaded = storage.get_messages("sess-b").unwrap();
    assert_eq!(loaded.len(), 1);
    assert_ne!(
        loaded[0].id, orig_id,
        "imported message should have a new ID"
    );
    assert_eq!(loaded[0].text_content(), "Original");
}

#[test]
fn test_import_preserves_ordering() {
    let storage = mem_storage();

    storage.create_session("sess-ord", "/tmp/ord").unwrap();
    for i in 0..5 {
        let msg = Message::user_text("sess-ord", format!("msg-{i}"));
        storage.create_message(&msg).unwrap();
    }

    let exported = storage.get_messages("sess-ord").unwrap();
    let json = serde_json::to_string(&exported).unwrap();
    let imported_msgs: Vec<Message> = serde_json::from_str(&json).unwrap();

    storage
        .create_session("sess-ord-imp", "/tmp/ord-imp")
        .unwrap();
    for msg in &imported_msgs {
        let new_msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "sess-ord-imp".to_string(),
            role: msg.role.clone(),
            parts: msg.parts.clone(),
            created_at: msg.created_at,
            updated_at: msg.updated_at,
        };
        storage.create_message(&new_msg).unwrap();
    }

    let loaded = storage.get_messages("sess-ord-imp").unwrap();
    assert_eq!(loaded.len(), 5);
    for (i, msg) in loaded.iter().enumerate() {
        assert_eq!(msg.text_content(), format!("msg-{i}"));
    }
}

#[test]
fn test_import_empty_file() {
    let json = "[]";
    let messages: Vec<Message> = serde_json::from_str(json).unwrap();
    assert!(messages.is_empty(), "empty array should parse to empty vec");
}

#[test]
fn test_import_invalid_json_fails() {
    let result: Result<Vec<Message>, _> = serde_json::from_str("not valid json");
    assert!(result.is_err(), "invalid JSON should fail");
}

#[test]
fn test_import_preserves_tool_call_parts() {
    use ragent_core::message::{ToolCallState, ToolCallStatus};

    let storage = mem_storage();
    storage.create_session("sess-tc", "/tmp/tc").unwrap();

    let msg = Message::new(
        "sess-tc",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "Let me check that.".to_string(),
            },
            MessagePart::ToolCall {
                tool: "read_file".to_string(),
                call_id: "call-123".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: serde_json::json!({"path": "foo.txt"}),
                    output: Some(serde_json::json!("file contents")),
                    error: None,
                    duration_ms: Some(42),
                },
            },
        ],
    );
    storage.create_message(&msg).unwrap();

    let exported = storage.get_messages("sess-tc").unwrap();
    let json = serde_json::to_string(&exported).unwrap();
    let imported: Vec<Message> = serde_json::from_str(&json).unwrap();

    storage.create_session("sess-tc2", "/tmp/tc2").unwrap();
    for m in &imported {
        let new_msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "sess-tc2".to_string(),
            role: m.role.clone(),
            parts: m.parts.clone(),
            created_at: m.created_at,
            updated_at: m.updated_at,
        };
        storage.create_message(&new_msg).unwrap();
    }

    let loaded = storage.get_messages("sess-tc2").unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].parts.len(), 2);
    assert!(matches!(&loaded[0].parts[1], MessagePart::ToolCall { tool, .. } if tool == "read_file"));
}
