//! Integration tests for sandbox secret injection and workspace persistence
//! (spec `openhands` T-006; FR-010, FR-023).
//!
//! The tests split into two groups:
//!
//! - **Hermetic** cases that need no container runtime: credential resolution
//!   from a store, the fail-without-provisioning path for a missing credential,
//!   the deterministic naming that keeps the *volume* stable while the
//!   *container* is replaced when the credential set changes (FR-023), a
//!   redacted injection summary, and the remote backend's store-resolved bearer
//!   key (FR-010).
//! - **Live** cases guarded on `podman`/`docker` plus a small image: a named
//!   credential is present as an environment variable inside the sandbox, the
//!   secret is absent from every workspace file, and a file created in the
//!   workspace survives replacing the container (FR-010, FR-023).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use ragent_agent::backend::{
    BackendError, BackendErrorKind, BackendToolCall, CONTAINER_WORKSPACE, ContainerBackend,
    ContainerRuntime, ExecutionBackend, MapSecretResolver, RemoteBackend, SecretResolver,
    StorageSecretResolver, container_env_args, credential_names, detect_container_runtime,
    resolve_backend_with_secrets, resolve_descriptor_secrets,
};
use ragent_agent::event::EventBus;
use ragent_agent::storage::Storage;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput};
use ragent_config::{BackendConfig, Config};
use serde_json::{Value, json};

/// Test image for the live sandbox cases: tiny, with a POSIX shell userland.
const TEST_IMAGE: &str = "docker.io/library/alpine:latest";

/// The example credential used throughout (FR-010).
const EXAMPLE_KEY: &str = "EXAMPLE_API_KEY";

/// The example secret value; distinct enough to grep for in the sandbox.
const EXAMPLE_VALUE: &str = "sandbox-secret-value-123";

/// A tool double that only reports a *name* and counts invocations.
struct NamedTool {
    name: &'static str,
    runs: Arc<AtomicUsize>,
}

impl NamedTool {
    fn new(name: &'static str) -> (Self, Arc<AtomicUsize>) {
        let runs = Arc::new(AtomicUsize::new(0));
        (
            Self {
                name,
                runs: Arc::clone(&runs),
            },
            runs,
        )
    }
}

#[async_trait]
impl Tool for NamedTool {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &'static str {
        "test double"
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput {
            content: "ran on host".to_string(),
            metadata: None,
        })
    }
}

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "sandbox-secret-test".to_string(),
        working_dir: std::env::current_dir().expect("cwd"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

/// A container descriptor with a distinct id per call and the given credentials.
fn container_descriptor(image: Option<&str>, credentials: Vec<String>) -> BackendConfig {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = format!("secret-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    BackendConfig {
        id,
        name: Some("Test sandbox".to_string()),
        kind: "podman".to_string(),
        image: image.map(str::to_string),
        workspace: None,
        url: None,
        api_key: None,
        credentials,
    }
}

/// A resolver holding the example credential.
fn resolver_with_example() -> MapSecretResolver {
    MapSecretResolver::default().with(EXAMPLE_KEY, EXAMPLE_VALUE)
}

// ---------------------------------------------------------------------------
// FR-010: credentials resolve from the store at spawn time.
// ---------------------------------------------------------------------------

#[test]
fn named_credentials_are_resolved_and_rendered_as_env_args() {
    // FR-010: a descriptor names a credential; the value comes from the store and
    // is rendered as an `-e NAME=VALUE` argument, never from the descriptor.
    let descriptor = container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]);
    let secrets = resolve_descriptor_secrets(&descriptor, &resolver_with_example())
        .expect("the named credential must resolve");
    assert_eq!(secrets.len(), 1);
    assert_eq!(secrets[0].name, EXAMPLE_KEY);
    assert_eq!(secrets[0].value, EXAMPLE_VALUE);

    let args = container_env_args(&secrets);
    assert_eq!(args, vec!["-e", "EXAMPLE_API_KEY=sandbox-secret-value-123"]);
}

#[test]
fn a_missing_credential_fails_rather_than_injecting_nothing() {
    // FR-010: a named credential absent from the store is a hard failure, so a
    // sandbox is never provisioned with the credential silently missing.
    let descriptor = container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]);
    let error = resolve_descriptor_secrets(&descriptor, &MapSecretResolver::default())
        .expect_err("a missing credential must fail");
    assert!(
        error.to_string().contains(EXAMPLE_KEY),
        "the error should name the credential: {error}"
    );
}

