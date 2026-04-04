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
}

impl std::fmt::Display for HookTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookTrigger::OnSessionStart => write!(f, "on_session_start"),
            HookTrigger::OnSessionEnd => write!(f, "on_session_end"),
            HookTrigger::OnError => write!(f, "on_error"),
            HookTrigger::OnPermissionDenied => write!(f, "on_permission_denied"),
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

fn default_hook_timeout() -> u64 {
    30
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
