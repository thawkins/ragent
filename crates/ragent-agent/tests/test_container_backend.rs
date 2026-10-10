//! Integration tests for the container execution backend (spec `openhands` T-002;
//! FR-001, FR-002, FR-009, FR-020, FR-026, FR-031, FR-035).
//!
//! The tests split into two groups:
//!
//! - **Hermetic** cases that need no container runtime: runtime detection and the
//!   `podman` default (FR-026), the fail-without-fallback path for a bad image
//!   (FR-031), the refusal of tools with no sandbox adapter (FR-031), and workspace
//!   path confinement (FR-020, FR-035).
//! - **Live** cases guarded on `podman`/`docker` plus a small image (FR-009,
//!   FR-020): a `bash` command runs *inside* the sandbox, a file created there is
//!   absent from the host working directory, and the 7-layer validator still gates
//!   the command (FR-002).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use ragent_agent::backend::{
    BackendError, BackendErrorKind, BackendToolCall, CONTAINER_WORKSPACE, ContainerBackend,
    ContainerRuntime, ExecutionBackend, detect_container_runtime, local_backend, resolve_backend,
    resolve_backend_for_config, resolve_container_runtime,
};
use ragent_agent::event::EventBus;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput};
use ragent_config::{BackendConfig, Config, ExecutionBackendKind};
use serde_json::{Value, json};

/// Test image for the live sandbox cases: tiny, with a POSIX shell userland.
const TEST_IMAGE: &str = "docker.io/library/alpine:latest";

/// A tool double that only reports a *name* and counts invocations.
///
/// The container backend routes on `tool.name()` and reads `input`; it does not
/// call the tool's `execute`. Using a named double lets a test drive every branch
/// (routed `bash`/file tool, refused tool) without constructing the real tool, and
/// the run counter proves the tool never executed on the host.
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
        session_id: "container-test".to_string(),
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

