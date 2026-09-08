#![allow(clippy::expect_used)]
//! F-1/F-4/F-5 regression tests: CRLF preservation, BOM round-trip, and the
//! precise non-UTF-8 rejection message for the edit tool family.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use ragent_tools_core::edit::EditTool;
use ragent_tools_core::multiedit::MultiEditTool;
use ragent_tools_core::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

/// Serialise the shared read-timestamp map across tests in this binary.
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

fn write_file_bytes(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn test_edit_preserves_crlf_via_flexible_lane_on_eol_mismatch() {
    let _guard = test_lock();
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "crlf2.txt", "alpha\r\nbeta\r\ngamma\r\n");
    let c = ctx(tmp.path());
    let input = json!({
        "file_path": "crlf2.txt",
        "old_string": "alpha\nbeta",
        "new_string": "alpha!\nbeta!",
    });
    let out = EditTool
        .execute(input, &c)
        .await
        .expect("flexible lane should resolve the EOL mismatch");
    assert_eq!(
        out.metadata.as_ref().unwrap()["match_lane"],
        "flexible",
        "the EOL mismatch is resolved by the flexible lane"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "alpha!\r\nbeta!\r\ngamma\r\n",
        "the matched span must stay CRLF after the edit"
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn test_edit_bom_round_trip() {
    let _guard = test_lock();
    let tmp = TempDir::new().unwrap();
    let path = write_file_bytes(
        tmp.path(),
        "bom.txt",
        b"\xef\xbb\xbffirst line\nsecond line\n",
    );
    let c = ctx(tmp.path());
    let input = json!({
        "file_path": "bom.txt",
        "old_string": "first line",
        "new_string": "first line edited",
    });
    EditTool
        .execute(input, &c)
        .await
        .expect("first-line edit on a BOM file should match after BOM strip");
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        &bytes[..3],
        b"\xef\xbb\xbf",
        "the BOM must survive the edit round-trip"
    );
    assert!(
        String::from_utf8(bytes[3..].to_vec())
            .unwrap()
            .starts_with("first line edited\n"),
        "the first line must be edited below the preserved BOM"
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn test_multiedit_bom_round_trip() {
    let _guard = test_lock();
    let tmp = TempDir::new().unwrap();
    let path = write_file_bytes(tmp.path(), "bom2.txt", b"\xef\xbb\xbfalpha\nbeta\n");
    let c = ctx(tmp.path());
    let input = json!({
        "edits": [
            {"path": "bom2.txt", "old_str": "alpha", "new_str": "alpha2"},
            {"path": "bom2.txt", "old_str": "beta", "new_str": "beta2"}
        ]
    });
    MultiEditTool
        .execute(input, &c)
        .await
        .expect("multi_edit should succeed on a BOM file");
    let bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[..3], b"\xef\xbb\xbf", "BOM must survive multi_edit");
    assert_eq!(
        String::from_utf8(bytes[3..].to_vec()).unwrap(),
        "alpha2\nbeta2\n"
    );
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn test_edit_rejects_non_utf8_with_precise_message() {
    let _guard = test_lock();
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("binary.bin");
    fs::write(&path, [0x66, 0x6f, 0x6f, 0xff, 0xfe]).expect("write invalid UTF-8");
    let c = ctx(tmp.path());
    let input = json!({
        "file_path": "binary.bin",
        "old_string": "foo",
        "new_string": "bar",
    });
    let err = EditTool
        .execute(input, &c)
        .await
        .expect_err("a non-UTF-8 file must be hard-rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("not valid UTF-8"),
        "the error must name the encoding problem: {msg}"
    );
    assert!(
        msg.contains("binary.bin"),
        "the error must carry the path: {msg}"
    );
}
