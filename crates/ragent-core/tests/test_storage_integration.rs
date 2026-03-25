//! Tests for test_storage_integration.rs

use ragent_core::message::Message;
use ragent_core::storage::{Storage, deobfuscate_key, obfuscate_key};
use std::sync::Arc;

// ── Session lifecycle ─────────────────────────────────────────────

#[test]
fn test_session_full_lifecycle() {
    let storage = Storage::open_in_memory().unwrap();

    storage.create_session("s1", "/project/one").unwrap();
    storage.create_session("s2", "/project/two").unwrap();

    let sessions = storage.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);

    storage.update_session("s1", "My First Session").unwrap();
    let s1 = storage.get_session("s1").unwrap().unwrap();
    assert_eq!(s1.title, "My First Session");
    assert_eq!(s1.directory, "/project/one");

    storage.archive_session("s1").unwrap();
    let sessions = storage.list_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "s2");

    // Archived session still retrievable by ID
    let archived = storage.get_session("s1").unwrap().unwrap();
    assert!(archived.archived_at.is_some());
}

#[test]
fn test_session_get_nonexistent() {
    let storage = Storage::open_in_memory().unwrap();
    assert!(storage.get_session("nonexistent").unwrap().is_none());
}

#[test]
fn test_session_list_empty() {
    let storage = Storage::open_in_memory().unwrap();
    let sessions = storage.list_sessions().unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn test_session_list_ordered_by_updated_at() {
    let storage = Storage::open_in_memory().unwrap();

    storage.create_session("old", "/old").unwrap();
    storage.create_session("new", "/new").unwrap();

    // Touch old session so it becomes most recently updated
    storage.update_session("old", "updated title").unwrap();

    let sessions = storage.list_sessions().unwrap();
    assert_eq!(sessions[0].id, "old");
    assert_eq!(sessions[1].id, "new");
}

// ── Message lifecycle ────────────────────────────────────────────

#[test]
fn test_message_create_and_retrieve_ordered() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp").unwrap();

    let msg1 = Message::user_text("s1", "First message");
    let msg2 = Message::user_text("s1", "Second message");
    let msg3 = Message::user_text("s1", "Third message");

    storage.create_message(&msg1).unwrap();
    storage.create_message(&msg2).unwrap();
    storage.create_message(&msg3).unwrap();

    let messages = storage.get_messages("s1").unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].text_content(), "First message");
    assert_eq!(messages[1].text_content(), "Second message");
    assert_eq!(messages[2].text_content(), "Third message");
}

#[test]
fn test_message_update_parts() {
    use ragent_core::message::{MessagePart, Role};

    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp").unwrap();

    let mut msg = Message::user_text("s1", "original");
    storage.create_message(&msg).unwrap();

    msg.parts = vec![MessagePart::Text {
        text: "updated content".to_string(),
    }];
    storage.update_message(&msg).unwrap();

    let messages = storage.get_messages("s1").unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text_content(), "updated content");
}

#[test]
fn test_messages_across_sessions() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/a").unwrap();
    storage.create_session("s2", "/b").unwrap();

    storage
        .create_message(&Message::user_text("s1", "msg in s1"))
        .unwrap();
    storage
        .create_message(&Message::user_text("s2", "msg in s2"))
        .unwrap();
    storage
        .create_message(&Message::user_text("s1", "another in s1"))
        .unwrap();

    let s1_msgs = storage.get_messages("s1").unwrap();
    let s2_msgs = storage.get_messages("s2").unwrap();
    assert_eq!(s1_msgs.len(), 2);
    assert_eq!(s2_msgs.len(), 1);
}

#[test]
fn test_messages_empty_session() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp").unwrap();
    let messages = storage.get_messages("s1").unwrap();
    assert!(messages.is_empty());
}

#[test]
fn test_message_touches_session_updated_at() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp").unwrap();

    let before = storage
        .get_session("s1")
        .unwrap()
        .unwrap()
        .updated_at
        .clone();

    std::thread::sleep(std::time::Duration::from_millis(10));
    storage
        .create_message(&Message::user_text("s1", "hello"))
        .unwrap();

    let after = storage.get_session("s1").unwrap().unwrap().updated_at;
    assert!(
        after > before,
        "Session updated_at should advance after adding a message"
    );
}

// ── Provider Auth CRUD ───────────────────────────────────────────

