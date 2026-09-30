//! Stderr-spool unit tests relocated out of the inline `#[cfg(test)]` block
//! in `src/stderr_spool/spool.rs` (ANTIPAT M2.5/F1). `Spool`, `SPOOL_MAX_LINES`
//! and `SPOOL_MAX_BYTES` are public API.

use std::sync::{Arc, Mutex};

use ragent_types::stderr_spool::{SPOOL_MAX_BYTES, SPOOL_MAX_LINES, Spool};

#[test]
fn spool_truncates_to_the_newest_lines() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stderr.log");
    let spool = Spool::new(path.clone());

    // Write 1200 lines through the spool in two batches.
    for i in 0..700 {
        spool.write(format!("line-{i}\n").as_bytes());
    }
    for i in 700..1200 {
        spool.write(format!("line-{i}\n").as_bytes());
    }

    let content = std::fs::read_to_string(&path).expect("read spool");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), SPOOL_MAX_LINES);
    // The newest lines survive, the oldest are gone.
    assert_eq!(lines[lines.len() - 1], "line-1199");
    assert_eq!(lines[0], format!("line-{}", 1200 - SPOOL_MAX_LINES));
}

#[test]
fn spool_counts_newlines_from_existing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stderr.log");
    // Seed a file already at the cap.
    let seeded: String = (0..SPOOL_MAX_LINES).map(|i| format!("old-{i}\n")).collect();
    std::fs::write(&path, seeded).expect("seed");

    let spool = Spool::new(path.clone());
    spool.write(b"fresh\n");

    let content = std::fs::read_to_string(&path).expect("read spool");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), SPOOL_MAX_LINES);
    assert_eq!(lines[lines.len() - 1], "fresh");
}

#[test]
fn spool_appends_a_newline_free_write_under_the_cap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stderr.log");
    std::fs::write(&path, "existing\n").expect("seed");
    let spool = Spool::new(path.clone());

    spool.write(b"partial output without newline");

    let content = std::fs::read_to_string(&path).expect("read spool");
    assert_eq!(content, "existing\npartial output without newline");
}

#[test]
fn spool_bounds_a_newline_free_stream() {
    // SEC-ragent-types-002 (SECTASKS T-058): a producer that never emits a
    // newline used to grow the file without bound because the line cap was
    // only consulted when `added > 0`.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stderr.log");
    let spool = Spool::new(path.clone());

    let chunk = "x".repeat(64 * 1024);
    for _ in 0..40 {
        spool.write(chunk.as_bytes());
    }

    let len = std::fs::metadata(&path).expect("stat spool").len() as usize;
    assert!(
        len <= SPOOL_MAX_BYTES + chunk.len(),
        "newline-free spool grew to {len} bytes (cap {SPOOL_MAX_BYTES})"
    );
}

#[test]
fn spool_mirrors_each_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stderr.log");
    let spool = Spool::new(path);
    let seen: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&seen);
    spool.set_mirror(Arc::new(move |s: &str| {
        if let Ok(mut acc) = sink.lock() {
            acc.push_str(s);
        }
    }));

    spool.write(b"hello ");
    spool.write(b"world\n");

    assert_eq!(seen.lock().expect("lock").as_str(), "hello world\n");
}
