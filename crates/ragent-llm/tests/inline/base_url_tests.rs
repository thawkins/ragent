#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Inline tests for `base_url::validate_base_url`.
//! Compiled as a submodule via `#[path]`, `super::*` resolves to `base_url`.

use super::*;

#[test]
fn test_accepts_http_and_https() {
    assert_eq!(
        validate_base_url("generic_openai", "http://127.0.0.1:8080").unwrap(),
        "http://127.0.0.1:8080"
    );
    assert_eq!(
        validate_base_url("generic_openai", "https://api.example.com").unwrap(),
        "https://api.example.com"
    );
}

#[test]
fn test_strips_trailing_slash() {
    assert_eq!(
        validate_base_url("generic_openai", "https://api.example.com/").unwrap(),
        "https://api.example.com"
    );
}

#[test]
fn test_rejects_non_http_scheme() {
    assert!(validate_base_url("x", "file:///etc/passwd").is_err());
    assert!(validate_base_url("x", "ftp://host").is_err());
    assert!(validate_base_url("x", "gopher://host").is_err());
}

#[test]
fn test_rejects_missing_scheme_and_host() {
    assert!(validate_base_url("x", "example.com").is_err());
    assert!(validate_base_url("x", "https://").is_err());
    assert!(validate_base_url("x", "http:///path").is_err());
}

#[test]
fn test_rejects_empty_and_whitespace() {
    assert!(validate_base_url("x", "").is_err());
    assert!(validate_base_url("x", "   ").is_err());
    assert!(validate_base_url("x", "http://a b").is_err());
}
