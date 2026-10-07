//! `ragent_info` - Report build and execution information about ragent itself.
//!
//! Implements a read-only introspection tool that returns the running ragent
//! version, when the binary was built, the git commit it was built from (when
//! available), the compiler that produced it, and runtime execution details of
//! the current process (pid, parent pid, start time, uptime, executable path,
//! working directory, user, memory, and thread count). This lets the LLM answer
//! "what version of ragent is running, when was it built, and how long has this
//! instance been up?" without running a shell command.

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use super::{Tool, ToolContext, ToolOutput};

/// Compile-time build time, embedded by `build.rs`.
const BUILD_TIME: &str = env!("BUILD_TIME");
/// Best-effort git commit hash, embedded by `build.rs` when available.
const GIT_COMMIT: Option<&str> = option_env!("GIT_COMMIT");
/// Best-effort git commit subject, embedded by `build.rs` when available.
const GIT_COMMIT_SUBJECT: Option<&str> = option_env!("GIT_COMMIT_SUBJECT");
/// Best-effort compiler version, embedded by `build.rs` when available.
const RUSTC_VERSION: Option<&str> = option_env!("RUSTC_VERSION");

/// Read-only tool that reports ragent build and execution information.
pub struct RagentInfoTool;

/// Normalized build and execution metadata returned by [`RagentInfoTool`].
#[derive(Debug, Clone, Serialize)]
struct RagentInfo {
    version: String,
    build_time: String,
    commit: Option<String>,
    commit_subject: Option<String>,
    rustc_version: Option<String>,
    execution: ExecutionInfo,
}

/// Runtime execution metadata for the current ragent process.
///
/// Every field is best-effort: a value the host cannot expose is reported as
/// `None` (rendered `unknown`) rather than failing the call.
#[derive(Debug, Clone, Serialize)]
struct ExecutionInfo {
    /// OS process id of the running ragent instance.
    pid: u32,
    /// Parent process id, when the host exposes it.
    parent_pid: Option<u32>,
    /// Wall-clock start time as an RFC 3339 UTC timestamp, when known.
    started_at: Option<String>,
    /// Seconds the process has been running, when known.
    uptime_seconds: Option<u64>,
    /// Human-readable uptime (`Nd Nh Nm Ns`), when known.
    uptime: Option<String>,
    /// Absolute path to the running executable, when resolvable.
    executable: Option<String>,
    /// Current working directory of the process, when resolvable.
    working_directory: Option<String>,
    /// Username of the account running ragent, when resolvable.
    user: Option<String>,
    /// Resident set size in bytes, when known.
    resident_memory_bytes: Option<u64>,
    /// Virtual memory size in bytes, when known.
    virtual_memory_bytes: Option<u64>,
    /// Thread (task) count, when the host exposes it.
    threads: Option<usize>,
}

impl RagentInfoTool {
    /// Collect the build and execution metadata for the running ragent binary.
    fn collect() -> RagentInfo {
        RagentInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_time: BUILD_TIME.to_string(),
            commit: GIT_COMMIT.map(str::to_string),
            commit_subject: GIT_COMMIT_SUBJECT.map(str::to_string),
            rustc_version: RUSTC_VERSION.map(str::to_string),
            execution: ExecutionInfo::collect(),
        }
    }
}

impl ExecutionInfo {
    /// Collect runtime execution metadata for the current process.
    ///
    /// Only the current pid is inspected, using `sysinfo`; the call spawns no
    /// process and touches no network. Fields the host does not expose stay
    /// `None` so the collector always succeeds.
    fn collect() -> Self {
        let pid_u32 = std::process::id();
        let pid = Pid::from_u32(pid_u32);

        let mut system = System::new();
        // `with_tasks` is required for [`sysinfo::Process::tasks`]; without it
        // the thread count stays `None` and the report renders `unknown` on every
        // platform even though the host exposes the task list.
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing()
                .with_memory()
                .with_exe(UpdateKind::Always)
                .with_cwd(UpdateKind::Always)
                .with_tasks(),
        );

        let process = system.process(pid);
        let uptime_seconds = process.map(sysinfo::Process::run_time).filter(|s| *s > 0);
        let started_at = process
            .map(sysinfo::Process::start_time)
            .filter(|s| *s > 0)
            .map(secs_to_rfc3339);

        Self {
            pid: pid_u32,
            parent_pid: process.and_then(sysinfo::Process::parent).map(Pid::as_u32),
            started_at,
            uptime_seconds,
            uptime: uptime_seconds.map(human_uptime),
            executable: process
                .and_then(sysinfo::Process::exe)
                .map(|p| p.display().to_string()),
            working_directory: process
                .and_then(sysinfo::Process::cwd)
                .map(|p| p.display().to_string())
                .or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|d| d.display().to_string())
                }),
            user: resolve_username(),
            resident_memory_bytes: process.map(sysinfo::Process::memory).filter(|b| *b > 0),
            virtual_memory_bytes: process
                .map(sysinfo::Process::virtual_memory)
                .filter(|b| *b > 0),
            threads: process
                .and_then(sysinfo::Process::tasks)
                .map(std::collections::HashSet::len),
        }
    }
}

