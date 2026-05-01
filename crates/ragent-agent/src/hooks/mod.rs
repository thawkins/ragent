//! Lifecycle hooks for ragent sessions.
//!
//! Hooks allow users to run shell commands at key points in the session
//! lifecycle. They are defined in `ragent.json` under the `hooks` key and
//! execute asynchronously (errors are logged, not fatal).
//!
//! # Example `ragent.json` configuration
//!
//! ```json
//! {
//!   "hooks": [
//!     {
//!       "trigger": "on_session_start",
//!       "command": "echo 'Session started' >> ~/.ragent/session.log"
//!     },
//!     {
//!       "trigger": "on_error",
//!       "command": "notify-send 'ragent error' '$RAGENT_ERROR'"
//!     }
//!   ]
//! }
//! ```
//!
//! ## Environment Variables Available to Hooks
//!
//! - `RAGENT_TRIGGER` — the trigger name (e.g., `on_session_start`)
//! - `RAGENT_WORKING_DIR` — the session working directory
//! - `RAGENT_ERROR` — error message (only for `on_error` trigger)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Trigger point for a lifecycle hook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTrigger {
    /// Fired when a session receives its first user message.
    OnSessionStart,
    /// Fired after a session completes processing a user message.
    OnSessionEnd,
    /// Fired when an LLM call or tool execution returns an error.
    OnError,
    /// Fired when a tool call is rejected due to a permission rule.
    OnPermissionDenied,
    /// Fired before a tool is executed, allowing hooks to approve/deny/modify.
    ///
    /// Hooks triggered by `PreToolUse` receive additional environment variables:
    /// - `RAGENT_TOOL_NAME` - the name of the tool being invoked
    /// - `RAGENT_TOOL_INPUT` - JSON string of the tool arguments
    ///
    /// Hooks can return a decision by writing to stdout:
    /// - `{"decision": "allow"}` - skip the UI prompt and allow the tool
    /// - `{"decision": "deny", "reason": "..."}` - deny with optional reason
    /// - `{"modified_input": {...}}` - modify the tool arguments
    /// - Empty output or invalid JSON - normal permission flow applies
    PreToolUse,
    /// Fired after a tool is executed, allowing hooks to inspect/modify results.
    ///
    /// Hooks triggered by `PostToolUse` receive additional environment variables:
    /// - `RAGENT_TOOL_NAME` - the name of the tool that was invoked
    /// - `RAGENT_TOOL_INPUT` - JSON string of the tool arguments
    /// - `RAGENT_TOOL_OUTPUT` - JSON string of the tool output
    /// - `RAGENT_TOOL_SUCCESS` - "true" or "false"
    ///
    /// Hooks can return modified output by writing to stdout:
    /// - `{"modified_output": {"content": "...", ...}}` - replace the tool output
    PostToolUse,
}

impl std::fmt::Display for HookTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OnSessionStart => write!(f, "on_session_start"),
            Self::OnSessionEnd => write!(f, "on_session_end"),
            Self::OnError => write!(f, "on_error"),
            Self::OnPermissionDenied => write!(f, "on_permission_denied"),
            Self::PreToolUse => write!(f, "pre_tool_use"),
            Self::PostToolUse => write!(f, "post_tool_use"),
        }
    }
}

/// A single hook configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// When to fire this hook.
    pub trigger: HookTrigger,
    /// Shell command to execute. Runs via `sh -c`.
    pub command: String,
    /// Optional timeout in seconds (default: 30).
    #[serde(default = "default_hook_timeout")]
    pub timeout_secs: u64,
}

/// Parses hook definitions loaded from config JSON into typed hook configs.
#[must_use]
pub fn parse_hook_configs(raw_hooks: &[Value]) -> Vec<HookConfig> {
    raw_hooks
        .iter()
        .filter_map(
            |value| match serde_json::from_value::<HookConfig>(value.clone()) {
                Ok(hook) => Some(hook),
                Err(error) => {
                    tracing::warn!(error = %error, "Ignoring invalid hook config entry");
                    None
                }
            },
        )
        .collect()
}

const fn default_hook_timeout() -> u64 {
    30
}

/// Result of running a pre-tool-use hook.
#[derive(Debug, Clone)]
pub enum PreToolUseResult {
    /// Allow the tool to execute without showing the UI prompt.
    Allow,
    /// Deny the tool execution with an optional reason.
    Deny {
        /// Reason for denying the tool execution.
        reason: String,
    },
    /// Modify the tool input arguments.
    ModifiedInput {
        /// The modified tool input arguments.
        input: serde_json::Value,
    },
    /// No decision from hook - use normal permission flow.
    NoDecision,
}

