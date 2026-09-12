#![allow(clippy::assert_is_empty)]
//! Behavioural smoke test for EDITPLAN T-14: strict exact-byte `edit` matching.
//!
//! Drives `ragent-tools-core` directly (the same code path that `ragent run` uses):
//!
//!  1. successful exact edit applies `new_string` verbatim,
//!  2. whitespace-mismatched `old_string` is rescued by the fallback cascade
//!     (metadata records which lane matched),
//!  3. stale-file rejection (FR-003) still fires after an external touch,
//!     and the P1.3 retry-once behaviour refreshes the read timestamp.

#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use ragent_tools_core::edit::EditTool;
use ragent_tools_core::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

/// Serialise all tests in this binary — the shared context holds a
/// global timestamp map and parallel runs would race on it.
fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn ctx(working_dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![working_dir.to_path_buf()],
    }
}

fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

/// Simulate "the file was read by the session" so the FR-003 stale check is
/// armed, as it would be in a real `ragent run` session.
fn record_read(c: &ToolContext, path: &Path) {
    let mut map = c.read_timestamps.write().unwrap();
    let millis = fs::metadata(path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    map.insert(path.to_path_buf(), millis);
}

/// Serialise this binary's test run: the guard is held across `await` points
/// intentionally so no second test (if any are added later) can race the
/// shared read-timestamp map while this one is mid-edit.
#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn t14_smoke_exact_mismatch_and_stale_behaviour() {
    let _guard = test_lock();
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "smoke.txt", "alpha\nbeta\ngamma\n");
    let c = ctx(tmp.path());

    // (a) Successful exact edit — applies new_string verbatim.
    record_read(&c, &path);
    let out = EditTool
        .execute(
            json!({
                "file_path": "smoke.txt",
                "old_string": "beta",
                "new_string": "BETA_REPLACED",
            }),
            &c,
        )
        .await
        .expect("exact-byte edit must succeed");
    assert!(
        out.content.contains("BETA_REPLACED"),
        "tool output shows the edit snippet: {}",
        out.content
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "alpha\nBETA_REPLACED\ngamma\n",
        "edit applies new_string verbatim"
    );

    // (b) Whitespace-mismatched old_string — FR-045 line-structure guard: a
    // needle ending in a trailing space (no further content) previously
    // folded the file's trailing newline into the whitespace run and spliced
    // new_str with a JOINED line ("nopegamma"). The guard now rejects the
    // fold, so the edit fails cleanly with a not-found error instead of
    // corrupting the file's line structure.
    record_read(&c, &path);
    let err = EditTool
        .execute(
            json!({
                "file_path": "smoke.txt",
                "old_string": "BETA_REPLACED ",
                "new_string": "nope",
            }),
            &c,
        )
        .await
        .expect_err("needle-space vs file-newline fold must be rejected (FR-045)");
    assert!(
        format!("{err}").contains("old_string not found"),
        "rejection should be a not-found error: {err}"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "alpha\nBETA_REPLACED\ngamma\n",
        "the failed edit must not touch the file"
    );

    // (c) Stale-file rejection (FR-003) still fires after an external touch.
    // Arm the check with a read timestamp in the past, then bump the mtime to
    // the future to simulate an external write.
    {
        let mut map = c.read_timestamps.write().unwrap();
        map.insert(
            path.clone(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                - 5_000,
        );
    }
    let future = SystemTime::now() + std::time::Duration::from_secs(10);
    let _ = filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future));

    let err = EditTool
        .execute(
            json!({
                "file_path": "smoke.txt",
                "old_string": "BETA_REPLACED",
                "new_string": "GAMMA",
            }),
            &c,
        )
        .await
        .expect_err("stale file must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("modified after") || msg.contains("stale"),
        "error should report stale file: {msg}"
    );
    // P1.3: after a stale rejection the read timestamp was refreshed, so the
    // exact same retry now succeeds without a manual re-read.
    EditTool
        .execute(
            json!({
                "file_path": "smoke.txt",
                "old_string": "BETA_REPLACED",
                "new_string": "GAMMA",
            }),
            &c,
        )
        .await
        .expect("retry after stale rejection should succeed (timestamp refreshed)");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "alpha\nGAMMA\ngamma\n",
        "retry applies the replacement"
    );
}
