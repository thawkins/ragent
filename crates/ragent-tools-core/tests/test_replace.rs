//! Integration tests for `ragent-tools-core` fuzzy replacement matching.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/replace.rs`
//! (T-008 of the testconsolidate spec). The tests exercise public fns and
//! one `pub(crate)` helper (`common_leading_ws`). The source module is
//! re-imported via `#[path]` (FR-008); `pub(crate)` items are visible because
//! the `#[path]` module becomes part of the test crate. `replace.rs` has no
//! `super::` or `crate::` dependencies, so no shims are needed.

// The full `replace` source module is re-imported via `#[path]` (FR-008) so
// that `pub(crate)` helpers (e.g. `common_leading_ws`) are visible. Only a
// subset of the module's public surface is exercised here; the remaining
// public items (`format_match_failure`, `find_batch_normalized_replacement_range`,
// and the `pass`/`closest_line` fields) are used by the library's `edit` and
// `multiedit` modules and by `test_multiedit_helpers`. They are dead within
// this compilation unit only, so we silence the lint.
#[allow(dead_code)]
#[path = "../src/replace.rs"]
mod replace;

use replace::{
    FindError, byte_offset_of_line, common_leading_ws, find_exact_replacement_range,
    find_replacement_range,
};

fn check(content: &str, needle: &str) -> (usize, usize) {
    let (s, e, _) = find_replacement_range(content, needle, "").expect("should find match");
    (s, e)
}

fn check_with_new(content: &str, needle: &str, new_str: &str) -> (usize, usize, String) {
    find_replacement_range(content, needle, new_str).expect("should find match")
}

#[test]
fn exact_match() {
    let c = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, "    bar\n");
    assert_eq!(&c[s..e], "    bar\n");
}

#[test]
fn crlf_normalised_match() {
    let c = "fn foo() {\r\n    bar\r\n}\r\n";
    let needle = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\r\n    bar\r\n}\r\n");
}

#[test]
fn trailing_whitespace_match() {
    let c = "fn foo() {  \n    bar  \n}\n";
    let needle = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {  \n    bar  \n}\n");
}

#[test]
fn trailing_whitespace_and_crlf() {
    let c = "fn foo() {  \r\n    bar  \r\n}\r\n";
    let needle = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], c);
}

#[test]
fn exact_replacement_helper_unique_match() {
    let c = "fn foo() {\n    bar\n}\n";
    let (s, e, effective) = find_exact_replacement_range(c, "    bar\n", "    baz\n").unwrap();
    assert_eq!(&c[s..e], "    bar\n");
    assert_eq!(effective, "    baz\n");
}

#[test]
fn exact_replacement_helper_not_found() {
    let c = "hello world\n";
    assert!(matches!(
        find_exact_replacement_range(c, "goodbye", ""),
        Err(FindError::NotFound)
    ));
}

#[test]
fn exact_replacement_helper_multiple_matches() {
    let c = "foo\nfoo\n";
    assert!(matches!(
        find_exact_replacement_range(c, "foo", ""),
        Err(FindError::MultipleMatches(2))
    ));
}

#[test]
fn exact_replacement_helper_whitespace_sensitive() {
    let c = "fn foo() {\n    bar\n}\n";
    assert!(matches!(
        find_exact_replacement_range(c, "     bar\n", ""),
        Err(FindError::NotFound)
    ));
}

#[test]
fn exact_replacement_helper_empty_needle() {
    let c = "hello\n";
    assert!(matches!(
        find_exact_replacement_range(c, "", ""),
        Err(FindError::MultipleMatches(_))
    ));
}

#[test]
fn multiple_matches_returns_err() {
    let c = "foo\nfoo\n";
    assert!(matches!(
        find_replacement_range(c, "foo", ""),
        Err(FindError::MultipleMatches(2))
    ));
}

#[test]
fn byte_offset_of_line_basic() {
    let s = "a\nb\nc\n";
    assert_eq!(byte_offset_of_line(s, 0), 0);
    assert_eq!(byte_offset_of_line(s, 1), 2);
    assert_eq!(byte_offset_of_line(s, 2), 4);
    assert_eq!(byte_offset_of_line(s, 3), 6);
    assert_eq!(byte_offset_of_line(s, 99), 6);
}

#[test]
fn leading_whitespace_stripped_match() {
    let c = "fn setup() {\n    registry.register(A);\n    registry.register(B);\n}\n";
    let needle = "registry.register(A);\nregistry.register(B);\n";
    let new_str = "registry.register(A);\nregistry.register(C);\n";
    let (s, e, effective) = check_with_new(c, needle, new_str);
    assert_eq!(
        &c[s..e],
        "    registry.register(A);\n    registry.register(B);\n"
    );
    assert_eq!(
        effective,
        "    registry.register(A);\n    registry.register(C);\n"
    );
}

