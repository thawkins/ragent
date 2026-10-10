//! Container execution backend: run tools inside a `podman`/`docker` sandbox
//! (spec `openhands` T-002; FR-001, FR-002, FR-009, FR-020, FR-026, FR-031, FR-035).
//!
//! [`ContainerBackend`] answers the "where" half of a tool invocation: instead of
//! running the tool on the host it provisions a container, mounts a workspace, and
//! dispatches the invocation into it with `<runtime> exec`.
//!
//! ## Session lifecycle (FR-009, FR-020)
//!
//! On the first dispatch the backend starts **one long-lived container** for the
//! session:
//!
//! ```text
//! <runtime> run -d --name ragent-sandbox-<hash> \
//!            -v ragent-sandbox-vol-<hash>:/projects -w /projects \
//!            <image> sleep infinity
//! ```
//!
//! The image is expected to provide a minimal POSIX shell userland (`sh`, `sed`,
//! `grep`, `find`, `base64`); the container command is `sleep infinity` so the
//! sandbox stays up for the whole session and each tool call is one cheap
//! `exec`. The container is created before the first tool call is dispatched, so
//! the project is "made available to it" (FR-009) as a workspace that survives the
//! session.
//!
//! ## Workspace isolation (FR-020, FR-035)
//!
//! The workspace is a **container-named volume**, never the host working
//! directory. A tool invocation inside the container therefore cannot read or
//! write a host path: `cat HOST_MARKER.txt` finds nothing and a `touch` inside the
//! sandbox does not appear on the host (test plan TC-003). The container has no
//! host bind mount at all, which is the strongest form of FR-035 ("shall not
//! expose the host working directory"). The `workspace` field of a
//! [`BackendConfig`] names the **container-side** mount point when it is an
//! absolute path; a relative value such as `"."` keeps the default `/projects`.
//! Mapping a host worktree into the sandbox (acceptance 2's volume mapping) is a
//! later, explicitly-opted step handled by T-006, which owns credential injection
//! and workspace persistence across container replacement (FR-010, FR-023).
//!
//! ## Dispatch
//!
//! - `bash` - the command is validated by the **7-layer host validator**
//!   ([`ragent_tools_core::bash::validate_shell_command`]) before it is executed,
//!   so the sandbox can never be used to bypass the security model (FR-002,
//!   FR-037); it then runs as `exec <name> sh -lc '<command>'` with the workspace
//!   as the working directory.
//! - `read`, `write`, `create`, `append_file`, `rm`, `mkdir`, `list`, `glob`,
//!   `grep` - translated to a sandbox `exec` script (content is transported
//!   base64-encoded so it cannot be mangled by quoting) and confined to the
//!   workspace mount point.
//! - Every other tool - **refused**. The container backend has no adapter for
//!   it and never falls back to host execution (FR-031).
//!
//! ## Fail without fallback (FR-031, FR-026)
//!
//! Provisioning failures - missing runtime, missing image, a pull/start error -
//! fail the turn and surface the provisioning error ([`BackendErrorKind::Provision`]).
//! The backend never runs the tool on the host instead.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use async_trait::async_trait;
use base64::Engine as _;
use serde_json::{Value, json};

use ragent_config::{BackendConfig, ExecutionBackendKind};

use super::detection::ContainerRuntime;
use super::secrets::{
    MapSecretResolver, ResolvedSecret, SecretResolver, container_env_args, credential_names,
    resolve_secret_names, secret_names_match,
};
use super::{BackendError, BackendErrorKind, BackendToolCall, ExecutionBackend};

/// Default container-side workspace mount point.
pub const CONTAINER_WORKSPACE: &str = "/projects";

/// `--name` / volume prefix for a session's sandbox.
const SANDBOX_PREFIX: &str = "ragent-sandbox";

/// Seconds allowed for provisioning (image pull + container start).
const PROVISION_TIMEOUT_SECS: u64 = 300;