#[test]
fn a_resolved_secret_is_registered_for_redaction() {
    // FR-035: once resolved, the value is in the shared redaction registry, so a
    // later rendering of tool output masks it.
    let _ = resolve_descriptor_secrets(
        &container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]),
        &resolver_with_example(),
    )
    .expect("resolve");

    let rendered = ragent_types::sanitize::redact_secrets(&format!(
        "the key is {EXAMPLE_VALUE} inside the sandbox"
    ));
    assert!(
        !rendered.contains(EXAMPLE_VALUE),
        "a resolved secret must be redacted in later renderings: {rendered}"
    );
}

#[test]
fn descriptor_credential_names_are_trimmed_and_deduplicated() {
    let descriptor = BackendConfig {
        credentials: vec![
            " B_KEY ".to_string(),
            "A_KEY".to_string(),
            "A_KEY".to_string(),
            String::new(),
        ],
        ..BackendConfig::default()
    };
    assert_eq!(credential_names(&descriptor), vec!["A_KEY", "B_KEY"]);
}

// ---------------------------------------------------------------------------
// FR-023: the workspace volume survives container replacement.
// ---------------------------------------------------------------------------

#[test]
fn changing_the_credential_set_replaces_the_container_but_keeps_the_volume() {
    // FR-023: the workspace volume is derived from the *project* identity, so it
    // is stable across container replacement. FR-010: the container name folds in
    // the credential set, so a changed set gets a fresh container environment.
    let working_dir = std::path::Path::new("/tmp/project");
    let without = container_descriptor(Some(TEST_IMAGE), Vec::new());
    let mut with = without.clone();
    with.credentials = vec![EXAMPLE_KEY.to_string()];

    let backend_without = ContainerBackend::from_descriptor(&without, working_dir);
    let backend_with = ContainerBackend::from_descriptor(&with, working_dir);

    assert_eq!(
        backend_without.volume_name(),
        backend_with.volume_name(),
        "the workspace volume must survive container replacement (FR-023)"
    );
    assert_ne!(
        backend_without.container_name(),
        backend_with.container_name(),
        "a changed credential set must not reuse a stale container (FR-010)"
    );
}

#[test]
fn an_unchanged_descriptor_reuses_the_same_container_and_volume() {
    // A restarted process must reuse one sandbox instead of leaking a new
    // container, so the names are deterministic for the same descriptor.
    let working_dir = std::path::Path::new("/tmp/project");
    let descriptor = container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]);
    let first = ContainerBackend::from_descriptor(&descriptor, working_dir);
    let second = ContainerBackend::from_descriptor(&descriptor, working_dir);
    assert_eq!(first.container_name(), second.container_name());
    assert_eq!(first.volume_name(), second.volume_name());
}

#[test]
fn the_injection_summary_never_reveals_a_secret_value() {
    // FR-035: the introspection summary names the credentials but never a value.
    let descriptor = container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]);
    let backend = ContainerBackend::from_descriptor(&descriptor, std::path::Path::new("."))
        .with_secret_resolver(Arc::new(resolver_with_example()));
    let summary = backend.injected_env_summary();
    assert_eq!(summary, vec!["EXAMPLE_API_KEY=[REDACTED]"]);
    assert!(
        !summary.iter().any(|line| line.contains(EXAMPLE_VALUE)),
        "the summary must not contain a secret value: {summary:?}"
    );
    assert_eq!(backend.credential_names(), &[EXAMPLE_KEY.to_string()]);
}

// ---------------------------------------------------------------------------
// FR-010: the encrypted store is a resolver, exercised end to end.
// ---------------------------------------------------------------------------

#[test]
fn the_storage_resolver_reads_the_encrypted_store() {
    // FR-010: the production resolver reads the same encrypted table the `/auth`
    // surface writes, so a stored credential is available at spawn time.
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .set_provider_auth(EXAMPLE_KEY, EXAMPLE_VALUE)
        .expect("store the credential");
    let resolver = StorageSecretResolver::new(Arc::clone(&storage));

    let descriptor = container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]);
    let secrets = resolve_descriptor_secrets(&descriptor, &resolver).expect("resolve");
    assert_eq!(secrets[0].value, EXAMPLE_VALUE);

    // An absent credential resolves to `None`, distinct from a read fault.
    assert_eq!(resolver.resolve("NOT_STORED").expect("read"), None);
}

