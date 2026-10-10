//! Integration tests for the durable backend registry (spec `openhands` T-004;
//! FR-004, FR-019, FR-026, FR-030, FR-035).
//!
//! The registry is the resolved read model of the durable config: every entry
//! carries a stable id, display name, kind, secret-free connection descriptor, and
//! a health state. These tests cover the hermetic cases (entry construction,
//! active-backend resolution, remote registration, secret-free descriptors) plus
//! the live `/health` probe against an in-process mock server.

use axum::{Router, routing::get};
use ragent_agent::backend::detection::command_on_path;
use ragent_agent::backend::{
    BackendRegistry, ConnectionDescriptor, HealthState, LOCAL_BACKEND_ID, probe_remote,
};
use ragent_config::{BackendConfig, Config, ExecutionBackendKind};
use serde_json::json;

/// Parse a `Config` from a JSON object.
fn config(value: serde_json::Value) -> Config {
    serde_json::from_value(value).expect("parse config")
}

fn dir() -> std::path::PathBuf {
    std::env::current_dir().expect("cwd")
}

// ---------------------------------------------------------------------------
// Registry construction and the built-in local entry (FR-004, FR-019).
// ---------------------------------------------------------------------------

#[test]
fn registry_always_includes_a_healthy_local_entry() {
    // FR-019, FR-004: with nothing configured, the registry still exposes `local`
    // as an entry, it is the active backend, and its health is `ok`.
    let registry = BackendRegistry::from_config(&Config::default(), &dir());
    let local = registry.get(LOCAL_BACKEND_ID).expect("local entry present");
    assert_eq!(local.kind(), ExecutionBackendKind::Local);
    assert_eq!(local.health(), HealthState::Ok);
    assert_eq!(local.connection().credential_names(), Vec::<String>::new());

    let active = registry
        .active_id(&Config::default())
        .expect("local is active");
    assert_eq!(active, LOCAL_BACKEND_ID);
}

#[test]
fn registry_exposes_every_configured_backend_with_kind_and_descriptor() {
    // FR-004: each configured backend appears once with the right kind and a
    // connection descriptor.
    let cfg = config(json!({
        "execution_backend": "local",
        "backends": [
            { "id": "box", "name": "Docker box", "kind": "docker",
              "image": "alpine:latest", "workspace": "/work" },
            { "id": "studio", "name": "Remote studio", "kind": "remote",
              "url": "http://127.0.0.1:9100", "api_key": "test-key" }
        ]
    }));
    let registry = BackendRegistry::from_config(&cfg, &dir());

    // local + box + studio.
    assert_eq!(registry.len(), 3);

    let box_entry = registry.get("box").expect("container entry");
    assert_eq!(box_entry.kind(), ExecutionBackendKind::Docker);
    assert_eq!(box_entry.name(), "Docker box");
    match box_entry.connection() {
        ConnectionDescriptor::Container {
            runtime,
            image,
            workspace,
            ..
        } => {
            assert_eq!(runtime.as_str(), "docker");
            assert_eq!(image.as_deref(), Some("alpine:latest"));
            assert_eq!(workspace, "/work");
        }
        other => panic!("expected a container descriptor, got {other:?}"),
    }

    let studio = registry.get("studio").expect("remote entry");
    assert_eq!(studio.kind(), ExecutionBackendKind::Remote);
    match studio.connection() {
        ConnectionDescriptor::Remote {
            url, has_api_key, ..
        } => {
            assert_eq!(url.as_deref(), Some("http://127.0.0.1:9100"));
            assert!(has_api_key);
        }
        other => panic!("expected a remote descriptor, got {other:?}"),
    }
}

#[test]
fn registry_entry_id_is_stable_from_the_descriptor_id() {
    // FR-004: the id is stable and addressable.
    let cfg = config(json!({
        "backends": [ { "id": "steady", "kind": "docker", "image": "alpine" } ]
    }));
    let registry = BackendRegistry::from_config(&cfg, &dir());
    assert!(registry.get("steady").is_some());
}

#[test]
fn registry_deduplicates_entries_by_id() {
    // FR-004: a backend both declared in `backends` and named as the active label
    // appears exactly once.
    let cfg = config(json!({
        "execution_backend": "podman",
        "backends": [ { "id": "box", "kind": "podman", "image": "alpine" } ]
    }));
    let registry = BackendRegistry::from_config(&cfg, &dir());
    assert_eq!(registry.len(), 2, "local + box only: {registry:?}");
    assert_eq!(registry.active_id(&cfg), Some("box"));
}