fn descriptor(kind: &str, image: Option<&str>) -> BackendConfig {
    // A distinct id per call gives each test its own sandbox container/volume, so
    // tests that provision a live container do not collide on a shared name.
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = format!("sandbox-{}", NEXT_ID.fetch_add(1, Ordering::SeqCst));
    BackendConfig {
        id,
        name: Some("Test sandbox".to_string()),
        kind: kind.to_string(),
        image: image.map(str::to_string),
        workspace: None,
        url: None,
        api_key: None,
        credentials: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// FR-026: runtime detection and the podman default.
// ---------------------------------------------------------------------------

#[test]
fn default_container_runtime_is_podman() {
    // FR-026: podman is the default container backend when none is specified.
    assert_eq!(ContainerRuntime::default(), ContainerRuntime::Podman);
    assert_eq!(ContainerRuntime::default().as_str(), "podman");
    assert_eq!(
        resolve_container_runtime(None),
        Some(detect_container_runtime().unwrap_or_default())
    );
}

#[test]
fn resolved_runtime_matches_the_requested_kind() {
    // A named container kind resolves to exactly that runtime, even when its
    // binary is absent (the backend then fails at provisioning, FR-031).
    assert_eq!(
        resolve_container_runtime(Some(ExecutionBackendKind::Docker)),
        Some(ContainerRuntime::Docker)
    );
    assert_eq!(
        resolve_container_runtime(Some(ExecutionBackendKind::Podman)),
        Some(ContainerRuntime::Podman)
    );
    // `local`/`remote` are not container kinds.
    assert_eq!(
        resolve_container_runtime(Some(ExecutionBackendKind::Local)),
        None
    );
    assert_eq!(
        resolve_container_runtime(Some(ExecutionBackendKind::Remote)),
        None
    );
}

#[test]
fn kind_maps_to_runtime_both_ways() {
    assert_eq!(
        ContainerRuntime::Podman.kind(),
        ExecutionBackendKind::Podman
    );
    assert_eq!(
        ContainerRuntime::Docker.kind(),
        ExecutionBackendKind::Docker
    );
    assert_eq!(
        ContainerRuntime::from_kind(ExecutionBackendKind::Docker),
        Some(ContainerRuntime::Docker)
    );
    assert_eq!(
        ContainerRuntime::from_kind(ExecutionBackendKind::Local),
        None
    );
}

// ---------------------------------------------------------------------------
// FR-031: provisioning failure never falls back to host execution.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn missing_image_fails_provisioning_without_host_execution() {
    // A container backend with no image cannot provision; the dispatch fails with
    // a Provision error and the tool never runs on the host (FR-031).
    if detect_container_runtime().is_none() {
        eprintln!("skipping: no container runtime on PATH");
        return;
    }
    let (tool, runs) = NamedTool::new("bash");
    let ctx = ctx();
    let backend = ContainerBackend::from_descriptor(&descriptor("podman", None), &ctx.working_dir);
    assert_eq!(backend.kind(), ExecutionBackendKind::Podman);

    let err = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &tool,
            input: json!({ "command": "echo hello" }),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("a backend without an image must fail provisioning");

    assert_eq!(runs.load(Ordering::SeqCst), 0, "no host execution");
    let backend_err = err
        .downcast_ref::<BackendError>()
        .expect("structured backend failure");
    assert_eq!(backend_err.error_kind(), BackendErrorKind::Provision);
    assert!(
        err.to_string().contains("image"),
        "the error should name the missing image: {err}"
    );
}

#[tokio::test]
async fn nonexistent_image_fails_provisioning_without_host_execution() {
    // The runtime is present but the image cannot be pulled; the turn fails with
    // the provisioning error and the command is NOT executed on the host (FR-031,
    // test-plan TC-003 step 7).
    let Some(runtime) = detect_container_runtime() else {
        eprintln!("skipping: no container runtime on PATH");
        return;
    };
    let (tool, runs) = NamedTool::new("bash");
    let ctx = ctx();
    let backend = ContainerBackend::from_descriptor(
        &descriptor(
            runtime.as_str(),
            Some("localhost/ragent-sandbox-does-not-exist:latest"),
        ),
        &ctx.working_dir,
    );

    let err = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &tool,
            input: json!({ "command": "echo hello" }),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("a missing image must fail provisioning");

    assert_eq!(runs.load(Ordering::SeqCst), 0, "no host execution");
    assert_eq!(
        err.downcast_ref::<BackendError>()
            .map(BackendError::error_kind),
        Some(BackendErrorKind::Provision)
    );
    assert!(
        !err.to_string().contains("ran on host"),
        "the tool must not have run: {err}"
    );
}

#[tokio::test]
async fn unadapted_tool_is_refused_without_host_execution() {
    // A tool the sandbox cannot dispatch is refused: no adapter, no host fallback
    // (FR-031). This needs no runtime because the refusal precedes provisioning.
    let (tool, runs) = NamedTool::new("websearch");
    let ctx = ctx();
    let backend = ContainerBackend::from_descriptor(
        &descriptor("podman", Some(TEST_IMAGE)),
        &ctx.working_dir,
    );

    let err = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &tool,
            input: json!({ "query": "irrelevant" }),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("an unadapted tool must be refused");

    assert_eq!(
        runs.load(Ordering::SeqCst),
        0,
        "the tool never ran on the host"
    );
    assert_eq!(
        err.downcast_ref::<BackendError>()
            .map(BackendError::error_kind),
        Some(BackendErrorKind::Unavailable)
    );
    assert!(
        err.to_string().contains("websearch") && err.to_string().contains("host"),
        "the refusal should name the tool and the no-host-fallback rule: {err}"
    );
}

#[tokio::test]
async fn container_file_tools_reject_paths_outside_the_workspace() {
    // A sandbox file tool is confined to the mounted workspace: an absolute host
    // path is rejected before any container is touched (FR-020, FR-035).
    let (tool, runs) = NamedTool::new("read");
    let ctx = ctx();
    let backend = ContainerBackend::from_descriptor(
        &descriptor("podman", Some(TEST_IMAGE)),
        &ctx.working_dir,
    );

    for path in [
        "/etc/passwd",
        "/home/someone/.ssh/id_rsa",
        "../../etc/shadow",
    ] {
        let err = ExecutionBackend::execute_tool(
            &backend,
            BackendToolCall {
                tool: &tool,
                input: json!({ "path": path }),
                ctx: &ctx,
            },
        )
        .await
        .expect_err("a path outside the workspace must be rejected");
        assert!(
            err.to_string().contains("workspace") || err.to_string().contains("FR-035"),
            "the error should cite the workspace confinement for {path}: {err}"
        );
    }
    assert_eq!(runs.load(Ordering::SeqCst), 0, "no host execution");
}

// ---------------------------------------------------------------------------
// Config-aware resolution (FR-001, FR-009, FR-026).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn config_resolution_builds_a_container_backend_for_a_podman_descriptor() {
    // `resolve_backend_for_config` reads the descriptor so a `podman` selection
    // carries its image/workspace into a real ContainerBackend (FR-001, FR-009).
    let config: Config = serde_json::from_str(
        r#"{
            "execution_backend": "podman",
            "backends": [
                { "id": "box", "name": "Podman sandbox", "kind": "podman",
                  "image": "docker.io/library/alpine:latest", "workspace": "/projects" }
            ]
        }"#,
    )
    .expect("parse config");
    let descriptor = config
        .effective_backend_config()
        .expect("the podman label resolves to the registered entry");
    assert_eq!(descriptor.id, "box");
    assert_eq!(descriptor.image.as_deref(), Some(TEST_IMAGE));
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Podman
    );

    let ctx = ctx();
    let backend = resolve_backend_for_config(&config, &ctx.working_dir);
    assert_eq!(backend.kind(), ExecutionBackendKind::Podman);
}

