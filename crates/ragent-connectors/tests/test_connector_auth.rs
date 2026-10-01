//! Tests for auth-shape handling over the encrypted credential store
//! (spec `connectors` T-007; FR-005, FR-014, FR-022, FR-023, FR-032).

use ragent_connectors::{
    AuthAction, AuthError, AuthRequirement, AuthState, ConnectorAuthShape, ConnectorDescriptor,
    ConnectorError, ConnectorId, ConnectorProvenance, ConnectorServer, CredentialStore,
    InMemoryCredentialStore, MapEnv, clear_secret, record_auth_failure, resolve, secret_value,
    store_secret,
};
use std::collections::BTreeMap;

/// A minimal stdio server (expressible).
fn server() -> ConnectorServer {
    ConnectorServer {
        id: "main".to_string(),
        transport: "stdio".to_string(),
        command: Some("/usr/bin/mcp-echo".to_string()),
        args: Vec::new(),
        env: BTreeMap::new(),
        url: None,
        headers: BTreeMap::new(),
    }
}

/// A descriptor with the given shape, credential name, and OAuth scopes.
fn descriptor(
    id: &str,
    auth: ConnectorAuthShape,
    credential: Option<&str>,
    scopes: &[&str],
) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("test id should be valid"),
        name: "Echo".to_string(),
        description: "echoes input".to_string(),
        category: "developer".to_string(),
        tags: vec!["test".to_string()],
        source: "catalogue:test".to_string(),
        provenance: ConnectorProvenance::Catalogue,
        auth,
        auth_scope: scopes.iter().map(|s| (*s).to_string()).collect(),
        credential: credential.map(str::to_string),
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

/// A store that always fails, to exercise the contained error path.
struct FailingStore;

impl CredentialStore for FailingStore {
    fn get(&self, _name: &str) -> Result<Option<String>, AuthError> {
        Err(AuthError::StoreFailed {
            detail: "database unreachable".to_string(),
        })
    }
    fn set(&self, _name: &str, _value: &str) -> Result<(), AuthError> {
        Err(AuthError::StoreFailed {
            detail: "database unreachable".to_string(),
        })
    }
    fn remove(&self, _name: &str) -> Result<(), AuthError> {
        Err(AuthError::StoreFailed {
            detail: "database unreachable".to_string(),
        })
    }
}

// --- AuthState labels and connect gating (FR-014, FR-022, FR-032) -----------

#[test]
fn test_auth_state_labels_are_ascii_and_exact() {
    assert_eq!(AuthState::NotRequired.label(), "none");
    assert_eq!(AuthState::NeedsAuth.label(), "needs auth");
    assert_eq!(AuthState::Satisfied.label(), "authenticated");
    assert_eq!(AuthState::Failed.label(), "auth failed");
    assert!(AuthState::NeedsAuth.to_string().is_ascii());
}

#[test]
fn test_auth_state_permits_connect_only_when_satisfied() {
    assert!(AuthState::NotRequired.permits_connect());
    assert!(AuthState::Satisfied.permits_connect());
    // FR-022 / FR-032: both non-satisfied states refuse to connect.
    assert!(!AuthState::NeedsAuth.permits_connect());
    assert!(!AuthState::Failed.permits_connect());
}

#[test]
fn test_record_auth_failure_yields_failed_state() {
    // FR-032: a recorded failure refuses to connect, which is what stops a loop.
    assert_eq!(record_auth_failure(), AuthState::Failed);
    assert!(!record_auth_failure().permits_connect());
}

// --- auth requirement surfacing (FR-023) -----------------------------------

#[test]
fn test_requirement_none_shape_is_empty() {
    let d = descriptor("echo", ConnectorAuthShape::None, None, &[]);
    let req = AuthRequirement::for_descriptor(&d);
    assert!(req.is_empty());
    assert!(!req.requires_credential());
    assert_eq!(req.describe(), "none");
    assert_eq!(req.guidance(), "");
    assert_eq!(req.action(), AuthAction::None);
}