/// Seconds allowed for a single non-shell tool `exec`.
const FILE_EXEC_TIMEOUT_SECS: u64 = 120;

/// Maximum lines a routed `read`/`grep`/`glob`/`list` result renders before it is
/// truncated, matching the core tools' output discipline.
const MAX_OUTPUT_LINES: usize = 500;

/// Provisioning state of a session's sandbox.
enum ProvisionState {
    /// The container has not been started yet.
    Unprovisioned,
    /// The container is up and every `exec` targets it.
    Ready,
    /// Provisioning failed; the stored message is replayed on every dispatch so
    /// the failure is stable and the tool is never run on the host (FR-031).
    Failed(String),
}

/// The `podman`/`docker` execution backend (FR-001).
pub struct ContainerBackend {
    /// The runtime this sandbox is built on.
    runtime: ContainerRuntime,
    /// The runtime binary, resolved on `PATH` at construction; `None` when it is
    /// absent, in which case provisioning fails with a clear error.
    runtime_path: Option<PathBuf>,
    /// The container image; `None` when the descriptor did not supply one.
    image: Option<String>,
    /// Container-side workspace mount point (default [`CONTAINER_WORKSPACE`]).
    workspace: String,
    /// The named volume backing the workspace.
    volume: String,
    /// This sandbox's container name.
    container: String,
    /// The descriptor's display name, for credential-miss messages (FR-010).
    owner: String,
    /// Credential names the descriptor declared. Their values are resolved from
    /// [`Self::resolver`] at spawn time, never stored in the descriptor
    /// (FR-010).
    credential_names: Vec<String>,
    /// The credential store the spawn resolves [`Self::credential_names`] from.
    /// Defaults to an empty store, so a backend built without the session's
    /// resolver fails to provision when it names a credential (FR-010).
    resolver: Arc<dyn SecretResolver>,
    /// Provisioning state.
    state: Mutex<ProvisionState>,
}

impl ContainerBackend {
    /// Build a container adapter from a backend descriptor.
    ///
    /// The runtime defaults to [`ContainerRuntime::default`] (`podman`, FR-026)
    /// when the descriptor names no container kind. The container and volume names
    /// are derived from `(runtime, image, workspace, credential names, host
    /// working dir)` so a restarted process reuses the same sandbox instead of
    /// leaking a new one on every launch, while a changed credential set gets a
    /// freshly-provisioned container (FR-010, FR-023).
    #[must_use]
    pub fn from_descriptor(descriptor: &BackendConfig, host_working_dir: &Path) -> Self {
        let runtime = descriptor
            .kind_parsed()
            .and_then(ContainerRuntime::from_kind)
            .unwrap_or_default();
        let image = descriptor
            .image
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let workspace = descriptor
            .workspace
            .as_deref()
            .map(str::trim)
            .filter(|w| w.starts_with('/'))
            .unwrap_or(CONTAINER_WORKSPACE)
            .to_string();
        let credential_names = credential_names(descriptor);
        let (container, volume) = sandbox_names(
            runtime,
            &descriptor.id,
            image.as_deref(),
            &workspace,
            &credential_names,
            host_working_dir,
        );
        let runtime_path = which(runtime.binary());
        Self {
            runtime,
            runtime_path,
            image,
            workspace,
            volume,
            container,
            owner: descriptor.display_name().to_string(),
            credential_names,
            resolver: Arc::new(MapSecretResolver::default()),
            state: Mutex::new(ProvisionState::Unprovisioned),
        }
    }

    /// Supply the credential store the sandbox resolves its credentials from at
    /// spawn time (FR-010).
    ///
    /// The session wires this to the encrypted SQLite store
    /// ([`StorageSecretResolver`](super::secrets::StorageSecretResolver)); a test
    /// supplies an in-memory [`MapSecretResolver`]. Building the backend without
    /// this leaves an empty store, so a descriptor that names a credential fails
    /// to provision rather than starting a sandbox without it.
    #[must_use]
    pub fn with_secret_resolver(mut self, resolver: Arc<dyn SecretResolver>) -> Self {
        self.resolver = resolver;
        self
    }

