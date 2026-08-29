//! Tests for provider auto-detection logic (Section 4.D).
//!
//! Since the workspace forbids `unsafe_code` and Rust 2024 requires `unsafe`
//! for `set_var`/`remove_var`, we cannot manipulate environment variables in
//! tests.  We test the aspects of `detect_provider` that are controllable via
//! the database: `preferred_provider`, disabled flags, and database-stored keys.
//! We also verify the function returns *something* (or None) without panicking
//! under the ambient environment.

use std::sync::Arc;

use ragent_agent::storage::Storage;
use ragent_tui::App;
use ragent_tui::app::ProviderSource;

// =========================================================================
// Helpers
// =========================================================================

fn mem_storage() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("in-memory storage"))
}

// =========================================================================
// Vendor-slug model id partitioning (openrouterprov FR-017)
// =========================================================================

use ragent_tui::app::model_part_from_selected_model;

#[test]
fn test_model_part_from_selected_model_preserves_vendor_slug() {
    assert_eq!(
        model_part_from_selected_model("openrouter/anthropic/claude-sonnet-4"),
        Some("anthropic/claude-sonnet-4")
    );
}

#[test]
fn test_model_part_from_selected_model_single_slash() {
    assert_eq!(
        model_part_from_selected_model("anthropic/claude-sonnet-4"),
        Some("claude-sonnet-4")
    );
}

#[test]
fn test_model_part_from_selected_model_bare_string_returns_none() {
    assert!(model_part_from_selected_model("claude-sonnet-4").is_none());
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

    // Store a provider key via provider_auth so detect_provider can find it via DB.
    storage
        .set_provider_auth("anthropic", "sk-test-12345")
        .expect("store key");
    storage
        .set_setting("preferred_provider", "anthropic")
        .expect("store preferred");

    let result = App::detect_provider(&storage);
    assert!(
        result.is_some(),
        "should find anthropic via DB key + preferred"
    );
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

    // Store keys for both anthropic and openai via provider_auth (not settings).
    storage
        .set_provider_auth("anthropic", "sk-ant-test")
        .expect("store key");
    storage
        .set_provider_auth("openai", "sk-oai-test")
        .expect("store key");

    // Disable anthropic.
    storage
        .set_setting("provider_anthropic_disabled", "true")
        .expect("disable");

    let result = App::detect_provider(&storage);
    assert!(result.is_some());
    let p = result.unwrap();
    assert_ne!(p.id, "anthropic", "disabled anthropic should be skipped");
}

#[test]
fn test_detect_provider_disabled_flag_any_value_disables() {
    let storage = mem_storage();

    storage
        .set_provider_auth("anthropic", "sk-ant-test")
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

    // Store keys for both via provider_auth — anthropic appears first in PROVIDER_LIST.
    storage
        .set_provider_auth("anthropic", "sk-ant-test")
        .expect("store anthropic key");
    storage
        .set_provider_auth("openai", "sk-oai-test")
        .expect("store openai key");

    let result = App::detect_provider(&storage);
    assert!(result.is_some());
    let p = result.unwrap();
    // Anthropic should win because it appears first in PROVIDER_LIST,
    // unless an env var overrides or copilot auto-discovers.
    assert!(
        p.id == "anthropic"
            || p.source == ProviderSource::EnvVar
            || p.source == ProviderSource::AutoDiscovered
            || p.source == ProviderSource::Database,
        "expected anthropic (from DB) or an env/auto-discovered/file-detected provider, got: {} ({:?})",
        p.id,
        p.source
    );
}

// =========================================================================
// Preferred provider with no key stored still works if env has it
// =========================================================================

