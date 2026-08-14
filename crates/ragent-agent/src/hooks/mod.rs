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
//!     },
//!     {
//!       "trigger": "on_turn_start",
//!       "command": "echo 'Turn starting' >> ~/.ragent/turns.log"
//!     },
//!     {
//!       "trigger": "on_compaction",
//!       "command": "echo 'Context compacted' >> ~/.ragent/compaction.log"
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
//! - `RAGENT_TURN_NUMBER` — current turn/iteration number (for `on_turn_start`/`on_turn_end`)
//! - `RAGENT_COMPACTION_REASON` — reason for compaction (for `on_compaction`)

use ragent_types::event::EventBus;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// Cap a string at `max` characters, trimming whitespace.
fn cap_stderr(stderr: &str, max: usize) -> String {
    let trimmed = stderr.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        trimmed.chars().take(max).collect::<String>()
    }
}

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
    /// Fired at the start of each agent turn/iteration.
    OnTurnStart,
    /// Fired at the end of each agent turn/iteration.
    OnTurnEnd,
    /// Fired when context compaction is performed.
    OnCompaction,
    /// Fired before a tool is executed, allowing hooks to approve/deny/modify/block.
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
    ///
    /// In addition, the hook exit code is interpreted as follows:
    /// - `0` - parse stdout JSON as above
    /// - `1` - allow the tool but emit a warning (`Event::HookWarning`)
    /// - `2` - block the tool; stderr is used as the reason and stdout JSON is ignored
    /// - `>= 3` - treat as a hook failure and fall through to normal permission flow
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
    ///
    /// In addition, the hook exit code is interpreted as follows:
    /// - `0` - parse stdout JSON as above
    /// - `1` - emit a warning (`Event::HookWarning`) but do not modify output
    /// - `2` - flag the tool result as policy-violated (`Event::ToolResultFlagged`);
    ///   the tool result is not suppressed but the flag appears in the session log
    /// - `>= 3` - treat as a hook failure (error diagnostic only)
    PostToolUse,
}

impl std::fmt::Display for HookTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OnSessionStart => write!(f, "on_session_start"),
            Self::OnSessionEnd => write!(f, "on_session_end"),
            Self::OnError => write!(f, "on_error"),
            Self::OnPermissionDenied => write!(f, "on_permission_denied"),
            Self::OnTurnStart => write!(f, "on_turn_start"),
            Self::OnTurnEnd => write!(f, "on_turn_end"),
            Self::OnCompaction => write!(f, "on_compaction"),
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
    /// The hook blocked the tool via a non-zero exit code (exit code 2).
    Blocked {
        /// Reason for blocking the tool execution, typically hook stderr.
        reason: String,
    },
    /// No decision from hook - use normal permission flow.
    NoDecision,
}

/// Result of running a post-tool-use hook.
///
/// The exit code of each hook determines the variant:
/// - `0` → [`Ok`](Self::Ok) with optional modified output from stdout JSON.
/// - `1` → [`Warn`](Self::Warn) — a warning was emitted.
/// - `2` → [`Flagged`](Self::Flagged) — the result is policy-violated.
/// - `>= 3` → treated as a hook failure (no effect on the result).
#[derive(Debug, Clone)]
pub enum PostToolUseResult {
    /// The hook completed successfully; optionally carries modified output.
    Ok {
        /// Last `modified_output` JSON value from a successful hook, if any.
        modified_output: Option<serde_json::Value>,
    },
    /// The hook flagged the tool result as policy-violated (exit code 2).
    Flagged {
        /// Reason for flagging, typically the hook's stderr.
        reason: String,
    },
    /// The hook emitted a warning (exit code 1).
    Warn {
        /// Warning message, typically the hook's stderr.
        message: String,
    },
}