    /// The runtime binary this sandbox uses.
    #[must_use]
    pub const fn runtime(&self) -> ContainerRuntime {
        self.runtime
    }

    /// The container-side workspace mount point.
    #[must_use]
    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    /// The sandbox container name.
    #[must_use]
    pub fn container_name(&self) -> &str {
        &self.container
    }

    /// The named volume backing the sandbox workspace.
    #[must_use]
    pub fn volume_name(&self) -> &str {
        &self.volume
    }

    /// The credential names the descriptor declared (FR-010).
    #[must_use]
    pub fn credential_names(&self) -> &[String] {
        &self.credential_names
    }

    /// The container `-e NAME=VALUE` arguments the next spawn would inject, with
    /// each value redacted (FR-010, FR-035).
    ///
    /// Read-only introspection for a diagnostic surface; it never returns a
    /// secret value, so it is safe to render.
    #[must_use]
    pub fn injected_env_summary(&self) -> Vec<String> {
        self.credential_names
            .iter()
            .map(|name| format!("{name}=[REDACTED]"))
            .collect()
    }

    /// Record a provisioning failure, returning the structured error.
    fn fail(&self, message: String) -> BackendError {
        if let Ok(mut guard) = self.state.lock() {
            *guard = ProvisionState::Failed(message.clone());
        }
        BackendError::new(self.runtime.kind(), BackendErrorKind::Provision, message)
    }

    /// Ensure the sandbox container is up, provisioning it on first use.
    ///
    /// # Errors
    ///
    /// Returns a [`BackendErrorKind::Provision`] failure when the runtime is
    /// missing, no image is configured, or the container cannot be started. The
    /// error is sticky: a later dispatch replays it rather than retrying or
    /// falling back to host execution (FR-031).
    async fn ensure_ready(&self) -> Result<(), BackendError> {
        if let Ok(guard) = self.state.lock() {
            match &*guard {
                ProvisionState::Ready => return Ok(()),
                ProvisionState::Failed(message) => {
                    return Err(BackendError::new(
                        self.runtime.kind(),
                        BackendErrorKind::Provision,
                        message.clone(),
                    ));
                }
                ProvisionState::Unprovisioned => {}
            }
        }

        let Some(bin) = self.runtime_path.clone() else {
            return Err(self.fail(format!(
                "container runtime '{}' was not found on PATH; the {} backend cannot \
                 provision a sandbox (install {} or select the local backend). \
                 No fallback to host execution is performed (FR-031, FR-026).",
                self.runtime, self.runtime, self.runtime
            )));
        };
        let Some(image) = self.image.clone() else {
            return Err(self.fail(format!(
                "the {} backend has no container image configured; set `image` on the \
                 backend descriptor. No fallback to host execution is performed (FR-031).",
                self.runtime
            )));
        };

        // Reuse a sandbox this process (or a previous one) already left running
        // for this descriptor: the workspace persists across container replacement
        // (FR-023), and a concurrent session in the same project must not have its
        // sandbox torn out from under it.
        //
        // The reuse is gated on the running container already carrying every
        // credential the descriptor names, so a container provisioned without a
        // credential is never reused for a session that needs it (FR-010).
        if self.container_running(&bin).await && self.container_has_credentials(&bin).await {
            if let Ok(mut guard) = self.state.lock() {
                *guard = ProvisionState::Ready;
            }
            return Ok(());
        }

        // Remove a stale stopped container with the same name (best effort - a
        // missing container is expected and not an error).
        let _ = Self::run(
            &bin,
            &["rm", "-f", &self.container],
            Duration::from_secs(30),
        )
        .await;

        // Resolve the credentials now, at spawn time, and inject them as
        // container environment variables (FR-010). A missing credential fails
        // the provision rather than starting a sandbox without it.
        let env = self.resolve_env()?;
        let mount = format!("{}:{}", self.volume, self.workspace);
        let mut run_args: Vec<&str> = vec![
            "run",
            "-d",
            "--name",
            self.container.as_str(),
            "-v",
            mount.as_str(),
            "-w",
            self.workspace.as_str(),
        ];
        run_args.extend(env.iter().map(String::as_str));
        run_args.extend([image.as_str(), "sleep", "infinity"]);

        let started = Self::run(&bin, &run_args, Duration::from_secs(PROVISION_TIMEOUT_SECS)).await;
        match started {
            Ok(output) if output.status.success() => {
                if let Ok(mut guard) = self.state.lock() {
                    *guard = ProvisionState::Ready;
                }
                tracing::info!(
                    runtime = self.runtime.as_str(),
                    container = %self.container,
                    image = %image,
                    credentials = self.credential_names.len(),
                    "container sandbox provisioned"
                );
                Ok(())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let detail = if stderr.trim().is_empty() {
                    stdout.trim().to_string()
                } else {
                    stderr.trim().to_string()
                };
                // The runtime may echo the environment back on a failure; route it
                // through the redactor so an injected secret cannot leak (FR-035).
                let detail = ragent_types::sanitize::redact_secrets(&detail);
                Err(self.fail(format!(
                    "failed to start the {} sandbox container from image '{image}': {detail}. \
                     The tool was not executed on the host (FR-031).",
                    self.runtime
                )))
            }
            Err(error) => Err(self.fail(format!(
                "failed to invoke the {} runtime while provisioning a sandbox from \
                 '{image}': {error}. The tool was not executed on the host (FR-031).",
                self.runtime
            ))),
        }
    }

