//! PERF-058: `reqwest::Client` must be constructed once, behind the shared
//! `OnceLock` helper, not per tool call.
//!
//! This scans the crate's own `src/` tree and fails if any module other than
//! `src/http_client.rs` constructs a `reqwest::Client` directly. The
//! `http_client` helper is the single allowed construction site (its `OnceLock`
//! initialiser, plus its fallback path if the builder fails).

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// The one module permitted to build a `reqwest::Client`.
const ALLOWED: &str = "src/http_client.rs";

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn test_no_client_new_outside_shared_helper() {
    let root = src_root();
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    assert!(
        !files.is_empty(),
        "expected to find source files under {root:?}"
    );

    let mut offenders = Vec::new();
    for file in &files {
        let rel = file
            .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        if rel == ALLOWED {
            continue;
        }
        let text = fs::read_to_string(file).unwrap_or_default();
        if text.contains("reqwest::Client::new()") {
            offenders.push(rel);
        }
    }

    assert!(
        offenders.is_empty(),
        "reqwest::Client must only be built in {ALLOWED}; offenders: {offenders:?}"
    );
}

#[test]
fn test_shared_client_is_once_cached() {
    let text = fs::read_to_string(src_root().join("http_client.rs")).unwrap();
    assert!(
        text.contains("OnceLock"),
        "the shared client must be cached in a OnceLock"
    );
    assert!(
        text.contains("Client::new()"),
        "the helper should keep a Client::new() fallback if the builder fails"
    );
}
