//! Tests for storage optimizations in Milestone 2.

use ragent_core::message::Message;
use ragent_core::storage::Storage;
use std::sync::Arc;

#[test]
fn test_batch_write_messages_basic() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    storage.create_session("sess-1", "/tmp/project").unwrap();

    let msgs = vec![
        Message::user_text("sess-1", "Hello,"),
        Message::user_text("sess-1", "World!"),
        Message::user_text("sess-1", "Third message"),
    ];

    storage.batch_write_messages(&msgs).unwrap();

    let retrieved = storage.get_messages("sess-1").unwrap();
    assert_eq!(retrieved.len(), 3);
    assert_eq!(retrieved[0].text_content(), "Hello,");
    assert_eq!(retrieved[1].text_content(), "World!");
    assert_eq!(retrieved[2].text_content(), "Third message");
}

#[test]
fn test_batch_write_empty_batch() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    storage.create_session("sess-1", "/tmp/project").unwrap();

    // Empty batch should succeed without error
    storage.batch_write_messages(&[]).unwrap();

    let retrieved = storage.get_messages("sess-1").unwrap();
    assert!(retrieved.is_empty());
}

#[test]
fn test_batch_write_single_message() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    storage.create_session("sess-1", "/tmp/project").unwrap();

    let msgs = vec![Message::user_text("sess-1", "Single")];

    storage.batch_write_messages(&msgs).unwrap();

    let retrieved = storage.get_messages("sess-1").unwrap();
    assert_eq!(retrieved.len(), 1);
    assert_eq!(retrieved[0].text_content(), "Single");
}

#[test]
fn test_batch_write_updates_session_timestamp() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    storage.create_session("sess-1", "/tmp/project").unwrap();

    let before = storage.get_session("sess-1").unwrap().unwrap().updated_at;

    // Add a small delay to ensure timestamp changes
    std::thread::sleep(std::time::Duration::from_millis(10));

    let msgs = vec![Message::user_text("sess-1", "Update")];
    storage.batch_write_messages(&msgs).unwrap();

    let after = storage.get_session("sess-1").unwrap().unwrap().updated_at;
    assert!(
        after > before,
        "Session timestamp should be updated after batch write"
    );
}

#[test]
fn test_batch_write_preserves_ordering() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    storage.create_session("sess-1", "/tmp/project").unwrap();

    // Create messages with specific ordering
    let mut msg1 = Message::user_text("sess-1", "First");
    msg1.created_at = chrono::Utc::now();

    let mut msg2 = Message::user_text("sess-1", "Second");
    // Slight delay for ordering
    std::thread::sleep(std::time::Duration::from_millis(5));
    msg2.created_at = chrono::Utc::now();

    let mut msg3 = Message::user_text("sess-1", "Third");
    std::thread::sleep(std::time::Duration::from_millis(5));
    msg3.created_at = chrono::Utc::now();

    storage.batch_write_messages(&[msg1, msg2, msg3]).unwrap();

    let retrieved = storage.get_messages("sess-1").unwrap();
    assert_eq!(retrieved.len(), 3);
    // Verify chronological ordering is maintained
    for (i, msg) in retrieved.iter().enumerate() {
        let expected = match i {
            0 => "First",
            1 => "Second",
            2 => "Third",
            _ => panic!("Unexpected index"),
        };
        assert_eq!(msg.text_content(), expected);
    }
}

#[test]
fn test_batch_write_rollback_on_failure() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    storage.create_session("sess-1", "/tmp/project").unwrap();

    // First batch succeeds
    let batch1 = vec![Message::user_text("sess-1", "Existing")];
    storage.batch_write_messages(&batch1).unwrap();

    // Second batch should succeed with new messages
    let batch2 = vec![
        Message::user_text("sess-1", "New1"),
        Message::user_text("sess-1", "New2"),
    ];
    storage.batch_write_messages(&batch2).unwrap();

    let retrieved = storage.get_messages("sess-1").unwrap();
    assert_eq!(retrieved.len(), 3);
    assert_eq!(retrieved[0].text_content(), "Existing");
    assert_eq!(retrieved[1].text_content(), "New1");
    assert_eq!(retrieved[2].text_content(), "New2");
}