    /// Resolve the descriptor's credentials from the encrypted store and render
    /// them as container `-e NAME=VALUE` arguments (FR-010).
    ///
    /// Resolution happens here, at spawn time, so a value never travels through
    /// the descriptor or the workspace. A miss is a
    /// [`BackendErrorKind::Provision`] failure so the sandbox is not started
    /// without a required credential. Each resolved value is registered with the
    /// shared redaction registry by [`resolve_secret_names`].
    fn resolve_env(&self) -> Result<Vec<String>, BackendError> {
        self.resolved_secrets()
            .map(|secrets| container_env_args(&secrets))
            .map_err(|error| self.fail(error.to_string()))
    }

    /// Resolve this sandbox's descriptor credentials from the encrypted store at
    /// spawn time (FR-010). A miss is an error so the container is never started
    /// without a required credential.
    fn resolved_secrets(&self) -> anyhow::Result<Vec<ResolvedSecret>> {
        resolve_secret_names(&self.credential_names, &self.owner, self.resolver.as_ref())
    }

    /// Whether this sandbox's running container already carries every credential
    /// the descriptor names (FR-010, FR-023).
    ///
    /// A container started by an older build - or by a session that had not yet
    /// been wired to the credential store - is replaced rather than reused when a
    /// credential it should carry is missing or its value has changed, so secret
    /// rotation takes effect at the next spawn.
    async fn container_has_credentials(&self, bin: &Path) -> bool {
        if self.credential_names.is_empty() {
            return true;
        }
        let Ok(secrets) = self.resolved_secrets() else {
            // A credential cannot be resolved now; do not reuse a container whose
            // environment may be stale (FR-010).
            return false;
        };
        let inspect = Self::run(
            bin,
            &[
                "container",
                "inspect",
                "-f",
                "{{range .Config.Env}}{{println .}}{{end}}",
                self.container.as_str(),
            ],
            Duration::from_secs(30),
        )
        .await;
        let Ok(output) = inspect else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        let env = String::from_utf8_lossy(&output.stdout);
        secret_names_match(&env, &secrets)
    }

