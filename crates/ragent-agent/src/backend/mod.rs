//! Execution backends: where a tool invocation actually runs.
//!
//! The agent loop, LLM providers, tool registry, permission system, and event
//! bus are unchanged by backend selection (spec `openhands` assumption A1):
//! only the tool-execution *leaf* is dispatched through the [`ExecutionBackend`]
//! trait. T-001 introduces the trait, the [`LocalBackend`] adapter over today's
//! host behaviour, and resolution from a configured
//! [`ExecutionBackendKind`](ragent_config::ExecutionBackendKind).
//!
//! T-002 adds the real container adapters: [`ContainerBackend`] runs tools inside
//! a `podman`/`docker` sandbox whose workspace is a container-named volume, so the
//! host working directory is never exposed to a sandbox tool (FR-020, FR-035), and
//! [`detection`] probes `PATH` for a runtime with `podman` as the default (FR-026).
//! A provisioning failure fails the call without falling back to host execution
//! (FR-031).
//!
//! T-003 adds [`RemoteBackend`] (module [`remote`]): it drives a second ragent
//! server over its REST+SSE API, mirrors that server's event stream into the local
//! bus, and surfaces mid-turn unreachability without local re-execution (FR-021,
//! FR-034). The session-level relay that renders those events lives in
//! [`crate::session::remote_dispatch`].
//!
//! T-006 adds [`secrets`]: a container or remote backend resolves the credentials
//! it needs from the encrypted credential store **at spawn time** and injects them
//! as environment variables, never baking them into an image or a workspace
//! (FR-010). A sandbox's named volume - the workspace - is never removed on
//! replacement, so a conversation's files survive a container restart (FR-023).
//!
//! T-004 adds [`registry`]: the durable backend registry (FR-004) built from the
//! configured [`BackendConfig`] entries, exposing each backend's stable id, display
//! name, kind, secret-free connection descriptor, and health state, plus remote
//! registration by URL and key (FR-030) and a live `/health` probe.

pub mod container;
pub mod detection;
pub mod registry;
pub mod remote;
pub mod secrets;

pub use container::{CONTAINER_WORKSPACE, ContainerBackend};
pub use detection::{ContainerRuntime, detect_container_runtime, resolve_container_runtime};
pub use registry::{
    BackendRegistry, BackendRegistryEntry, ConnectionDescriptor, HealthState, LOCAL_BACKEND_ID,
    probe_remote,
};
pub use remote::{
    RemoteBackend, RemoteError, RemoteErrorKind, RemoteTurnOutcome, RemoteUpdate,
    cached_remote_session, forget_remote_session, parse_remote_update, relay_turn,
    remember_remote_session, remote_backend_config, split_sse_frames, validate_remote_backend,
};
pub use secrets::{
    MapSecretResolver, ResolvedSecret, SecretResolver, StorageSecretResolver, container_env_args,
    credential_names, resolve_descriptor_secrets,
};

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use ragent_config::{BackendConfig, Config, ExecutionBackendKind};

use crate::tool::{Tool, ToolContext, ToolOutput};

/// Why a backend could not satisfy a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendErrorKind {
    /// The backend could not be provisioned (container start, remote handshake,
    /// missing runtime, ...). The turn must fail rather than run on the host
    /// (FR-031).
    Provision,
    /// The backend kind has no adapter for this build (the `docker`/`podman`
    /// [`PendingBackend`] resolved without a descriptor, or a tool with no sandbox
    /// adapter). The call must fail rather than run on the host (FR-031).
    Unavailable,
    /// The backend was reachable but the protocol exchange failed midway
    /// (for example a remote stream that dropped during a turn, FR-034).
    Protocol,
}

/// A backend-level failure carrying the backend that failed and a coarse cause.
///
/// Implements [`std::error::Error`] so it can travel inside an
/// [`anyhow::Error`]; callers that need the structured cause downcast it with
/// [`anyhow::Error::downcast_ref`]. Tool-execution errors raised *by* a tool are
/// propagated untouched, so an existing error chain is never flattened into a
/// string here.
#[derive(Debug)]
pub struct BackendError {
    backend: ExecutionBackendKind,
    error_kind: BackendErrorKind,
    message: String,
}

impl BackendError {
    /// Build a backend failure.
    #[must_use]
    pub fn new(
        backend: ExecutionBackendKind,
        error_kind: BackendErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            backend,
            error_kind,
            message: message.into(),
        }
    }

    /// The backend kind that failed.
    #[must_use]
    pub const fn backend(&self) -> ExecutionBackendKind {
        self.backend
    }

    /// The coarse cause of the failure.
    #[must_use]
    pub const fn error_kind(&self) -> BackendErrorKind {
        self.error_kind
    }

    /// The human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "execution backend '{}' failed ({:?}): {}",
            self.backend, self.error_kind, self.message
        )
    }
}

impl std::error::Error for BackendError {}

