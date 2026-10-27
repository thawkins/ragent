//! Isolation coverage for session message search, cross-session search,
//! conversation stats, and the FTS warm-up rebuild (`search_conversation`,
//! `search_session_messages`, `conversation_stats`, `warm_message_search_index`).

use ragent_storage::storage::{SessionSearchParams, Storage};
use ragent_types::message::{Message, MessagePart, Role};

fn storage() -> Storage {
    Storage::open_in_memory().expect("storage")
}

fn compaction(session: &str, text: &str) -> Message {
    Message::new(
        session,
        Role::Compaction,
        vec![MessagePart::Text {
            text: text.to_string(),
        }],
    )
}

#[test]
fn conversation_stats_breaks_down_by_role() {
    let storage = storage();
    storage
        .create_session("sess-1", "/tmp/proj")
        .expect("session");
    storage
        .create_message(&Message::user_text("sess-1", "hello"))
        .expect("user");
    storage
        .create_message(&Message::assistant_text("sess-1", "hi there"))
        .expect("assistant");
    storage
        .create_message(&compaction("sess-1", "summary of prior turns"))
        .expect("compaction");

    let stats = storage.conversation_stats("sess-1").expect("stats");
    assert_eq!(stats.total, 3);
    assert_eq!(stats.user_count, 1);
    assert_eq!(stats.assistant_count, 1);
    assert_eq!(stats.compaction_count, 1);
    assert!(stats.has_compaction);

    let empty = storage
        .conversation_stats("no-such-session")
        .expect("empty");
    assert_eq!(empty.total, 0);
    assert!(!empty.has_compaction);
}

#[test]
fn search_conversation_ranks_and_scopes_to_session() {
    let storage = storage();
    storage.create_session("s1", "/tmp/a").expect("s1");
    storage.create_session("s2", "/tmp/b").expect("s2");
    storage
        .create_message(&Message::user_text("s1", "database migration plan"))
        .expect("m1");
    storage
        .create_message(&Message::assistant_text("s1", "starting the migration"))
        .expect("m2");
    storage
        .create_message(&Message::user_text("s2", "unrelated migration note"))
        .expect("m3");

    let hits = storage
        .search_conversation("s1", "migration", 10)
        .expect("search");
    assert_eq!(
        hits.len(),
        2,
        "only s1 messages, all with the session scope"
    );
    assert!(hits.iter().all(|h| h.session_id == "s1"));
    assert!(hits.iter().all(|h| h.content.contains("migration")));
    assert_eq!(hits[0].session_directory.as_deref(), Some("/tmp/a"));

    // An empty/blank query returns nothing rather than erroring.
    assert!(
        storage
            .search_conversation("s1", "   ", 10)
            .expect("blank")
            .is_empty()
    );
    // limit truncates.
    assert_eq!(
        storage
            .search_conversation("s1", "migration", 1)
            .expect("limit")
            .len(),
        1
    );
}

#[test]
fn search_session_messages_applies_filters() {
    let storage = storage();
    storage.create_session("s1", "/tmp/a").expect("s1");
    storage.create_session("s2", "/tmp/b").expect("s2");
    storage
        .create_message(&Message::user_text("s1", "migration alpha"))
        .expect("m1");
    storage
        .create_message(&Message::assistant_text("s1", "migration beta"))
        .expect("m2");
    storage
        .create_message(&Message::user_text("s2", "migration gamma"))
        .expect("m3");

    // Unfiltered: all three sessions' rows.
    let all = storage
        .search_session_messages(&SessionSearchParams {
            query: "migration".to_string(),
            limit: 10,
            ..Default::default()
        })
        .expect("all");
    assert_eq!(all.len(), 3);

    // working_doc filter.
    let by_dir = storage
        .search_session_messages(&SessionSearchParams {
            query: "migration".to_string(),
            limit: 10,
            working_dir: Some("/tmp/a".to_string()),
            ..Default::default()
        })
        .expect("dir");
    assert_eq!(by_dir.len(), 2);
    assert!(
        by_dir
            .iter()
            .all(|r| r.session_directory.as_deref() == Some("/tmp/a"))
    );

    // roles filter (only the single user message in s1).
    let by_role = storage
        .search_session_messages(&SessionSearchParams {
            query: "migration".to_string(),
            limit: 10,
            roles: Some(vec!["user".to_string()]),
            ..Default::default()
        })
        .expect("role");
    assert_eq!(by_role.len(), 2);
    assert!(by_role.iter().all(|r| r.role == "user"));

    // session_id filter.
    let by_session = storage
        .search_session_messages(&SessionSearchParams {
            query: "migration".to_string(),
            limit: 10,
            session_id: Some("s2".to_string()),
            ..Default::default()
        })
        .expect("session");
    assert_eq!(by_session.len(), 1);
    assert_eq!(by_session[0].session_id, "s2");

    // max_per_session caps each session to one row.
    let capped = storage
        .search_session_messages(&SessionSearchParams {
            query: "migration".to_string(),
            limit: 10,
            max_per_session: Some(1),
            ..Default::default()
        })
        .expect("capped");
    assert_eq!(capped.len(), 2, "one row each for s1 and s2");

    // Blank query short-circuits.
    let blank = storage
        .search_session_messages(&SessionSearchParams {
            query: "  ".to_string(),
            limit: 10,
            ..Default::default()
        })
        .expect("blank");
    assert!(blank.is_empty());
}