#[async_trait::async_trait]
impl Tool for RagentInfoTool {
    fn name(&self) -> &'static str {
        "ragent_info"
    }

    fn description(&self) -> &'static str {
        "Report build and execution information about the running ragent binary. \
         Returns the ragent version, the build timestamp, the git commit it was \
         built from (when available), the compiler version, and runtime details \
         of this instance (pid, parent pid, start time, uptime, executable path, \
         working directory, user, resident and virtual memory, and thread \
         count). No parameters required. This tool does not access the network \
         or write to the filesystem and always succeeds."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["text", "json"],
                    "description": "Output format: 'text' (human-readable markdown, default) or 'json' (structured metadata only)"
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    /// # Errors
    ///
    /// This tool never returns an error - it always succeeds with build info.
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let format = input["format"].as_str().unwrap_or("text");
        let info = Self::collect();
        let value = serde_json::to_value(&info)?;

        match format {
            "json" => Ok(ToolOutput {
                content: serde_json::to_string_pretty(&value)?,
                metadata: Some(value),
            }),
            _ => Ok(ToolOutput {
                content: render_text(&info),
                metadata: Some(value),
            }),
        }
    }
}

/// Render a human-readable markdown report.
fn render_text(info: &RagentInfo) -> String {
    let mut lines = vec![
        "## ragent Build Information".to_string(),
        String::new(),
        format!("- **Version**: {}", info.version),
        format!("- **Built**: {}", info.build_time),
    ];

    if let Some(commit) = &info.commit {
        match &info.commit_subject {
            Some(subject) => lines.push(format!("- **Commit**: {commit} ({subject})")),
            None => lines.push(format!("- **Commit**: {commit}")),
        }
    } else {
        lines.push("- **Commit**: (unavailable)".to_string());
    }

    match &info.rustc_version {
        Some(rustc) => lines.push(format!("- **Compiler**: {rustc}")),
        None => lines.push("- **Compiler**: (unavailable)".to_string()),
    }

    lines.extend(execution_text_lines(&info.execution));
    lines.join("\n")
}

/// Render the runtime execution section as markdown lines.
fn execution_text_lines(exec: &ExecutionInfo) -> Vec<String> {
    // The process has no threads on platforms where `sysinfo` does not expose a
    // task list; leave the field `unknown` rather than claiming zero.
    vec![
        String::new(),
        "## ragent Execution Information".to_string(),
        format!("- **PID**: {}", exec.pid),
        format!("- **Parent PID**: {}", opt(exec.parent_pid)),
        format!("- **Started At**: {}", opt_str(exec.started_at.as_deref())),
        format!(
            "- **Uptime**: {}",
            match (&exec.uptime, exec.uptime_seconds) {
                (Some(human), Some(secs)) => format!("{human} ({secs}s)"),
                _ => UNKNOWN.to_string(),
            }
        ),
        format!("- **Executable**: {}", opt_str(exec.executable.as_deref())),
        format!(
            "- **Working Directory**: {}",
            opt_str(exec.working_directory.as_deref())
        ),
        format!("- **User**: {}", opt_str(exec.user.as_deref())),
        format!(
            "- **Resident Memory**: {}",
            opt_bytes(exec.resident_memory_bytes)
        ),
        format!(
            "- **Virtual Memory**: {}",
            opt_bytes(exec.virtual_memory_bytes)
        ),
        format!("- **Threads**: {}", opt(exec.threads)),
    ]
}

/// Placeholder for a value the host did not expose.
const UNKNOWN: &str = "unknown";

/// Render an optional displayable value, or the `unknown` placeholder.
fn opt<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| UNKNOWN.to_string(), |v| v.to_string())
}

/// Render an optional string or the `unknown` placeholder.
fn opt_str(value: Option<&str>) -> String {
    value.map_or_else(|| UNKNOWN.to_string(), str::to_string)
}

/// Render an optional byte count with a human-readable MiB rendering.
fn opt_bytes(value: Option<u64>) -> String {
    value.map_or_else(
        || UNKNOWN.to_string(),
        |bytes| format!("{bytes} bytes ({:.2} MiB)", bytes as f64 / 1_048_576.0),
    )
}

/// Render a second count as `Nd Nh Nm Ns`.
fn human_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;
    format!("{days}d {hours}h {minutes}m {secs}s")
}

/// Convert seconds since the Unix epoch to an RFC 3339 UTC timestamp.
fn secs_to_rfc3339(secs: u64) -> String {
    i64::try_from(secs)
        .ok()
        .and_then(|s| chrono::DateTime::from_timestamp(s, 0))
        .map_or_else(
            || UNKNOWN.to_string(),
            |dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        )
}

/// Resolve the username of the account running ragent.
///
/// Checks `USER` (Unix), `USERNAME` (Windows), then `LOGNAME`, skipping blank
/// values. Returns `None` when none is set so the report renders `unknown`; no
/// other environment variable is read, so no secret material can leak.
fn resolve_username() -> Option<String> {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
}
