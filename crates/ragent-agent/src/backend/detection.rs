//! Container-runtime detection (spec `openhands` T-002; FR-026).
//!
//! A container execution backend can only be *selected* when the host has a
//! usable container runtime. This module probes `PATH` for the two supported
//! runtimes - `podman` and `docker` - and resolves which one a container
//! backend should use.
//!
//! FR-026 fixes two rules the code keeps explicit:
//!
//! - A container runtime is **optional**: ragent runs unchanged when neither is
//!   present, and never requires one for the default (`local`) backend.
//! - `podman` is the **default** container backend when no runtime is specified
//!   (or when both are present and none is named), so [`ContainerRuntime::default`]
//!   is `Podman`.
//!
//! Detection is a `PATH` probe only - it never spawns a process - so it is cheap
//! enough to run at startup, in the TUI, and in tests. Whether a *specific image*
//! can be pulled is a provisioning concern handled by the backend itself, which
//! fails the turn rather than falling back to host execution (FR-031).

use std::path::Path;

use ragent_config::ExecutionBackendKind;

/// A supported container runtime (FR-026).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerRuntime {
    /// Podman (`podman`); the default container runtime (FR-026).
    Podman,
    /// Docker (`docker`).
    Docker,
}

impl ContainerRuntime {
    /// The config/log label for this runtime (`podman`, `docker`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Podman => "podman",
            Self::Docker => "docker",
        }
    }

    /// The binary this runtime is invoked through.
    #[must_use]
    pub const fn binary(self) -> &'static str {
        self.as_str()
    }

    /// The [`ExecutionBackendKind`] this runtime backs.
    #[must_use]
    pub const fn kind(self) -> ExecutionBackendKind {
        match self {
            Self::Podman => ExecutionBackendKind::Podman,
            Self::Docker => ExecutionBackendKind::Docker,
        }
    }

    /// Map a backend kind to the runtime it names, or `None` for `local`/`remote`.
    #[must_use]
    pub const fn from_kind(kind: ExecutionBackendKind) -> Option<Self> {
        match kind {
            ExecutionBackendKind::Podman => Some(Self::Podman),
            ExecutionBackendKind::Docker => Some(Self::Docker),
            ExecutionBackendKind::Local | ExecutionBackendKind::Remote => None,
        }
    }
}

impl Default for ContainerRuntime {
    /// `podman` - the default container backend when none is specified (FR-026).
    fn default() -> Self {
        Self::Podman
    }
}

impl std::fmt::Display for ContainerRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether `name` resolves to an executable file on `PATH`.
///
/// On Windows the `PATHEXT` suffixes are also tried (`.exe`, `.cmd`, ...), since
/// `podman.exe`/`docker.exe` are the shipped names there.
#[must_use]
pub fn command_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let candidates = binary_candidates(name);
    std::env::split_paths(&path).any(|dir| {
        candidates
            .iter()
            .any(|candidate| is_executable(&dir.join(candidate)))
    })
}

/// The file names to look for a binary under on this platform.
fn binary_candidates(name: &str) -> Vec<String> {
    let candidates = vec![name.to_string()];
    #[cfg(windows)]
    {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_string());
        for ext in pathext.split(';').filter(|e| !e.is_empty()) {
            candidates.push(format!("{name}{}", ext.to_ascii_lowercase()));
            candidates.push(format!("{name}{}", ext.to_ascii_uppercase()));
        }
    }
    candidates
}

/// Whether `candidate` is a regular file (executability is not checked: a
/// non-executable file on `PATH` still surfaces a clear provisioning error later).
fn is_executable(candidate: &Path) -> bool {
    candidate.is_file()
}

/// Every container runtime present on this host, `podman` first (FR-026).
#[must_use]
pub fn detect_container_runtimes() -> Vec<ContainerRuntime> {
    let mut found = Vec::new();
    for runtime in [ContainerRuntime::Podman, ContainerRuntime::Docker] {
        if command_on_path(runtime.binary()) {
            found.push(runtime);
        }
    }
    found
}

/// The preferred container runtime present on this host, or `None` when neither
/// `podman` nor `docker` resolves on `PATH`.
///
/// When both are present `podman` wins, matching the FR-026 default.
#[must_use]
pub fn detect_container_runtime() -> Option<ContainerRuntime> {
    detect_container_runtimes().into_iter().next()
}

/// Resolve the runtime a container backend should use.
///
/// - `preferred` names the kind the caller asked for (`Some(Docker)` /
///   `Some(Podman)`); it is honoured even when the binary is absent, so the
///   backend fails at provisioning with a clear error naming the missing runtime
///   rather than silently switching (FR-031).
/// - `None` means "no runtime specified": the first *detected* runtime wins, or
///   [`ContainerRuntime::default`] (`podman`) when none is detected, so a backend
///   selected without a runtime still has a deterministic identity (FR-026).
#[must_use]
pub fn resolve_container_runtime(
    preferred: Option<ExecutionBackendKind>,
) -> Option<ContainerRuntime> {
    match preferred {
        Some(kind) => ContainerRuntime::from_kind(kind),
        None => Some(detect_container_runtime().unwrap_or_default()),
    }
}