#[test]
fn search_session_messages_since_until_window() {
    let storage = storage();
    storage.create_session("s1", "/tmp/a").expect("s1");
    storage
        .create_message(&Message::user_text("s1", "windowed migration"))
        .expect("m1");

    // `since` in the future excludes the row; `until` in the past excludes it.
    let future = "2999-01-01T00:00:00+00:00".to_string();
    let past = "2000-01-01T00:00:00+00:00".to_string();

    let since_future = storage
        .search_session_messages(&SessionSearchParams {
            query: "windowed".to_string(),
            limit: 10,
            since: Some(future.clone()),
            ..Default::default()
        })
        .expect("since");
    assert!(since_future.is_empty());

    let until_past = storage
        .search_session_messages(&SessionSearchParams {
            query: "windowed".to_string(),
            limit: 10,
            until: Some(past.clone()),
            ..Default::default()
        })
        .expect("until");
    assert!(until_past.is_empty());

    let in_window = storage
        .search_session_messages(&SessionSearchParams {
            query: "windowed".to_string(),
            limit: 10,
            since: Some(past),
            until: Some(future),
            ..Default::default()
        })
        .expect("window");
    assert_eq!(in_window.len(), 1);
}

#[test]
fn warm_message_search_index_is_noop_then_backfills() {
    let storage = storage();
    storage.create_session("s1", "/tmp/a").expect("s1");
    storage
        .create_message(&Message::user_text("s1", "warm me up"))
        .expect("m1");

    // Rows are indexed incrementally on insert, so the warm-up is a no-op.
    assert_eq!(storage.warm_message_search_index().expect("warm"), 0);

    // Simulate an upgrade: drop the FTS row *and* its key row, then the warm-up
    // backfills it. Dropping the key row is what a legacy database looks like -
    // the index is contentless, so a bare index row with no `message_fts_keys`
    // row is already treated as missing (SQLITE T-01).
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute("DELETE FROM messages_fts", [])
            .expect("clear fts");
        conn.execute("DELETE FROM message_fts_keys", [])
            .expect("clear fts keys");
    }
    assert_eq!(storage.warm_message_search_index().expect("backfill"), 1);
    // A second run finds nothing missing.
    assert_eq!(storage.warm_message_search_index().expect("noop"), 0);

    let hits = storage
        .search_conversation("s1", "warm", 10)
        .expect("search");
    assert_eq!(hits.len(), 1);
}

#[test]
fn warm_message_search_index_skips_corrupt_parts() {
    let storage = storage();
    storage.create_session("s1", "/tmp/a").expect("s1");
    storage
        .create_message(&Message::user_text("s1", "valid message"))
        .expect("m1");
    // Plant a message with corrupt parts JSON in the base table but no FTS row.
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute(
            "INSERT INTO messages (id, session_id, role, parts, created_at, updated_at) \
             VALUES ('broken', 's1', 'user', 'not-json', '2024-01-01T00:00:00+00:00', \
             '2024-01-01T00:00:00+00:00')",
            [],
        )
        .expect("plant broken");
        conn.execute("DELETE FROM messages_fts", []).expect("clear");
        conn.execute("DELETE FROM message_fts_keys", [])
            .expect("clear keys");
    }
    // Only the valid message is backfilled. The report counts rows the warm-up
    // actually repaired, so the corrupt message - visited, logged and skipped -
    // does not inflate it (SQLITE T-01: a skipped row is not a repair).
    assert_eq!(storage.warm_message_search_index().expect("backfill"), 1);
    let hits = storage
        .search_conversation("s1", "valid", 10)
        .expect("search");
    assert_eq!(hits.len(), 1);
}