/// A single tool invocation routed to a backend.
///
/// Bundles the tool, its parsed input, and the session context so a backend can
/// dispatch without borrowing a wide argument list. Input is owned because the
/// dispatch consumes it, matching the [`Tool::execute`] contract.
pub struct BackendToolCall<'a> {
    /// The tool to run.
    pub tool: &'a dyn Tool,
    /// The tool's parsed JSON input.
    pub input: Value,
    /// The session context the tool executes against.
    pub ctx: &'a ToolContext,
}

/// Where a tool invocation runs (spec `openhands` FR-001).
///
/// Implementations must be `Send + Sync + 'static` so a backend handle can be
/// cloned into the per-tool dispatch task. A backend must never re-route a call
/// to host execution when it cannot satisfy it - it fails instead (FR-031).
#[async_trait]
pub trait ExecutionBackend: Send + Sync {
    /// The kind this adapter implements.
    fn kind(&self) -> ExecutionBackendKind;

    /// Execute `call` in this backend.
    ///
    /// # Errors
    ///
    /// Returns a [`BackendError`] (wrapped in [`anyhow::Error`]) when the
    /// backend cannot run the call, or the tool's own error when the tool
    /// itself fails.
    async fn execute_tool(&self, call: BackendToolCall<'_>) -> anyhow::Result<ToolOutput>;
}

/// The host-execution adapter: runs the tool exactly as today (FR-019).
///
/// This is the only adapter implemented in T-001. It is a thin pass-through, so
/// selecting `local` is behaviour-preserving for every existing session.
pub struct LocalBackend;

impl LocalBackend {
    /// Construct the local adapter (zero-sized).
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LocalBackend {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl ExecutionBackend for LocalBackend {
    fn kind(&self) -> ExecutionBackendKind {
        ExecutionBackendKind::Local
    }

    async fn execute_tool(&self, call: BackendToolCall<'_>) -> anyhow::Result<ToolOutput> {
        call.tool.execute(call.input, call.ctx).await
    }
}

/// Placeholder adapter for a backend kind that has no implementation yet.
///
/// It never runs the tool. Every call fails with [`BackendErrorKind::Unavailable`]
/// so a session configured for a non-local backend fails loudly instead of
/// silently executing on the host (FR-031).
pub struct PendingBackend {
    kind: ExecutionBackendKind,
}

impl PendingBackend {
    /// Construct a placeholder for `kind`.
    #[must_use]
    pub const fn new(kind: ExecutionBackendKind) -> Self {
        Self { kind }
    }
}

#[async_trait]
impl ExecutionBackend for PendingBackend {
    fn kind(&self) -> ExecutionBackendKind {
        self.kind
    }

    async fn execute_tool(&self, call: BackendToolCall<'_>) -> anyhow::Result<ToolOutput> {
        let tool = call.tool.name().to_string();
        tracing::warn!(
            backend = self.kind.as_str(),
            tool = %tool,
            "tool call refused: execution backend has no adapter in this build"
        );
        Err(BackendError::new(
            self.kind,
            BackendErrorKind::Unavailable,
            format!(
                "execution backend '{}' is not implemented in this build; \
                 tool '{tool}' was not executed on the host",
                self.kind
            ),
        )
        .into())
    }
}

/// A shared handle to the `local` backend, created once per process.
///
/// Most sessions run on the host, so resolving a fresh `Arc` per step would
/// allocate for no reason; the zero-sized adapter is cached instead.
#[must_use]
pub fn local_backend() -> Arc<dyn ExecutionBackend> {
    static LOCAL: std::sync::OnceLock<Arc<dyn ExecutionBackend>> = std::sync::OnceLock::new();
    LOCAL.get_or_init(|| Arc::new(LocalBackend::new())).clone()
}

/// Resolve the adapter for `kind` (FR-001, FR-019).
///
/// `Local` resolves to the host adapter, `Docker`/`Podman` to a
/// [`PendingBackend`] because a container needs a descriptor (image, workspace),
/// and `Remote` to a [`RemoteBackend`] with no URL, which fails at provisioning.
/// Use [`resolve_backend_for_config`] on a dispatch path so the descriptor's
/// image, workspace, URL, and key are honoured.
#[must_use]
pub fn resolve_backend(kind: ExecutionBackendKind) -> Arc<dyn ExecutionBackend> {
    match kind {
        ExecutionBackendKind::Local => local_backend(),
        ExecutionBackendKind::Docker | ExecutionBackendKind::Podman => {
            Arc::new(PendingBackend::new(kind))
        }
        ExecutionBackendKind::Remote => Arc::new(RemoteBackend::from_descriptor(
            &ragent_config::BackendConfig::default(),
        )),
    }
}

/// Resolve the adapter for a full config, honouring a container descriptor
/// (FR-001, FR-009, FR-020, FR-026) or a remote descriptor (FR-021).
///
/// Unlike [`resolve_backend`] this reads the configured backend entry, so a
/// `docker`/`podman` selection carries its image and workspace into a real
/// [`ContainerBackend`], and a `remote` selection carries its URL and key into a
/// [`RemoteBackend`]. Each result is cached for the life of the process - keyed by
/// the descriptor's connection fields so a session reuses one container or one
/// remote connection pool (FR-009) instead of provisioning a new one per tool
/// call.
#[must_use]
pub fn resolve_backend_for_config(
    config: &Config,
    working_dir: &std::path::Path,
) -> Arc<dyn ExecutionBackend> {
    resolve_backend_with_secrets(config, working_dir, None)
}

/// Resolve the adapter for a full config, wiring in the encrypted credential
/// store so a sandbox's credentials are resolved at spawn time (FR-010).
///
/// This is [`resolve_backend_for_config`] plus the credential seam: a
/// `docker`/`podman`/`remote` descriptor that names credentials resolves their
/// values from `storage` (the encrypted SQLite store) when the sandbox is
/// provisioned, never from the descriptor. A remote resolution failure is
/// surfaced by the backend at provisioning; a container resolution failure is
/// the [`BackendErrorKind::Provision`] failure the spawn reports.
///
/// Pass `None` for `storage` to build the adapters without a credential store;
/// a descriptor that names a credential then fails at provisioning rather than
/// starting a sandbox without it (FR-010, FR-031).
#[must_use]
pub fn resolve_backend_with_secrets(
    config: &Config,
    working_dir: &std::path::Path,
    storage: Option<Arc<crate::storage::Storage>>,
) -> Arc<dyn ExecutionBackend> {
    let kind = config.effective_execution_backend();
    let has_store = storage.is_some();
    let resolver: Arc<dyn SecretResolver> = match storage {
        Some(storage) => Arc::new(StorageSecretResolver::new(storage)),
        None => Arc::new(MapSecretResolver::default()),
    };
    match kind {
        ExecutionBackendKind::Local => local_backend(),
        ExecutionBackendKind::Docker | ExecutionBackendKind::Podman => {
            let descriptor = config
                .effective_backend_config()
                .cloned()
                .unwrap_or_default();
            let key = backend_cache_key(
                if has_store {
                    "container-store"
                } else {
                    "container"
                },
                &descriptor,
                working_dir,
            );
            cached_backend(&key, || {
                Arc::new(
                    ContainerBackend::from_descriptor(&descriptor, working_dir)
                        .with_secret_resolver(Arc::clone(&resolver)),
                )
            })
        }
        ExecutionBackendKind::Remote => {
            let descriptor = config
                .effective_backend_config()
                .cloned()
                .unwrap_or_default();
            // The remote descriptor's URL is the connection identity; the working
            // directory does not affect the remote conversation, so it is left out
            // of the key to keep two local sessions on one server sharing a pool.
            let key = remote_cache_key(&descriptor, has_store);
            cached_backend(&key, || {
                let backend = RemoteBackend::resolve_credentials(&descriptor, resolver.as_ref())
                    .unwrap_or_else(|_| RemoteBackend::from_descriptor(&descriptor));
                Arc::new(backend)
            })
        }
    }
}

/// Process-wide backend-adapter cache keyed by a descriptor's connection fields,
/// so repeated dispatch of one session's backend reuses one container or one
/// remote connection pool (FR-009) instead of provisioning a new one per tool.
type BackendCache = std::sync::Mutex<std::collections::HashMap<String, Arc<dyn ExecutionBackend>>>;

fn backend_cache() -> &'static BackendCache {
    static CACHE: std::sync::OnceLock<BackendCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Get-or-insert a cached backend under `key`.
fn cached_backend<F>(key: &str, build: F) -> Arc<dyn ExecutionBackend>
where
    F: FnOnce() -> Arc<dyn ExecutionBackend>,
{
    let cache = backend_cache();
    let mut guard = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.entry(key.to_string()).or_insert_with(build).clone()
}

/// The cache key identifying one container descriptor + host working directory.
fn backend_cache_key(
    scope: &str,
    descriptor: &BackendConfig,
    working_dir: &std::path::Path,
) -> String {
    // The credential *names* are part of the identity: a descriptor that differs
    // only in its credential set must not reuse an adapter built for a different
    // set (the container name and injected environment both depend on it; FR-010).
    let credentials = descriptor.credentials.join(",");
    format!(
        "{scope}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        descriptor.kind,
        descriptor.id,
        descriptor.image.as_deref().unwrap_or(""),
        descriptor.workspace.as_deref().unwrap_or(""),
        working_dir.display(),
        credentials,
    )
}

/// The cache key identifying one remote backend: its descriptor connection fields
/// (`url` + `api_key` + credential names + `id`) plus whether a credential store
/// was wired in, so a reused selection shares one HTTP client, while a changed
/// credential set or a different store does not reuse a handle built for another
/// key (FR-010).
fn remote_cache_key(descriptor: &BackendConfig, has_store: bool) -> String {
    format!(
        "remote\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        descriptor.id,
        descriptor.url.as_deref().unwrap_or(""),
        descriptor.api_key.as_deref().unwrap_or(""),
        descriptor.credentials.join(","),
        has_store,
    )
}