#[test]
fn test_requirement_token_names_the_credential() {
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    let req = AuthRequirement::for_descriptor(&d);
    assert!(req.requires_credential());
    assert_eq!(req.credential_name(), Some("NEEDS_TOKEN_VALUE"));
    // FR-023: the requirement is surfaced with the non-secret name.
    assert_eq!(req.describe(), "token (NEEDS_TOKEN_VALUE)");
    assert!(req.guidance().contains("NEEDS_TOKEN_VALUE"));
    assert_eq!(
        req.action(),
        AuthAction::Prompt {
            credential: "NEEDS_TOKEN_VALUE".to_string(),
            shape: ConnectorAuthShape::Token,
        }
    );
}

#[test]
fn test_requirement_env_reads_the_named_variable() {
    let d = descriptor(
        "needs-env",
        ConnectorAuthShape::Env,
        Some("ECHO_TOKEN"),
        &[],
    );
    let req = AuthRequirement::for_descriptor(&d);
    assert_eq!(req.env_var.as_deref(), Some("ECHO_TOKEN"));
    assert_eq!(req.credential_name(), Some("ECHO_TOKEN"));
    assert_eq!(req.describe(), "env var ECHO_TOKEN");
    assert!(req.guidance().contains("ECHO_TOKEN"));
}

#[test]
fn test_requirement_oauth_lists_scopes() {
    let d = descriptor(
        "github",
        ConnectorAuthShape::Oauth,
        Some("GITHUB_TOKEN"),
        &["repo", "read:user"],
    );
    let req = AuthRequirement::for_descriptor(&d);
    assert_eq!(
        req.scopes,
        vec!["repo".to_string(), "read:user".to_string()]
    );
    assert_eq!(
        req.describe(),
        "oauth (GITHUB_TOKEN; scopes: repo, read:user)"
    );
    assert_eq!(
        req.action(),
        AuthAction::OAuth {
            credential: "GITHUB_TOKEN".to_string(),
            scopes: vec!["repo".to_string(), "read:user".to_string()],
        }
    );
}

#[test]
fn test_requirement_password_names_the_credential() {
    let d = descriptor("pg", ConnectorAuthShape::Password, Some("PG_PASSWORD"), &[]);
    let req = AuthRequirement::for_descriptor(&d);
    assert_eq!(req.describe(), "password (PG_PASSWORD)");
    assert!(matches!(req.action(), AuthAction::Prompt { .. }));
}

// --- resolve (FR-022) ------------------------------------------------------

#[test]
fn test_resolve_none_shape_is_not_required() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor("echo", ConnectorAuthShape::None, None, &[]);
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::NotRequired
    );
}

#[test]
fn test_resolve_token_without_secret_needs_auth() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    let state = resolve(&d, &store, &env).expect("no store fault");
    assert_eq!(state, AuthState::NeedsAuth);
    assert!(
        !state.permits_connect(),
        "needs auth must refuse to connect"
    );
}

#[test]
fn test_resolve_token_with_secret_is_satisfied() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    store
        .set("NEEDS_TOKEN_VALUE", "test-token-12345")
        .expect("settable");
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::Satisfied
    );
}

#[test]
fn test_resolve_blank_stored_secret_needs_auth() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    store.set("NEEDS_TOKEN_VALUE", "   ").expect("settable");
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::NeedsAuth
    );
}

#[test]
fn test_resolve_env_uses_environment_when_set() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new().with_var("ECHO_TOKEN", "from-env");
    let d = descriptor(
        "needs-env",
        ConnectorAuthShape::Env,
        Some("ECHO_TOKEN"),
        &[],
    );
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::Satisfied
    );
}

#[test]
fn test_resolve_env_falls_back_to_stored_value() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor(
        "needs-env",
        ConnectorAuthShape::Env,
        Some("ECHO_TOKEN"),
        &[],
    );
    store.set("ECHO_TOKEN", "stored").expect("settable");
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::Satisfied
    );
}

#[test]
fn test_resolve_env_without_value_needs_auth() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor(
        "needs-env",
        ConnectorAuthShape::Env,
        Some("ECHO_TOKEN"),
        &[],
    );
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::NeedsAuth
    );
}