#[test]
fn test_provider_auth_full_lifecycle() {
    let storage = Storage::open_in_memory().unwrap();

    // Initially empty
    assert_eq!(storage.get_provider_auth("anthropic").unwrap(), None);

    // Set and get
    storage
        .set_provider_auth("anthropic", "sk-test-key-1")
        .unwrap();
    assert_eq!(
        storage.get_provider_auth("anthropic").unwrap(),
        Some("sk-test-key-1".to_string())
    );

    // Update (upsert)
    storage
        .set_provider_auth("anthropic", "sk-test-key-2")
        .unwrap();
    assert_eq!(
        storage.get_provider_auth("anthropic").unwrap(),
        Some("sk-test-key-2".to_string())
    );

    // Delete
    storage.delete_provider_auth("anthropic").unwrap();
    assert_eq!(storage.get_provider_auth("anthropic").unwrap(), None);
}

#[test]
fn test_provider_auth_multiple_providers() {
    let storage = Storage::open_in_memory().unwrap();

    storage.set_provider_auth("anthropic", "sk-ant").unwrap();
    storage.set_provider_auth("openai", "sk-oai").unwrap();
    storage.set_provider_auth("copilot", "ghu_abc").unwrap();

    assert_eq!(
        storage.get_provider_auth("anthropic").unwrap(),
        Some("sk-ant".to_string())
    );
    assert_eq!(
        storage.get_provider_auth("openai").unwrap(),
        Some("sk-oai".to_string())
    );
    assert_eq!(
        storage.get_provider_auth("copilot").unwrap(),
        Some("ghu_abc".to_string())
    );

    // Deleting one doesn't affect others
    storage.delete_provider_auth("openai").unwrap();
    assert_eq!(
        storage.get_provider_auth("anthropic").unwrap(),
        Some("sk-ant".to_string())
    );
    assert_eq!(storage.get_provider_auth("openai").unwrap(), None);
    assert_eq!(
        storage.get_provider_auth("copilot").unwrap(),
        Some("ghu_abc".to_string())
    );
}

// ── Settings CRUD ────────────────────────────────────────────────

#[test]
fn test_settings_full_lifecycle() {
    let storage = Storage::open_in_memory().unwrap();

    assert_eq!(storage.get_setting("theme").unwrap(), None);

    storage.set_setting("theme", "dark").unwrap();
    assert_eq!(
        storage.get_setting("theme").unwrap(),
        Some("dark".to_string())
    );

    storage.set_setting("theme", "light").unwrap();
    assert_eq!(
        storage.get_setting("theme").unwrap(),
        Some("light".to_string())
    );

    storage.delete_setting("theme").unwrap();
    assert_eq!(storage.get_setting("theme").unwrap(), None);
}

#[test]
fn test_settings_multiple_keys() {
    let storage = Storage::open_in_memory().unwrap();

    storage.set_setting("key1", "val1").unwrap();
    storage.set_setting("key2", "val2").unwrap();
    storage.set_setting("key3", "val3").unwrap();

    assert_eq!(
        storage.get_setting("key1").unwrap(),
        Some("val1".to_string())
    );
    assert_eq!(
        storage.get_setting("key2").unwrap(),
        Some("val2".to_string())
    );
    assert_eq!(
        storage.get_setting("key3").unwrap(),
        Some("val3".to_string())
    );
}

// ── Obfuscation roundtrip ────────────────────────────────────────

#[test]
fn test_obfuscation_roundtrip() {
    let keys = [
        "sk-test-simple",
        "sk-ant-api03-very-long-key-with-many-characters-1234567890abcdef",
        "",
        "short",
        "ghu_1234567890ABCDEFghijklmnop",
    ];
    for key in &keys {
        let obfuscated = obfuscate_key(key);
        let recovered = deobfuscate_key(&obfuscated);
        assert_eq!(
            &recovered, key,
            "Obfuscation roundtrip failed for key: {:?}",
            key
        );
    }
}

#[test]
fn test_obfuscation_produces_different_output() {
    let key = "sk-test-key";
    let obfuscated = obfuscate_key(key);
    assert_ne!(
        obfuscated, key,
        "Obfuscated key should differ from original"
    );
}

#[test]
fn test_deobfuscate_invalid_base64() {
    let _result = deobfuscate_key("not-valid-base64!!!");
    assert_eq!(result, "", "Invalid base64 should return empty string");
}

// ── Thread safety ────────────────────────────────────────────────

#[test]
fn test_storage_concurrent_access() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    storage.create_session("s1", "/tmp").unwrap();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let s = Arc::clone(&storage);
            std::thread::spawn(move || {
                let msg = Message::user_text("s1", format!("msg-{}", i));
                s.create_message(&msg).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let messages = storage.get_messages("s1").unwrap();
    assert_eq!(messages.len(), 10);
}
