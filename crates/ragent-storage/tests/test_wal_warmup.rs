//! Regression tests for the startup FTS warm-up lock contention (PERF).
//!
//! The message-search FTS index is rebuilt at startup on a *separate*
//! `Storage` connection in the background.  With the default
//! `journal_mode=delete`, that writer's open transaction serialises every
//! concurrent reader behind it, stalling `get_setting`/`detect_provider`
//! on the main thread for seconds.
//!
//! `Storage::open` now sets `journal_mode=WAL` + `busy_timeout`, so a
//! background writer must NOT block a concurrent `get_setting` read on a
//! second connection.  These tests assert that WAL mode is active on both
//! the main and warm-up connections and that a reader proceeds promptly
//! while the warm-up writer holds an open transaction.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ragent_storage::Storage;
use ragent_types::message::Message;

/// Returns a fresh temp file path for a file-backed database.
fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("ragent-wal-test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    path
}

#[test]
fn open_sets_wal_journal_mode() {
    // A file-backed `Storage::open` must switch the database to WAL mode so
    // a background writer never serialises concurrent readers behind it.
    let path = temp_db_path("wal-mode.db");
    let storage = Storage::open(&path).expect("open storage");
    let journal_mode = storage.journal_mode().expect("read journal_mode");
    assert_eq!(
        journal_mode.to_lowercase(),
        "wal",
        "Storage::open should enable journal_mode=WAL"
    );
    drop(storage);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn background_warmup_does_not_block_reader() {
    // Reproduces the startup scenario: a separate connection rebuilds the
    // FTS index (the warm-up) inside a transaction while the main connection
    // performs a `get_setting` read.  With WAL the reader must complete
    // promptly instead of stalling for the whole rebuild.
    let path = temp_db_path("warmup-contention.db");

    // Main connection.
    let main = Arc::new(Storage::open(&path).expect("open main storage"));
    main.create_session("sess-cont", "/tmp/cont")
        .expect("create session");

    // Insert enough messages that a rebuild takes a measurable amount of time.
    for i in 0..2_000 {
        let msg = Message::user_text(
            "sess-cont",
            format!("warm-up message number {i} with some text content"),
        );
        main.create_message(&msg).expect("create message");
    }
    // Seed a setting so `get_setting` has a row to find.
    main.set_setting("preferred_provider", "anthropic")
        .expect("set setting");

    // Background warm-up on a SEPARATE connection (as main.rs does).
    let path_warm = path.clone();
    let warmup = std::thread::spawn(move || {
        let warm = Storage::open(&path_warm).expect("open warm storage");
        warm.warm_message_search_index().expect("warm index")
    });

    // Give the warm-up writer a head start to acquire its write lock.
    std::thread::sleep(Duration::from_millis(50));

    // The reader must complete promptly (< 500ms) rather than stalling until
    // the warm-up transaction commits.
    let t0 = Instant::now();
    let val = main.get_setting("preferred_provider").expect("get setting");
    let elapsed = t0.elapsed();

    assert_eq!(val.as_deref(), Some("anthropic"), "reader sees the setting");
    assert!(
        elapsed < Duration::from_millis(500),
        "get_setting blocked for {elapsed:?} behind the background warm-up writer (WAL should prevent this)"
    );

    let count = warmup.join().expect("warm-up joined");
    // PERF-013: `create_message` maintains `messages_fts` incrementally, so a
    // warm-up on an already-synced database is a fast no-op and must report
    // zero missing rows rather than re-indexing everything.
    assert_eq!(
        count, 0,
        "warm-up should find zero missing rows (FTS kept in sync)"
    );

    drop(main);
    let _ = std::fs::remove_file(&path);
}

/// `Storage::open` must install `busy_timeout` **before** it runs
/// `PRAGMA journal_mode=WAL`.
///
/// Switching a rollback-journal database to WAL requires a brief exclusive lock
/// to rewrite the file header, and that pragma honours whatever busy handler is
/// installed when it runs. With the timeout set afterwards it used rusqlite's
/// 5 s default, so a second ragent process holding a read lock on the shared
/// `~/.config/ragent/ragent.db` made `Storage::open` fail outright with
/// `Error code 5: database is locked` instead of waiting. This test holds a read
/// transaction on a `journal_mode=delete` database from a second connection and
/// requires `Storage::open` to succeed anyway.
#[test]
fn open_waits_out_a_contended_wal_switch() {
    let path = temp_db_path("busy-timeout-order.db");

    // A peer connection holds a SHARED read lock on a rollback-journal database,
    // which is what blocks the header rewrite that `journal_mode=WAL` performs.
    let peer = rusqlite::Connection::open(&path).expect("open peer connection");
    peer.pragma_update(None, "journal_mode", "DELETE")
        .expect("force rollback journal");
    peer.execute_batch("CREATE TABLE peer_marker (a INTEGER);")
        .expect("create marker table");
    peer.execute_batch("BEGIN;")
        .expect("begin peer read transaction");
    let held: i64 = peer
        .query_row("SELECT COUNT(*) FROM peer_marker", [], |row| row.get(0))
        .expect("hold a SHARED lock");
    assert_eq!(held, 0, "the peer table starts empty");

    // Hold the lock for longer than rusqlite's built-in 5 s default but well
    // inside `BUSY_TIMEOUT_MS` (30 s). With the correct pragma order `open`
    // waits the peer out and succeeds; with the old order the WAL switch burned
    // the 5 s default and failed regardless of how long the caller was willing
    // to wait.
    const PEER_HOLD: Duration = Duration::from_millis(7_000);
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(PEER_HOLD);
        peer.execute_batch("COMMIT;").expect("commit peer");
    });

    let opened = Storage::open(&path);
    assert!(
        opened.is_ok(),
        "Storage::open must wait a contended WAL switch past rusqlite's 5 s \
         default, got: {:?}",
        opened.err()
    );
    drop(opened);

    releaser.join().expect("releaser joined");

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}