#[test]
fn registry_synthesizes_an_entry_for_a_bare_kind_label() {
    // FR-004: a bare `execution_backend` label with no matching descriptor still
    // yields a visible entry for the active backend.
    let cfg = config(json!({ "execution_backend": "podman" }));
    let registry = BackendRegistry::from_config(&cfg, &dir());
    let entry = registry.get("podman").expect("synthesized podman entry");
    assert_eq!(entry.kind(), ExecutionBackendKind::Podman);
    assert_eq!(registry.active_id(&cfg), Some("podman"));
}

// ---------------------------------------------------------------------------
// Health reporting (FR-004, FR-026).
// ---------------------------------------------------------------------------

#[test]
fn container_health_reflects_runtime_presence() {
    // FR-026: a container backend is healthy iff its runtime resolves on PATH.
    let cfg = config(json!({
        "backends": [ { "id": "box", "kind": "docker", "image": "alpine" } ]
    }));
    let registry = BackendRegistry::from_config(&cfg, &dir());
    let entry = registry.get("box").expect("container entry");
    if command_on_path("docker") {
        assert_eq!(entry.health(), HealthState::Ok);
        assert!(entry.detail().is_none());
    } else {
        assert_eq!(entry.health(), HealthState::Unavailable);
        assert!(entry.detail().unwrap_or("").contains("docker"));
    }
}

#[test]
fn remote_health_requires_a_url_and_a_key() {
    // FR-030: a remote backend with a URL and a key is reachable; a missing URL or
    // key is `unavailable` with an actionable detail.
    let with_key = config(json!({
        "backends": [ { "id": "r1", "kind": "remote",
                       "url": "http://example.invalid:1", "api_key": "k" } ]
    }));
    let registry = BackendRegistry::from_config(&with_key, &dir());
    assert_eq!(registry.get("r1").expect("r1").health(), HealthState::Ok);

    let without_key = config(json!({
        "backends": [ { "id": "r2", "kind": "remote", "url": "http://example.invalid:1" } ]
    }));
    let registry = BackendRegistry::from_config(&without_key, &dir());
    let entry = registry.get("r2").expect("r2");
    assert_eq!(entry.health(), HealthState::Unavailable);
    assert!(entry.detail().unwrap_or("").contains("key"));

    let without_url = config(json!({
        "backends": [ { "id": "r3", "kind": "remote", "api_key": "k" } ]
    }));
    let registry = BackendRegistry::from_config(&without_url, &dir());
    let entry = registry.get("r3").expect("r3");
    assert_eq!(entry.health(), HealthState::Unavailable);
    assert!(entry.detail().unwrap_or("").contains("url"));
}

// ---------------------------------------------------------------------------
// Remote registration by URL and key (FR-030).
// ---------------------------------------------------------------------------

#[test]
fn register_remote_adds_and_replaces_an_entry() {
    // FR-030: a remote backend is registered by URL and key.
    let mut registry = BackendRegistry::new();
    let descriptor = BackendConfig {
        id: "studio".to_string(),
        kind: "remote".to_string(),
        url: Some("http://example.invalid:9100".to_string()),
        api_key: Some("secret".to_string()),
        ..BackendConfig::default()
    };
    let entry = registry.register_remote(&descriptor).expect("registered");
    assert_eq!(entry.id(), "studio");
    assert_eq!(entry.kind(), ExecutionBackendKind::Remote);
    assert_eq!(entry.health(), HealthState::Ok);

    // Re-registering the same id replaces in place rather than duplicating.
    let updated = BackendConfig {
        url: Some("http://example.invalid:9200".to_string()),
        ..descriptor
    };
    registry.register_remote(&updated).expect("re-registered");
    assert_eq!(registry.len(), 1);
    match registry.get("studio").expect("studio").connection() {
        ConnectionDescriptor::Remote { url, .. } => {
            assert_eq!(url.as_deref(), Some("http://example.invalid:9200"));
        }
        other => panic!("expected a remote descriptor, got {other:?}"),
    }

    // A non-remote descriptor is refused.
    let local = BackendConfig {
        id: "nope".to_string(),
        kind: "local".to_string(),
        ..BackendConfig::default()
    };
    assert!(registry.register_remote(&local).is_none());
    assert!(registry.get("nope").is_none());
}

// ---------------------------------------------------------------------------
// Secret safety (FR-035, FR-010).
// ---------------------------------------------------------------------------

