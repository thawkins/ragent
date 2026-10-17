//! Tests-only coverage for the automation helper functions
//! (`ragent_agent::automation`): the per-session backend override ledger, the
//! backend-label resolution, prompt rendering, and schedule next-due computation.
//!
//! These exercise the pure functions only; the run/dispatch path is covered by
//! `test_automation_service.rs`.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use ragent_agent::automation::{
    apply_session_backend_override, backend_descriptor, clear_session_backend, compute_next_due,
    register_session_backend, render_prompt, session_backend_override,
};
use ragent_config::{BackendConfig, Config, ExecutionBackend};

fn descriptor(id: &str, kind: &str) -> BackendConfig {
    BackendConfig {
        id: id.to_string(),
        kind: kind.to_string(),
        ..BackendConfig::default()
    }
}

#[test]
fn session_backend_override_round_trips() {
    let session = "automation-override-session";
    clear_session_backend(session);
    assert!(session_backend_override(session).is_none());

    register_session_backend(session, descriptor("box", "podman"));
    let stored = session_backend_override(session).expect("override stored");
    assert_eq!(stored.id, "box");
    assert_eq!(stored.kind, "podman");

    clear_session_backend(session);
    assert!(session_backend_override(session).is_none());
}

#[test]
fn apply_session_backend_override_without_an_entry_returns_the_same_arc() {
    let session = "no-override-session";
    clear_session_backend(session);
    let config = Arc::new(Config::default());
    let applied = apply_session_backend_override(&config, session);
    assert!(
        Arc::ptr_eq(&config, &applied),
        "no override must return the original Arc unchanged"
    );
}

#[test]
fn apply_session_backend_override_installs_the_descriptor() {
    let session = "install-override-session";
    register_session_backend(session, descriptor("box", "podman"));
    let config = Arc::new(Config::default());
    let applied = apply_session_backend_override(&config, session);
    assert!(
        !Arc::ptr_eq(&config, &applied),
        "an override clones the config"
    );
    match &applied.execution_backend {
        Some(ExecutionBackend::Config(d)) => assert_eq!(d.id, "box"),
        other => panic!("expected a Config backend override, got {other:?}"),
    }
    clear_session_backend(session);
}

#[test]
fn backend_descriptor_reuses_a_registered_kind_entry() {
    let mut config = Config::default();
    config.backends = vec![BackendConfig {
        id: "remote-prod".to_string(),
        kind: "remote".to_string(),
        url: Some("http://localhost:9999".to_string()),
        ..BackendConfig::default()
    }];

    let resolved = backend_descriptor(&config, "remote");
    assert_eq!(resolved.id, "remote-prod");
    assert_eq!(resolved.url.as_deref(), Some("http://localhost:9999"));
}

#[test]
fn backend_descriptor_synthesises_an_unregistered_label() {
    let config = Config::default();
    let resolved = backend_descriptor(&config, "docker");
    assert_eq!(resolved.id, "docker");
    assert_eq!(resolved.kind, "docker");
    assert!(resolved.url.is_none());
}

#[test]
fn render_prompt_substitutes_payload() {
    assert_eq!(
        render_prompt("run with {{payload}} now", "abc"),
        "run with abc now"
    );
    // An empty payload removes the placeholder.
    assert_eq!(render_prompt("x{{payload}}y", ""), "xy");
    // A template without the placeholder passes through unchanged.
    assert_eq!(render_prompt("no placeholder", "abc"), "no placeholder");
}

#[test]
fn compute_next_due_parses_valid_and_rejects_invalid() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    assert!(compute_next_due("every 30m", now).is_some());
    assert!(
        compute_next_due("not a schedule at all", now).is_none(),
        "an unparseable expression yields None"
    );
}