/// Returns the current on-disk size of a database's `-wal` sidecar.
fn wal_size(path: &std::path::Path) -> u64 {
    std::fs::metadata(path.with_extension("db-wal")).map_or(0, |m| m.len())
}

/// `Storage::open` must set `journal_size_limit` so the WAL file cannot keep
/// the high-water mark of a large write burst forever.
///
/// `wal_autocheckpoint` only decides *when* a checkpoint runs; it never
/// truncates the WAL back down. With the SQLite default of
/// `journal_size_limit=-1` a single large burst (a big history import, a bulk
/// compaction write) left a 670 MB WAL that stayed at 670 MB for the rest of
/// the database's life - the observed production symptom on a 2 GB
/// `ragent.db`. A bounded `journal_size_limit` lets a later checkpoint shrink
/// the file back to the bound once the burst is over.
///
/// This test writes a burst larger than the bound, lets an auto-checkpoint run,
/// then requires the WAL to have come back down. It fails with the old
/// `journal_size_limit=-1`.
#[test]
fn wal_is_capped_after_a_large_write_burst() {
    let path = temp_db_path("wal-size-limit.db");
    let storage = Storage::open(&path).expect("open storage");

    // `journal_size_limit` is written to the database header by `open`; it
    // must be a positive bound. -1 means "no limit" (the old behaviour).
    let limit: i64 = {
        let conn = storage.conn_lock_for_test().expect("lock connection");
        conn.query_row("PRAGMA journal_size_limit", [], |row| row.get(0))
            .expect("read journal_size_limit")
    };
    assert!(
        limit > 0,
        "Storage::open must set a positive journal_size_limit (got {limit}); \
         -1 lets the WAL keep the high-water mark of every write burst forever"
    );

    // Write comfortably more than the bound so a checkpoint has real slack to
    // reclaim. One large transaction keeps the WAL pinned near its peak until
    // it commits.
    let burst = limit as usize * 3;
    {
        let conn = storage.conn_lock_for_test().expect("lock connection");
        conn.execute_batch("CREATE TABLE wal_burst (blob BLOB);")
            .expect("create burst table");
        let tx = conn.unchecked_transaction().expect("begin burst");
        {
            let mut stmt = tx
                .prepare("INSERT INTO wal_burst (blob) VALUES (?1)")
                .expect("prepare insert");
            let chunk = vec![0x5au8; 1 << 20];
            let mut written = 0usize;
            while written < burst {
                stmt.execute([&chunk[..]]).expect("insert chunk");
                written += chunk.len();
            }
        }
        tx.commit().expect("commit burst");
    }

    let peak = wal_size(&path);
    assert!(
        peak > limit as u64,
        "test precondition: the burst ({peak} bytes) must exceed the limit \
         ({limit} bytes) for the cap to be observable"
    );

    // Ordinary commits after the burst, so an auto-checkpoint runs once the
    // burst transaction is no longer the newest one. With `journal_size_limit`
    // set, that checkpoint truncates the WAL back to the bound.
    for _ in 0..8 {
        let conn = storage.conn_lock_for_test().expect("lock connection");
        conn.execute("INSERT INTO wal_burst (blob) VALUES (?1)", [&b"x"[..]])
            .expect("small write");
    }

    let settled = wal_size(&path);
    assert!(
        settled <= limit as u64,
        "the WAL must be truncated back to journal_size_limit ({limit} bytes) \
         after the burst, but it is still {settled} bytes (peak {peak})"
    );

    // Data written during the burst survives the truncation.
    let rows: i64 = {
        let conn = storage.conn_lock_for_test().expect("lock connection");
        conn.query_row("SELECT COUNT(*) FROM wal_burst", [], |row| row.get(0))
            .expect("count rows")
    };
    assert!(rows > 3, "burst rows must survive the WAL truncation");

    drop(storage);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
}