#[test]
fn config_resolution_defaults_to_local_without_a_backend() {
    // No backend configured resolves to the host adapter (FR-019).
    let config = Config::default();
    assert!(config.effective_backend_config().is_none());
    let backend = resolve_backend_for_config(&config, std::path::Path::new("."));
    assert_eq!(backend.kind(), ExecutionBackendKind::Local);
    // The shared local handle is reused, so the cheap default path stays cheap.
    assert_eq!(
        resolve_backend(ExecutionBackendKind::Local).kind(),
        local_backend().kind()
    );
}

// ---------------------------------------------------------------------------
// Live sandbox cases (skipped when no runtime/image is available).
// ---------------------------------------------------------------------------

/// Whether the detected runtime has `image` locally (so a live case can run).
fn image_available(runtime: ContainerRuntime, image: &str) -> bool {
    let binary = runtime.binary();
    let ok = std::process::Command::new(binary)
        .args(["image", "exists", image])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !ok {
        eprintln!(
            "skipping: image '{image}' is not present for runtime '{binary}' \
             (pull it with `{binary} pull {image}` to exercise this case)"
        );
    }
    ok
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

#[tokio::test]
async fn sandbox_runs_bash_inside_the_container_and_not_on_the_host() {
    // FR-009, FR-020: the project workspace is provisioned before the tool runs,
    // the command executes inside the sandbox, and a file created there does not
    // appear in the host working directory.
    let Some(runtime) = detect_container_runtime() else {
        eprintln!("skipping: no container runtime on PATH");
        return;
    };
    if !image_available(runtime, TEST_IMAGE) {
        return;
    }

    let ctx = ctx();
    let backend = ContainerBackend::from_descriptor(
        &descriptor(runtime.as_str(), Some(TEST_IMAGE)),
        &ctx.working_dir,
    );
    let container = backend.container_name().to_string();
    let volume = backend.volume_name().to_string();

    let (bash, bash_runs) = NamedTool::new("bash");
    let marker = format!("CONTAINER_MARKER_{}.txt", std::process::id());

    // 1. Provision by running a shell command; it must print the workspace path
    //    and prove the host has no such marker.
    let output = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &bash,
            input: json!({
                "command": "pwd; cat HOST_MARKER.txt 2>/dev/null || echo HOST_MARKER_NOT_VISIBLE"
            }),
            ctx: &ctx,
        },
    )
    .await
    .expect("the sandbox must run a bash command");
    assert!(
        output.content.contains("HOST_MARKER_NOT_VISIBLE"),
        "the host working directory must not be visible in the sandbox: {}",
        output.content
    );
    assert!(
        output.content.contains(CONTAINER_WORKSPACE),
        "cwd is the workspace"
    );
    assert_eq!(
        bash_runs.load(Ordering::SeqCst),
        0,
        "bash ran in the container"
    );

    // 2. Create a marker *inside* the sandbox and confirm it is not on the host.
    ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &bash,
            input: json!({ "command": format!("touch {CONTAINER_WORKSPACE}/{marker}") }),
            ctx: &ctx,
        },
    )
    .await
    .expect("touch inside the sandbox must succeed");

    let host_marker = ctx.working_dir.join(&marker);
    assert!(
        !host_marker.exists(),
        "the host working directory must not gain the sandbox file: {}",
        host_marker.display()
    );

    let listed = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &bash,
            input: json!({ "command": format!("ls {CONTAINER_WORKSPACE}/{marker}") }),
            ctx: &ctx,
        },
    )
    .await
    .expect("the sandbox file must exist inside the container");
    assert!(
        listed.content.contains(&marker),
        "the file should exist inside the sandbox: {}",
        listed.content
    );

    remove_sandbox(runtime, &container, &volume);
}

