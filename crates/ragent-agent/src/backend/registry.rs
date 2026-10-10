//! The durable backend registry (spec `openhands` T-004; FR-004, FR-030).
//!
//! FR-004 requires each configured execution backend to be exposed as a durable
//! registry entry carrying a stable id, a display name, a kind, a connection
//! descriptor, and a health state. This module builds that view from the durable
//! configuration ([`Config::backends`] plus the active `execution_backend`), so
//! the registry is always reconstructable from disk - the config file is the
//! durable record, the registry is its resolved read model.
//!
//! The registry never executes a tool. It is consumed by the TUI backend surface
//! (T-005, FR-008) to show the active backend and the health of every registered
//! backend, and by remote registration (FR-030) to add a remote server by URL and
//! key.
//!
//! # Secret safety
//!
//! A [`ConnectionDescriptor`] carries credential *names* and presence flags only -
//! never a credential value (FR-010, FR-035). Remote health probing targets the
//! server's public `/health` endpoint, so no bearer key is materialised or sent.
//!
//! # Health states
//!
//! - `local` is always `ok` (the host is always available).
//! - `docker`/`podman` are `ok` when the runtime binary resolves on `PATH`, else
//!   `unavailable` with a detail naming the missing binary (FR-026).
//! - `remote` is `ok` when a base URL and a key are configured (FR-030), then
//!   refined by a live `/health` probe ([`BackendRegistry::refresh_health`],
//!   FR-004).

use std::path::Path;
use std::time::Duration;

use ragent_config::{BackendConfig, Config, ExecutionBackendKind};

use super::container::ContainerBackend;
use super::detection::{ContainerRuntime, command_on_path};
use super::remote::RemoteBackend;
use super::secrets::credential_names;

/// The stable registry id of the built-in host backend (FR-019).
pub const LOCAL_BACKEND_ID: &str = "local";

/// How long a remote health probe waits for `/health` before giving up.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// The health state of a registered backend (FR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HealthState {
    /// Not yet probed.
    #[default]
    Unknown,
    /// Reachable / runnable.
    Ok,
    /// Present but not currently reachable or runnable.
    Unavailable,
}

impl HealthState {
    /// The stable lowercase label (`ok`, `unavailable`, `unknown`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unavailable => "unavailable",
            Self::Unknown => "unknown",
        }
    }

    /// Whether the backend is healthy.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The non-secret connection details of a registered backend (FR-004).
///
/// A descriptor records *how* a backend is reached - a runtime and image for a
/// container, a base URL for a remote - plus the credential *names* it will
/// resolve at spawn time. It never holds a credential value (FR-010, FR-035).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionDescriptor {
    /// Host execution; no connection.
    Local,
    /// A container sandbox (FR-020).
    Container {
        /// The container runtime backing the sandbox.
        runtime: ContainerRuntime,
        /// The container image, when one was configured.
        image: Option<String>,
        /// Container-side workspace mount point.
        workspace: String,
        /// The sandbox container name.
        container: String,
        /// The named volume backing the workspace.
        volume: String,
        /// Credential names resolved at spawn time (never values).
        credential_names: Vec<String>,
    },
    /// A remote ragent server (FR-021, FR-030).
    Remote {
        /// The configured base URL.
        url: Option<String>,
        /// Whether a bearer key is configured (a literal `api_key` or a named
        /// credential resolved from the store).
        has_api_key: bool,
        /// Credential names resolved at spawn time (never values).
        credential_names: Vec<String>,
    },
}

impl ConnectionDescriptor {
    /// Build the descriptor for `descriptor`, or `None` when its `kind` is not a
    /// known backend kind (FR-004).
    ///
    /// `host_working_dir` is only consulted for a container kind, to derive the
    /// sandbox's deterministic container and volume names; a `local`/`remote`
    /// descriptor ignores it.
    #[must_use]
    pub fn from_descriptor(descriptor: &BackendConfig, host_working_dir: &Path) -> Option<Self> {
        match descriptor.kind_parsed()? {
            ExecutionBackendKind::Local => Some(Self::Local),
            ExecutionBackendKind::Docker | ExecutionBackendKind::Podman => {
                let backend = ContainerBackend::from_descriptor(descriptor, host_working_dir);
                Some(Self::Container {
                    runtime: backend.runtime(),
                    image: descriptor
                        .image
                        .as_deref()
                        .map(str::trim)
                        .filter(|image| !image.is_empty())
                        .map(str::to_string),
                    workspace: backend.workspace().to_string(),
                    container: backend.container_name().to_string(),
                    volume: backend.volume_name().to_string(),
                    credential_names: credential_names(descriptor),
                })
            }
            ExecutionBackendKind::Remote => {
                let backend = RemoteBackend::from_descriptor(descriptor);
                let names = credential_names(descriptor);
                Some(Self::Remote {
                    url: backend.url().map(str::to_string),
                    has_api_key: backend.has_bearer_key() || !names.is_empty(),
                    credential_names: names,
                })
            }
        }
    }

