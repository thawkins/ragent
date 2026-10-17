//! Tests-only coverage for `SessionManager` lifecycle
//! (`ragent_agent::session`): create/get/list/archive, the per-session state
//! cache, and `finish_reason_label`.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::session::{SessionManager, finish_reason_label};
use ragent_agent::storage::Storage;
use ragent_types::message::Message;

fn manager() -> (SessionManager, Arc<EventBus>) {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let bus = Arc::new(EventBus::new(64));
    (SessionManager::new(storage, bus.clone()), bus)
}

#[test]
fn create_get_list_and_archive_session() {
    let (mgr, bus) = manager();
    let session = mgr
        .create_session(PathBuf::from("/tmp/proj"))
        .expect("create");
    assert_ne!(session.id, "");
    assert_eq!(session.directory, PathBuf::from("/tmp/proj"));

    let fetched = mgr.get_session(&session.id).expect("get").expect("present");
    assert_eq!(fetched.id, session.id);

    let listed = mgr.list_sessions().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, session.id);

    // get on an unknown id yields None.
    assert!(mgr.get_session("nope").expect("get").is_none());

    // Subscribe BEFORE archiving so the SessionUpdated event is observed.
    let mut rx = bus.subscribe();
    mgr.archive_session(&session.id).expect("archive");
    assert!(mgr.list_sessions().expect("list after archive").is_empty());

    let mut saw_updated = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::SessionUpdated { .. }) {
            saw_updated = true;
        }
    }
    assert!(saw_updated, "SessionUpdated event expected");
}

#[test]
fn messages_start_empty_and_round_trip() {
    let (mgr, _bus) = manager();
    let session = mgr.create_session(PathBuf::from(".")).expect("create");
    assert_eq!(mgr.get_messages(&session.id).expect("messages").len(), 0);

    mgr.storage()
        .create_message(&Message::user_text(&session.id, "hi"))
        .expect("message");
    let messages = mgr.get_messages(&session.id).expect("messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text_content(), "hi");
}

#[test]
fn session_state_cache_is_stable_and_removable() {
    let (mgr, _bus) = manager();
    let a = mgr.session_state_cache("sess-a");
    let a2 = mgr.session_state_cache("sess-a");
    assert!(
        Arc::ptr_eq(&a, &a2),
        "the same session returns the same cache Arc"
    );

    let b = mgr.session_state_cache("sess-b");
    assert!(
        !Arc::ptr_eq(&a, &b),
        "distinct sessions get distinct caches"
    );

    // Removing a session state then re-requesting creates a fresh Arc.
    mgr.remove_session_state("sess-a");
    let a3 = mgr.session_state_cache("sess-a");
    assert!(!Arc::ptr_eq(&a, &a3), "removal evicts the cached Arc");
}

#[test]
fn finish_reason_label_matches_the_enum_labels() {
    use ragent_types::event::FinishReason;
    assert_eq!(finish_reason_label(&FinishReason::Stop), "stop");
    assert_eq!(finish_reason_label(&FinishReason::ToolUse), "tool_use");
    assert_eq!(finish_reason_label(&FinishReason::Length), "length");
    assert_eq!(
        finish_reason_label(&FinishReason::ContentFilter),
        "content_filter"
    );
    assert_eq!(finish_reason_label(&FinishReason::Truncation), "truncation");
}
