//! F-L10: `constant_time_eq` was previously a nested fn inside
//! `auth_middleware` and therefore untestable. It is hoisted to module scope
//! and exposed for direct coverage here.

use ragent_server::routes::constant_time_eq;

#[test]
fn equal_tokens_compare_equal() {
    assert!(constant_time_eq("secret", "secret"));
    assert!(constant_time_eq("", ""));
    assert!(constant_time_eq(
        "a-much-longer-correct-token-value",
        "a-much-longer-correct-token-value"
    ));
}

#[test]
fn different_tokens_do_not_compare_equal() {
    assert!(!constant_time_eq("secret", "Secret"));
    assert!(!constant_time_eq("secret", "secrez"));
    assert!(!constant_time_eq("", "x"));
}

#[test]
fn differing_lengths_do_not_compare_equal() {
    // A same-prefix token of a different length must be rejected: the length
    // early-return was removed, so this exercises the hashed comparison.
    assert!(!constant_time_eq(
        "a-much-longer-correct-token-value",
        "a-much-longer-correct-token-valu"
    ));
    assert!(!constant_time_eq("token", "token "));
}
