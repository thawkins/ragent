//! Tests for test_sanitize.rs

use ragent_core::sanitize::redact_secrets;

// ── sk- keys ─────────────────────────────────────────────────────

#[test]
fn test_redact_sk_key() {
    let input = "My API key is sk-abcdefghijklmnopqrstuvwxyz1234";
    let _result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("sk-abcdef"));
}

#[test]
fn test_redact_long_sk_key() {
    let input = "token: sk-1234567890abcdefghijklmnopqrstuvwxyzABCDEF";
    let _result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("sk-1234"));
}

// ── key- keys ────────────────────────────────────────────────────

#[test]
fn test_redact_key_prefix() {
    let input = "Using key-abcdefghijklmnopqrstuvwxyz for auth";
    let _result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("key-abcdef"));
}

// ── Bearer tokens ────────────────────────────────────────────────

#[test]
fn test_redact_bearer_token() {
    let input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz-1234";
    let _result = redact_secrets(input);
    assert!(result.contains("[REDACTED]"));
    assert!(!result.contains("Bearer abcdef"));
}

// ── Non-secret text ──────────────────────────────────────────────

#[test]
fn test_no_redaction_for_normal_text() {
    let input = "Hello, this is a normal message with no secrets.";
    let _result = redact_secrets(input);
    assert_eq!(result, input);
}

#[test]
fn test_no_redaction_for_short_sk() {
    // sk- followed by less than 20 chars should not be redacted
    let input = "sk-short";
    let _result = redact_secrets(input);
    assert_eq!(result, input);
}

// ── Multiple secrets in one string ───────────────────────────────

#[test]
fn test_redact_multiple_secrets() {
    let input = "key1=sk-aaaaaaaaaabbbbbbbbbbcccccc key2=key-ddddddddddeeeeeeeeeefffff";
    let _result = redact_secrets(input);
    assert!(!result.contains("sk-aaa"));
    assert!(!result.contains("key-ddd"));
    // Should have two [REDACTED] tokens
    assert_eq!(result.matches("[REDACTED]").count(), 2);
}

// ── Secret at start and end of string ────────────────────────────

#[test]
fn test_redact_at_boundaries() {
    let start = "sk-abcdefghijklmnopqrstuvwxyz is my key";
    let _result = redact_secrets(start);
    assert!(result.starts_with("[REDACTED]"));

    let end = "my key is sk-abcdefghijklmnopqrstuvwxyz";
    let _result = redact_secrets(end);
    assert!(result.ends_with("[REDACTED]"));
}

// ── Preserves surrounding text ───────────────────────────────────

#[test]
fn test_redact_preserves_context() {
    let input =
        "Error: authentication failed with token sk-abcdefghijklmnopqrstuvwxyz. Please retry.";
    let _result = redact_secrets(input);
    assert!(result.contains("Error: authentication failed with token"));
    assert!(result.contains(". Please retry."));
    assert!(result.contains("[REDACTED]"));
}