#[test]
fn resolve_backend_with_secrets_wires_the_store_into_a_container_adapter() {
    // FR-010: the config-level resolution path carries the store into the
    // container adapter, so a session's encrypted credentials reach the sandbox.
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .set_provider_auth(EXAMPLE_KEY, EXAMPLE_VALUE)
        .expect("store the credential");
    let config: Config = serde_json::from_value(json!({
        "execution_backend": "podman",
        "backends": [
            { "id": "box", "kind": "podman", "image": TEST_IMAGE,
              "credentials": [EXAMPLE_KEY] }
        ]
    }))
    .expect("parse config");

    let backend = resolve_backend_with_secrets(
        &config,
        std::path::Path::new("/tmp/project"),
        Some(Arc::clone(&storage)),
    );
    assert_eq!(backend.kind(), ragent_config::ExecutionBackendKind::Podman);
}

// ---------------------------------------------------------------------------
// FR-010: the remote backend resolves its bearer key from the store.
// ---------------------------------------------------------------------------

fn remote_descriptor(api_key: Option<&str>, credentials: Vec<String>) -> BackendConfig {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = format!("remote-secret-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    BackendConfig {
        id,
        name: Some("Remote sandbox".to_string()),
        kind: "remote".to_string(),
        url: Some("http://127.0.0.1:9".to_string()),
        api_key: api_key.map(str::to_string),
        credentials,
        ..BackendConfig::default()
    }
}

#[test]
fn the_remote_bearer_key_is_resolved_from_the_store() {
    // FR-010: a remote descriptor that names a credential (no literal api_key)
    // gets its bearer key from the encrypted store, so it never appears in the
    // config file.
    let descriptor = remote_descriptor(None, vec![EXAMPLE_KEY.to_string()]);
    assert!(descriptor.api_key.is_none());
    let backend = RemoteBackend::resolve_credentials(&descriptor, &resolver_with_example())
        .expect("the stored credential resolves");
    assert!(
        backend.has_bearer_key(),
        "the resolved backend must carry the store's key"
    );
}

#[test]
fn a_missing_remote_credential_is_a_provisioning_failure() {
    // FR-010, FR-031: a named credential the store does not hold fails the remote
    // resolution rather than driving the turn unauthenticated.
    let descriptor = remote_descriptor(None, vec![EXAMPLE_KEY.to_string()]);
    let error = RemoteBackend::resolve_credentials(&descriptor, &MapSecretResolver::default())
        .expect_err("a missing credential must fail");
    assert!(error.to_string().contains(EXAMPLE_KEY), "{error}");
}

#[test]
fn a_literal_remote_api_key_still_wins() {
    // Backward compatibility: a descriptor with a literal api_key is unchanged and
    // no store read is required.
    let descriptor = remote_descriptor(Some("literal-key"), vec![EXAMPLE_KEY.to_string()]);
    let backend = RemoteBackend::resolve_credentials(&descriptor, &MapSecretResolver::default())
        .expect("a literal key needs no store");
    assert!(backend.has_bearer_key());
}

// ---------------------------------------------------------------------------
// Live sandbox cases (skipped when no runtime/image is available).
// ---------------------------------------------------------------------------

