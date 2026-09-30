//! Regression tests for the unified redaction implementation (SECTASKS MS-05,
//! T-069).
//!
//! Every disclosure surface — `GET /config`, telemetry attributes, log lines,
//! the SSE stream, and tool output — must share one redaction implementation.
//! These tests pin the shared chokepoint (`ragent_types::sanitize`) and the
//! credential-bearing `Event` `Debug` hand-impl, so a future crate cannot
//! silently re-introduce a private registry or a leaking derive.

use ragent_types::event::Event;
use ragent_types::sanitize::{
    clear_secret_registry, redact_secrets, redact_secrets_cow, register_secret,
};

const REGISTERED: &str = "registry-only-value-8f3a1c";
const GITLAB_PAT: &str = "glpat-abcdefghijklmnopqrst";

/// The regex layer must catch a credential shape that was never registered.
///
/// This half of the redaction contract is independent of the process-global
/// registry, so it cannot be perturbed by another test in this binary.
#[test]
fn test_pattern_layer_redacts_an_unregistered_gitlab_pat() {
    let out = redact_secrets(&format!(
        "remote=https://oauth2:{GITLAB_PAT}@gitlab.example"
    ));
    assert!(!out.contains(GITLAB_PAT), "GitLab PAT leaked: {out}");
}

/// The exact-match registry layer must mask a registered value on every path,
/// and the `Cow` fast path must not defeat it: an unregistered, harmless
/// message may be borrowed, but a message carrying a registered secret must
/// allocate a masked copy.
///
/// `SECRET_REGISTRY` is process-global, so this test serialises on a private
/// mutex and restores the registry. Without that, a concurrent test that calls
/// `clear_secret_registry` in the crate-wide `cargo test` run can wipe the
/// registration between the `register_secret` call and the redaction.
#[test]
fn test_registry_layer_redacts_and_cow_path_still_masks() {
    static REGISTRY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // A panic under the lock can only be this test's own assertion.
    let _guard = REGISTRY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    clear_secret_registry();
    let clean = redact_secrets_cow("nothing sensitive here");
    assert!(matches!(clean, std::borrow::Cow::Borrowed(_)));

    register_secret(REGISTERED);
    let dirty_input = format!("token {REGISTERED}");
    let dirty = redact_secrets_cow(&dirty_input);
    assert!(!dirty.contains(REGISTERED), "Cow path leaked: {dirty}");

    let exact = redact_secrets(&format!("header: {REGISTERED} end"));
    assert!(
        !exact.contains(REGISTERED),
        "registered secret leaked: {exact}"
    );

    clear_secret_registry();
}

/// `Event` must not expose credential fields through its `Debug` rendering.
///
/// Before MS-05 the enum derived `Debug`, so any `tracing::debug!("{event:?}")`
/// printed the Copilot OAuth token and the device code verbatim.
#[test]
fn test_event_debug_masks_copilot_credentials() {
    let token = "gho_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let complete = Event::CopilotDeviceFlowComplete {
        token: token.to_string(),
        api_base: "https://api.githubcopilot.com".to_string(),
    };
    let rendered = format!("{complete:?}");
    assert!(
        !rendered.contains(token),
        "Copilot token leaked through Debug: {rendered}"
    );
    assert!(
        rendered.contains("token_present: true"),
        "Debug should report presence: {rendered}"
    );

    let start = Event::CopilotDeviceFlowStartResult {
        user_code: Some("ABCD-1234".to_string()),
        verification_uri: Some("https://github.com/login/device".to_string()),
        device_code: Some("device-code-secret-value".to_string()),
        interval: Some(5),
        error: None,
    };
    let rendered = format!("{start:?}");
    assert!(
        !rendered.contains("device-code-secret-value"),
        "device code leaked through Debug: {rendered}"
    );
    assert!(
        rendered.contains("device_code_present: true"),
        "Debug should report device-code presence: {rendered}"
    );
}

/// A benign event must still render its variant name, so logs stay useful.
#[test]
fn test_event_debug_keeps_the_variant_name() {
    let event = Event::SessionCreated {
        session_id: "abc-123".to_string(),
    };
    let rendered = format!("{event:?}");
    assert!(
        rendered.contains("SessionCreated"),
        "variant name missing from Debug: {rendered}"
    );
}
