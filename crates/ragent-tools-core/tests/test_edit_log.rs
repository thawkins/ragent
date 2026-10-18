//! Tests for edit-log outcome normalisation.
//!
//! These tests exercise the private `normalize_outcome` helper by re-importing
//! the source module via `#[path]` (see the testconsolidate migration pattern).

#[path = "../src/edit_log.rs"]
mod edit_log;

// `edit_log` delegates its JSONL append/pick helpers to `crate::jsonl_log`
// (T-305). `crate::` resolves at the consolidated test binary root, so
// `tests/all.rs` re-includes `src/jsonl_log.rs` there; a copy here would be a
// second, unused module of this child. See AGENTS.md ("Test Layout").

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