/// Whether the detected runtime has `image` locally.
fn image_available(runtime: ContainerRuntime, image: &str) -> bool {
    let binary = runtime.binary();
    std::process::Command::new(binary)
        .args(["image", "exists", image])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Remove a test sandbox container and its backing volume (best effort).
fn remove_sandbox(runtime: ContainerRuntime, container: &str, volume: &str) {
    let _ = std::process::Command::new(runtime.binary())
        .args(["rm", "-f", container])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let _ = std::process::Command::new(runtime.binary())
        .args(["volume", "rm", "-f", volume])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Run a `bash` command through a spawned container backend.
async fn run_bash(backend: &ContainerBackend, ctx: &ToolContext, command: &str) -> ToolOutput {
    let (tool, _runs) = NamedTool::new("bash");
    ExecutionBackend::execute_tool(
        backend,
        BackendToolCall {
            tool: &tool,
            input: json!({ "command": command }),
            ctx,
        },
    )
    .await
    .expect("the sandbox must run the command")
}

#[tokio::test]
async fn the_sandbox_receives_the_credential_as_an_env_var_not_a_file() {
    // FR-010, FR-035: the credential is injected as an environment variable and
    // is never written into the workspace.
    let Some(runtime) = detect_container_runtime() else {
        eprintln!("skipping: no container runtime on PATH");
        return;
    };
    if !image_available(runtime, TEST_IMAGE) {
        return;
    }

    let ctx = ctx();
    let descriptor = container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]);
    let backend = ContainerBackend::from_descriptor(&descriptor, &ctx.working_dir)
        .with_secret_resolver(Arc::new(resolver_with_example()));
    let container = backend.container_name().to_string();
    let volume = backend.volume_name().to_string();

    // 1. The variable is present exactly once (FR-010).
    let count = run_bash(
        &backend,
        &ctx,
        &format!("env | grep -c {EXAMPLE_KEY} || true"),
    )
    .await;
    assert!(
        count.content.contains('1'),
        "the credential must be present as an env var: {}",
        count.content
    );

    // 2. The value is absent from every workspace file (FR-035).
    let scanned = run_bash(
        &backend,
        &ctx,
        &format!("grep -r {EXAMPLE_VALUE} . 2>/dev/null || echo NOT_IN_WORKSPACE"),
    )
    .await;
    assert!(
        scanned.content.contains("NOT_IN_WORKSPACE"),
        "the secret must not be written into the workspace: {}",
        scanned.content
    );

    remove_sandbox(runtime, &container, &volume);
}

#[tokio::test]
async fn the_workspace_survives_replacing_the_container() {
    // FR-023: a file created in the sandbox workspace is still there after the
    // container is removed and a fresh one is provisioned from the same
    // descriptor - the named volume persists.
    let Some(runtime) = detect_container_runtime() else {
        eprintln!("skipping: no container runtime on PATH");
        return;
    };
    if !image_available(runtime, TEST_IMAGE) {
        return;
    }

    let ctx = ctx();
    let descriptor = container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]);
    let backend = ContainerBackend::from_descriptor(&descriptor, &ctx.working_dir)
        .with_secret_resolver(Arc::new(resolver_with_example()));
    let container = backend.container_name().to_string();
    let volume = backend.volume_name().to_string();
    let marker = format!("PERSISTED_{}.txt", std::process::id());

    // 1. Provision and create a workspace file.
    run_bash(
        &backend,
        &ctx,
        &format!("touch {CONTAINER_WORKSPACE}/{marker}"),
    )
    .await;

    // 2. Replace the container but keep the volume.
    let _ = std::process::Command::new(runtime.binary())
        .args(["rm", "-f", &container])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    // 3. A fresh adapter for the same descriptor re-provisions and the file is
    //    still present, proving the workspace survived container replacement.
    let rebuilt = ContainerBackend::from_descriptor(&descriptor, &ctx.working_dir)
        .with_secret_resolver(Arc::new(resolver_with_example()));
    assert_eq!(rebuilt.volume_name(), volume);
    let listed = run_bash(
        &rebuilt,
        &ctx,
        &format!("ls {CONTAINER_WORKSPACE}/{marker}"),
    )
    .await;
    assert!(
        listed.content.contains(&marker),
        "the workspace must survive container replacement (FR-023): {}",
        listed.content
    );

    remove_sandbox(runtime, &container, &volume);
}

#[tokio::test]
async fn a_missing_live_credential_fails_provisioning_without_host_execution() {
    // FR-010, FR-031: a named credential the store does not hold fails the
    // provision; the tool is never executed on the host.
    let Some(runtime) = detect_container_runtime() else {
        eprintln!("skipping: no container runtime on PATH");
        return;
    };
    if !image_available(runtime, TEST_IMAGE) {
        return;
    }

    let (tool, runs) = NamedTool::new("bash");
    let ctx = ctx();
    // The default resolver is an empty store.
    let backend = ContainerBackend::from_descriptor(
        &container_descriptor(Some(TEST_IMAGE), vec![EXAMPLE_KEY.to_string()]),
        &ctx.working_dir,
    );

    let error = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &tool,
            input: json!({ "command": "echo hello" }),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("a missing credential must fail provisioning");

    assert_eq!(
        runs.load(Ordering::SeqCst),
        0,
        "the tool must not run on the host"
    );
    assert_eq!(
        error
            .downcast_ref::<BackendError>()
            .map(BackendError::error_kind),
        Some(BackendErrorKind::Provision)
    );
}
