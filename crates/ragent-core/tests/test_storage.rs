use ragent_core::message::Message;
use ragent_core::storage::Storage;

#[test]
fn test_storage_roundtrip() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp/test").unwrap();

    let session = storage.get_session("s1").unwrap().unwrap();
    assert_eq!(session.directory, "/tmp/test");

    let msg = Message::user_text("s1", "Hello!");
    storage.create_message(&msg).unwrap();

    let messages = storage.get_messages("s1").unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text_content(), "Hello!");
}

#[test]
fn test_storage_provider_auth_crud() {
    let storage = Storage::open_in_memory().unwrap();
    storage.set_provider_auth("anthropic", "sk-test-123").unwrap();
    let key = storage.get_provider_auth("anthropic").unwrap();
    assert_eq!(key, Some("sk-test-123".to_string()));
}

#[test]
fn test_storage_archive_session() {
    let storage = Storage::open_in_memory().unwrap();
    storage.create_session("s1", "/tmp").unwrap();
    storage.archive_session("s1").unwrap();
    let sessions = storage.list_sessions().unwrap();
    assert!(sessions.is_empty());
}
