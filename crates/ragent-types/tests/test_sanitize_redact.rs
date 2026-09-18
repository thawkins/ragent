//! Tests for the `Cow`-returning secret redactor (PERF-055).
//!
//! The secret registry is process-wide, so the tests that mutate it serialise
//! on a local mutex and clear it before and after.

use std::borrow::Cow;
use std::sync::Mutex;

use ragent_types::sanitize::{
    clear_secret_registry, redact_secrets, redact_secrets_cow, register_secret, unregister_secret,
};

/// Serialises tests that touch the global registry.
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn cow_borrows_clean_payload_without_allocating() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    let clean = redact_secrets_cow("tool output with no credentials in it");
    assert!(
        matches!(clean, Cow::Borrowed(_)),
        "clean payload must borrow"
    );
    assert_eq!(clean, "tool output with no credentials in it");
    assert_eq!(redact_secrets("still clean"), "still clean");
}

#[test]
fn cow_owns_when_regex_pattern_matches() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    let input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz";
    let redacted = redact_secrets_cow(input);
    assert!(matches!(redacted, Cow::Owned(_)));
    assert_eq!(redacted, "Authorization: [REDACTED]");
    assert!(!redacted.contains("abcdefghijklmnopqrstuvwxyz"));
}

#[test]
fn registered_secret_is_redacted_and_unregister_restores_borrow() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    register_secret("perf055-custom-secret-value");
    let input = "the token is perf055-custom-secret-value here";
    let redacted = redact_secrets_cow(input);
    assert!(matches!(redacted, Cow::Owned(_)));
    assert_eq!(redacted, "the token is [REDACTED] here");

    // The owned path must agree with the `String` wrapper.
    assert_eq!(redact_secrets(input), "the token is [REDACTED] here");

    unregister_secret("perf055-custom-secret-value");
    assert!(matches!(redact_secrets_cow(input), Cow::Borrowed(_)));

    clear_secret_registry();
}

#[test]
fn longest_secret_is_replaced_first() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    // The short secret is a substring of the long one; the long secret must be
    // replaced as a unit rather than leaving a partial match behind.
    register_secret("shortsecret");
    register_secret("shortsecret-value-with-suffix");
    let out = redact_secrets("x shortsecret-value-with-suffix y");
    assert_eq!(out, "x [REDACTED] y");

    clear_secret_registry();
}

#[test]
fn json_shaped_token_redacts_value_and_preserves_key() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    let input = r#"{"token": "abcdefghijklmnop"}"#;
    let redacted = redact_secrets(input);
    assert_eq!(redacted, r#"{"token": "[REDACTED]"}"#);
    assert!(!redacted.contains("abcdefghijklmnop"));

    clear_secret_registry();
}

#[test]
fn base64_secret_with_slash_plus_equals_redacts_fully() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    let input = "token = abcdefghijklmnop+/xyz=";
    let redacted = redact_secrets(input);
    assert!(
        !redacted.contains("abcdefghijklmnop"),
        "prefix redacted, got: {redacted}"
    );
    assert!(
        !redacted.contains("+/xyz="),
        "tail also redacted, got: {redacted}"
    );

    clear_secret_registry();
}