#[test]
fn connection_descriptor_never_carries_a_secret_value() {
    // FR-035: the descriptor is secret-free - it records key *presence* and
    // credential *names*, never the value.
    let cfg = config(json!({
        "backends": [
            { "id": "studio", "kind": "remote", "url": "http://host:9100",
              "api_key": "super-secret-value",
              "credentials": ["REMOTE_BEARER"] },
            { "id": "box", "kind": "podman", "image": "alpine",
              "credentials": ["OPENAI_API_KEY", "ANTHROPIC_API_KEY"] }
        ]
    }));
    let registry = BackendRegistry::from_config(&cfg, &dir());

    let remote = registry.get("studio").expect("studio");
    let summary = remote.connection().summary();
    assert!(!summary.contains("super-secret-value"), "leaked: {summary}");
    assert!(summary.contains("key=set"));
    assert_eq!(remote.connection().credential_names(), ["REMOTE_BEARER"]);

    let container = registry.get("box").expect("box");
    let summary = container.connection().summary();
    assert!(!summary.contains("OPENAI_API_KEY="));
    assert!(summary.contains("credentials=2"));
}

// ---------------------------------------------------------------------------
// Live health probe (FR-004, FR-030).
// ---------------------------------------------------------------------------

async fn start_health_server(status_ok: bool) -> (String, tokio::task::JoinHandle<()>) {
    use axum::http::StatusCode;

    async fn ok() -> &'static str {
        "ok"
    }
    async fn down() -> StatusCode {
        StatusCode::SERVICE_UNAVAILABLE
    }

    let app = if status_ok {
        Router::new().route("/health", get(ok))
    } else {
        Router::new().route("/health", get(down))
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind health server");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), handle)
}

#[tokio::test]
async fn refresh_health_probes_the_remote_health_endpoint() {
    // FR-004, FR-030: a live `/health` probe refines the remote health state.
    let (base, handle) = start_health_server(true).await;
    let cfg = config(json!({
        "backends": [ { "id": "studio", "kind": "remote", "url": base, "api_key": "k" } ]
    }));
    let mut registry = BackendRegistry::from_config(&cfg, &dir());
    registry.refresh_health().await;
    let entry = registry.get("studio").expect("studio");
    assert_eq!(entry.health(), HealthState::Ok);
    assert!(entry.detail().is_none());
    handle.abort();

    // A server that answers non-2xx is `unavailable`.
    let (base, handle) = start_health_server(false).await;
    let cfg = config(json!({
        "backends": [ { "id": "studio", "kind": "remote", "url": base, "api_key": "k" } ]
    }));
    let mut registry = BackendRegistry::from_config(&cfg, &dir());
    registry.refresh_health().await;
    let entry = registry.get("studio").expect("studio");
    assert_eq!(entry.health(), HealthState::Unavailable);
    assert!(entry.detail().unwrap_or("").contains("503"));
    handle.abort();
}

#[tokio::test]
async fn probe_remote_reports_unreachable_without_a_url() {
    // FR-030: no URL is an explicit, actionable `unavailable`.
    let (state, detail) = probe_remote(None).await;
    assert_eq!(state, HealthState::Unavailable);
    assert!(detail.unwrap_or_default().contains("url"));
}

#[tokio::test]
async fn probe_remote_reports_a_dead_endpoint_as_unreachable() {
    // FR-004: a refused connection is `unavailable` rather than a panic.
    let (state, detail) = probe_remote(Some("http://127.0.0.1:9")).await;
    assert_eq!(state, HealthState::Unavailable);
    assert!(detail.is_some());
}

// ---------------------------------------------------------------------------
// Active-backend resolution (FR-019, FR-004).
// ---------------------------------------------------------------------------

#[test]
fn active_id_prefers_the_named_descriptor() {
    // FR-004, FR-019: the active backend is the configured selection.
    let cfg = config(json!({
        "execution_backend": { "id": "studio", "kind": "remote",
                               "url": "http://host:9100", "api_key": "k" },
        "backends": [ { "id": "box", "kind": "docker", "image": "alpine" } ]
    }));
    let registry = BackendRegistry::from_config(&cfg, &dir());
    assert_eq!(registry.active_id(&cfg), Some("studio"));
}

#[test]
fn registry_entries_are_ordered_local_first() {
    // FR-004: `local` is always the first row so the TUI can render it
    // consistently.
    let cfg = config(json!({
        "backends": [ { "id": "box", "kind": "docker", "image": "alpine" } ]
    }));
    let registry = BackendRegistry::from_config(&cfg, &dir());
    assert_eq!(
        registry.entries().first().map(|e| e.id()),
        Some(LOCAL_BACKEND_ID)
    );
}