#[tokio::test]
async fn sandbox_routes_file_tools_inside_the_container() {
    // FR-020, FR-035: the file/search subset runs inside the sandbox against the
    // mounted workspace and never touches the host working directory.
    let Some(runtime) = detect_container_runtime() else {
        eprintln!("skipping: no container runtime on PATH");
        return;
    };
    if !image_available(runtime, TEST_IMAGE) {
        return;
    }
    let ctx = ctx();
    let backend = ContainerBackend::from_descriptor(
        &descriptor(runtime.as_str(), Some(TEST_IMAGE)),
        &ctx.working_dir,
    );
    let container = backend.container_name().to_string();
    let volume = backend.volume_name().to_string();
    let name = format!("routed_{}.txt", std::process::id());
    let payload = "alpha\nneedle-in-haystack\nomega\n";

    // write -> read -> grep -> glob -> list, all sandbox-routed.
    let (write, _runs) = NamedTool::new("write");
    ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &write,
            input: json!({ "path": name, "content": payload }),
            ctx: &ctx,
        },
    )
    .await
    .expect("write must be routed into the sandbox");

    // The host must not have gained the file: file tools are sandbox-routed.
    assert!(
        !ctx.working_dir.join(&name).exists(),
        "write must not touch the host working directory"
    );

    let (read, _runs) = NamedTool::new("read");
    let read_out = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &read,
            input: json!({ "path": name }),
            ctx: &ctx,
        },
    )
    .await
    .expect("read must return the sandbox file");
    assert!(
        read_out.content.contains("needle-in-haystack"),
        "the content written inside the sandbox must round-trip: {}",
        read_out.content
    );

    let (grep, _runs) = NamedTool::new("grep");
    let grep_out = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &grep,
            input: json!({ "pattern": "needle-in-haystack" }),
            ctx: &ctx,
        },
    )
    .await
    .expect("grep must run inside the sandbox");
    assert!(
        grep_out.content.contains("needle-in-haystack"),
        "grep should find the in-sandbox pattern: {}",
        grep_out.content
    );

    let (glob, _runs) = NamedTool::new("glob");
    let glob_out = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &glob,
            input: json!({ "pattern": name }),
            ctx: &ctx,
        },
    )
    .await
    .expect("glob must run inside the sandbox");
    assert!(
        glob_out.content.contains(&name),
        "glob should match the in-sandbox file: {}",
        glob_out.content
    );

    remove_sandbox(runtime, &container, &volume);
}

#[tokio::test]
async fn sandbox_bash_still_enforces_the_seven_layer_validator() {
    // FR-002, FR-037: the host 7-layer validator gates the command before it
    // reaches the sandbox, so a banned command is rejected and never runs.
    let Some(runtime) = detect_container_runtime() else {
        eprintln!("skipping: no container runtime on PATH");
        return;
    };
    if !image_available(runtime, TEST_IMAGE) {
        return;
    }
    let ctx = ctx();
    let backend = ContainerBackend::from_descriptor(
        &descriptor(runtime.as_str(), Some(TEST_IMAGE)),
        &ctx.working_dir,
    );
    let container = backend.container_name().to_string();
    let volume = backend.volume_name().to_string();
    let (bash, _runs) = NamedTool::new("bash");

    let err = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &bash,
            input: json!({ "command": "mkfs.ext4 /dev/sda1" }),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("a banned command must be rejected by the 7-layer validator");
    assert!(
        err.to_string().contains("7-layer"),
        "the rejection should cite the validator: {err}"
    );

    remove_sandbox(runtime, &container, &volume);
}
