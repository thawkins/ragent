//! Regression tests for the startup FTS warm-up plan and repair.
//!
//! `messages_fts` is contentless (`content = ''`, `contentless_delete = 1`)
//! and keyed through the ordinary `message_fts_keys` table (SQLITE T-01). The
//! index row's rowid is the message's `messages.rowid`, so the reconcile is a
//! rowid diff between two real tables - never a join against the virtual
//! table, whose key columns are `UNINDEXED` and therefore force an
//! O(messages x index-rows) nested scan.
//!
//! That scan is what a mature history used to pay on every start: ragent
//! burned a CPU core in its own SQLite work while no tool call was executing,
//! which looked exactly like the agent loop stalling after a bash call.
//!
//! The tests below cover the plan, the source-level choice of lookup, the
//! backfill behaviour, the orphan sweep, and the repair of an index row whose
//! key row has gone missing.

use ragent_storage::Storage;
use ragent_types::message::Message;

/// Returns a fresh file-backed temp database path inside the workspace.
fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/temp/warm-index-scan");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    path
}

/// The warm-up must not answer "which messages are missing an index row" by
/// joining `messages` against the FTS virtual table on an `UNINDEXED` key
/// column. It diffs the base rowids against `message_fts_keys.rowid`, which is
/// an ordinary indexed integer primary key.
#[test]
fn warm_path_diffs_the_key_table_by_rowid() {
    let source = include_str!("../src/storage.rs");
    let warm_start = source
        .find("fn backfill_missing_fts_rows")
        .expect("the warm-up backfill must exist");
    let warm_end = source
        .find("\n/// Statistics about a session's message history.")
        .expect("end of the storage impl block");
    let warm = &source[warm_start..warm_end];

    warm.find("FROM message_fts_keys")
        .expect("the warm-up must diff against the `message_fts_keys` table");
    assert!(
        !warm.contains("ON f.message_id = m.id"),
        "the warm-up must not join the FTS virtual table on an UNINDEXED column"
    );
    assert!(
        !warm.contains("messages_fts_content"),
        "a contentless FTS5 table has no `messages_fts_content` shadow table to diff"
    );
}

/// The replacement lookup must be answered by a rowid seek into the key table -
/// a `USING INTEGER PRIMARY KEY` / `ROWID SEARCH` plan - never a scan of the
/// virtual table.
#[test]
fn warm_index_lookup_plan_seeks_the_key_table() {
    let path = temp_db_path("plan.db");
    let storage = Storage::open(&path).expect("open storage");
    storage.create_session("s1", "/tmp/a").expect("session");
    storage
        .create_message(&Message::user_text("s1", "hello"))
        .expect("message");

    {
        let conn = storage.conn_lock_for_test().expect("lock");
        let plan: Vec<String> = conn
            .prepare(
                "EXPLAIN QUERY PLAN \
                 SELECT m.id FROM messages m WHERE m.rowid NOT IN \
                 (SELECT k.rowid FROM message_fts_keys k)",
            )
            .expect("prepare")
            .query_map([], |row| row.get::<_, String>(3))
            .expect("query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect");

        let joined = plan.join("\n");
        assert!(
            !joined.contains("VIRTUAL TABLE"),
            "the warm-up lookup must never touch the FTS vtable: {joined}"
        );
        assert!(
            joined.contains("message_fts_keys"),
            "the plan must go through the key table: {joined}"
        );
        assert!(
            joined.contains("USING ROWID SEARCH ON TABLE message_fts_keys"),
            "the key-table side must be a rowid seek, not a scan: {joined}"
        );
    }

    drop(storage);
    let _ = std::fs::remove_file(&path);
}

/// A message with no index row is backfilled, and a message with one is not
/// re-indexed (the rowid diff must agree with presence in `messages_fts`).
#[test]
fn warm_index_backfills_only_missing_rows() {
    let path = temp_db_path("backfill.db");
    let storage = Storage::open(&path).expect("open storage");
    storage.create_session("s1", "/tmp/a").expect("session");
    storage
        .create_message(&Message::user_text("s1", "indexed already"))
        .expect("first message");
    storage
        .create_message(&Message::user_text("s1", "needs backfill"))
        .expect("second message");

    // Simulate the drift: the base row survives while its index row is gone.
    // A contentless table keeps no document, so the row is selected by rowid.
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute(
            "DELETE FROM messages_fts WHERE rowid = \
             (SELECT rowid FROM messages WHERE id = \
              (SELECT message_id FROM message_fts_keys \
               WHERE message_id = (SELECT id FROM messages \
                                   ORDER BY rowid DESC LIMIT 1)))",
            [],
        )
        .expect("drop one fts row");
        conn.execute(
            "DELETE FROM message_fts_keys WHERE message_id = \
             (SELECT id FROM messages ORDER BY rowid DESC LIMIT 1)",
            [],
        )
        .expect("drop its key row");
    }

    assert_eq!(
        storage.warm_message_search_index().expect("warm"),
        1,
        "exactly the message whose index row was removed is re-indexed"
    );
    assert_eq!(
        storage.warm_message_search_index().expect("second warm"),
        0,
        "a warm run after backfill has nothing left to do"
    );

    let hits = storage
        .search_conversation("s1", "backfill", 10)
        .expect("search");
    assert_eq!(hits.len(), 1, "the backfilled row is searchable");

    drop(storage);
    let _ = std::fs::remove_file(&path);
}

/// A key row whose message has been deleted must be swept, together with its
/// index row. Left behind, the stale key makes a session-scoped search return a
/// phantom message id and re-indexes a row that no longer has a base row.
#[test]
fn warm_index_sweeps_orphaned_key_rows() {
    let path = temp_db_path("ghost.db");
    let storage = Storage::open(&path).expect("open storage");
    storage.create_session("s1", "/tmp/a").expect("session");
    let msg = Message::user_text("s1", "original text");
    storage.create_message(&msg).expect("message");

    // Delete the base row directly, leaving the index and key rows behind -
    // the shape an interrupted delete (or a crash between statements) leaves.
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute("DELETE FROM messages WHERE id = ?1", [&msg.id])
            .expect("delete the base row only");
    }

    assert_eq!(
        storage.warm_message_search_index().expect("warm"),
        1,
        "the warm-up reports the orphaned row it swept"
    );
    assert!(
        storage
            .search_conversation("s1", "original", 10)
            .expect("search")
            .is_empty(),
        "the orphaned index row no longer answers"
    );
    assert_eq!(
        storage.warm_message_search_index().expect("second warm"),
        0,
        "the sweep is idempotent"
    );

    drop(storage);
    let _ = std::fs::remove_file(&path);
}
