//! Regression tests for the PERF-069 dedicated read-only connection.
//!
//! Before PERF-069 every read and write shared a single `Mutex<Connection>`,
//! so a reader (and even an *unrelated* reader) could not proceed while a write
//! transaction was open.  A dedicated read-only connection — combined with the
//! WAL journal already enabled by `Storage::open` — lets reads complete
//! concurrently with an open write transaction.
//!
//! These tests pin three contracts:
//! 1. A file-backed `Storage` opens a distinct reader connection.
//! 2. A read on the main handle completes promptly while a write transaction is
//!    held open on the same handle.
//! 3. A read observes a committed write (the reader is not a stale snapshot).

use std::time::{Duration, Instant};

use ragent_storage::Storage;
use ragent_types::message::Message;

/// Returns a fresh temp file path for a file-backed database.
fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("ragent-reader-test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    path
}

#[test]
fn file_backed_storage_opens_a_reader() {
    let path = temp_db_path("reader-present.db");
    let storage = Storage::open(&path).expect("open storage");
    assert!(
        storage.has_reader(),
        "file-backed Storage::open should open a dedicated read-only connection"
    );
    drop(storage);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn in_memory_storage_has_no_reader() {
    // An in-memory database cannot be shared with a second connection, so
    // reads must fall back to the writer connection.
    let storage = Storage::open_in_memory().expect("open in-memory storage");
    assert!(!storage.has_reader());
}

#[test]
fn read_completes_while_write_transaction_is_open() {
    let path = temp_db_path("reader-during-write.db");
    let storage = Storage::open(&path).expect("open storage");
    storage
        .create_session("sess-rw", "/tmp/rw")
        .expect("session");
    storage.set_setting("k", "v").expect("setting");

    // Hold an open write transaction on the writer connection.
    {
        let conn = storage.conn_lock_for_test().expect("lock writer");
        conn.execute_batch("BEGIN IMMEDIATE").expect("begin");
        conn.execute(
            "INSERT INTO messages (id, session_id, role, parts, created_at, updated_at) \
             VALUES ('m-open', 'sess-rw', 'user', '[]', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("insert inside open tx");

        // A read on the *same* handle must not block behind the open write
        // transaction: with a dedicated reader connection it proceeds.
        let t0 = Instant::now();
        let val = storage.get_setting("k").expect("read setting");
        let elapsed = t0.elapsed();
        assert_eq!(val.as_deref(), Some("v"), "reader sees the setting");
        assert!(
            elapsed < Duration::from_millis(500),
            "read blocked for {elapsed:?} behind an open write transaction"
        );

        conn.execute_batch("ROLLBACK").expect("rollback");
    }

    drop(storage);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn reader_observes_committed_writes() {
    let path = temp_db_path("reader-observes.db");
    let storage = Storage::open(&path).expect("open storage");
    storage
        .create_session("sess-obs", "/tmp/obs")
        .expect("session");

    // The reader must see rows committed after it was opened (WAL readers
    // refresh to the latest committed snapshot).
    for i in 0..5 {
        let msg = Message::user_text("sess-obs", format!("message {i}"));
        storage.create_message(&msg).expect("create message");
    }
    let messages = storage.get_messages("sess-obs").expect("read messages");
    assert_eq!(messages.len(), 5, "reader observes all committed messages");

    drop(storage);
    let _ = std::fs::remove_file(&path);
}
