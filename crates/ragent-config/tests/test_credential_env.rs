//! Tests for the canonical credential-env resolver (audit T-111).
//!
//! `std::env::set_var` is `unsafe` in Rust 2024; this target opts back in
//! explicitly and uses unique variable names so mutation is contained.

#![allow(unsafe_code)]

use ragent_config::credential_env::{
    first_credential_env, provider_credential_env, provider_credential_env_vars,
    read_credential_env,
};

fn set_var(name: &str, value: &str) {
    // SAFETY: unique names, contained to this test binary.
    unsafe { std::env::set_var(name, value) };
}

fn remove_var(name: &str) {
    // SAFETY: unique names, contained to this test binary.
    unsafe { std::env::remove_var(name) };
}

#[test]
fn test_read_credential_env_trims_and_rejects_blank() {
    set_var("RAGENT_TEST_CRED_WS", "  token-value  ");
    assert_eq!(
        read_credential_env("RAGENT_TEST_CRED_WS").as_deref(),
        Some("token-value")
    );

    set_var("RAGENT_TEST_CRED_BLANK", "   ");
    assert!(read_credential_env("RAGENT_TEST_CRED_BLANK").is_none());

    remove_var("RAGENT_TEST_CRED_UNSET");
    assert!(read_credential_env("RAGENT_TEST_CRED_UNSET").is_none());
}

#[test]
fn test_first_credential_env_precedence_and_blank_skip() {
    set_var("RAGENT_TEST_CRED_PRIMARY", "   ");
    set_var("RAGENT_TEST_CRED_SECONDARY", "secondary-token");
    assert_eq!(
        first_credential_env(&["RAGENT_TEST_CRED_PRIMARY", "RAGENT_TEST_CRED_SECONDARY"])
            .as_deref(),
        Some("secondary-token"),
        "a present-but-blank primary must fall through to the next variable"
    );
}

#[test]
fn test_provider_credential_env_vars_known_ids() {
    assert_eq!(
        provider_credential_env_vars("anthropic"),
        &["ANTHROPIC_API_KEY"]
    );
    assert_eq!(
        provider_credential_env_vars("ollama_cloud"),
        &["OLLAMA_CLOUD_API_KEY", "OLLAMA_API_KEY"]
    );
    // Ollama local keyless provider has no env key.
    assert_eq!(provider_credential_env_vars("ollama").len(), 0);
    // Unknown providers resolve to no variables.
    assert_eq!(provider_credential_env_vars("does-not-exist").len(), 0);
}

#[test]
fn test_provider_credential_env_reads_first_non_blank() {
    set_var("RAGENT_TEST_PROV_KEY", "provider-key-123");
    // Reuse a known id shape via first_credential_env directly to avoid
    // overwriting a real provider variable.
    assert_eq!(
        first_credential_env(&["RAGENT_TEST_PROV_KEY"]).as_deref(),
        Some("provider-key-123")
    );
    // provider_credential_env on an unknown id is None.
    assert!(provider_credential_env("does-not-exist").is_none());
}
