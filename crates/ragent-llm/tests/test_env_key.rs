//! Tests for the single provider -> API-key environment resolver (audit T-403).
//!
//! `std::env::set_var` is `unsafe` in Rust 2024; this target opts back in
//! explicitly and mutates only provider variables this file owns (guarded by a
//! mutex so the table and value assertions cannot race within the binary).

#![allow(unsafe_code)]

use ragent_llm::provider::env_key::{provider_env_key, provider_env_key_vars};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

#[tokio::test]
async fn test_provider_env_key_vars_table() {
    assert_eq!(provider_env_key_vars("anthropic"), &["ANTHROPIC_API_KEY"]);
    assert_eq!(
        provider_env_key_vars("generic_openai"),
        &["GENERIC_OPENAI_API_KEY", "OPENAI_API_KEY"]
    );
    assert_eq!(
        provider_env_key_vars("ollama_cloud"),
        &["OLLAMA_CLOUD_API_KEY", "OLLAMA_API_KEY"]
    );
    // The resolver adds the one key the shared credential table omits.
    assert_eq!(provider_env_key_vars("ollama"), &["OLLAMA_API_KEY"]);
    // Unknown ids and credential-chain providers resolve to no variables.
    assert_eq!(provider_env_key_vars("does-not-exist"), &[] as &[&str]);
    assert_eq!(provider_env_key_vars("bedrock"), &[] as &[&str]);
}

#[tokio::test]
async fn test_provider_env_key_reads_and_skips_blank() {
    let _guard = ENV_LOCK.lock().await;

    // SAFETY: exclusive access to OLLAMA_API_KEY while ENV_LOCK is held.
    unsafe { std::env::remove_var("OLLAMA_API_KEY") };
    assert!(provider_env_key("ollama").is_none());

    // SAFETY: exclusive access to OLLAMA_API_KEY while ENV_LOCK is held.
    unsafe { std::env::set_var("OLLAMA_API_KEY", "  some-key  ") };
    assert_eq!(provider_env_key("ollama").as_deref(), Some("some-key"));

    // SAFETY: exclusive access to OLLAMA_API_KEY while ENV_LOCK is held.
    unsafe { std::env::set_var("OLLAMA_API_KEY", "   ") };
    assert!(
        provider_env_key("ollama").is_none(),
        "a present-but-blank value must resolve to None"
    );

    // SAFETY: exclusive access to OLLAMA_API_KEY while ENV_LOCK is held.
    unsafe { std::env::remove_var("OLLAMA_API_KEY") };
}
