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
//! - `CLAUDE_PLUGIN_ROOT` — the declaring plugin's root (only for a
//!   plugin-contributed hook whose command references `${CLAUDE_PLUGIN_ROOT}`)
//! - `CLAUDE_PROJECT_DIR` — alias of `RAGENT_WORKING_DIR`, for plugin hooks
//! - `RAGENT_TOOL_*` — see [`HookTrigger`] (only for the tool-scoped triggers)
//!
//! A plugin-contributed hook additionally receives the event as JSON on stdin
//! (the Claude hook protocol shape), so a dialect script can read
//! `hook_event_name`, `tool_name`, `tool_input`, and `cwd`.

use ragent_types::event::EventBus;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;

/// Cap a string at `max` characters, trimming whitespace.
fn cap_stderr(stderr: &str, max: usize) -> String {
    stderr.trim().chars().take(max).collect::<String>()
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

impl HookTrigger {
    /// Parse a hook trigger name into a [`HookTrigger`] (FR-033).
    ///
    /// Accepts both ragent's own snake_case spellings (`pre_tool_use`) and the
    /// Claude plugin dialect's PascalCase spellings (`PreToolUse`), ignoring
    /// `-`/`_` separators and case. Claude's `UserPromptSubmit` maps onto
    /// [`OnTurnStart`](Self::OnTurnStart), `Stop`/`SessionEnd` onto
    /// [`OnSessionEnd`](Self::OnSessionEnd), and `PreCompact` onto
    /// [`OnCompaction`](Self::OnCompaction). Returns `None` for an unrecognised
    /// trigger, which the caller drops.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let normalised: String = name
            .trim()
            .chars()
            .filter(|c| !matches!(c, '-' | '_'))
            .collect::<String>()
            .to_ascii_lowercase();
        match normalised.as_str() {
            "onsessionstart" | "sessionstart" => Some(Self::OnSessionStart),
            "onsessionend" | "sessionend" | "stop" => Some(Self::OnSessionEnd),
            "onerror" | "error" => Some(Self::OnError),
            "onpermissiondenied" | "permissiondenied" => Some(Self::OnPermissionDenied),
            "onturnstart" | "turnstart" | "userpromptsubmit" => Some(Self::OnTurnStart),
            "onturnend" | "turnend" => Some(Self::OnTurnEnd),
            "oncompaction" | "compaction" | "precompact" => Some(Self::OnCompaction),
            "pretooluse" => Some(Self::PreToolUse),
            "posttooluse" => Some(Self::PostToolUse),
            _ => None,
        }
    }
}

/// Convert plugin-contributed hooks into session hook configs (FR-033).
///
/// A hook whose trigger name is not recognised by [`HookTrigger::parse`] is
/// dropped (it cannot fire) rather than failing the whole plugin.
#[must_use]
pub fn plugin_hook_configs(plugin_hooks: &[ragent_plugins::PluginHook]) -> Vec<HookConfig> {
    plugin_hooks
        .iter()
        .filter_map(|hook| {
            let trigger = HookTrigger::parse(&hook.trigger)?;
            Some(HookConfig {
                trigger,
                command: hook.command.clone(),
                timeout_secs: hook.timeout_secs.unwrap_or_else(default_hook_timeout),
                plugin_root: Some(hook.plugin_root.clone()),
                matcher: hook.matcher.clone(),
            })
        })
        .collect()
}

/// Merge plugin-contributed hooks with the configured hooks (FR-033).
///
/// Configured hooks run first, then plugin hooks, so a user's own hooks fire
/// before any plugin-contributed hook at the same trigger.
#[must_use]
pub fn merge_hook_configs(
    configured: &[HookConfig],
    plugin_hooks: &[ragent_plugins::PluginHook],
) -> Vec<HookConfig> {
    let mut merged = configured.to_vec();
    merged.extend(plugin_hook_configs(plugin_hooks));
    merged
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
    /// Absolute plugin root exported as `CLAUDE_PLUGIN_ROOT` when the hook
    /// runs. `None` for a hook configured directly in `ragent.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_root: Option<std::path::PathBuf>,
    /// Optional tool match expression (a Claude-dialect `matcher`, such as
    /// `Edit|Write` or `Bash(git commit:*)`). `None` matches every tool. Only
    /// consulted by the tool-scoped triggers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
}