/// Run hooks for PreToolUse synchronously and collect their decisions.
///
/// This function runs hooks synchronously (unlike `fire_hooks` which is async)
/// because it needs to potentially modify or block tool execution.
///
/// # Returns
///
/// Returns the first hook result that makes a decision (Allow, Deny, or ModifiedInput).
/// If no hooks make a decision, returns `NoDecision`.
///
/// # Examples
///
/// ```
/// use ragent_core::hooks::{run_pre_tool_use_hooks, HookConfig, HookTrigger};
/// use std::path::Path;
///
/// let hooks = vec![];
/// let result = run_pre_tool_use_hooks(
///     &hooks,
///     Path::new("/tmp"),
///     "read",
///     r#"{"path": "src/main.rs"}"#,
/// );
/// // Returns NoDecision when no hooks configured
/// ```
pub fn run_pre_tool_use_hooks(
    hooks: &[HookConfig],
    working_dir: &Path,
    tool_name: &str,
    tool_input: &str,
) -> PreToolUseResult {
    let matching: Vec<HookConfig> = hooks
        .iter()
        .filter(|h| h.trigger == HookTrigger::PreToolUse)
        .cloned()
        .collect();

    if matching.is_empty() {
        return PreToolUseResult::NoDecision;
    }

    for hook in matching {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&hook.command)
            .current_dir(working_dir)
            .env("RAGENT_TRIGGER", "pre_tool_use")
            .env("RAGENT_WORKING_DIR", working_dir.display().to_string())
            .env("RAGENT_TOOL_NAME", tool_name)
            .env("RAGENT_TOOL_INPUT", tool_input)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let trimmed = stdout.trim();

                if trimmed.is_empty() {
                    continue;
                }

                // Try to parse as JSON decision
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    // Check for decision field
                    if let Some(decision) = json.get("decision").and_then(|v| v.as_str()) {
                        match decision {
                            "allow" => {
                                tracing::debug!(
                                    tool = %tool_name,
                                    hook_command = %hook.command,
                                    "PreToolUse hook returned 'allow' - skipping UI prompt"
                                );
                                return PreToolUseResult::Allow;
                            }
                            "deny" => {
                                let reason = json
                                    .get("reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Denied by hook")
                                    .to_string();
                                tracing::info!(
                                    tool = %tool_name,
                                    hook_command = %hook.command,
                                    reason = %reason,
                                    "PreToolUse hook returned 'deny'"
                                );
                                return PreToolUseResult::Deny { reason };
                            }
                            _ => {}
                        }
                    }

                    // Check for modified_input field
                    if let Some(modified) = json.get("modified_input") {
                        tracing::debug!(
                            tool = %tool_name,
                            hook_command = %hook.command,
                            "PreToolUse hook returned modified input"
                        );
                        return PreToolUseResult::ModifiedInput {
                            input: modified.clone(),
                        };
                    }
                }

                // Non-JSON output is treated as no decision
                tracing::trace!(
                    tool = %tool_name,
                    hook_command = %hook.command,
                    stdout = %trimmed,
                    "PreToolUse hook returned non-JSON output - treating as no decision"
                );
            }
            Ok(out) => {
                tracing::warn!(
                    trigger = "pre_tool_use",
                    command = %hook.command,
                    exit_code = ?out.status.code(),
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "PreToolUse hook exited with non-zero status"
                );
            }
            Err(e) => {
                tracing::error!(
                    trigger = "pre_tool_use",
                    command = %hook.command,
                    error = %e,
                    "PreToolUse hook execution failed"
                );
            }
        }
    }

    PreToolUseResult::NoDecision
}

