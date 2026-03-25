//! Tests for test_skill_args.rs

//! Phase 10.2: External tests for argument substitution.
//!
//! Covers edge cases in `$ARGUMENTS`, `$ARGUMENTS[N]`, `$N`,
//! and `${RAGENT_*}` variable replacement.

use ragent_core::skill::args::{parse_args, substitute_args};
use std::path::Path;

// ── parse_args edge cases ────────────────────────────────────────

#[test]
fn test_parse_args_unclosed_double_quote() {
    let args = parse_args(r#"hello "world"#);
    // Unclosed quote — implementation-defined, just don't panic
    assert!(!args.is_empty());
}

#[test]
fn test_parse_args_unclosed_single_quote() {
    let args = parse_args("hello 'world");
    assert!(!args.is_empty());
}

#[test]
fn test_parse_args_escaped_content_in_quotes() {
    let args = parse_args(r#""hello world" "foo bar""#);
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "hello world");
    assert_eq!(args[1], "foo bar");
}

#[test]
fn test_parse_args_adjacent_quotes() {
    let args = parse_args(r#""a""b""#);
    // Two adjacent quoted strings without space — varies by impl
    assert!(!args.is_empty());
}

#[test]
fn test_parse_args_many_args() {
    let input = (0..20)
        .map(|i| format!("arg{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let args = parse_args(&input);
    assert_eq!(args.len(), 20);
    assert_eq!(args[0], "arg0");
    assert_eq!(args[19], "arg19");
}

#[test]
fn test_parse_args_tabs_as_separators() {
    let args = parse_args("hello\tworld\tfoo");
    assert_eq!(args.len(), 3);
}

// ── substitute_args: $ARGUMENTS ──────────────────────────────────

#[test]
fn test_substitute_arguments_full_string() {
    let _result = substitute_args("Run: $ARGUMENTS", "fix the bug", "sess-1", Path::new("/sk"));
    assert_eq!(result, "Run: fix the bug");
}

#[test]
fn test_substitute_arguments_multiple_occurrences() {
    let _result = substitute_args(
        "First: $ARGUMENTS\nSecond: $ARGUMENTS",
        "hello",
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "First: hello\nSecond: hello");
}

#[test]
fn test_substitute_arguments_empty() {
    let _result = substitute_args("Run: $ARGUMENTS end", "", "sess-1", Path::new("/sk"));
    assert_eq!(result, "Run:  end");
}

// ── substitute_args: $ARGUMENTS[N] ──────────────────────────────

#[test]
fn test_substitute_indexed_zero() {
    let _result = substitute_args(
        "File: $ARGUMENTS[0]",
        "main.rs lib.rs",
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "File: main.rs");
}

#[test]
fn test_substitute_indexed_second() {
    let _result = substitute_args(
        "Target: $ARGUMENTS[1]",
        "main.rs lib.rs",
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "Target: lib.rs");
}

#[test]
fn test_substitute_indexed_out_of_bounds_empty() {
    let _result = substitute_args(
        "Missing: $ARGUMENTS[5]",
        "only-one",
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "Missing: ");
}

// ── substitute_args: $N shorthand ────────────────────────────────

#[test]
fn test_substitute_positional_one() {
    // $N is 0-indexed: $1 = second argument
    let _result = substitute_args("Second: $1", "alpha beta", "sess-1", Path::new("/sk"));
    assert_eq!(result, "Second: beta");
}

#[test]
fn test_substitute_positional_two() {
    let _result = substitute_args("Third: $2", "alpha beta gamma", "sess-1", Path::new("/sk"));
    assert_eq!(result, "Third: gamma");
}

#[test]
fn test_substitute_positional_out_of_bounds() {
    let _result = substitute_args("Missing: $9", "only-one", "sess-1", Path::new("/sk"));
    assert_eq!(result, "Missing: ");
}

#[test]
fn test_substitute_positional_double_digit() {
    let args_str = (0..12)
        .map(|i| format!("a{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let _result = substitute_args("Eleventh: $10", &args_str, "sess-1", Path::new("/sk"));
    assert_eq!(result, "Eleventh: a10");
}

// ── substitute_args: ${RAGENT_*} env vars ────────────────────────

#[test]
fn test_substitute_ragent_session_id() {
    let _result = substitute_args(
        "Session: ${RAGENT_SESSION_ID}",
        "",
        "my-session-42",
        Path::new("/sk"),
    );
    assert_eq!(result, "Session: my-session-42");
}

#[test]
fn test_substitute_ragent_skill_dir() {
    let _result = substitute_args(
        "Dir: ${RAGENT_SKILL_DIR}",
        "",
        "sess-1",
        Path::new("/home/user/.ragent/skills/deploy"),
    );
    assert_eq!(result, "Dir: /home/user/.ragent/skills/deploy");
}

#[test]
fn test_substitute_all_types_combined() {
    // $0 is 0-indexed first arg, $ARGUMENTS[0] is also first arg
    let _result = substitute_args(
        "All: $ARGUMENTS | $0 | $ARGUMENTS[0] | ${RAGENT_SESSION_ID} | ${RAGENT_SKILL_DIR}",
        "hello world",
        "sess-99",
        Path::new("/skills/test"),
    );
    assert_eq!(
        result,
        "All: hello world | hello | hello | sess-99 | /skills/test"
    );
}

// ── substitute_args: edge cases ──────────────────────────────────

#[test]
fn test_substitute_preserves_dollar_sign_without_digit() {
    // $N with digits is positional, but $X with non-digit is preserved
    let _result = substitute_args(
        "Cost: $USD and $ARGUMENTS",
        "items",
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "Cost: $USD and items");
}

#[test]
fn test_substitute_preserves_unrecognized_braces() {
    let _result = substitute_args("Path: ${HOME}/data", "", "sess-1", Path::new("/sk"));
    // ${HOME} is not a RAGENT_ variable, should be preserved
    assert_eq!(result, "Path: ${HOME}/data");
}

#[test]
fn test_substitute_multiline_body() {
    // $0 = first arg, $1 = second arg (0-indexed)
    let _result = substitute_args(
        "Line 1: $0\nLine 2: $1\nAll: $ARGUMENTS\n",
        "foo bar",
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "Line 1: foo\nLine 2: bar\nAll: foo bar\n");
}

#[test]
fn test_substitute_quoted_args_passed_through() {
    // $0 = first arg "main.rs", $1 = second arg "hello world"
    let _result = substitute_args(
        "File: $0 Msg: $1",
        r#"main.rs "hello world""#,
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "File: main.rs Msg: hello world");
}

#[test]
fn test_substitute_no_args_no_placeholders() {
    let _result = substitute_args(
        "Static body with no variables",
        "",
        "sess-1",
        Path::new("/sk"),
    );
    assert_eq!(result, "Static body with no variables");
}

#[test]
fn test_substitute_empty_body() {
    let _result = substitute_args("", "ignored", "sess-1", Path::new("/sk"));
    assert_eq!(result, "");
}