impl HookConfig {
    /// Whether this hook applies to `tool_name` (FR-033).
    ///
    /// A hook with no matcher applies to every tool. A matcher is a `|`- or
    /// `,`-separated list of tool names, each optionally wrapped in a
    /// `Name(pattern)` guard whose pattern is matched as a shell-style glob
    /// against the tool's JSON arguments. A matcher that is not a valid guard
    /// form still matches by bare tool name, so an unrecognised Claude `if`
    /// expression degrades to "runs for that tool" rather than being dropped.
    #[must_use]
    pub fn matches_tool(&self, tool_name: &str, tool_input: &str) -> bool {
        let Some(matcher) = self.matcher.as_deref() else {
            return true;
        };
        matcher.split(['|', ',']).any(|part| {
            let part = part.trim();
            let (name, guard) = match part.split_once('(') {
                Some((name, rest)) => (name.trim(), rest.strip_suffix(')').map(str::trim)),
                None => (part, None),
            };
            if !name.is_empty() && !name.eq_ignore_ascii_case(tool_name) {
                return false;
            }
            match guard {
                Some(pattern) if !pattern.is_empty() && pattern != "*" => {
                    guard_matches_tool_input(pattern, tool_input)
                }
                _ => true,
            }
        })
    }
}

/// Whether a Claude `Name(pattern)` guard matches a tool's JSON arguments.
///
/// A `pattern` ending in `:*` is the Claude prefix form (`Bash(git commit:*)`
/// matches any command starting with `git commit`); any other `*` is a glob
/// wildcard. The pattern is compared against every string value in the
/// tool-input object and the object's compact serialisation.
fn guard_matches_tool_input(pattern: &str, tool_input: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix(":*") {
        let prefix = prefix.trim();
        return candidates_for(tool_input)
            .iter()
            .any(|candidate| candidate.trim_start().starts_with(prefix));
    }
    let Ok(glob) = globset::Glob::new(pattern) else {
        return true;
    };
    let matcher = glob.compile_matcher();
    candidates_for(tool_input)
        .iter()
        .any(|candidate| matcher.is_match(candidate))
}

/// The strings a guard pattern is compared against: the raw tool input, plus
/// every scalar string nested inside it.
fn candidates_for(tool_input: &str) -> Vec<String> {
    let mut candidates = vec![tool_input.to_string()];
    if let Ok(value) = serde_json::from_str::<Value>(tool_input) {
        collect_string_values(&value, &mut candidates);
    }
    candidates
}

/// Collect every scalar string in a JSON value (for the guard match).
fn collect_string_values(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => {
            for item in items {
                collect_string_values(item, out);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_string_values(item, out);
            }
        }
        _ => {}
    }
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

/// The Claude hook-protocol event name for a trigger, or `None` when the
/// trigger has no Claude equivalent (so no stdin payload is written).
fn claude_event_name(trigger: &HookTrigger) -> Option<&'static str> {
    match trigger {
        HookTrigger::OnSessionStart => Some("SessionStart"),
        HookTrigger::OnSessionEnd => Some("Stop"),
        HookTrigger::OnTurnStart => Some("UserPromptSubmit"),
        HookTrigger::PreToolUse => Some("PreToolUse"),
        HookTrigger::PostToolUse => Some("PostToolUse"),
        HookTrigger::OnCompaction => Some("PreCompact"),
        HookTrigger::OnError | HookTrigger::OnPermissionDenied | HookTrigger::OnTurnEnd => None,
    }
}

