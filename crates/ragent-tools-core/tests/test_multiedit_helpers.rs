//! Integration tests for `ragent-tools-core` replace-matcher helpers.
//!
//! Following EDITPLAN Milestone 1/2, batch resolution calls
//! `find_exact_replacement_range` directly (the two-pass `resolve_batch_edit`
//! helper was deleted). This file now provides the helper-level coverage for
//! the strict exact-byte matcher and the slimmed failure diagnostics:
//!
//! - `find_exact_replacement_range` — exact match, not-found, multiple-matches
//! - slimmed `FindDiag` constructors (`not_found()` / `multiple(n)`)
//! - `format_match_failure` — actionable re-read + context hints, exact
//!   byte-for-byte wording, path mention.
//!
//! The `#[path]` re-import of `multiedit.rs` (previously needed to reach the
//! private `resolve_batch_edit`) is no longer required and has been removed
//! together with the shims.

use ragent_tools_core::path_util::resolve_path;
use ragent_tools_core::replace::{
    FindDiag, FindDiagKind, FindError, find_exact_replacement_range, format_match_failure,
};
use std::path::{Path, PathBuf};

#[test]
fn resolve_path_relative() {
    let p = resolve_path(Path::new("/work"), "src/main.rs");
    assert_eq!(p, PathBuf::from("/work/src/main.rs"));
}

#[test]
fn resolve_path_absolute() {
    let p = resolve_path(Path::new("/work"), "/etc/hosts");
    assert_eq!(p, PathBuf::from("/etc/hosts"));
}

#[test]
fn find_exact_replacement_range_exact_match() {
    let content = "fn a() { 1 }\nfn b() { 2 }\n";
    let (s, e, effective) =
        find_exact_replacement_range(content, "fn a() { 1 }", "fn a() { 10 }").unwrap();
    assert_eq!(&content[s..e], "fn a() { 1 }");
    assert_eq!(effective, "fn a() { 10 }");
}

#[test]
fn find_exact_replacement_range_rejects_crlf_and_trailing_space_mismatch() {
    let content = "fn a() {  \r\n    bar  \r\n}\r\n";
    let needle = "fn a() {\n    bar\n}\n";
    let err = find_exact_replacement_range(content, needle, "fn a() {\n    baz\n}\n").unwrap_err();
    assert!(
        matches!(err, FindError::NotFound),
        "strict exact matcher must reject CRLF/trailing-whitespace mismatch: {err:?}"
    );
}

#[test]
fn find_exact_replacement_range_not_found() {
    let content = "fn a() { 1 }\n";
    let err = find_exact_replacement_range(content, "nonexistent", "x").unwrap_err();
    assert!(matches!(err, FindError::NotFound));
}

#[test]
fn find_exact_replacement_range_multiple_matches() {
    let content = "dup\nmid\ndup\n";
    let err = find_exact_replacement_range(content, "dup", "DUP").unwrap_err();
    assert!(matches!(err, FindError::MultipleMatches(2)));
}

#[test]
fn find_diag_constructors_carry_kind_only() {
    let nf = FindDiag::not_found();
    assert!(matches!(nf.kind, FindDiagKind::NotFound));

    let mm = FindDiag::multiple(3);
    assert!(matches!(mm.kind, FindDiagKind::MultipleMatches(3)));
}

#[test]
fn format_match_failure_not_found_is_actionable() {
    let msg = format_match_failure(&FindDiag::not_found(), Path::new("/tmp/some.rs"));
    assert!(msg.contains("/tmp/some.rs"), "should name the path: {msg}");
    assert!(msg.contains("not found"), "should say not found: {msg}");
    assert!(
        msg.contains("byte-for-byte"),
        "should demand exact match: {msg}"
    );
    assert!(
        msg.contains("Re-read the file"),
        "should include the re-read hint: {msg}"
    );
}

#[test]
fn format_match_failure_multiple_matches_asks_for_more_context() {
    let msg = format_match_failure(&FindDiag::multiple(2), Path::new("/tmp/dup.rs"));
    assert!(msg.contains("2 times"), "should report match count: {msg}");
    assert!(msg.contains("/tmp/dup.rs"), "should name the path: {msg}");
    assert!(
        msg.contains("exactly once"),
        "should demand a unique match: {msg}"
    );
    assert!(
        msg.contains("more surrounding context"),
        "should ask for more context: {msg}"
    );
}
