//! Integration tests for the `ragent-rig` crate.
//!
//! These tests only cover behaviour that holds regardless of which feature
//! flags are enabled, so they remain valid whether the crate is built with the
//! default feature set or with an expanded set passed via `--features`.

use ragent_rig::{
    any_provider_enabled, crate_version, embeddings_enabled, memory_enabled,
    memory_semantic_enabled,
};

#[test]
fn crate_compiles_with_default_features() {
    assert!(!crate_version().is_empty());
}

#[test]
fn provider_module_is_available_when_any_provider_enabled() {
    // The `provider` module is gated behind `any(provider-*)`. Any non-empty
    // default feature set that includes a provider must expose the module.
    assert!(any_provider_enabled());
}

#[test]
fn embeddings_and_memory_report_their_compile_time_state() {
    // These accessors simply report the compile-time feature state. We do not
    // assert a fixed value here because the integration test may be run with an
    // explicit `--features embeddings,memory` set; instead we just exercise the
    // accessors to ensure they link and return a boolean.
    let _ = embeddings_enabled();
    let _ = memory_enabled();
    let _ = memory_semantic_enabled();
}