/// The Claude hook-protocol event JSON written to a hook's stdin.
///
/// Only emitted for a plugin-contributed hook at a trigger Claude recognises;
/// a `ragent.json` hook (no `plugin_root`) receives no stdin payload, keeping
/// the existing behaviour for host-configured hooks.
fn claude_event_payload(
    hook: &HookConfig,
    working_dir: &Path,
    tool_name: Option<&str>,
    tool_input: Option<&str>,
    tool_output: Option<&str>,
    tool_success: Option<bool>,
    session_id: Option<&str>,
) -> Option<String> {
    let event = claude_event_name(&hook.trigger)?;
    // Only a plugin-contributed hook (which carries a plugin root) speaks the
    // Claude protocol; a `ragent.json` hook keeps the env-only contract.
    hook.plugin_root.as_ref()?;
    let mut obj = serde_json::Map::new();
    obj.insert("hook_event_name".into(), Value::String(event.to_string()));
    obj.insert(
        "cwd".into(),
        Value::String(working_dir.display().to_string()),
    );
    if let Some(session_id) = session_id {
        obj.insert("session_id".into(), Value::String(session_id.to_string()));
    }
    if let Some(name) = tool_name {
        obj.insert("tool_name".into(), Value::String(name.to_string()));
    }
    if let Some(input) = tool_input {
        let value = serde_json::from_str::<Value>(input).unwrap_or(Value::Null);
        obj.insert("tool_input".into(), value);
    }
    if let Some(output) = tool_output {
        let value = serde_json::from_str::<Value>(output).unwrap_or(Value::Null);
        obj.insert("tool_response".into(), value);
    }
    if let Some(success) = tool_success {
        obj.insert("tool_success".into(), Value::Bool(success));
    }
    Some(serde_json::to_string(&Value::Object(obj)).unwrap_or_default())
}

/// Attach the shared hook environment to a command builder.
fn apply_hook_env(cmd: &mut std::process::Command, hook: &HookConfig, working_dir: &Path) {
    cmd.env("RAGENT_TRIGGER", hook.trigger.to_string())
        .env("RAGENT_WORKING_DIR", working_dir.display().to_string())
        .env("CLAUDE_PROJECT_DIR", working_dir.display().to_string());
    if let Some(root) = &hook.plugin_root {
        cmd.env("CLAUDE_PLUGIN_ROOT", root.display().to_string());
    }
}