/// Run hooks for PostToolUse asynchronously.
///
/// This function runs hooks asynchronously and allows them to modify the tool output.
/// The output modifications are collected and the last one is returned.
pub async fn run_post_tool_use_hooks(
    hooks: &[HookConfig],
    working_dir: &Path,
    tool_name: &str,
    tool_input: &str,
    tool_output: &str,
    success: bool,
) -> Option<serde_json::Value> {
    let matching: Vec<HookConfig> = hooks
        .iter()
        .filter(|h| h.trigger == HookTrigger::PostToolUse)
        .cloned()
        .collect();

    if matching.is_empty() {
        return None;
    }

    let mut last_modified_output: Option<serde_json::Value> = None;

    for hook in matching {
        let wd = working_dir.to_path_buf();
        let tool_name = tool_name.to_string();
        let tool_input = tool_input.to_string();
        let tool_output = tool_output.to_string();
        let success_str = success.to_string();
        let command = hook.command.clone();
        let timeout = std::time::Duration::from_secs(hook.timeout_secs);

        let task = tokio::task::spawn_blocking({
            let tool_name = tool_name.clone();
            let command = command.clone();
            move || {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .current_dir(&wd)
                    .env("RAGENT_TRIGGER", "post_tool_use")
                    .env("RAGENT_WORKING_DIR", wd.display().to_string())
                    .env("RAGENT_TOOL_NAME", &tool_name)
                    .env("RAGENT_TOOL_INPUT", &tool_input)
                    .env("RAGENT_TOOL_OUTPUT", &tool_output)
                    .env("RAGENT_TOOL_SUCCESS", &success_str)
                    .output()
            }
        });
        match tokio::time::timeout(timeout, task).await {
            Ok(Ok(Ok(out))) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let trimmed = stdout.trim();

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(modified) = json.get("modified_output") {
                        tracing::debug!(
                            tool = %tool_name,
                            hook_command = %command,
                            "PostToolUse hook returned modified output"
                        );
                        last_modified_output = Some(modified.clone());
                    }
                }
            }
            Ok(Ok(Ok(out))) => {
                tracing::warn!(
                    trigger = "post_tool_use",
                    command = %command,
                    exit_code = ?out.status.code(),
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    "PostToolUse hook exited with non-zero status"
                );
            }
            Ok(Ok(Err(e))) => {
                tracing::error!(
                    trigger = "post_tool_use",
                    command = %command,
                    error = %e,
                    "PostToolUse hook execution failed"
                );
            }
            Ok(Err(_)) => {
                tracing::warn!(
                    trigger = "post_tool_use",
                    command = %command,
                    "PostToolUse hook task panicked"
                );
            }
            Err(_) => {
                tracing::warn!(
                    trigger = "post_tool_use",
                    command = %command,
                    timeout_secs = hook.timeout_secs,
                    "PostToolUse hook timed out"
                );
            }
        }
    }

    last_modified_output
}

/// Fire all hooks matching `trigger`, asynchronously.
///
/// Each hook runs in a spawned task. Errors are logged but never propagate.
/// The calling task is not blocked.
pub fn fire_hooks(
    hooks: &[HookConfig],
    trigger: HookTrigger,
    working_dir: &Path,
    extra_env: &[(&str, &str)],
) {
    let matching: Vec<HookConfig> = hooks
        .iter()
        .filter(|h| h.trigger == trigger)
        .cloned()
        .collect();

    if matching.is_empty() {
        return;
    }

    let working_dir = working_dir.to_path_buf();
    let trigger_str = trigger.to_string();
    let extra: Vec<(String, String)> = extra_env
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    tokio::spawn(async move {
        for hook in matching {
            let wd = working_dir.clone();
            let trigger_s = trigger_str.clone();
            let extra_e = extra.clone();
            let timeout = std::time::Duration::from_secs(hook.timeout_secs);
            let command = hook.command.clone();
            let command_for_warn = command.clone();
            let timeout_secs = hook.timeout_secs;

            let task = tokio::spawn(async move {
                let mut cmd = tokio::process::Command::new("sh");
                cmd.arg("-c")
                    .arg(&command)
                    .current_dir(&wd)
                    .env("RAGENT_TRIGGER", &trigger_s)
                    .env("RAGENT_WORKING_DIR", wd.display().to_string());
                for (k, v) in &extra_e {
                    cmd.env(k, v);
                }
                match cmd.output().await {
                    Ok(out) if !out.status.success() => {
                        tracing::warn!(
                            trigger = %trigger_s,
                            command = %command,
                            exit_code = ?out.status.code(),
                            stderr = %String::from_utf8_lossy(&out.stderr),
                            "Hook exited with non-zero status"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            trigger = %trigger_s,
                            command = %command,
                            error = %e,
                            "Hook execution failed"
                        );
                    }
                    _ => {}
                }
            });

            if tokio::time::timeout(timeout, task).await.is_err() {
                tracing::warn!(
                    trigger = %trigger_str,
                    command = %command_for_warn,
                    timeout_secs,
                    "Hook timed out"
                );
            }
        }
    });
}