    /// A one-line, secret-free description for a list/detail surface.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Local => "host execution".to_string(),
            Self::Container {
                runtime,
                image,
                workspace,
                container,
                volume,
                credential_names,
            } => format!(
                "{runtime} container={container} image={} workspace={workspace} volume={volume}{}",
                image.as_deref().unwrap_or("<no image>"),
                credential_suffix(credential_names),
            ),
            Self::Remote {
                url,
                has_api_key,
                credential_names,
            } => format!(
                "{} key={}{}",
                url.as_deref().unwrap_or("<no url>"),
                if *has_api_key { "set" } else { "absent" },
                credential_suffix(credential_names),
            ),
        }
    }

    /// Whether this descriptor names any credentials (FR-010).
    #[must_use]
    pub fn credential_names(&self) -> &[String] {
        match self {
            Self::Local => &[],
            Self::Container {
                credential_names, ..
            }
            | Self::Remote {
                credential_names, ..
            } => credential_names,
        }
    }
}

/// Render ` credentials=N` when a descriptor names credentials, else nothing.
fn credential_suffix(names: &[String]) -> String {
    if names.is_empty() {
        String::new()
    } else {
        format!(" credentials={}", names.len())
    }
}

/// A durable backend registry entry (FR-004).
#[derive(Debug, Clone)]
pub struct BackendRegistryEntry {
    id: String,
    name: String,
    kind: ExecutionBackendKind,
    connection: ConnectionDescriptor,
    health: HealthState,
    detail: Option<String>,
}

impl BackendRegistryEntry {
    /// Build an entry, deriving its initial health from the kind and descriptor.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ExecutionBackendKind,
        connection: ConnectionDescriptor,
    ) -> Self {
        let (health, detail) = initial_health(kind, &connection);
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            connection,
            health,
            detail,
        }
    }

    /// The built-in `local` entry - always registered and always healthy
    /// (FR-019).
    #[must_use]
    pub fn local() -> Self {
        Self::new(
            LOCAL_BACKEND_ID,
            "Local (host)",
            ExecutionBackendKind::Local,
            ConnectionDescriptor::Local,
        )
    }

    /// Build an entry from a configured descriptor, or `None` when its `kind` is
    /// missing or unknown (FR-004).
    #[must_use]
    pub fn from_descriptor(descriptor: &BackendConfig, host_working_dir: &Path) -> Option<Self> {
        let kind = descriptor.kind_parsed()?;
        let connection = ConnectionDescriptor::from_descriptor(descriptor, host_working_dir)?;
        Some(Self::new(
            stable_id(descriptor, kind),
            descriptor.display_name().to_string(),
            kind,
            connection,
        ))
    }

    /// Build a `remote` entry from a descriptor, registering it reachable when a
    /// base URL and a key are configured (FR-030).
    ///
    /// Returns `None` when the descriptor is not a `remote` kind.
    #[must_use]
    pub fn remote_from_descriptor(descriptor: &BackendConfig) -> Option<Self> {
        if descriptor.kind_parsed() != Some(ExecutionBackendKind::Remote) {
            return None;
        }
        let backend = RemoteBackend::from_descriptor(descriptor);
        let names = credential_names(descriptor);
        let connection = ConnectionDescriptor::Remote {
            url: backend.url().map(str::to_string),
            has_api_key: backend.has_bearer_key() || !names.is_empty(),
            credential_names: names,
        };
        Some(Self::new(
            stable_id(descriptor, ExecutionBackendKind::Remote),
            descriptor.display_name().to_string(),
            ExecutionBackendKind::Remote,
            connection,
        ))
    }

    /// The stable registry id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The backend kind.
    #[must_use]
    pub const fn kind(&self) -> ExecutionBackendKind {
        self.kind
    }

    /// The connection descriptor.
    #[must_use]
    pub const fn connection(&self) -> &ConnectionDescriptor {
        &self.connection
    }

    /// The current health state.
    #[must_use]
    pub const fn health(&self) -> HealthState {
        self.health
    }

    /// A human-readable detail for a non-`ok` health, if any.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// Record a new health state and detail.
    pub fn set_health(&mut self, health: HealthState, detail: Option<String>) {
        self.health = health;
        self.detail = detail;
    }

    /// Re-probe this entry's live health (FR-004).
    ///
    /// `local` is always `ok`; a container checks its runtime on `PATH` (FR-026);
    /// a remote probes the server's public `/health` endpoint (FR-030).
    pub async fn refresh_health(&mut self) {
        match self.kind {
            ExecutionBackendKind::Local => self.set_health(HealthState::Ok, None),
            ExecutionBackendKind::Docker | ExecutionBackendKind::Podman => {
                let (health, detail) = container_health(self.kind);
                self.set_health(health, detail);
            }
            ExecutionBackendKind::Remote => {
                let url = match &self.connection {
                    ConnectionDescriptor::Remote { url, .. } => url.clone(),
                    _ => None,
                };
                let (health, detail) = probe_remote(url.as_deref()).await;
                self.set_health(health, detail);
            }
        }
    }
}