    /// Whether this sandbox's container is already running.
    ///
    /// Used to reuse a sandbox left by a previous process (or a concurrent session)
    /// instead of removing and recreating it, so the workspace survives container
    /// replacement (FR-023).
    async fn container_running(&self, bin: &Path) -> bool {
        Self::run(
            bin,
            &[
                "container",
                "inspect",
                "-f",
                "{{.State.Running}}",
                self.container.as_str(),
            ],
            Duration::from_secs(30),
        )
        .await
        .is_ok_and(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
        })
    }

    /// Run one `<runtime> exec <container> ...` command.
    async fn exec_in_container(
        &self,
        script: &str,
        timeout: Duration,
    ) -> anyhow::Result<std::process::Output> {
        let bin = self
            .runtime_path
            .as_ref()
            .context("container runtime binary is not available")?;
        let args = ["exec", self.container.as_str(), "sh", "-lc", script];
        Self::run(bin, &args, timeout).await
    }

    /// Spawn `bin args...` and capture its output, bounded by `timeout`.
    ///
    /// `kill_on_drop` plus the `timeout` race means a hung runtime (a stalled image
    /// pull, a wedged `exec`) is killed with the future rather than leaking a
    /// process for the rest of the session.
    async fn run(
        bin: &Path,
        args: &[&str],
        timeout: Duration,
    ) -> anyhow::Result<std::process::Output> {
        let mut command = tokio::process::Command::new(bin);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = command
            .spawn()
            .with_context(|| format!("failed to spawn container runtime '{}'", bin.display()))?;
        match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(result) => result.with_context(|| {
                format!(
                    "container runtime '{}' did not produce a result",
                    bin.display()
                )
            }),
            Err(_) => anyhow::bail!(
                "container runtime '{}' timed out after {}s",
                bin.display(),
                timeout.as_secs()
            ),
        }
    }
}

#[async_trait]
impl ExecutionBackend for ContainerBackend {
    fn kind(&self) -> ExecutionBackendKind {
        self.runtime.kind()
    }

    async fn execute_tool(
        &self,
        call: BackendToolCall<'_>,
    ) -> anyhow::Result<crate::tool::ToolOutput> {
        let tool_name = call.tool.name().to_string();

        // 1. `bash`: validate against the host 7-layer model first (FR-002,
        //    FR-037), then run the command inside the sandbox.
        if tool_name == "bash" {
            let command = call.input["command"]
                .as_str()
                .context("Missing required 'command' parameter")?
                .to_string();
            ragent_tools_core::bash::validate_shell_command(&command, &call.ctx.working_dir)
                .await
                .map_err(|error| {
                    anyhow::anyhow!("bash command rejected by the 7-layer validator: {error}")
                })?;
            self.ensure_ready().await?;

            let timeout_secs = call.input["timeout"].as_u64().unwrap_or(120);
            let script = format!("cd {} && {command}", sq(&self.workspace));
            let started = Instant::now();
            let output = self
                .exec_in_container(&script, Duration::from_secs(timeout_secs.max(1)))
                .await
                .with_context(|| {
                    format!(
                        "container '{}' (runtime {}) failed to execute the command",
                        self.container, self.runtime
                    )
                })?;
            return Ok(self.bash_output(output, started.elapsed(), timeout_secs, &command));
        }

        // 2. The file/search subset the sandbox can serve through `exec`.
        if let Some(script) = container_script(&tool_name, &call.input, &self.workspace)? {
            self.ensure_ready().await?;
            let output = self
                .exec_in_container(&script, Duration::from_secs(FILE_EXEC_TIMEOUT_SECS))
                .await
                .with_context(|| {
                    format!(
                        "container '{}' (runtime {}) failed to execute '{tool_name}'",
                        self.container, self.runtime
                    )
                })?;
            return file_output(output, &tool_name, self.runtime);
        }

        // 3. No adapter: refuse. Never run the tool on the host (FR-031).
        tracing::warn!(
            backend = self.runtime.as_str(),
            tool = %tool_name,
            "container backend has no adapter for this tool; refusing (no host fallback)"
        );
        Err(BackendError::new(
            self.runtime.kind(),
            BackendErrorKind::Unavailable,
            format!(
                "the container execution backend cannot dispatch the '{tool_name}' tool inside \
                 a sandbox; it was not executed on the host (FR-031). Run this tool through the \
                 local backend instead."
            ),
        )
        .into())
    }
}

