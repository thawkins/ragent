//! Tests for edit-log outcome normalisation.
//!
//! These tests exercise the private `normalize_outcome` helper by re-importing
//! the source module via `#[path]` (see the testconsolidate migration pattern).

#[path = "../src/edit_log.rs"]
mod edit_log;

// `edit_log` now delegates its JSONL append/pick helpers to the crate's
// `jsonl_log` module (T-305); re-include it here so `crate::jsonl_log`
// resolves when `edit_log.rs` is compiled as part of this test target.
#[path = "../src/jsonl_log.rs"]
mod jsonl_log;

use edit_log::normalize_outcome;

#[test]
fn normalize_outcome_collapses_paths() {
    assert_eq!(normalize_outcome("not found"), "not found");
    assert_eq!(
        normalize_outcome("old exact text not found in /work/src/main.rs"),
        "old exact text not found in <file>"
    );
    assert_eq!(
        normalize_outcome("stale-file rejected: File src/foo.rs was modified"),
        "stale-file rejected: File <file> was modified"
    );
    assert_eq!(
        normalize_outcome("create rejected: file already exists"),
        "create rejected: file already exists"
    );
}