/// The registry id for a descriptor: its trimmed `id`, or the kind label when the
/// id is empty (so a descriptor with no id is still addressable).
fn stable_id(descriptor: &BackendConfig, kind: ExecutionBackendKind) -> String {
    let id = descriptor.id.trim();
    if id.is_empty() {
        kind.as_str().to_string()
    } else {
        id.to_string()
    }
}

/// Probe a remote server's `/health` endpoint for a health state (FR-004,
/// FR-030).
///
/// ragent serves `/health` publicly (before its bearer-token check), so the probe
/// needs no key. A missing URL, a transport failure, or a non-2xx answer yields
/// [`HealthState::Unavailable`] with a detail that never carries a URL credential.
#[must_use]
pub async fn probe_remote(url: Option<&str>) -> (HealthState, Option<String>) {
    let Some(base) = url.map(str::trim).filter(|u| !u.is_empty()) else {
        return (
            HealthState::Unavailable,
            Some("no base URL configured; set `url` on the backend descriptor".to_string()),
        );
    };
    let base = base.trim_end_matches('/');
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(PROBE_TIMEOUT)
        .timeout(PROBE_TIMEOUT)
        .build()
    else {
        return (
            HealthState::Unavailable,
            Some("could not build an HTTP client for the health probe".to_string()),
        );
    };
    match client.get(format!("{base}/health")).send().await {
        Ok(response) if response.status().is_success() => (HealthState::Ok, None),
        Ok(response) => (
            HealthState::Unavailable,
            Some(format!(
                "health check returned HTTP {}",
                response.status().as_u16()
            )),
        ),
        Err(error) => (
            HealthState::Unavailable,
            Some(format!("unreachable: {error}")),
        ),
    }
}

/// The durable view of every configured execution backend (FR-004).
///
/// Built from [`Config`] so it is reconstructable from disk; entries are unique by
/// id. The built-in `local` backend is always present (FR-019).
#[derive(Debug, Clone, Default)]
pub struct BackendRegistry {
    entries: Vec<BackendRegistryEntry>,
}

impl BackendRegistry {
    /// An empty registry (no `local` entry).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Build the registry for `config`, always including `local` and every
    /// configured backend (FR-004, FR-019).
    ///
    /// Sources, in order: the built-in `local` entry, each entry in
    /// [`Config::backends`], the active backend (an inline `execution_backend`
    /// descriptor or the entry it names by id), and - for a bare kind label such
    /// as `execution_backend: "podman"` with no matching descriptor - a synthesized
    /// entry so the active backend is always visible. Duplicate ids are collapsed.
    #[must_use]
    pub fn from_config(config: &Config, host_working_dir: &Path) -> Self {
        let mut registry = Self::new();
        registry.push(BackendRegistryEntry::local());

        for descriptor in &config.backends {
            if let Some(entry) = BackendRegistryEntry::from_descriptor(descriptor, host_working_dir)
            {
                registry.push_if_absent(entry);
            }
        }
        if let Some(descriptor) = config.effective_backend_config() {
            if let Some(entry) = BackendRegistryEntry::from_descriptor(descriptor, host_working_dir)
            {
                registry.push_if_absent(entry);
            }
        }

        let kind = config.effective_execution_backend();
        if !kind.is_local() && !registry.any_of_kind(kind) {
            let descriptor = BackendConfig {
                id: kind.as_str().to_string(),
                kind: kind.as_str().to_string(),
                ..BackendConfig::default()
            };
            if let Some(entry) =
                BackendRegistryEntry::from_descriptor(&descriptor, host_working_dir)
            {
                registry.push_if_absent(entry);
            }
        }

        registry
    }