/// Run hooks for PreToolUse synchronously and collect their decisions.
///
/// This function runs hooks synchronously (unlike `fire_hooks` which is async)
/// because it needs to potentially modify or block tool execution.
///
/// `session_id` and `event_bus` are used to publish `Event::HookWarning` when a
/// hook exits with code 1. When `event_bus` is `None`, the warning is only
/// logged via `tracing::warn!`.
///
/// # Returns
///
/// Returns the first hook result that makes a decision (`Allow`, `Deny`,
/// `ModifiedInput`, or `Blocked`). If no hooks make a decision, returns
/// `NoDecision`.
///
/// # Examples
///
/// ```
/// use ragent_agent::hooks::{run_pre_tool_use_hooks, HookConfig, HookTrigger};
/// use ragent_types::event::EventBus;
/// use std::path::Path;
///
/// let hooks = vec![];
/// let bus = EventBus::default();
/// let result = run_pre_tool_use_hooks(
///     &hooks,
///     Path::new("/tmp"),
///     "read",
///     r#"{"path": "src/main.rs"}"#,
///     "sess-doc",
///     Some(&bus),
/// );
/// // Returns NoDecision when no hooks configured
/// ```
pub fn run_pre_tool_use_hooks(
    hooks: &[HookConfig],
    working_dir: &Path,
    tool_name: &str,
    tool_input: &str,
    session_id: &str,
    event_bus: Option<&EventBus>,
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
                let code = out.status.code();
                let stderr = String::from_utf8_lossy(&out.stderr);
                let capped_stderr = cap_stderr(&stderr, 500);

                match code {
                    Some(2) => {
                        tracing::info!(
                            tool = %tool_name,
                            hook_command = %hook.command,
                            exit_code = 2,
                            stderr = %capped_stderr,
                            "PreToolUse hook blocked tool via exit code 2"
                        );
                        return PreToolUseResult::Blocked {
                            reason: capped_stderr.clone(),
                        };
                    }
                    Some(1) => {
                        tracing::warn!(
                            trigger = "pre_tool_use",
                            command = %hook.command,
                            tool = %tool_name,
                            exit_code = 1,
                            stderr = %capped_stderr,
                            "PreToolUse hook exited with code 1 - allowing with warning"
                        );
                        if let Some(bus) = event_bus {
                            bus.publish(ragent_types::event::Event::HookWarning {
                                session_id: session_id.to_string(),
                                hook_command: hook.command.clone(),
                                tool: tool_name.to_string(),
                                stderr: capped_stderr,
                            });
                        }
                    }
                    _ => {
                        tracing::error!(
                            trigger = "pre_tool_use",
                            command = %hook.command,
                            tool = %tool_name,
                            exit_code = ?code,
                            stderr = %capped_stderr,
                            "PreToolUse hook failed with exit code >=3 - falling through to normal permission flow"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    trigger = "pre_tool_use",
                    command = %hook.command,
                    error = %e,
                    "PreToolUse hook spawn failed — treating as hook error (exit >=3)"
                );
            }
        }
    }

    PreToolUseResult::NoDecision
}