/// Run a fully-configured hook command, writing `payload` to its stdin when
/// present.
///
/// A write error is ignored: the hook may exit without reading its stdin.
fn run_hook_command(
    cmd: &mut std::process::Command,
    payload: Option<String>,
) -> std::io::Result<std::process::Output> {
    use std::io::Write;
    let Some(payload) = payload else {
        return cmd.output();
    };
    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    child.wait_with_output()
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
    /// The hook completed successfully; optionally carries modified output and
    /// guidance to feed to the model.
    Ok {
        /// Last `modified_output` JSON value from a successful hook, if any.
        modified_output: Option<serde_json::Value>,
        /// Guidance text (`additionalContext`) contributed by hooks, in
        /// declaration order. The session appends it to the tool's result so
        /// the model sees it on the next iteration.
        additional_context: Vec<String>,
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
        .filter(|h| h.trigger == HookTrigger::PreToolUse && h.matches_tool(tool_name, tool_input))
        .cloned()
        .collect();

    if matching.is_empty() {
        return PreToolUseResult::NoDecision;
    }

    for hook in matching {
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c")
            .arg(&hook.command)
            .current_dir(working_dir)
            .env("RAGENT_TOOL_NAME", tool_name)
            .env("RAGENT_TOOL_INPUT", tool_input)
            .env("RAGENT_SESSION_ID", session_id);
        apply_hook_env(&mut cmd, &hook, working_dir);
        let output = run_hook_command(
            &mut cmd,
            claude_event_payload(
                &hook,
                working_dir,
                Some(tool_name),
                Some(tool_input),
                None,
                None,
                Some(session_id),
            ),
        );

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
        .filter(|h| h.trigger == HookTrigger::PostToolUse && h.matches_tool(tool_name, tool_input))
        .cloned()
        .collect();

    if matching.is_empty() {
        return PostToolUseResult::Ok {
            modified_output: None,
            additional_context: Vec::new(),
        };
    }

    let mut last_modified_output: Option<serde_json::Value> = None;
    let mut warn_message: Option<String> = None;
    let mut flagged_reason: Option<String> = None;
    let mut additional_contexts: Vec<String> = Vec::new();

    for hook in matching {
        let wd = working_dir.to_path_buf();
        let tool_name = tool_name.to_string();
        let tool_input = tool_input.to_string();
        let tool_output = tool_output.to_string();
        let success_str = success.to_string();
        let session_id = session_id.to_string();
        let command = hook.command.clone();
        let timeout = std::time::Duration::from_secs(hook.timeout_secs);
        let payload = claude_event_payload(
            &hook,
            working_dir,
            Some(&tool_name),
            Some(&tool_input),
            Some(&tool_output),
            Some(success),
            Some(&session_id),
        );
        let plugin_root = hook.plugin_root.clone();
        let trigger = hook.trigger.to_string();
        let session_id_for_cmd = session_id.clone();

        let task = tokio::task::spawn_blocking({
            let tool_name = tool_name.clone();
            let command = command.clone();
            move || {
                let mut cmd = std::process::Command::new("sh");
                cmd.arg("-c")
                    .arg(&command)
                    .current_dir(&wd)
                    .env("RAGENT_TRIGGER", &trigger)
                    .env("RAGENT_WORKING_DIR", wd.display().to_string())
                    .env("CLAUDE_PROJECT_DIR", wd.display().to_string())
                    .env("RAGENT_TOOL_NAME", &tool_name)
                    .env("RAGENT_TOOL_INPUT", &tool_input)
                    .env("RAGENT_TOOL_OUTPUT", &tool_output)
                    .env("RAGENT_TOOL_SUCCESS", &success_str)
                    .env("RAGENT_SESSION_ID", &session_id_for_cmd);
                if let Some(root) = &plugin_root {
                    cmd.env("CLAUDE_PLUGIN_ROOT", root.display().to_string());
                }
                run_hook_command(&mut cmd, payload)
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
                    // FR-033: a Claude-dialect hook delivers guidance to the
                    // model through `hookSpecificOutput.additionalContext`;
                    // also accept a top-level `additionalContext`.
                    let guidance = json
                        .pointer("/hookSpecificOutput/additionalContext")
                        .or_else(|| json.get("additionalContext"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty());
                    if let Some(guidance) = guidance {
                        additional_contexts.push(guidance.to_string());
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
            additional_context: additional_contexts,
        }
    }
}

/// Run the stop (`on_session_end`) hooks and collect any guidance that should
/// be fed back to the model (the Claude `Stop` / `asyncRewake` shape).
///
/// A hook signals a continuation in three ways, all accepted here:
///
/// - exit code **2** — the hook blocked; its stdout JSON (Claude's
///   `decision: "block"` / `hookSpecificOutput.additionalContext`) and then its
///   stderr are captured as the findings;
/// - exit code **0** with `hookSpecificOutput.additionalContext`;
/// - exit code **0** with a top-level `decision: "block"` and `reason`.
///
/// Every other outcome (exit 0 without guidance, exit 1 warning, exit >=3
/// failure, timeout) contributes nothing, so a well-behaved hook does not
/// extend the turn. Returns the collected guidance in declaration order; an
/// empty result means the turn should end normally.
pub async fn run_stop_hooks(
    hooks: &[HookConfig],
    working_dir: &Path,
    session_id: &str,
) -> Vec<String> {
    let matching: Vec<HookConfig> = hooks
        .iter()
        .filter(|h| h.trigger == HookTrigger::OnSessionEnd)
        .cloned()
        .collect();

    let mut guidance = Vec::new();
    for hook in matching {
        let wd = working_dir.to_path_buf();
        let command = hook.command.clone();
        let timeout = std::time::Duration::from_secs(hook.timeout_secs);
        let payload =
            claude_event_payload(&hook, working_dir, None, None, None, None, Some(session_id));
        let plugin_root = hook.plugin_root.clone();
        let trigger = hook.trigger.to_string();
        let session_id = session_id.to_string();

        let task = tokio::task::spawn_blocking({
            let command = command.clone();
            move || {
                let mut cmd = std::process::Command::new("sh");
                cmd.arg("-c")
                    .arg(&command)
                    .current_dir(&wd)
                    .env("RAGENT_TRIGGER", &trigger)
                    .env("RAGENT_WORKING_DIR", wd.display().to_string())
                    .env("CLAUDE_PROJECT_DIR", wd.display().to_string())
                    .env("RAGENT_SESSION_ID", &session_id);
                if let Some(root) = &plugin_root {
                    cmd.env("CLAUDE_PLUGIN_ROOT", root.display().to_string());
                }
                run_hook_command(&mut cmd, payload)
            }
        });

        let Ok(Ok(Ok(out))) = tokio::time::timeout(timeout, task).await else {
            tracing::warn!(
                trigger = "on_session_end",
                command = %command,
                "Stop hook timed out or failed to run; ending the turn normally"
            );
            continue;
        };

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let json = serde_json::from_str::<serde_json::Value>(stdout.trim()).ok();

        // Claude's `hookSpecificOutput.additionalContext` is the primary
        // delivery channel for a Stop hook.
        if let Some(guidance_text) = json
            .as_ref()
            .and_then(|json| json.pointer("/hookSpecificOutput/additionalContext"))
            .or_else(|| json.as_ref().and_then(|json| json.get("additionalContext")))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            guidance.push(guidance_text.to_string());
        }

        match out.status.code() {
            Some(2) => {
                // Blocking Stop hook: prefer the JSON reason, then stderr.
                let reason = json
                    .as_ref()
                    .and_then(|json| json.get("reason"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| cap_stderr(&stderr, 8000));
                if !reason.is_empty() && !guidance.iter().any(|g| g == &reason) {
                    guidance.push(reason);
                }
                tracing::info!(
                    trigger = "on_session_end",
                    command = %command,
                    "Stop hook blocked the turn; findings will be fed back to the model"
                );
            }
            Some(0) => {
                if let Some(reason) = json
                    .as_ref()
                    .filter(|json| {
                        json.get("decision").and_then(serde_json::Value::as_str) == Some("block")
                    })
                    .and_then(|json| json.get("reason"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    guidance.push(reason.to_string());
                }
            }
            _ => {}
        }
    }
    guidance
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
            let trigger_s = trigger.to_string();
            let extra_e = extra.clone();
            let timeout = std::time::Duration::from_secs(hook.timeout_secs);
            let command = hook.command.clone();
            let command_for_warn = command.clone();
            let timeout_secs = hook.timeout_secs;
            let plugin_root = hook.plugin_root.clone();
            let payload = claude_event_payload(&hook, &working_dir, None, None, None, None, None);

            let task = tokio::spawn(async move {
                let mut cmd = tokio::process::Command::new("sh");
                cmd.arg("-c")
                    .arg(&command)
                    .current_dir(&wd)
                    .env("RAGENT_TRIGGER", &trigger_s)
                    .env("RAGENT_WORKING_DIR", wd.display().to_string())
                    .env("CLAUDE_PROJECT_DIR", wd.display().to_string());
                for (k, v) in &extra_e {
                    cmd.env(k, v);
                }
                if let Some(root) = &plugin_root {
                    cmd.env("CLAUDE_PLUGIN_ROOT", root.display().to_string());
                }
                match run_hook_command_async(cmd, payload).await {
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

/// Run a hook command asynchronously, writing `payload` to its stdin when
/// present (used by the fire-and-forget triggers).
async fn run_hook_command_async(
    mut cmd: tokio::process::Command,
    payload: Option<String>,
) -> std::io::Result<std::process::Output> {
    use tokio::io::AsyncWriteExt;
    let Some(payload) = payload else {
        return cmd.output().await;
    };
    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes()).await;
    }
    child.wait_with_output().await
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
