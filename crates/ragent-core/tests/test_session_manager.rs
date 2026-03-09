use ragent_core::event::{Event, EventBus};
use ragent_core::message::Message;
use ragent_core::session::SessionManager;
use ragent_core::storage::Storage;
use std::path::PathBuf;
use std::sync::Arc;

fn create_test_manager() -> (SessionManager, Arc<EventBus>) {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(64));
    let manager = SessionManager::new(storage, event_bus.clone());
    (manager, event_bus)
}

#[test]
fn test_session_manager_create_emits_event() {
    let (manager, event_bus) = create_test_manager();
    let mut rx = event_bus.subscribe();

    let session = manager.create_session(PathBuf::from("/project")).unwrap();

    let event = rx.try_recv().unwrap();
    match event {
        Event::SessionCreated { session_id } => {
            assert_eq!(session_id, session.id);
        }
        other => panic!("Expected SessionCreated, got: {:?}", other),
    }
}

#[test]
fn test_session_manager_archive_emits_event() {
    let (manager, event_bus) = create_test_manager();
    let session = manager.create_session(PathBuf::from("/project")).unwrap();

    let mut rx = event_bus.subscribe();
    manager.archive_session(&session.id).unwrap();

    let event = rx.try_recv().unwrap();
    match event {
        Event::SessionUpdated { session_id } => {
            assert_eq!(session_id, session.id);
        }
        other => panic!("Expected SessionUpdated, got: {:?}", other),
    }
}

#[test]
fn test_session_manager_list_excludes_archived() {
    let (manager, _) = create_test_manager();

    let s1 = manager.create_session(PathBuf::from("/a")).unwrap();
    let _s2 = manager.create_session(PathBuf::from("/b")).unwrap();

    manager.archive_session(&s1.id).unwrap();

    let sessions = manager.list_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_ne!(sessions[0].id, s1.id);
}

#[test]
fn test_session_manager_get_nonexistent() {
    let (manager, _) = create_test_manager();
    assert!(manager.get_session("nonexistent").unwrap().is_none());
}

#[test]
fn test_session_manager_get_existing() {
    let (manager, _) = create_test_manager();
    let session = manager.create_session(PathBuf::from("/project")).unwrap();

    let retrieved = manager.get_session(&session.id).unwrap().unwrap();
    assert_eq!(retrieved.id, session.id);
    assert_eq!(retrieved.directory, PathBuf::from("/project"));
}

#[test]
fn test_session_manager_messages() {
    let (manager, _) = create_test_manager();
    let session = manager.create_session(PathBuf::from("/project")).unwrap();

    // Store messages via the underlying storage
    let msg1 = Message::user_text(&session.id, "Hello");
    let msg2 = Message::user_text(&session.id, "World");
    manager.storage().create_message(&msg1).unwrap();
    manager.storage().create_message(&msg2).unwrap();

    let messages = manager.get_messages(&session.id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text_content(), "Hello");
    assert_eq!(messages[1].text_content(), "World");
}

#[test]
fn test_session_manager_multiple_sessions() {
    let (manager, event_bus) = create_test_manager();
    let mut rx = event_bus.subscribe();

    let s1 = manager.create_session(PathBuf::from("/a")).unwrap();
    let s2 = manager.create_session(PathBuf::from("/b")).unwrap();
    let s3 = manager.create_session(PathBuf::from("/c")).unwrap();

    // 3 SessionCreated events
    for _ in 0..3 {
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, Event::SessionCreated { .. }));
    }

    let sessions = manager.list_sessions().unwrap();
    assert_eq!(sessions.len(), 3);

    manager.archive_session(&s2.id).unwrap();
    let sessions = manager.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);

    let ids: Vec<_> = sessions.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&s1.id.as_str()));
    assert!(ids.contains(&s3.id.as_str()));
}

#[test]
fn test_session_fields_populated() {
    let (manager, _) = create_test_manager();
    let session = manager
        .create_session(PathBuf::from("/my/project"))
        .unwrap();

    assert!(!session.id.is_empty());
    assert_eq!(session.directory, PathBuf::from("/my/project"));
    assert_eq!(session.title, "");
    assert_eq!(session.version, 1);
    assert!(session.archived_at.is_none());
    assert!(session.summary.is_none());
}
