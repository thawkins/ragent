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
    CascadeFail, CascadeMatch, FindDiag, FindDiagKind, FindError, MatchLane, decode_escapes,
    disambiguation_hint, find_exact_replacement_range, find_flexible_replacement_range,
    find_replacement_cascade, format_match_failure, nearest_window, not_found_hint,
};
use std::path::{Path, PathBuf};

#[test]
fn not_found_hint_no_near_miss_falls_back_to_plain_message() {
    let content = "one\ntwo\nthree\n";
    let needle = "alpha\nbeta\ngamma\n";
    let hint = not_found_hint(content, needle, Path::new("f.rs"), None, false);
    assert!(hint.contains("old_string not found"), "got: {hint}");
    assert!(hint.contains("f.rs"), "path should be named, got: {hint}");
    assert!(!hint.contains("Edit"), "no edit index, got: {hint}");
    assert!(
        !hint.contains("almost matches"),
        "no near-miss, got: {hint}"
    );
}

#[test]
fn not_found_hint_includes_near_miss_and_collapse_suffix() {
    let content = "line a\nline b\nline c\nline d\nline e\n";
    let needle = "line b\nline c\nline d\n";
    let hint = not_found_hint(content, needle, Path::new("f.rs"), Some(2), true);
    assert!(hint.starts_with("Edit 2: "), "got: {hint}");
    assert!(
        hint.contains("almost matches a block starting at line 2"),
        "got: {hint}"
    );
    assert!(hint.contains("line b"), "snippet included, got: {hint}");
    assert!(hint.contains("collapse_whitespace mode"), "got: {hint}");
}

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

// ── Match cascade (editplan P2) ──────────────────────────────────────────────

#[test]
fn cascade_exact_lane_wins_when_present() {
    let content = "fn a() { 1 }\n";
    match find_replacement_cascade(content, "fn a() { 1 }", "fn a() { 2 }") {
        CascadeMatch::Found {
            lane,
            start,
            end,
            new_str,
            ..
        } => {
            assert_eq!(lane, MatchLane::Exact);
            assert_eq!(&content[start..end], "fn a() { 1 }");
            assert_eq!(new_str, "fn a() { 2 }");
        }
        other => panic!("expected exact Found, got {other:?}"),
    }
}

#[test]
fn cascade_flexible_lane_rescues_whitespace_mismatch() {
    // Two-space gap in needle vs three-space gap in content: exact fails,
    // flexible succeeds.
    let content = "alpha   beta\n";
    match find_replacement_cascade(content, "alpha beta", "X") {
        CascadeMatch::Found {
            lane, start, end, ..
        } => {
            assert_eq!(lane, MatchLane::Flexible);
            assert_eq!(&content[start..end], "alpha   beta");
        }
        other => panic!("expected flexible Found, got {other:?}"),
    }
}

#[test]
fn cascade_indent_normalised_lane_rescues_trailing_newline_ghost() {
    // Needle identical to the file block except it adds a trailing "\n" that
    // the file content does not have (content ends after the final "}").
    // Exact fails; flexible fails because the trailing whitespace run in the
    // needle needs a non-empty run in the content (absent at EOF); only the
    // indent-normalised lane can rescue it.
    let content = "fn do_work() {\n    work();\n}";
    let needle = "fn do_work() {\n    work();\n}\n";
    match find_replacement_cascade(content, needle, "fn do_work() {\n    patched();\n}\n") {
        CascadeMatch::Found {
            lane,
            start,
            end,
            new_str,
        } => {
            assert_eq!(lane, MatchLane::IndentNormalised);
            assert_eq!(&content[start..end], "fn do_work() {\n    work();\n}");
            assert_eq!(new_str, "fn do_work() {\n    patched();\n}\n");
        }
        other => panic!("expected indent_normalised Found, got {other:?}"),
    }
}

#[test]
fn cascade_not_found_when_needle_is_absent() {
    let content = "fn a() { 1 }\n";
    match find_replacement_cascade(content, "totally absent text", "x") {
        CascadeMatch::Failed(CascadeFail::NotFound) => {}
        other => panic!("expected Failed(NotFound), got {other:?}"),
    }
}

#[test]
fn cascade_multiple_matches_reported_with_offsets() {
    let content = "dup\nmid\ndup\n";
    match find_replacement_cascade(content, "dup", "DUP") {
        CascadeMatch::Failed(CascadeFail::MultipleMatches {
            count,
            starts,
            lane,
        }) => {
            assert_eq!(count, 2);
            assert_eq!(lane, MatchLane::Exact);
            assert_eq!(starts, vec![0, 8]);
        }
        other => panic!("expected Failed(MultipleMatches), got {other:?}"),
    }
}

#[test]
fn nearest_window_finds_close_block() {
    let content = "line a\nline b\nline c\nline d\nline e\n";
    let needle = "line b\nline cX\nline d\n";
    // All three lines exist in content, but only "line b" and "line d" match
    // verbatim. That's 2/3 ≈ 67 %, below the 75 % threshold for a hint.
    assert!(nearest_window(content, needle).is_none());

    let needle2 = "line b\nline c\nline d\n";
    // Exact match — nearest_window is not needed, but the helper should still
    // find a 100 % window when called (callers only invoke it on NotFound).
    let (line, _n, matched, total, snippet) =
        nearest_window(content, needle2).expect("100 % match should hint");
    assert_eq!(line, 2);
    assert_eq!(matched, 3);
    assert_eq!(total, 3);
    assert!(snippet.contains("line b"));
}

#[test]
fn nearest_window_below_threshold_returns_none() {
    let content = "one\ntwo\nthree\nfour\nfive\n";
    let needle = "one\nXYZ\nPDQ\nCBA\n";
    // Only 1/4 lines match (25 %).
    assert!(nearest_window(content, needle).is_none());
}

#[test]
fn disambiguation_hint_lists_offsets_and_lines() {
    let content = "foo = 1;\nbar();\nfoo = 2;\n";
    let starts: Vec<usize> = content.match_indices("foo").map(|(i, _)| i).collect();
    let hint = disambiguation_hint(content, "foo", &starts);
    assert!(
        hint.contains("offset 0"),
        "should list first offset: {hint}"
    );
    assert!(
        hint.contains("offset 16"),
        "should list second offset: {hint}"
    );
    assert!(hint.contains("line 1"), "should name line numbers: {hint}");
    assert!(hint.contains("line 3"), "should name line numbers: {hint}");
    assert!(hint.contains("foo = 1;"), "should show context: {hint}");
    assert!(hint.contains("foo = 2;"), "should show context: {hint}");
}

#[test]
fn disambiguation_hint_numbers_context_lines_for_multiline_needle() {
    let content = "fn a() {\n    x = 1;\n}\nfn b() {\n    x = 2;\n}\n";
    let needle = "    x = ?;\n}";
    let starts: Vec<usize> = content
        .match_indices("    x = 1;\n}")
        .map(|(i, _)| i)
        .collect();
    let hint = disambiguation_hint(content, needle, &starts);
    // The new hint is line-numbered and includes surrounding function names.
    assert!(
        hint.contains("   1 | fn a() {"),
        "should show function context: {hint}"
    );
    assert!(
        hint.contains("   2 |     x = 1;"),
        "should show matched line: {hint}"
    );
    assert!(
        hint.contains("Match candidates (add unique context from one of these blocks)"),
        "should invite unique context: {hint}"
    );
}