    /// Append an entry unconditionally.
    pub fn push(&mut self, entry: BackendRegistryEntry) {
        self.entries.push(entry);
    }

    /// Append an entry unless one with the same id is already registered.
    pub fn push_if_absent(&mut self, entry: BackendRegistryEntry) {
        if self.get(entry.id()).is_none() {
            self.entries.push(entry);
        }
    }

    /// Register a remote backend by URL and key, replacing any entry with the
    /// same id (FR-030).
    ///
    /// Returns the registered entry, or `None` when `descriptor` is not a
    /// `remote` kind.
    pub fn register_remote(&mut self, descriptor: &BackendConfig) -> Option<&BackendRegistryEntry> {
        let entry = BackendRegistryEntry::remote_from_descriptor(descriptor)?;
        let id = entry.id().to_string();
        if let Some(slot) = self.get_mut(&id) {
            *slot = entry;
        } else {
            self.entries.push(entry);
        }
        self.get(&id)
    }

    /// Every registered entry, in registration order.
    #[must_use]
    pub fn entries(&self) -> &[BackendRegistryEntry] {
        &self.entries
    }

    /// The number of registered entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entry is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether at least one registered entry has the given backend kind.
    #[must_use]
    pub fn any_of_kind(&self, kind: ExecutionBackendKind) -> bool {
        self.entries.iter().any(|entry| entry.kind == kind)
    }

    /// Look up an entry by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&BackendRegistryEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Look up a mutable entry by id.
    #[must_use]
    pub fn get_mut(&mut self, id: &str) -> Option<&mut BackendRegistryEntry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    /// The id of the backend `config` resolves to as active (FR-019).
    ///
    /// Prefers the id of the active descriptor, then the first entry whose kind
    /// matches the effective kind; returns `None` only when nothing matches.
    #[must_use]
    pub fn active_id(&self, config: &Config) -> Option<&str> {
        if let Some(descriptor) = config.effective_backend_config() {
            if let Some(entry) = self.get(&descriptor.id) {
                return Some(entry.id.as_str());
            }
        }
        let kind = config.effective_execution_backend();
        if kind.is_local() {
            return self.get(LOCAL_BACKEND_ID).map(|entry| entry.id.as_str());
        }
        self.entries
            .iter()
            .find(|entry| entry.kind == kind)
            .map(|entry| entry.id.as_str())
    }

    /// Re-probe every registered backend and record its live health (FR-004,
    /// FR-008).
    pub async fn refresh_health(&mut self) {
        for entry in &mut self.entries {
            entry.refresh_health().await;
        }
    }
}

/// The health of a container backend: `ok` when its runtime resolves on `PATH`,
/// otherwise `unavailable` naming the missing runtime (FR-004, FR-026).
fn container_health(kind: ExecutionBackendKind) -> (HealthState, Option<String>) {
    let runtime = ContainerRuntime::from_kind(kind).unwrap_or_default();
    if command_on_path(runtime.binary()) {
        (HealthState::Ok, None)
    } else {
        (
            HealthState::Unavailable,
            Some(format!("container runtime '{runtime}' not found on PATH")),
        )
    }
}

/// The initial health of an entry, before any live probe (FR-004, FR-026,
/// FR-030).
fn initial_health(
    kind: ExecutionBackendKind,
    connection: &ConnectionDescriptor,
) -> (HealthState, Option<String>) {
    match kind {
        ExecutionBackendKind::Local => (HealthState::Ok, None),
        ExecutionBackendKind::Docker | ExecutionBackendKind::Podman => container_health(kind),
        ExecutionBackendKind::Remote => match connection {
            ConnectionDescriptor::Remote {
                url: Some(_),
                has_api_key: true,
                ..
            } => (HealthState::Ok, None),
            ConnectionDescriptor::Remote { url: Some(_), .. } => (
                HealthState::Unavailable,
                Some(
                    "no API key configured; set `api_key` or name a credential on \
                     the backend descriptor"
                        .to_string(),
                ),
            ),
            _ => (
                HealthState::Unavailable,
                Some("no base URL configured; set `url` on the backend descriptor".to_string()),
            ),
        },
    }
}