#[test]
fn test_resolve_reports_store_read_failure() {
    let env = MapEnv::new();
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    let err = resolve(&d, &FailingStore, &env).expect_err("store fault must surface");
    assert!(matches!(err, AuthError::StoreFailed { .. }));
}

// --- store_secret / secret_value / clear_secret (FR-005, FR-014) -----------

#[test]
fn test_store_secret_encrypts_under_name_and_reports_state_without_value() {
    let store = InMemoryCredentialStore::new();
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    let state = store_secret(&d, "test-token-12345", &store).expect("storable");
    // FR-014: the reported state carries no secret material.
    assert_eq!(state, AuthState::Satisfied);
    assert!(!state.label().contains("test-token-12345"));
    // FR-005: the value is stored under the credential name, and is readable back.
    assert_eq!(
        store.get("NEEDS_TOKEN_VALUE").expect("readable"),
        Some("test-token-12345".to_string())
    );
}

#[test]
fn test_secret_value_returns_the_stored_secret() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    store_secret(&d, "abc123", &store).expect("storable");
    assert_eq!(
        secret_value(&d, &store, &env).expect("no store fault"),
        Some("abc123".to_string())
    );
}

#[test]
fn test_secret_value_env_prefers_live_environment() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new().with_var("ECHO_TOKEN", "live");
    let d = descriptor(
        "needs-env",
        ConnectorAuthShape::Env,
        Some("ECHO_TOKEN"),
        &[],
    );
    store.set("ECHO_TOKEN", "stale").expect("settable");
    assert_eq!(
        secret_value(&d, &store, &env).expect("no store fault"),
        Some("live".to_string())
    );
}

#[test]
fn test_secret_value_none_shape_is_none() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor("echo", ConnectorAuthShape::None, None, &[]);
    assert_eq!(
        secret_value(&d, &store, &env).expect("no store fault"),
        None
    );
}

#[test]
fn test_store_secret_without_a_credential_name_is_refused() {
    let store = InMemoryCredentialStore::new();
    let d = descriptor("bad", ConnectorAuthShape::Token, None, &[]);
    let err = store_secret(&d, "secret", &store).expect_err("no name must be refused");
    assert!(matches!(err, AuthError::MissingCredentialName { .. }));
}

#[test]
fn test_clear_secret_reverts_to_needs_auth() {
    let store = InMemoryCredentialStore::new();
    let env = MapEnv::new();
    let d = descriptor(
        "needs-token",
        ConnectorAuthShape::Token,
        Some("NEEDS_TOKEN_VALUE"),
        &[],
    );
    store_secret(&d, "abc123", &store).expect("storable");
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::Satisfied
    );
    clear_secret(&d, &store).expect("clearable");
    assert_eq!(
        resolve(&d, &store, &env).expect("no store fault"),
        AuthState::NeedsAuth
    );
}

#[test]
fn test_store_secret_none_shape_is_a_noop() {
    let store = InMemoryCredentialStore::new();
    let d = descriptor("echo", ConnectorAuthShape::None, None, &[]);
    assert_eq!(
        store_secret(&d, "ignored", &store).expect("noop"),
        AuthState::NotRequired
    );
}

// --- error conversion ------------------------------------------------------

#[test]
fn test_auth_error_converts_to_connector_error_without_secret() {
    let err = AuthError::MissingCredentialName {
        connector: "bad".to_string(),
        shape: ConnectorAuthShape::Token,
    };
    let converted: ConnectorError = err.into();
    assert!(matches!(converted, ConnectorError::Auth { .. }));
    assert!(converted.to_string().contains("bad"));
}

// --- Debug never leaks a secret --------------------------------------------

#[test]
fn test_in_memory_store_debug_prints_names_not_values() {
    let store = InMemoryCredentialStore::new();
    store
        .set("NEEDS_TOKEN_VALUE", "super-secret-value")
        .expect("settable");
    let rendered = format!("{store:?}");
    assert!(rendered.contains("NEEDS_TOKEN_VALUE"));
    assert!(!rendered.contains("super-secret-value"));
}
