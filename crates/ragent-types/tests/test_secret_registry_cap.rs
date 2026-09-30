//! Tests for the F14 secret-registry bounding and de-duplication.
//!
//! The secret registry is process-wide, so tests here serialise on a local
//! mutex and clear it before and after. Cargo runs each test file as its own
//! binary, so the registry these tests touch is not shared with other test
//! binaries.

use std::sync::Mutex;

use ragent_types::sanitize::{
    MAX_SECRET_REGISTRY_ENTRIES, clear_secret_registry, redact_secrets, register_secret,
    seed_secrets, unregister_secret,
};

/// Serialises tests that touch the global registry.
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

/// A fixed-length value that the secret regex does not match, so exact-match
/// redaction is the only layer that can mask it.
fn synthetic_secret(index: usize) -> String {
    format!("snt-{index:06}-padpadpadpadpad")
}

#[test]
fn seed_secrets_deduplicates_identical_values() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    // Seeding the same value twice must store it once, so a single
    // `unregister_secret` removes every copy.
    seed_secrets(vec![
        "f14-dup-value".to_string(),
        "f14-dup-value".to_string(),
    ]);
    let input = "token f14-dup-value here";
    assert!(!redact_secrets(input).contains("f14-dup-value"));

    unregister_secret("f14-dup-value");
    assert_eq!(
        redact_secrets(input),
        input,
        "a duplicate copy survived a single unregister"
    );

    clear_secret_registry();
}

#[test]
fn register_secret_deduplicates_identical_values() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    register_secret("f14-register-dup");
    register_secret("f14-register-dup");
    unregister_secret("f14-register-dup");
    assert_eq!(
        redact_secrets("value f14-register-dup end"),
        "value f14-register-dup end",
        "a duplicate copy survived a single unregister"
    );

    clear_secret_registry();
}

#[test]
fn registry_is_bounded_at_the_named_cap() {
    let _guard = REGISTRY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    clear_secret_registry();

    let total = MAX_SECRET_REGISTRY_ENTRIES + 25;
    seed_secrets((0..total).map(synthetic_secret));

    // Every retained entry is still redacted; count them to prove the cap.
    let retained = (0..total)
        .filter(|i| {
            let secret = synthetic_secret(*i);
            let redacted = redact_secrets(&format!("x {secret} y"));
            !redacted.contains(secret.as_str())
        })
        .count();

    assert_eq!(
        retained, MAX_SECRET_REGISTRY_ENTRIES,
        "registry must retain exactly the named cap"
    );

    clear_secret_registry();
}
