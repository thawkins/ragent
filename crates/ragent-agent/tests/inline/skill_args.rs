//! Tests for args.rs (M8/T8.4).
//! Compiled as a submodule via #[path], super::* resolves to the source module.

use super::*;
use std::path::PathBuf;

// --- parse_args tests ---

#[test]
fn test_parse_empty() {
    assert!(parse_args("").is_empty());
}

#[test]
fn test_parse_single_arg() {
    assert_eq!(parse_args("staging"), vec!["staging"]);
}

#[test]
fn test_parse_multiple_args() {
    assert_eq!(parse_args("a b c"), vec!["a", "b", "c"]);
}

#[test]
fn test_parse_extra_whitespace() {
    assert_eq!(parse_args("  a   b  c  "), vec!["a", "b", "c"]);
}

#[test]
fn test_parse_double_quoted() {
    assert_eq!(
        parse_args(r#""hello world" foo"#),
        vec!["hello world", "foo"]
    );
}

#[test]
fn test_parse_single_quoted() {
    assert_eq!(parse_args("'hello world' foo"), vec!["hello world", "foo"]);
}

#[test]
fn test_parse_mixed_quotes() {
    assert_eq!(
        parse_args(r#""first arg" 'second arg' third"#),
        vec!["first arg", "second arg", "third"]
    );
}

#[test]
fn test_parse_only_whitespace() {
    assert!(parse_args("   ").is_empty());
}

// --- substitute_args tests ---

#[test]
fn test_substitute_arguments_all() {
    let result = substitute_args(
        "Deploy $ARGUMENTS now",
        "staging",
        "s1",
        Path::new("/skills/deploy"),
    );
    assert_eq!(result, "Deploy staging now");
}

#[test]
fn test_substitute_arguments_multi_word() {
    let result = substitute_args(
        "Run: $ARGUMENTS",
        "staging prod",
        "s1",
        Path::new("/skills/deploy"),
    );
    assert_eq!(result, "Run: staging prod");
}

#[test]
fn test_substitute_indexed_args() {
    let result = substitute_args(
        "Env: $ARGUMENTS[0], Target: $ARGUMENTS[1]",
        "staging us-east-1",
        "s1",
        Path::new("/skills/deploy"),
    );
    assert_eq!(result, "Env: staging, Target: us-east-1");
}

#[test]
fn test_substitute_indexed_out_of_bounds() {
    let result = substitute_args(
        "Arg: $ARGUMENTS[5]",
        "only-one",
        "s1",
        Path::new("/skills/deploy"),
    );
    assert_eq!(result, "Arg: ");
}

#[test]
fn test_substitute_positional_shorthand() {
    let result = substitute_args(
        "First: $0, Second: $1",
        "alpha beta",
        "s1",
        Path::new("/skills/deploy"),
    );
    assert_eq!(result, "First: alpha, Second: beta");
}

#[test]
fn test_substitute_positional_out_of_bounds() {
    let result = substitute_args("Missing: $3", "a b", "s1", Path::new("/skills/deploy"));
    assert_eq!(result, "Missing: ");
}

#[test]
fn test_substitute_session_id() {
    let result = substitute_args(
        "Session: ${RAGENT_SESSION_ID}",
        "",
        "my-session-42",
        Path::new("/skills/deploy"),
    );
    assert_eq!(result, "Session: my-session-42");
}

#[test]
fn test_substitute_skill_dir() {
    let result = substitute_args(
        "Dir: ${RAGENT_SKILL_DIR}",
        "",
        "s1",
        Path::new("/project/.ragent/skills/deploy"),
    );
    assert_eq!(result, "Dir: /project/.ragent/skills/deploy");
}

#[test]
fn test_substitute_all_variable_types() {
    let result = substitute_args(
        "All: $ARGUMENTS, First: $0, Indexed: $ARGUMENTS[1], Session: ${RAGENT_SESSION_ID}, Dir: ${RAGENT_SKILL_DIR}",
        "foo bar",
        "sess-99",
        Path::new("/my/skills/test"),
    );
    assert_eq!(
        result,
        "All: foo bar, First: foo, Indexed: bar, Session: sess-99, Dir: /my/skills/test"
    );
}

#[test]
fn test_substitute_no_placeholders() {
    let result = substitute_args(
        "Just plain text with no variables",
        "args",
        "s1",
        Path::new("/skills"),
    );
    assert_eq!(result, "Just plain text with no variables");
}

#[test]
fn test_substitute_empty_args() {
    let result = substitute_args(
        "Deploy $ARGUMENTS here, first: $0",
        "",
        "s1",
        Path::new("/skills"),
    );
    assert_eq!(result, "Deploy  here, first: ");
}

#[test]
fn test_substitute_quoted_args() {
    let result = substitute_args(
        "Message: $0, Target: $1",
        r#""hello world" production"#,
        "s1",
        Path::new("/skills"),
    );
    assert_eq!(result, "Message: hello world, Target: production");
}

#[test]
fn test_substitute_dollar_not_variable() {
    let result = substitute_args("Price is $50 dollars", "args", "s1", Path::new("/skills"));
    // $5 matches positional $5 (out of bounds → empty), "0" stays
    // Actually $50 is parsed as index 50, which is out of bounds
    assert_eq!(result, "Price is  dollars");
}

#[test]
fn test_substitute_preserves_multiline() {
    let body = "Line 1: $0\n\nLine 3: $1\n\n## Section\n\n$ARGUMENTS";
    let result = substitute_args(body, "alpha beta", "s1", Path::new("/skills"));
    assert_eq!(
        result,
        "Line 1: alpha\n\nLine 3: beta\n\n## Section\n\nalpha beta"
    );
}

#[test]
fn test_substitute_double_digit_index() {
    let args_str = "a0 a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11";
    let result = substitute_args("Tenth: $10, Eleventh: $11", args_str, "s1", Path::new("/s"));
    assert_eq!(result, "Tenth: a10, Eleventh: a11");
}

#[test]
fn test_substitute_skill_dir_with_pathbuf() {
    let dir = PathBuf::from("/home/user/.ragent/skills/my-skill");
    let result = substitute_args("Script: ${RAGENT_SKILL_DIR}/scripts/run.sh", "", "s1", &dir);
    assert_eq!(
        result,
        "Script: /home/user/.ragent/skills/my-skill/scripts/run.sh"
    );
}