impl ContainerBackend {
    /// Format a `bash` result the way the host `bash` tool does.
    fn bash_output(
        &self,
        output: std::process::Output,
        elapsed: Duration,
        timeout_secs: u64,
        command: &str,
    ) -> crate::tool::ToolOutput {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);
        let elapsed_ms = elapsed.as_millis() as u64;

        let mut content = String::new();
        if !stdout.is_empty() {
            content.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str("[stderr]\n");
            content.push_str(&stderr);
        }
        if content.is_empty() {
            content = "(no output)".to_string();
        }
        if !output.status.success() && output.status.code().is_none() {
            content = format!(
                "Command timed out after {timeout_secs}s inside the sandbox and was killed.\n{content}"
            );
        }
        let content = ragent_types::sanitize::redact_secrets(&content);
        let content =
            ragent_tools_core::truncate::truncate_content(&content, MAX_OUTPUT_LINES.max(40));
        let line_count = content.lines().count();
        let exit_desc = output
            .status
            .code()
            .map_or_else(|| "unknown".to_string(), |c| c.to_string());

        crate::tool::ToolOutput {
            content: format!(
                "Exit code: {exit_desc}\nDuration: {elapsed_ms}ms\nBackend: {} sandbox '{}'\n\n{content}",
                self.runtime, self.container
            ),
            metadata: Some(json!({
                "exit_code": exit_code,
                "duration_ms": elapsed_ms,
                "line_count": line_count,
                "backend": self.runtime.as_str(),
                "container": self.container,
                "command": ragent_types::sanitize::redact_secrets(command),
            })),
        }
    }
}