/// Run hooks for PostToolUse asynchronously.
///
/// This function runs hooks asynchronously and allows them to modify the tool
/// output. It also interprets exit codes to publish warnings and flags:
///
/// - **Exit code 0** — parse stdout JSON for `modified_output`.
/// - **Exit code 1** — emit `tracing::warn!` and publish `Event::HookWarning`.
/// - **Exit code 2** — publish `Event::ToolResultFlagged` with stderr as the
///   reason. The tool result is not suppressed, but the flag appears in the
///   session log and TUI.
/// - **Exit code ≥ 3** — treat as a hook failure (`tracing::error!`).
///
/// `session_id` and `event_bus` are used to publish events. When `event_bus` is
/// `None`, warnings and flags are only logged.
///
/// # Returns
///
/// Returns [`PostToolUseResult::Flagged`] if any hook exited with code 2,
/// [`PostToolUseResult::Warn`] if any hook exited with code 1 (and none with 2),
/// or [`PostToolUseResult::Ok`] with the last `modified_output` from a
/// successful hook.
pub async fn run_post_tool_use_hooks(
    hooks: &[HookConfig],
    working_dir: &Path,
    tool_name: &str,
    tool_input: &str,
    tool_output: &str,
    success: bool,
    session_id: &str,
    event_bus: Option<&EventBus>,
) -> PostToolUseResult {
    let matching: Vec<HookConfig> = hooks
        .iter()
        .filter(|h| h.trigger == HookTrigger::PostToolUse)
        .cloned()
        .collect();

    if matching.is_empty() {
        return PostToolUseResult::Ok {
            modified_output: None,
        };
    }

    let mut last_modified_output: Option<serde_json::Value> = None;
    let mut warn_message: Option<String> = None;
    let mut flagged_reason: Option<String> = None;

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
                let code = out.status.code();
                let stderr = String::from_utf8_lossy(&out.stderr);
                let capped_stderr = cap_stderr(&stderr, 500);

                match code {
                    Some(2) => {
                        tracing::info!(
                            tool = %tool_name,
                            hook_command = %command,
                            exit_code = 2,
                            stderr = %capped_stderr,
                            "PostToolUse hook flagged tool result as policy-violated"
                        );
                        if let Some(bus) = event_bus {
                            bus.publish(ragent_types::event::Event::ToolResultFlagged {
                                session_id: session_id.to_string(),
                                tool: tool_name.to_string(),
                                hook_command: command.clone(),
                                reason: capped_stderr.clone(),
                            });
                        }
                        flagged_reason = Some(capped_stderr);
                    }
                    Some(1) => {
                        tracing::warn!(
                            trigger = "post_tool_use",
                            command = %command,
                            tool = %tool_name,
                            exit_code = 1,
                            stderr = %capped_stderr,
                            "PostToolUse hook exited with code 1 - warning"
                        );
                        if let Some(bus) = event_bus {
                            bus.publish(ragent_types::event::Event::HookWarning {
                                session_id: session_id.to_string(),
                                hook_command: command.clone(),
                                tool: tool_name.to_string(),
                                stderr: capped_stderr.clone(),
                            });
                        }
                        warn_message = Some(capped_stderr);
                    }
                    _ => {
                        tracing::error!(
                            trigger = "post_tool_use",
                            command = %command,
                            tool = %tool_name,
                            exit_code = ?code,
                            stderr = %capped_stderr,
                            "PostToolUse hook failed with exit code >=3"
                        );
                    }
                }
            }
            Ok(Ok(Err(e))) => {
                tracing::error!(
                    trigger = "post_tool_use",
                    command = %command,
                    error = %e,
                    "PostToolUse hook spawn failed — treating as hook error (exit >=3)"
                );
            }
            Ok(Err(_)) => {
                tracing::error!(
                    trigger = "post_tool_use",
                    command = %command,
                    "PostToolUse hook task panicked"
                );
            }
            Err(_) => {
                tracing::error!(
                    trigger = "post_tool_use",
                    command = %command,
                    timeout_secs = hook.timeout_secs,
                    "PostToolUse hook timed out — treating as hook error (exit >=3)"
                );
            }
        }
    }

    if let Some(reason) = flagged_reason {
        PostToolUseResult::Flagged { reason }
    } else if let Some(message) = warn_message {
        PostToolUseResult::Warn { message }
    } else {
        PostToolUseResult::Ok {
            modified_output: last_modified_output,
        }
    }
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

/// Fire hooks for turn start event.
///
/// # Arguments
///
/// * `hooks` - Parsed hook configurations
/// * `working_dir` - Working directory for hook execution
/// * `turn_number` - Current turn/iteration number
pub fn fire_turn_start_hooks(hooks: &[HookConfig], working_dir: &Path, turn_number: usize) {
    fire_hooks(
        hooks,
        HookTrigger::OnTurnStart,
        working_dir,
        &[("RAGENT_TURN_NUMBER", &turn_number.to_string())],
    );
}

/// Fire hooks for turn end event.
///
/// # Arguments
///
/// * `hooks` - Parsed hook configurations
/// * `working_dir` - Working directory for hook execution
/// * `turn_number` - Current turn/iteration number
pub fn fire_turn_end_hooks(hooks: &[HookConfig], working_dir: &Path, turn_number: usize) {
    fire_hooks(
        hooks,
        HookTrigger::OnTurnEnd,
        working_dir,
        &[("RAGENT_TURN_NUMBER", &turn_number.to_string())],
    );
}

/// Fire hooks for compaction event.
///
/// # Arguments
///
/// * `hooks` - Parsed hook configurations
/// * `working_dir` - Working directory for hook execution
/// * `reason` - Reason for compaction (e.g., "auto", "manual")
/// * `tokens_before` - Token count before compaction
/// * `tokens_after` - Token count after compaction
pub fn fire_compaction_hooks(
    hooks: &[HookConfig],
    working_dir: &Path,
    reason: &str,
    tokens_before: usize,
    tokens_after: usize,
) {
    fire_hooks(
        hooks,
        HookTrigger::OnCompaction,
        working_dir,
        &[
            ("RAGENT_COMPACTION_REASON", reason),
            (
                "RAGENT_COMPACTION_TOKENS_BEFORE",
                &tokens_before.to_string(),
            ),
            ("RAGENT_COMPACTION_TOKENS_AFTER", &tokens_after.to_string()),
        ],
    );
}
