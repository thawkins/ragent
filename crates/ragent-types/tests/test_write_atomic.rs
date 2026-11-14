//! The shared atomic write (`ragent_types::guard::write_atomic`).
//!
//! Four crates previously carried their own temp-file-plus-rename, and only one
//! of them used the hardened `create_new` form, so a pre-placed symlink at the
//! temp path was followed by the others. These tests pin the properties that
//! form exists for: the target is replaced atomically with the new bytes, no
//! temp file is left behind, the parent directory is created, the file is
//! owner-only on Unix, and a symlink planted at a temp path is refused rather
//! than followed.

use std::path::{Path, PathBuf};

use ragent_types::guard::{write_atomic, write_atomic_stream, write_atomic_with};

/// A per-test scratch directory under `target/temp` (NFR-004: tests never write
/// outside the workspace).
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("temp")
        .join("write-atomic")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn test_write_atomic_creates_the_file_and_its_parents() {
    let dir = scratch("parents");
    let target = dir.join("nested").join("deep").join("state.json");
    write_atomic(&target, b"{\"a\":1}").expect("write");
    assert_eq!(std::fs::read(&target).expect("read"), b"{\"a\":1}");
}

#[test]
fn test_write_atomic_replaces_existing_contents_whole() {
    let dir = scratch("replace");
    let target = dir.join("state.json");
    std::fs::write(
        &target,
        b"a much longer previous body that must not survive",
    )
    .expect("seed");
    write_atomic(&target, b"new").expect("write");
    assert_eq!(std::fs::read(&target).expect("read"), b"new");
}

#[test]
fn test_write_atomic_leaves_no_temp_file_behind() {
    let dir = scratch("no-temp");
    let target = dir.join("state.json");
    write_atomic(&target, b"body").expect("write");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["state.json".to_string()],
        "only the target may remain: {names:?}"
    );
}

#[test]
fn test_write_atomic_successive_writes_all_land() {
    // The temp name carries a monotonic counter, so two writes in one process
    // cannot collide on the same temp path.
    let dir = scratch("successive");
    let target = dir.join("state.json");
    for i in 0..5_u8 {
        write_atomic(&target, &[i]).expect("write");
        assert_eq!(std::fs::read(&target).expect("read"), vec![i]);
    }
}

#[cfg(unix)]
#[test]
fn test_write_atomic_creates_the_target_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;
    // The config and the credential store go through this helper, so the file
    // must never be group- or world-readable.
    let dir = scratch("owner-only");
    let target = dir.join("secret.json");
    write_atomic(&target, b"{\"token\":\"x\"}").expect("write");
    let mode = std::fs::metadata(&target)
        .expect("stat")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "expected owner-only, got {mode:o}");
}

#[cfg(unix)]
#[test]
fn test_write_atomic_refuses_a_symlink_at_the_temp_path() {
    // SEC-072: `create_new` gives `O_EXCL`, so a symlink an attacker placed at
    // a predictable temp path is refused (the first attempt falls through to a
    // different suffix) instead of being followed to write elsewhere.
    let dir = scratch("symlink");
    let target = dir.join("state.json");
    let elsewhere = dir.join("attacker-controlled");
    std::fs::write(&elsewhere, b"untouched").expect("seed victim");

    // Plant a symlink at every suffix the first attempts would use.
    let pid = std::process::id();
    for seq in 0..4 {
        let link = dir.join(format!(".state.json.{pid}.{seq}.tmp"));
        std::os::unix::fs::symlink(&elsewhere, &link).expect("plant symlink");
    }
    let before = std::fs::read(&target).is_ok();
    write_atomic(&target, b"safe").expect("write must still succeed");
    assert_eq!(
        std::fs::read(&elsewhere).expect("victim readable"),
        b"untouched",
        "the planted symlink target must never be written through"
    );
    assert_eq!(std::fs::read(&target).expect("target written"), b"safe");
    assert!(!before, "the target must not have existed before the write");
}

#[test]
fn test_write_atomic_to_a_bare_file_name_uses_the_current_directory() {
    // `Path::new("state.json")` has no parent; the write must not fail on that.
    let dir = scratch("bare-name");
    let previous = std::env::current_dir().expect("cwd");
    // Serialise against every other test that observes the process cwd.
    let _lock = crate::global_env_lock();
    std::env::set_current_dir(&dir).expect("chdir");
    let result = write_atomic(Path::new("bare.json"), b"body");
    std::env::set_current_dir(&previous).expect("restore cwd");
    result.expect("bare file name writes");
    assert_eq!(std::fs::read(dir.join("bare.json")).expect("read"), b"body");
}

#[test]
fn test_write_atomic_stream_streams_each_part_into_one_file() {
    // The `Write`-sink form exists so a caller holding a body by reference writes
    // its header and body separately through one sink, with no concatenated copy
    // of the payload in between.
    let dir = scratch("stream-sink");
    let target = dir.join("report.md");
    let body = "the borrowed body";
    write_atomic_stream(&target, |out| {
        out.write_all(b"# header\n")?;
        out.write_all(body.as_bytes())
    })
    .expect("write");
    assert_eq!(
        std::fs::read(&target).expect("read"),
        b"# header\nthe borrowed body"
    );
}

#[test]
fn test_write_atomic_stream_keeps_the_target_until_the_last_part_lands() {
    // The sink form shares the hardened path: the target only appears once every
    // part wrote and the temp was synced, so a reader never sees a partial file.
    let dir = scratch("stream-atomic");
    let target = dir.join("report.md");
    std::fs::write(&target, b"previous").expect("seed");
    write_atomic_stream(&target, |out| {
        out.write_all(b"# header\n")?;
        out.flush()
    })
    .expect("write");
    assert_eq!(std::fs::read(&target).expect("read"), b"# header\n");
}

#[test]
fn test_write_atomic_with_streams_each_part_into_one_file() {
    // The streaming form exists so a caller holding a body by reference writes
    // its header and body separately, with no concatenated copy in between.
    use std::io::Write as _;

    let dir = scratch("streamed");
    let target = dir.join("report.md");
    write_atomic_with(&target, |file| {
        file.write_all(b"# header\n")?;
        file.write_all(b"the borrowed body")
    })
    .expect("write");
    assert_eq!(
        std::fs::read(&target).expect("read"),
        b"# header\nthe borrowed body"
    );
}

#[test]
fn test_write_atomic_with_leaves_the_target_intact_when_the_writer_fails() {
    // A writer that fails part-way must behave like a failed buffered write: the
    // previous contents survive and no temp file is left behind.
    let dir = scratch("streamed-failure");
    let target = dir.join("report.md");
    std::fs::write(&target, b"previous").expect("seed");
    let err = write_atomic_with(&target, |_file| {
        Err(std::io::Error::new(
            std::io::ErrorKind::WriteZero,
            "writer gave up",
        ))
    })
    .expect_err("a failing writer must fail the write");
    assert_eq!(err.kind(), std::io::ErrorKind::WriteZero);
    assert_eq!(std::fs::read(&target).expect("read"), b"previous");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["report.md".to_string()],
        "the failed write must clean up its temp: {names:?}"
    );
}
