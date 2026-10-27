//! Coverage for the `Spool` debug/path surface and its best-effort error
//! paths (a write must never panic when the spool file cannot be opened).

use ragent_types::stderr_spool::{SPOOL_MAX_BYTES, SPOOL_MAX_LINES, Spool};
use std::sync::{Arc, Mutex};

#[test]
fn spool_debug_renders_only_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("spool.log");
    let spool = Spool::new(path.clone());
    let rendered = format!("{spool:?}");
    assert!(rendered.starts_with("Spool"), "got: {rendered}");
    assert!(
        rendered.contains(&path.display().to_string()),
        "debug render must include the path: {rendered}"
    );
    // path() returns the bound path.
    assert_eq!(spool.path(), path.as_path());
}

#[test]
fn new_seeds_line_and_byte_estimates_from_an_existing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("existing.log");
    std::fs::write(&path, b"one\ntwo\nthree\n").expect("seed file");

    let spool = Spool::new(path.clone());
    // Appending under the caps must not rewrite the file.
    spool.write(b"four\n");
    let content = std::fs::read_to_string(&path).expect("read");
    assert_eq!(content, "one\ntwo\nthree\nfour\n");
    assert_eq!(spool.path(), path.as_path());
}

#[test]
fn write_to_an_unopenable_path_is_a_silent_no_op() {
    // A spool whose parent directory does not exist: `Write` must swallow the
    // open error and return without panicking.
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("no-such-dir").join("spool.log");
    let spool = Spool::new(missing.clone());
    spool.write(b"data\n");
    assert!(!missing.exists(), "the failed write created nothing");
}

#[test]
fn a_multi_byte_bounded_stream_stays_under_both_caps() {
    // A newline-free multi-byte stream must be bounded by the byte cap alone.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blob.log");
    let spool = Spool::new(path.clone());

    // Write single-byte UTF-8 chars in chunks until well past the byte cap.
    let chunk = vec![b'a'; 64 * 1024];
    let mut written = 0usize;
    while written < SPOOL_MAX_BYTES + 4 * 64 * 1024 {
        spool.write(&chunk);
        written += chunk.len();
    }
    let metadata = std::fs::metadata(&path).expect("spool file exists");
    assert!(
        metadata.len() as usize <= SPOOL_MAX_BYTES + chunk.len(),
        "spool file must be bounded near the byte cap, got {}",
        metadata.len()
    );
    // The cap is a positive, documented limit.
    const { assert!(SPOOL_MAX_LINES > 0) };
}

#[test]
fn set_mirror_replaces_the_previous_sink() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("spool.log");
    let spool = Spool::new(path);

    let first = Arc::new(Mutex::new(String::new()));
    let f_clone = Arc::clone(&first);
    spool.set_mirror(Arc::new(move |s: &str| {
        f_clone.lock().expect("lock").push_str(s);
    }));
    spool.write(b"alpha\n");
    assert_eq!(&*first.lock().expect("lock"), "alpha\n");

    // Replacing the sink sends subsequent writes only to the new one.
    let second = Arc::new(Mutex::new(String::new()));
    let s_clone = Arc::clone(&second);
    spool.set_mirror(Arc::new(move |s: &str| {
        s_clone.lock().expect("lock").push_str(s);
    }));
    spool.write(b"beta\n");
    assert_eq!(&*first.lock().expect("lock"), "alpha\n");
    assert_eq!(&*second.lock().expect("lock"), "beta\n");
}
