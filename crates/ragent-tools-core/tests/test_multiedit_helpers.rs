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
    FindDiag, FindDiagKind, FindError, decode_escapes, find_exact_replacement_range,
    find_flexible_replacement_range, format_match_failure,
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

// ── collapse_whitespace: flexible matcher (opt-in) ─────────────────────────

#[test]
fn test_decode_escapes_basic() {
    assert_eq!(decode_escapes("a\\tb"), "a\tb");
    assert_eq!(decode_escapes("a\\nb"), "a\nb");
    assert_eq!(decode_escapes("a\\rb"), "a\rb");
    assert_eq!(decode_escapes("a\\\\b"), "a\\b");
    // Unknown escapes are kept verbatim (no mangling).
    assert_eq!(decode_escapes("C:\\\\path"), "C:\\path");
    assert_eq!(decode_escapes("plain"), "plain");
}

#[test]
fn test_flexible_matcher_collapses_multiple_spaces() {
    let content = "alpha   beta   gamma\n";
    // Needle has single spaces; content has runs of 3.
    let (s, e, effective) = find_flexible_replacement_range(content, "alpha beta", "X").unwrap();
    assert_eq!(&content[s..e], "alpha   beta");
    assert_eq!(effective, "X");
}

#[test]
fn test_flexible_matcher_decodes_tab_escape() {
    let content = "let x = 1;\n\tlet y = 2;\n";
    let needle = "let x = 1;\\n\\tlet y = 2;";
    let (s, e, _new) = find_flexible_replacement_range(content, needle, "R").unwrap();
    assert_eq!(&content[s..e], "let x = 1;\n\tlet y = 2;");
}

#[test]
fn test_flexible_matcher_matches_blank_line_collapse() {
    let content = "fn a() {\n\n\n    bar\n}\n";
    let needle = "fn a() {\n    bar\n}\n";
    let (s, e, _) = find_flexible_replacement_range(content, needle, "R\n").unwrap();
    assert_eq!(&content[s..e], "fn a() {\n\n\n    bar\n}\n");
}

#[test]
fn test_flexible_matcher_exact_hit_still_requires_uniqueness() {
    // Needle occurs twice byte-for-byte: must stay ambiguous even in flexible mode.
    let content = "dup and dup\n";
    let err = find_flexible_replacement_range(content, "dup", "DUP").unwrap_err();
    assert!(
        matches!(err, FindError::MultipleMatches(2)),
        "exact duplicates must remain rejected: {err:?}"
    );
}

#[test]
fn test_flexible_matcher_ambiguous_flexible_hits_rejected() {
    // Two locations that both match under whitespace-collapse → ambiguous.
    let content = "a  b\na    b\n";
    let err = find_flexible_replacement_range(content, "a b", "X").unwrap_err();
    assert!(
        matches!(err, FindError::MultipleMatches(_)),
        "two flexible hits must be rejected: {err:?}"
    );
}

#[test]
fn test_flexible_matcher_not_found() {
    let content = "fn a() { 1 }\n";
    let err = find_flexible_replacement_range(content, "totally absent", "x").unwrap_err();
    assert!(matches!(err, FindError::NotFound));
}
