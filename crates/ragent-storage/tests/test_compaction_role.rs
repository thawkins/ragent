use ragent_storage::storage::Storage;
use ragent_types::message::{Message, MessagePart, Role};

#[test]
fn test_compaction_message_roundtrip() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s-compact", "/tmp/project").unwrap();

    let msg = Message {
        id: "m1".to_string(),
        session_id: "s-compact".to_string(),
        role: Role::Compaction,
        parts: vec![MessagePart::Text {
            text: "## Objective\n- Foo".to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    storage.create_message(&msg).unwrap();

    let loaded = storage.get_messages("s-compact").unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].role, Role::Compaction);
    assert_eq!(loaded[0].text_content(), "## Objective\n- Foo");
}

#[test]
fn test_has_assistant_messages_includes_compaction() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s2", "/tmp/project").unwrap();
    assert!(!storage.has_assistant_messages("s2").unwrap());

    let msg = Message {
        id: "c1".to_string(),
        session_id: "s2".to_string(),
        role: Role::Compaction,
        parts: vec![MessagePart::Text {
            text: "summary".to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    storage.create_message(&msg).unwrap();
    assert!(storage.has_assistant_messages("s2").unwrap());
}