/// Translate a core file/search tool into a sandbox `sh -lc` script.
///
/// Returns `Ok(None)` when the tool is not routable through the sandbox, in which
/// case the caller refuses it. Every path is confined to `workspace`, so a routed
/// tool cannot read or write a host path (FR-020, FR-035).
fn container_script(tool: &str, input: &Value, workspace: &str) -> anyhow::Result<Option<String>> {
    let script = match tool {
        "read" => {
            let path = container_path(workspace, required_str(input, "path")?)?;
            let start = input["start_line"].as_u64();
            let num = input["num_lines"].as_u64();
            let end = input["end_line"].as_u64();
            if start == Some(0) || end == Some(0) || num == Some(0) {
                anyhow::bail!("Invalid line range: start_line/end_line/num_lines must be >= 1");
            }
            let first = start.unwrap_or(1);
            let last = match (end, num) {
                (Some(e), _) => e.to_string(),
                (None, Some(n)) => first.saturating_add(n).saturating_sub(1).to_string(),
                (None, None) => "$".to_string(),
            };
            let range = format!("{first},{last}p");
            format!(
                "f={path}; if [ ! -e \"$f\" ]; then echo \"Error: no such file: $f\"; exit 1; fi; \
                 if [ -d \"$f\" ]; then echo \"Error: $f is a directory\"; exit 1; fi; \
                 total=$(wc -l < \"$f\"); echo \"File: $f (lines {first}-{last} of $total)\"; \
                 sed -n {range} \"$f\"",
                path = sq(&path),
                range = sq(&range),
            )
        }
        "create" | "write" => {
            let path = container_path(workspace, required_str(input, "path")?)?;
            let content = required_str(input, "content")?;
            format!(
                "f={path}; mkdir -p \"$(dirname \"$f\")\"; printf '%s' {payload} | base64 -d > \"$f\" && echo \"wrote $f\"",
                path = sq(&path),
                payload = sq(&base64_std(content.as_bytes())),
            )
        }
        "append_file" => {
            let path = container_path(workspace, required_str(input, "path")?)?;
            let content = required_str(input, "content")?;
            format!(
                "f={path}; mkdir -p \"$(dirname \"$f\")\"; printf '%s' {payload} | base64 -d >> \"$f\"; echo \"appended to $f\"",
                path = sq(&path),
                payload = sq(&base64_std(content.as_bytes())),
            )
        }
        "rm" => {
            let raw = required_str(input, "path")?;
            if raw.contains('*') || raw.contains('?') {
                anyhow::bail!("'rm' does not accept wildcards; provide a single file path");
            }
            let path = container_path(workspace, raw)?;
            format!(
                "f={path}; if [ -d \"$f\" ] && [ ! -L \"$f\" ]; then echo \"Error: $f is a directory\"; exit 1; fi; rm -f \"$f\"; echo \"removed $f\"",
                path = sq(&path),
            )
        }
        "mkdir" => {
            let path = container_path(workspace, required_str(input, "path")?)?;
            format!(
                "mkdir -p {path} && echo \"created {path}\"",
                path = sq(&path)
            )
        }
        "list" => {
            let raw = input["path"].as_str().unwrap_or(".");
            let path = container_path(workspace, raw)?;
            let depth = input["depth"].as_u64().unwrap_or(2).clamp(1, 10);
            format!(
                "if [ ! -d {path} ]; then echo \"Error: not a directory: {path}\"; exit 1; fi; \
                 echo \"Listing: {path}\"; find {path} -maxdepth {depth} -mindepth 1 -print | sort",
                path = sq(&path),
            )
        }
        "glob" => {
            let pattern = required_str(input, "pattern")?;
            let base = match input["path"].as_str() {
                Some(raw) => container_path(workspace, raw)?,
                None => workspace.to_string(),
            };
            let (predicate, argument) = glob_predicate(pattern, &base);
            format!(
                "if [ ! -d {base} ]; then echo \"Error: not a directory: {base}\"; exit 1; fi; \
                 find {base} -type f {predicate} {argument} | sort | head -n {cap}",
                base = sq(&base),
                predicate = predicate,
                argument = sq(&argument),
                cap = MAX_OUTPUT_LINES,
            )
        }
        "grep" => {
            let pattern = required_str(input, "pattern")?;
            let base = match input["path"].as_str() {
                Some(raw) => container_path(workspace, raw)?,
                None => workspace.to_string(),
            };
            let mut flags = String::from("-rIn");
            if input["case_insensitive"].as_bool().unwrap_or(false) {
                flags.push('i');
            }
            let mut extras = String::new();
            if let Some(include) = input["include"].as_str() {
                extras.push_str(&format!(" --include={}", sq(include)));
            }
            if let Some(exclude) = input["exclude"].as_str() {
                extras.push_str(&format!(" --exclude={}", sq(exclude)));
            }
            format!(
                "grep {flags}{extras} -e {pattern} {base} 2>/dev/null | head -n {cap}; true",
                pattern = sq(&pattern),
                base = sq(&base),
                cap = MAX_OUTPUT_LINES,
            )
        }
        _ => return Ok(None),
    };
    Ok(Some(script))
}

/// Map an absolute or relative path to its container-side location, confined to
/// `workspace`.
///
/// Absolute paths must already sit inside the workspace mount, and `..` components
/// are rejected outright. The check is purely lexical: it runs on the host without
/// touching the container filesystem, so it can never be defeated by a symlink the
/// sandbox creates later (the sandbox itself is disposable).
fn container_path(workspace: &str, raw: &str) -> anyhow::Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("path must not be empty");
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        let workspace_prefix = format!("{workspace}/");
        if trimmed == workspace || trimmed.starts_with(&workspace_prefix) {
            return Ok(trimmed.to_string());
        }
        anyhow::bail!(
            "path '{trimmed}' is outside the container workspace '{workspace}'; container file \
             tools are confined to the mounted workspace (FR-035)"
        );
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        anyhow::bail!(
            "path '{trimmed}' escapes the container workspace '{workspace}' via '..' (FR-035)"
        );
    }
    let normalised = path.to_string_lossy().trim_start_matches("./").to_string();
    Ok(format!("{workspace}/{normalised}"))
}