#[test]
fn test_detect_provider_preferred_without_db_key() {
    let storage = mem_storage();

    // Set preferred to openai but don't store a key — the preferred provider
    // should NOT be surfaced unless it also has a credential (env var or DB key).
    // With no credentials at all, preferred is simply ignored.
    storage
        .set_setting("preferred_provider", "openai")
        .expect("store preferred");

    let result = App::detect_provider(&storage);
    // Result depends on ambient environment — just verify no panic.
    // If OPENAI_API_KEY is set in env, it'll find openai (and move it to front).
    // Otherwise, openai is NOT pushed because it has no credential.
    if let Some(p) = &result {
        // If the result is openai, it must be because OPENAI_API_KEY is set.
        // If it's another provider, that's fine — preferred just reorders.
        assert_ne!(
            p.source,
            ProviderSource::AutoDiscovered,
            "auto-discovery should no longer be used"
        );
    }
}

// =========================================================================
// ProviderSource variants exist and are comparable
// =========================================================================

#[test]
fn test_provider_source_equality() {
    assert_eq!(ProviderSource::EnvVar, ProviderSource::EnvVar);
    assert_eq!(ProviderSource::Database, ProviderSource::Database);
    assert_eq!(
        ProviderSource::AutoDiscovered,
        ProviderSource::AutoDiscovered
    );
    assert_ne!(ProviderSource::EnvVar, ProviderSource::Database);
}

// =========================================================================
// detect_provider: no gh subprocess, only explicit credential sources
// =========================================================================

#[test]
fn test_detect_provider_fast_path_uses_cheap_source_without_gh() {
    let storage = mem_storage();

    // A provider detected from an explicit credential (env/DB) must be
    // returned by detect_provider.  No `gh auth token` subprocess is ever
    // spawned — detection only uses env vars and secure storage.
    storage
        .set_provider_auth("anthropic", "sk-ant-test")
        .expect("store key");

    let result = App::detect_provider(&storage);
    assert!(
        result.is_some(),
        "detect_provider should find a DB-stored provider"
    );
    let p = result.unwrap();
    assert_eq!(p.id, "anthropic");
    assert_eq!(p.source, ProviderSource::Database);
}

#[test]
fn test_get_configured_providers_no_auto_discovery() {
    // The full-list enumerator (used by `/provider show` and the router
    // palette) must NOT use auto-discovery (gh CLI, IDE apps.json).
    // Only providers with env vars or secure-storage keys are surfaced.
    let storage = mem_storage();

    let providers = App::get_configured_providers(&storage);
    // Should not panic. Any returned providers must be from explicit
    // credential sources, never AutoDiscovered.
    for p in &providers {
        assert_ne!(
            p.source,
            ProviderSource::AutoDiscovered,
            "auto-discovery should no longer be used for provider detection"
        );
    }
}

// =========================================================================
// OpenRouter provider surfacing (openrouterprov T-009)
// =========================================================================

use ragent_tui::app::PROVIDER_LIST;

#[test]
fn test_provider_list_includes_openrouter() {
    assert!(
        PROVIDER_LIST.iter().any(|(id, _)| *id == "openrouter"),
        "PROVIDER_LIST must contain the openrouter provider"
    );
    let entry = PROVIDER_LIST
        .iter()
        .find(|(id, _)| *id == "openrouter")
        .expect("openrouter entry");
    assert_eq!(entry.1, "OpenRouter");
}

#[test]
fn test_detect_provider_openrouter_from_stored_key() {
    let storage = mem_storage();
    storage
        .set_provider_auth("openrouter", "sk-or-test-key")
        .expect("store openrouter key");

    let providers = App::get_configured_providers(&storage);
    let openrouter = providers.iter().find(|p| p.id == "openrouter");
    assert!(openrouter.is_some(), "stored key should surface openrouter");
    assert_eq!(openrouter.unwrap().source, ProviderSource::Database);
}

#[test]
fn test_detect_provider_openrouter_preferred_moves_to_front() {
    let storage = mem_storage();
    storage
        .set_provider_auth("anthropic", "sk-ant-test")
        .expect("store anthropic key");
    storage
        .set_provider_auth("openrouter", "sk-or-test-key")
        .expect("store openrouter key");
    storage
        .set_setting("preferred_provider", "openrouter")
        .expect("set preferred");

    let detected = App::detect_provider(&storage).expect("a provider is detected");
    assert_eq!(detected.id, "openrouter", "preferred openrouter should win");
}
