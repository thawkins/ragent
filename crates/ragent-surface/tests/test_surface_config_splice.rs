//! Coverage for `ragent-surface::config_splice::write_atomic_json`, the shared
//! atomic JSON-config write the `/plugins` and `/connectors` marketplace
//! families delegate to.
//!
//! The happy path round-trips a value through a real file, asserts the
//! owner-only mode and that no temp file is left behind, and the error paths
//! cover a parent that cannot be created and a target that cannot be replaced.

use std::fs;
use std::path::Path;

use ragent_surface::config_splice::write_atomic_json;
use serde_json::json;

fn workdir(tag: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("ragent-surface-splice-{tag}-"))
        .tempdir()
        .expect("tempdir")
}

#[test]
fn write_atomic_json_round_trips_a_value() {
    let tmp = workdir("ok");
    let target = tmp.path().join("ragent.json");
    let value = json!({"plugins": {"enabled": true}, "n": 7});
    write_atomic_json(&target, &value).expect("write succeeds");

    let text = fs::read_to_string(&target).expect("read back");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    assert_eq!(parsed, value);
    // Pretty-printed output, so the file is multi-line.
    assert!(text.contains('\n'));
}

#[test]
fn write_atomic_json_creates_the_parent_directory() {
    let tmp = workdir("mkdir");
    let target = tmp.path().join("nested/deeper/ragent.json");
    write_atomic_json(&target, &json!({"a": 1})).expect("write succeeds");
    assert!(target.is_file());
}

#[test]
fn write_atomic_json_leaves_no_temp_file_behind() {
    let tmp = workdir("clean");
    let target = tmp.path().join("ragent.json");
    write_atomic_json(&target, &json!({"a": 1})).expect("write succeeds");
    let leftovers: Vec<_> = fs::read_dir(tmp.path())
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp."))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
}

#[cfg(unix)]
#[test]
fn write_atomic_json_writes_an_owner_only_file() {
    use std::os::unix::fs::PermissionsExt as _;
    let tmp = workdir("mode");
    let target = tmp.path().join("ragent.json");
    write_atomic_json(&target, &json!({"a": 1})).expect("write succeeds");
    let mode = fs::metadata(&target)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn write_atomic_json_fails_when_the_parent_cannot_be_created() {
    let tmp = workdir("badparent");
    // A regular file where a directory is needed: create_dir_all must fail.
    let blocker = tmp.path().join("blocker");
    fs::write(&blocker, b"not a dir").expect("write blocker");
    let target = blocker.join("ragent.json");
    let err = write_atomic_json(&target, &json!({"a": 1})).expect_err("must fail");
    assert!(err.starts_with("cannot create"), "unexpected error: {err}");
}

#[test]
fn write_atomic_json_fails_when_the_target_cannot_be_replaced() {
    let tmp = workdir("replace");
    // The target path is a directory, so the final rename must fail.
    let target = tmp.path().join("target-dir");
    fs::create_dir_all(&target).expect("mkdir target");
    let err = write_atomic_json(&target, &json!({"a": 1})).expect_err("must fail");
    assert!(err.starts_with("cannot replace"), "unexpected error: {err}");
    // The original directory survives and no temp file lingers.
    assert!(target.is_dir());
    let leftovers: Vec<_> = fs::read_dir(tmp.path())
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp."))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
}

#[test]
fn write_atomic_json_overwrites_an_existing_file() {
    let tmp = workdir("overwrite");
    let target = tmp.path().join("ragent.json");
    fs::write(&target, b"{\"old\": true}").expect("seed file");
    write_atomic_json(&target, &json!({"new": true})).expect("write succeeds");
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).expect("read")).expect("json");
    assert_eq!(parsed, json!({"new": true}));
    assert_eq!(parsed.get("old"), None);
}

/// A path with no parent component writes without attempting a mkdir.
#[test]
fn write_atomic_json_handles_a_bare_filename() {
    let tmp = workdir("bare");
    let target = tmp.path().join("bare.json");
    write_atomic_json(&target, &json!({"k": "v"})).expect("write succeeds");
    assert!(Path::new(&target).is_file());
}