#[test]
fn leading_whitespace_match_preserves_relative_indent() {
    let c = "    fn foo() {\n        let x = 1;\n    }\n";
    let needle = "fn foo() {\n    let x = 1;\n}\n";
    let new_str = "fn foo() {\n    let x = 2;\n}\n";
    let (s, e, effective) = check_with_new(c, needle, new_str);
    assert_eq!(&c[s..e], "    fn foo() {\n        let x = 1;\n    }\n");
    assert_eq!(effective, "    fn foo() {\n        let x = 2;\n    }\n");
}

#[test]
fn collapsed_whitespace_match() {
    let c = "\tlet  x  =  1;\n\tlet  y  =  2;\n";
    let needle = "let x = 1;\nlet y = 2;\n";
    let new_str = "let x = 1;\nlet y = 99;\n";
    let (s, e, effective) = check_with_new(c, needle, new_str);
    assert_eq!(&c[s..e], "\tlet  x  =  1;\n\tlet  y  =  2;\n");
    assert_eq!(effective, "\tlet x = 1;\n\tlet y = 99;\n");
}

#[test]
fn blank_line_leading_in_needle() {
    let c = "fn foo() {\n    bar\n}\n";
    let needle = "\nfn foo() {\n    bar\n}\n";
    let new_str = "fn foo() {\n    baz\n}\n";
    let (s, e, effective) = check_with_new(c, needle, new_str);
    assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
    assert_eq!(effective, new_str);
}

#[test]
fn blank_line_trailing_in_needle() {
    let c = "fn foo() {\n    bar\n}\n";
    let needle = "fn foo() {\n    bar\n}\n\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
}

#[test]
fn blank_line_leading_in_file() {
    let c = "\nfn foo() {\n    bar\n}\n";
    let needle = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
}

#[test]
fn blank_line_trailing_in_file() {
    let c = "fn foo() {\n    bar\n}\n\n";
    let needle = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
}

#[test]
fn blank_line_both_edges_differ() {
    let c = "\nfn foo() {\n    bar\n}\n\n";
    let needle = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
}

#[test]
fn final_newline_file_has_needle_lacks() {
    let c = "fn foo() {\n    bar\n}\n";
    let needle = "fn foo() {\n    bar\n}";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\n    bar\n}");
}

#[test]
fn final_newline_needle_has_file_lacks() {
    let c = "fn foo() {\n    bar\n}";
    let needle = "fn foo() {\n    bar\n}\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\n    bar\n}");
}

#[test]
fn final_newline_crlf_disagreement() {
    let c = "fn foo() {\r\n    bar\r\n}\r\n";
    let needle = "fn foo() {\n    bar\n}";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "fn foo() {\r\n    bar\r\n}");
}

#[test]
fn collapsed_disambiguates_by_whitespace_proximity() {
    let c = "        let  x = 1;\n    let  x = 1;\n";
    let needle = "let  x = 1;\n";
    let (s, e) = check(c, needle);
    assert_eq!(&c[s..e], "    let  x = 1;\n");
}

#[test]
fn collapsed_tie_still_errors() {
    let c = "    let  x = 1;\n    let  x = 1;\n";
    let needle = "let  x = 1;\n";
    assert!(matches!(
        find_replacement_range(c, needle, ""),
        Err(FindError::MultipleMatches(_))
    ));
}

#[test]
fn reindent_preserves_relative_indentation_nested() {
    let c = "    fn foo() {\n        let x = 1;\n    }\n";
    let needle = "fn foo() {\n    let x = 1;\n}\n";
    let new_str = "fn foo() {\n    let x = 2;\n}\n";
    let (s, e, effective) = check_with_new(c, needle, new_str);
    assert_eq!(&c[s..e], "    fn foo() {\n        let x = 1;\n    }\n");
    assert_eq!(effective, "    fn foo() {\n        let x = 2;\n    }\n");
}

#[test]
fn reindent_tab_vs_space_preserves_relative() {
    let c = "\tfn foo() {\n\t\tlet x = 1;\n\t}\n";
    let needle = "fn foo() {\n    let x = 1;\n}\n";
    let new_str = "fn foo() {\n    let x = 2;\n}\n";
    let (s, e, effective) = check_with_new(c, needle, new_str);
    assert_eq!(&c[s..e], "\tfn foo() {\n\t\tlet x = 1;\n\t}\n");
    assert_eq!(effective, "\tfn foo() {\n\t    let x = 2;\n\t}\n");
}

#[test]
fn reindent_blank_lines_left_untouched() {
    let c = "    fn foo() {\n\n    }\n";
    let needle = "fn foo() {\n\n}\n";
    let new_str = "fn foo() {\n\n}\n";
    let (s, e, effective) = check_with_new(c, needle, new_str);
    assert_eq!(&c[s..e], "    fn foo() {\n\n    }\n");
    assert_eq!(effective, "    fn foo() {\n\n    }\n");
}

#[test]
fn common_leading_ws_handles_mixed_indent() {
    let lines = vec!["    foo", "  bar"];
    assert_eq!(common_leading_ws(&lines), "  ");
    let lines = vec!["    foo", "", "    bar"];
    assert_eq!(common_leading_ws(&lines), "    ");
    let lines = vec!["", "  ", ""];
    assert_eq!(common_leading_ws(&lines), "");
}
