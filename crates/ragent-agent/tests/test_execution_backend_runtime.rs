//! Integration tests for the execution-backend trait and the `local` adapter
//! (spec `openhands` T-001; FR-001, FR-019, FR-031).
//!
//! These tests exercise the backend dispatch contract directly: the `local`
//! adapter runs a tool exactly as before (FR-019), and an unimplemented
//! non-local kind fails the call without ever invoking the tool on the host
//! (FR-031).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use ragent_agent::backend::{
    BackendError, BackendToolCall, ExecutionBackend, LocalBackend, resolve_backend,
};
use ragent_agent::event::EventBus;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput};
use ragent_config::{Config, ExecutionBackendKind};
use serde_json::{Value, json};

/// A tool that records how many times it was actually run and returns a marker.
struct CountingTool {
    runs: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingTool {
    fn name(&self) -> &'static str {
        "counting_tool"
    }

    fn description(&self) -> &'static str {
        "test double that counts its invocations"
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
            content: "counting_tool ran".to_string(),
            metadata: None,
        })
    }
}

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "session-1".to_string(),
        working_dir: std::path::PathBuf::from("target/temp"),
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

#[tokio::test]
async fn local_adapter_runs_the_tool_on_the_host() {
    // FR-001, FR-019: `local` is the host adapter and executes the tool as today.
    let runs = Arc::new(AtomicUsize::new(0));
    let tool = CountingTool {
        runs: Arc::clone(&runs),
    };
    let ctx = ctx();

    let backend = LocalBackend::new();
    assert_eq!(backend.kind(), ExecutionBackendKind::Local);
    let output = ExecutionBackend::execute_tool(
        &backend,
        BackendToolCall {
            tool: &tool,
            input: json!({}),
            ctx: &ctx,
        },
    )
    .await
    .expect("local backend must run the tool");

    assert_eq!(output.content, "counting_tool ran");
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn resolve_local_returns_a_runnable_backend() {
    let runs = Arc::new(AtomicUsize::new(0));
    let tool = CountingTool {
        runs: Arc::clone(&runs),
    };
    let ctx = ctx();

    let backend = resolve_backend(ExecutionBackendKind::Local);
    assert_eq!(backend.kind(), ExecutionBackendKind::Local);
    ExecutionBackend::execute_tool(
        backend.as_ref(),
        BackendToolCall {
            tool: &tool,
            input: json!({}),
            ctx: &ctx,
        },
    )
    .await
    .expect("resolved local backend must run the tool");
    assert_eq!(runs.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unimplemented_kinds_fail_without_running_on_the_host() {
    // FR-001, FR-031: docker/podman/remote are part of the vocabulary but have
    // no adapter yet; dispatching to them fails and never falls back to host
    // execution.
    for kind in [
        ExecutionBackendKind::Docker,
        ExecutionBackendKind::Podman,
        ExecutionBackendKind::Remote,
    ] {
        let runs = Arc::new(AtomicUsize::new(0));
        let tool = CountingTool {
            runs: Arc::clone(&runs),
        };
        let ctx = ctx();

        let backend = resolve_backend(kind);
        assert_eq!(backend.kind(), kind);
        let err = ExecutionBackend::execute_tool(
            backend.as_ref(),
            BackendToolCall {
                tool: &tool,
                input: json!({}),
                ctx: &ctx,
            },
        )
        .await
        .expect_err("an unimplemented backend must fail");

        assert_eq!(
            runs.load(Ordering::SeqCst),
            0,
            "kind {kind} must not execute the tool on the host"
        );
        let backend_err = err
            .downcast_ref::<BackendError>()
            .expect("error should carry the structured backend failure");
        assert_eq!(backend_err.backend(), kind);
        assert_eq!(
            backend_err.error_kind(),
            ragent_agent::backend::BackendErrorKind::Unavailable
        );
        assert!(
            err.to_string().contains(kind.as_str()),
            "the error should name the failing backend: {err}"
        );
    }
}

#[tokio::test]
async fn configured_backend_kind_selects_the_matching_adapter() {
    // The processor resolves `ctx.config.effective_execution_backend()` into a
    // backend per dispatch; this asserts that resolution maps a configured kind
    // to the right adapter. `local` runs the tool; a configured `podman` refuses.
    let local_cfg: Config =
        serde_json::from_str(r#"{ "execution_backend": "local" }"#).expect("parse");
    assert_eq!(
        local_cfg.effective_execution_backend(),
        ExecutionBackendKind::Local
    );

    let podman_cfg: Config =
        serde_json::from_str(r#"{ "execution_backend": "podman" }"#).expect("parse");
    assert_eq!(
        podman_cfg.effective_execution_backend(),
        ExecutionBackendKind::Podman
    );

    let runs = Arc::new(AtomicUsize::new(0));
    let tool = CountingTool {
        runs: Arc::clone(&runs),
    };
    let ctx = ctx();

    // `local`: runs.
    let local_backend = resolve_backend(local_cfg.effective_execution_backend());
    ExecutionBackend::execute_tool(
        local_backend.as_ref(),
        BackendToolCall {
            tool: &tool,
            input: json!({}),
            ctx: &ctx,
        },
    )
    .await
    .expect("local should run");
    assert_eq!(runs.load(Ordering::SeqCst), 1);

    // `podman`: refuses, no host execution.
    let podman_backend = resolve_backend(podman_cfg.effective_execution_backend());
    let err = ExecutionBackend::execute_tool(
        podman_backend.as_ref(),
        BackendToolCall {
            tool: &tool,
            input: json!({}),
            ctx: &ctx,
        },
    )
    .await
    .expect_err("podman is unimplemented in this build");
    assert!(err.to_string().contains("podman"));
    assert_eq!(runs.load(Ordering::SeqCst), 1, "no extra host execution");
}

#[tokio::test]
async fn default_config_dispatches_to_local() {
    // FR-019: with nothing configured, the effective kind is `local`, so the
    // processor's per-dispatch resolution yields the host adapter.
    let config = Config::default();
    assert_eq!(
        config.effective_execution_backend(),
        ExecutionBackendKind::Local
    );
    let backend = resolve_backend(config.effective_execution_backend());
    assert_eq!(backend.kind(), ExecutionBackendKind::Local);
}
