//! Tests for provider auto-detection logic (Section 4.D).
//!
//! Since the workspace forbids `unsafe_code` and Rust 2024 requires `unsafe`
//! for `set_var`/`remove_var`, we cannot manipulate environment variables in
//! tests.  We test the aspects of `detect_provider` that are controllable via
//! the database: preferred_provider, disabled flags, and database-stored keys.
//! We also verify the function returns *something* (or None) without panicking
//! under the ambient environment.

use std::sync::Arc;

use ragent_core::storage::Storage;
use ragent_tui::app::ProviderSource;
use ragent_tui::App;

// =========================================================================
// Helpers
// =========================================================================

fn mem_storage() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("in-memory storage"))
}

// =========================================================================
// Basic: detect_provider doesn't panic with empty storage
// =========================================================================

#[test]
fn test_detect_provider_no_panic_empty_storage() {
    let storage = mem_storage();
    // Should not panic regardless of ambient env vars.
    let _ = App::detect_provider(&storage);
}

// =========================================================================
// Database-stored preferred_provider
// =========================================================================

#[test]
fn test_detect_provider_preferred_from_db() {
    let storage = mem_storage();

    // Store a provider key so detect_provider can find it via DB.
    storage
        .set_setting("provider_anthropic_key", "sk-test-12345")
        .expect("store key");
    storage
        .set_setting("preferred_provider", "anthropic")
        .expect("store preferred");

    let result = App::detect_provider(&storage);
    assert!(result.is_some(), "should find anthropic via DB key + preferred");
    let p = result.unwrap();
    assert_eq!(p.id, "anthropic");
    assert_eq!(p.source, ProviderSource::Database);
}

#[test]
fn test_detect_provider_preferred_unknown_id_ignored() {
    let storage = mem_storage();

    // Set preferred to a non-existent provider — should be ignored gracefully.
    storage
        .set_setting("preferred_provider", "nonexistent_provider")
        .expect("store preferred");

    // Should not panic; may return None or find another provider via env.
    let result = App::detect_provider(&storage);
    if let Some(p) = &result {
        assert_ne!(
            p.id, "nonexistent_provider",
            "should never resolve a non-existent provider"
        );
    }
}

// =========================================================================
// Disabled flag via database
// =========================================================================

#[test]
fn test_detect_provider_disabled_flag_skips_provider() {
    let storage = mem_storage();

    // Store keys for both anthropic and openai.
    storage
        .set_setting("provider_anthropic_key", "sk-ant-test")
        .expect("store key");
    storage
        .set_setting("provider_openai_key", "sk-oai-test")
        .expect("store key");

    // Disable anthropic.
    storage
        .set_setting("provider_anthropic_disabled", "true")
        .expect("disable");

    let result = App::detect_provider(&storage);
    assert!(result.is_some());
    let p = result.unwrap();
    assert_ne!(
        p.id, "anthropic",
        "disabled anthropic should be skipped"
    );
}

#[test]
fn test_detect_provider_disabled_flag_any_value_disables() {
    let storage = mem_storage();

    storage
        .set_setting("provider_anthropic_key", "sk-ant-test")
        .expect("store key");
    storage
        .set_setting("preferred_provider", "anthropic")
        .expect("store preferred");
    // Current implementation: any stored value (including "false") disables the provider.
    // This is presence-based, not value-based.
    storage
        .set_setting("provider_anthropic_disabled", "false")
        .expect("disable=false");

    let result = App::detect_provider(&storage);
    // Because is_disabled checks `.is_some()`, "false" still disables.
    if let Some(p) = &result {
        assert_ne!(
            p.id, "anthropic",
            "presence-based disable means any value disables (even 'false')"
        );
    }
}

// =========================================================================
// Multiple DB keys — first in PROVIDER_LIST wins
// =========================================================================

#[test]
fn test_detect_provider_db_keys_follow_provider_list_order() {
    let storage = mem_storage();

    // Store keys for both — anthropic appears first in PROVIDER_LIST.
    storage
        .set_setting("provider_anthropic_key", "sk-ant-test")
        .expect("store anthropic key");
    storage
        .set_setting("provider_openai_key", "sk-oai-test")
        .expect("store openai key");

    let result = App::detect_provider(&storage);
    assert!(result.is_some());
    let p = result.unwrap();
    // Anthropic should win because it appears first in PROVIDER_LIST,
    // unless an env var overrides or copilot auto-discovers.
    assert!(
        p.id == "anthropic" || p.source == ProviderSource::EnvVar || p.source == ProviderSource::AutoDiscovered,
        "expected anthropic (from DB) or an env/auto-discovered provider, got: {} ({:?})",
        p.id, p.source
    );
}

// =========================================================================
// Preferred provider with no key stored still works if env has it
// =========================================================================

#[test]
fn test_detect_provider_preferred_without_db_key() {
    let storage = mem_storage();

    // Set preferred to openai but don't store a key — should fall back to env detection.
    storage
        .set_setting("preferred_provider", "openai")
        .expect("store preferred");

    let result = App::detect_provider(&storage);
    // Result depends on ambient environment — just verify no panic.
    // If OPENAI_API_KEY is set in env, it'll find openai. Otherwise whatever is available.
    let _ = result;
}

// =========================================================================
// ProviderSource variants exist and are comparable
// =========================================================================

#[test]
fn test_provider_source_equality() {
    assert_eq!(ProviderSource::EnvVar, ProviderSource::EnvVar);
    assert_eq!(ProviderSource::Database, ProviderSource::Database);
    assert_eq!(ProviderSource::AutoDiscovered, ProviderSource::AutoDiscovered);
    assert_ne!(ProviderSource::EnvVar, ProviderSource::Database);
}
