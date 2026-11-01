//! Tests for age-based message retention (`Storage::prune_messages_older_than`)
//! and the `storage.retention` configuration block.

#![forbid(unsafe_code)]

use ragent_storage::storage::Storage;
use ragent_types::message::Message;

/// Build a user message whose `created_at` is `age_days` in the past.
fn aged_message(id: &str, text: &str, age_days: i64) -> Message {
    let mut msg = Message::user_text("sess-1", text);
    msg.id = id.to_string();
    let created = chrono::Utc::now() - chrono::Duration::days(age_days);
    msg.created_at = created;
    msg.updated_at = created;
    msg
}

/// A message older than the window is deleted; a fresher one survives.
#[test]
fn prune_deletes_only_messages_past_the_window() {
    let storage = Storage::open_in_memory().expect("open");
    storage
        .create_session("sess-1", "/tmp/project")
        .expect("session");
    storage
        .create_message(&aged_message("old", "stale text", 30))
        .expect("old");
    storage
        .create_message(&aged_message("fresh", "current text", 0))
        .expect("fresh");

    let deleted = storage
        .prune_messages_older_than(chrono::Duration::days(14))
        .expect("prune");

    assert_eq!(deleted, 1, "only the 30-day-old message expires");
    let ids: Vec<String> = storage
        .get_messages("sess-1")
        .expect("get")
        .into_iter()
        .map(|m| m.id)
        .collect();
    assert_eq!(ids, vec!["fresh".to_string()]);
}

/// A zero window expires every stored message.
#[test]
fn prune_with_zero_window_expires_everything() {
    let storage = Storage::open_in_memory().expect("open");
    storage
        .create_session("sess-1", "/tmp/project")
        .expect("session");
    storage
        .create_message(&aged_message("a", "one", 1))
        .expect("a");
    storage
        .create_message(&aged_message("b", "two", 2))
        .expect("b");

    let deleted = storage
        .prune_messages_older_than(chrono::Duration::zero())
        .expect("prune");

    assert_eq!(deleted, 2);
    assert!(storage.get_messages("sess-1").expect("get").is_empty());
}

/// An empty store returns zero rather than failing.
#[test]
fn prune_on_empty_store_is_a_no_op() {
    let storage = Storage::open_in_memory().expect("open");
    let deleted = storage
        .prune_messages_older_than(chrono::Duration::days(14))
        .expect("prune");
    assert_eq!(deleted, 0);
}

/// Pruning expires messages but keeps the session row, so an old conversation
/// stays listed and resumable with an empty transcript.
#[test]
fn prune_keeps_the_session_row() {
    let storage = Storage::open_in_memory().expect("open");
    storage
        .create_session("sess-1", "/tmp/project")
        .expect("session");
    storage
        .create_message(&aged_message("old", "stale text", 30))
        .expect("old");

    storage
        .prune_messages_older_than(chrono::Duration::days(14))
        .expect("prune");

    let sessions = storage.list_sessions().expect("list");
    assert_eq!(sessions.len(), 1, "session survives message expiry");
    assert!(storage.get_messages("sess-1").expect("get").is_empty());
}

/// Pruning must not desync the FTS index: an expired message's text stops
/// matching a search in its session.
#[test]
fn prune_keeps_the_fts_index_consistent() {
    let storage = Storage::open_in_memory().expect("open");
    storage
        .create_session("sess-1", "/tmp/project")
        .expect("session");
    storage
        .create_message(&aged_message("old", "ancientneedle", 30))
        .expect("old");
    storage
        .create_message(&aged_message("fresh", "freshneedle", 0))
        .expect("fresh");

    let before = storage
        .search_conversation("sess-1", "ancientneedle", 10)
        .expect("search");
    assert_eq!(before.len(), 1, "indexed before the prune");

    storage
        .prune_messages_older_than(chrono::Duration::days(14))
        .expect("prune");

    assert!(
        storage
            .search_conversation("sess-1", "ancientneedle", 10)
            .expect("search")
            .is_empty(),
        "expired message must leave the FTS index"
    );
    assert_eq!(
        storage
            .search_conversation("sess-1", "freshneedle", 10)
            .expect("search")
            .len(),
        1,
        "surviving message must stay searchable"
    );
}
