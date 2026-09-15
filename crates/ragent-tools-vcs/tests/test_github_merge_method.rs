//! Regression tests for FUNC-038: `github_merge_pr` must reject an unrecognised
//! merge method instead of silently defaulting to a real `merge`.
//!
//! The tool's `execute` path is not unit-testable without a live GitHub client,
//! so the validation logic is extracted into `parse_merge_method` and pinned
//! here.

use ragent_tools_vcs::github::parse_merge_method;

#[test]
fn test_merge_method_absent_defaults_to_merge() {
    assert_eq!(
        parse_merge_method(None).expect("absent method should default"),
        "merge"
    );
}

#[test]
fn test_merge_method_explicit_merge_ok() {
    assert_eq!(
        parse_merge_method(Some("merge")).expect("merge is valid"),
        "merge"
    );
}

#[test]
fn test_merge_method_squash_ok() {
    assert_eq!(
        parse_merge_method(Some("squash")).expect("squash is valid"),
        "squash"
    );
}

#[test]
fn test_merge_method_rebase_ok() {
    assert_eq!(
        parse_merge_method(Some("rebase")).expect("rebase is valid"),
        "rebase"
    );
}

#[test]
fn test_merge_method_unknown_rejected() {
    let err = parse_merge_method(Some("force")).expect_err("unknown method must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("Invalid merge method 'force'"),
        "error should name the bad value, got: {msg}"
    );
    assert!(
        msg.contains("merge, squash, rebase"),
        "error should list the valid values, got: {msg}"
    );
}

#[test]
fn test_merge_method_empty_string_rejected() {
    // An empty string is not a valid method and must not silently merge.
    let err = parse_merge_method(Some("")).expect_err("empty method must be rejected");
    assert!(err.to_string().contains("Invalid merge method"));
}

#[test]
fn test_merge_method_case_sensitive_rejected() {
    // GitHub's API expects lowercase; anything else is rejected rather than merged.
    let err = parse_merge_method(Some("Squash")).expect_err("wrong-case method must be rejected");
    assert!(err.to_string().contains("Invalid merge method 'Squash'"));
}