/// Translate a glob pattern into a `find` predicate (`-name`/`-path` and argument).
///
/// Shell globs cannot express `**`; the pattern is approximated from its most
/// specific segment - the text after the last `**/`, or the whole pattern when it
/// has none. A remaining `/` selects `-path` (which matches `/` with `*`), a bare
/// final segment selects `-name`.
fn glob_predicate(pattern: &str, base: &str) -> (&'static str, String) {
    let tail = match pattern.rfind("**/") {
        Some(index) => &pattern[index + 3..],
        None => pattern,
    };
    if tail.contains('/') {
        ("-path", format!("{base}/{tail}"))
    } else {
        ("-name", tail.to_string())
    }
}

/// The tool's required string parameter.
fn required_str<'a>(input: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    input[key]
        .as_str()
        .with_context(|| format!("Missing required '{key}' parameter"))
}

/// POSIX single-quote a string so it survives `sh -lc`.
fn sq(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// Standard base64 (no line wrapping) for transporting file content.
fn base64_std(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Package a routed file/search tool result.
fn file_output(
    output: std::process::Output,
    tool: &str,
    runtime: ContainerRuntime,
) -> anyhow::Result<crate::tool::ToolOutput> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);
    let body = if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout.trim_end().to_string()
    };
    let body = ragent_types::sanitize::redact_secrets(&body);
    let body = ragent_tools_core::truncate::truncate_content(&body, MAX_OUTPUT_LINES);
    let line_count = body.lines().count();

    if !output.status.success() {
        anyhow::bail!("container '{runtime}' sandbox '{tool}' failed (exit {exit_code}): {body}");
    }
    Ok(crate::tool::ToolOutput {
        content: if body.is_empty() {
            "(no output)".to_string()
        } else {
            body
        },
        metadata: Some(json!({
            "exit_code": exit_code,
            "line_count": line_count,
            "backend": runtime.as_str(),
            "tool": tool,
        })),
    })
}

/// Derive the sandbox container and volume names for a descriptor.
///
/// The **volume** name is deterministic from `(runtime, descriptor id, image,
/// workspace, host working dir)` - the *project* identity - so the conversation's
/// workspace survives replacement of the container (FR-023): recreating the
/// container always re-mounts the same named volume.
///
/// The **container** name additionally folds in the descriptor's credential
/// *names*, so changing the credential set provisions a fresh container with the
/// new environment instead of reusing one whose environment is stale - while
/// still mounting the same persisted volume (FR-010, FR-023).
fn sandbox_names(
    runtime: ContainerRuntime,
    descriptor_id: &str,
    image: Option<&str>,
    workspace: &str,
    credentials: &[String],
    host_working_dir: &Path,
) -> (String, String) {
    use std::hash::{Hash, Hasher};

    let mut project = std::collections::hash_map::DefaultHasher::new();
    runtime.as_str().hash(&mut project);
    descriptor_id.hash(&mut project);
    image.unwrap_or("<none>").hash(&mut project);
    workspace.hash(&mut project);
    host_working_dir.hash(&mut project);
    let volume_digest = format!("{:016x}", project.finish());

    let mut container = std::collections::hash_map::DefaultHasher::new();
    volume_digest.hash(&mut container);
    credentials.hash(&mut container);
    let container_digest = format!("{:016x}", container.finish());

    (
        format!("{SANDBOX_PREFIX}-{container_digest}"),
        format!("{SANDBOX_PREFIX}-vol-{volume_digest}"),
    )
}

/// Resolve `name` on `PATH`, returning the absolute path to the binary.
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}
