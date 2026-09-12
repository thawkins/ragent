//! Bash command splitting and permission-check helpers for the agent loop.
//!
//! These helpers are used by [`crate::session::processor::SessionProcessor`]
//! to decide whether a tool invocation needs an interactive permission
//! prompt. They cover:
//!
//! - extracting the resource identifier from tool input JSON,
//! - splitting bash command strings on `&&`/`||`/`;` so each sub-command is
//!   checked independently,
//! - stripping the `timeout [nnn]` wrapper prefix,
//! - identifying tools that are hardwired to auto-approve (codeindex, team,
//!   task, `ask_user`), and
//! - driving the interactive permission prompt via the event bus.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, bail};
use serde_json::Value;
use tokio::sync::broadcast::error::RecvError;
use tracing::debug;
use uuid::Uuid;

use crate::event::{Event, EventBus};
use crate::permission::{PermissionAction, PermissionChecker};

/// Extract a resource identifier from tool input JSON for permission checks.
///
/// Tries common parameter names (`path`, `command`, `url`, `pattern`, `query`)
/// and falls back to the tool name if none are found.
pub(crate) fn extract_resource_from_input(input: &Value, tool_name: &str) -> String {
    input
        .get("path")
        .or_else(|| input.get("command"))
        .or_else(|| input.get("url"))
        .or_else(|| input.get("pattern"))
        .or_else(|| input.get("query"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("tool:{tool_name}"))
}

/// Strip the `timeout [nnn]` prefix from a command string if present.
///
/// The timeout command takes another command as an argument. When evaluating
/// permissions, we need to check the actual command being executed, not the
/// timeout wrapper itself.
///
/// # Examples
///
/// ```ignore
/// // `strip_timeout_prefix` is `pub(crate)`: see the crate's integration tests
/// // for direct invocation.
/// assert_eq!(strip_timeout_prefix("timeout 600 cargo build"), "cargo build");
/// assert_eq!(strip_timeout_prefix("timeout 10 ls -la"), "ls -la");
/// assert_eq!(strip_timeout_prefix("ls -la"), "ls -la");
/// ```
pub(crate) fn strip_timeout_prefix(command: &str) -> &str {
    let trimmed = command.trim();

    // Check if the command starts with "timeout"
    if let Some(rest) = trimmed.strip_prefix("timeout") {
        // Must be followed by whitespace
        if rest.starts_with(char::is_whitespace) {
            let rest = rest.trim_start();

            // Next token should be a number (the timeout value)
            if let Some(space_pos) = rest.find(char::is_whitespace) {
                let potential_number = &rest[..space_pos];
                if potential_number.chars().all(|c| c.is_ascii_digit()) {
                    // Found "timeout [nnn] ...", return the rest after the number
                    return rest[space_pos..].trim_start();
                }
            }
        }
    }

    // No timeout prefix found, return original
    trimmed
}

/// Split a bash command string on common delimiters (`&&`, `||`, `;`) to
/// extract individual sub-commands for separate permission checks.
///
/// This handles simple cases but does NOT parse full bash syntax (quotes,
/// heredocs, etc.). It's a best-effort split for permission UX.
pub(crate) fn split_bash_command(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    // Flush a completed sub-command into `parts` (trim + strip timeout prefix).
    let mut flush = |current: &mut String| {
        let trimmed = current.trim();
        if !trimmed.is_empty() {
            // Strip timeout prefix before adding to parts
            parts.push(strip_timeout_prefix(trimmed).to_string());
        }
        current.clear();
    };

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current.push(c);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current.push(c);
            }
            '&' | '|' | ';' if !in_single_quote && !in_double_quote => {
                // Check for && or ||
                if (c == '&' || c == '|') && chars.peek() == Some(&c) {
                    chars.next(); // consume the second character
                    flush(&mut current);
                } else if c == ';' {
                    flush(&mut current);
                } else {
                    // Single & or | - add to current command
                    current.push(c);
                }
            }
            _ => current.push(c),
        }
    }

    // Add the final part
    flush(&mut current);
    // If no delimiters found, return the original command (with timeout stripped)
    if parts.is_empty() {
        vec![strip_timeout_prefix(command).to_string()]
    } else {
        parts
    }
}

/// Extract just the command name (first word) from a bash command string.
/// This is used for permission checking so that "ls -la" matches against "ls" patterns.
pub(crate) fn extract_command_name(command: &str) -> String {
    let trimmed = command.trim();
    // Find the first whitespace, if any
    if let Some(space_pos) = trimmed.find(char::is_whitespace) {
        trimmed[..space_pos].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Return `true` if the tool is hardwired to auto-approve (no interactive
/// prompt). Covers codeindex tools, team tools, sub-agent tools
/// (`new_agent`, `cancel_agent`, `list_agents`, `wait_agents`,
/// `agent_complete`), `task_*` (T-011, FR-017),
/// and `ask_user`.
pub(crate) fn is_hardwired_auto_approved_tool(tool_name: &str) -> bool {
    is_hardwired_codeindex_tool(tool_name)
        || tool_name.starts_with("team_")
        || AUTO_APPROVED_AGENT_TOOLS.contains(&tool_name)
        || tool_name.starts_with("task_")
        || tool_name == "ask_user"
        || tool_name == "model_info"
}

/// Return `true` for the read-only codeindex tools that are always allowed
/// regardless of skill restrictions.
pub(crate) fn is_hardwired_codeindex_tool(tool_name: &str) -> bool {
    const AUTO_APPROVED_CODEINDEX_TOOLS: &[&str] = &[
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_reindex",
    ];
    AUTO_APPROVED_CODEINDEX_TOOLS.contains(&tool_name)
}

const AUTO_APPROVED_AGENT_TOOLS: &[&str] = &[
    "new_agent",
    "cancel_agent",
    "list_agents",
    "wait_agents",
    "agent_complete",
];

/// Interactive tools (subagent reliability fix): tools whose execution waits
/// on a live user attached to the session. Sub-agent runs (background, cron,
/// server, forked skill) have no attached user, so these are filtered from
/// the tool surface and denied at dispatch - an unanswered interactive
/// dialog would otherwise block the sub-agent run until the watchdog.
pub(crate) const INTERACTIVE_TOOLS: &[&str] = &["ask_user"];

/// Return `true` when `tool_name` waits on live user interaction and must
/// be denied in sub-agent runs ([`INTERACTIVE_TOOLS`]).
pub fn is_interactive_tool(tool_name: &str) -> bool {
    INTERACTIVE_TOOLS.contains(&tool_name)
}

/// Destructive tool names (FR-015, T-010): tool calls that irreversibly
/// mutate the workspace or protected state and therefore require a
/// destructive-action checkpoint before they execute. Deletion, config
/// writes, dependency installation, and destructive git operations are all
/// marked here; the checkpoint layer (`SessionProcessor` tool dispatch)
/// consults [`is_destructive_tool`] before every permission check.
pub const DESTRUCTIVE_TOOLS: &[&str] = &[
    // Deletion
    "rm",
    // Config writes (ragent.json and agent/config state)
    "config_write",
    "config_save",
    // Dependency installation
    "npm_install",
    "cargo_install",
];

/// Destructive git tool names (T-010). The destructive variants are keyed
/// on the request flags carried in the resource string — `git_push` only
/// with `--force`, `git_reset` only for hard modes, `git_stash` only for
/// drop/clear — so read-only git workflows are not blocked.
pub const DESTRUCTIVE_GIT_TOOLS: &[&str] = &[
    "git_push",
    "git_reset",
    "git_checkout",
    "git_stash",
    "git_tag",
    "git_merge",
    "git_cherry_pick",
    "git_commit",
    "git_clone",
];

/// Bash command prefixes that count as destructive actions for the
/// checkpoint layer (T-010): deletion, destructive git sub-commands, config
/// writes, and dependency installation. Package/build commands (cargo test,
/// npm run, ...) stay on the normal permission path.
pub(crate) const BASH_DESTRUCTIVE_COMMANDS: &[&str] = &[
    // Deletion (the safe-command allowlist already excludes rm; the
    // checkpoint additionally intercepts it when a loop runs).
    "rm",
    "rmdir",
    "shred",
    // Destructive git sub-commands (checkpointed even under the `git`
    // safe-command allowlist).
    "git push --force",
    "git push -f",
    "git reset --hard",
    "git checkout -- .",
    "git stash drop",
    "git tag -d",
    "git branch -D",
    // Config writes
    "ragent config",
    // Dependency installation
    "cargo install",
    "npm install",
    "yarn add",
    "pnpm add",
];

/// Whether a tool call performs a destructive action (FR-015, T-010).
///
/// Consulted by the checkpoint layer with the parsed tool input. Deletion
/// (`rm`), config writes, dependency installation, and destructive git
/// operations are marked; destructive git variants are keyed on the request
/// flags (`git_push` only with `force`, `git_reset` only for hard modes, so
/// read-only git workflows are not blocked). `false` when the call is not
/// destructive and the normal permission path applies unchanged. `bash` is
/// special-cased: its sub-commands are scanned for destructive command
/// prefixes.
pub(crate) fn is_destructive_tool(tool_name: &str, input: &Value) -> bool {
    if DESTRUCTIVE_TOOLS.contains(&tool_name) {
        return true;
    }
    if DESTRUCTIVE_GIT_TOOLS.contains(&tool_name) {
        return is_destructive_git_invocation(tool_name, input);
    }
    if tool_name == "bash" {
        let command = input.get("command").and_then(Value::as_str).unwrap_or("");
        return bash_has_destructive_command(command);
    }
    false
}

/// Whether one git tool invocation is destructive (T-010), keyed on the
/// request flags in the tool input JSON.
fn is_destructive_git_invocation(tool_name: &str, input: &Value) -> bool {
    match tool_name {
        "git_push" => input.get("force").and_then(Value::as_bool).unwrap_or(false),
        "git_reset" => {
            matches!(
                input.get("mode").and_then(Value::as_str),
                Some("hard") | Some("keep")
            )
        }
        "git_checkout" => {
            input.get("paths").is_some()
                || input.get("force").and_then(Value::as_bool).unwrap_or(false)
        }
        "git_stash" => {
            matches!(
                input.get("action").and_then(Value::as_str),
                Some("drop") | Some("clear")
            )
        }
        "git_tag" => {
            matches!(input.get("action").and_then(Value::as_str), Some("delete"))
        }
        // Merge/cherry-pick always rewrite or create history and count as
        // destructive for the checkpoint layer.
        _ => true,
    }
}

/// Whether one bash sub-command is destructive (best-effort word-boundary
/// match against [`BASH_DESTRUCTIVE_COMMANDS`], mirroring the
/// `is_safe_command` prefix logic).
fn bash_is_destructive(sub_command: &str) -> bool {
    BASH_DESTRUCTIVE_COMMANDS.iter().any(|pattern| {
        sub_command == *pattern
            || sub_command
                .strip_prefix(pattern)
                .is_some_and(|rest| rest.starts_with(' '))
    })
}

/// Whether a bash command string contains a destructive sub-command
/// (FR-015, T-010). Best-effort, mirroring the permission layer's bash
/// splitting: the command is split on `&&`/`||`/`;` and each sub-command is
/// checked against [`BASH_DESTRUCTIVE_COMMANDS`].
pub(crate) fn bash_has_destructive_command(command: &str) -> bool {
    split_bash_command(command)
        .iter()
        .any(|sub| bash_is_destructive(sub))
}

/// The denial observation text appended to the model's context when a
/// destructive-action checkpoint is denied (timeout or user refusal).
pub(crate) fn checkpoint_reason(tool_name: &str, resource: &str) -> String {
    format!(
        "checkpoint denied: destructive action '{tool_name}' on '{resource}' was \
         not approved — a human-approval checkpoint protects destructive \
         actions (file deletion, config writes, dependency installation, \
         destructive git) for this loop. Continue without performing the \
         destructive action, or ask the user to approve it manually."
    )
}

/// Test-only re-export of [`checkpoint_reason`]: integration tests live in a
/// separate crate and cannot reach the `pub(crate)` helper directly.
#[doc(hidden)]
pub fn checkpoint_reason_for_test(tool_name: &str, resource: &str) -> String {
    checkpoint_reason(tool_name, resource)
}

/// Test-only re-export of [`is_destructive_tool`]: integration tests live in
/// a separate crate and cannot reach the `pub(crate)` helper directly.
#[doc(hidden)]
pub fn is_destructive_tool_for_test(tool_name: &str, input: &Value) -> bool {
    is_destructive_tool(tool_name, input)
}

/// Drive one interactive permission prompt: publish a
/// `PermissionRequested` event and wait for the matching
/// `PermissionReplied` event up to `timeout_secs`; a timeout counts as
/// denial (FR-015: the intervention defaults to the safe choice).
///
/// "Always" grants recorded via the reply are stored permanently on the
/// checker.
async fn prompt_for_permission(
    checker: &Arc<parking_lot::RwLock<PermissionChecker>>,
    event_bus: &Arc<EventBus>,
    session_id: &str,
    permission: &str,
    resource: &str,
    tool_name: &str,
    timeout_secs: u64,
) -> Result<PermissionAction> {
    let request_id = Uuid::new_v4().to_string();
    let mut rx = event_bus.subscribe();

    // Publish request.
    event_bus.publish(Event::PermissionRequested {
        session_id: session_id.to_string(),
        request_id: request_id.clone(),
        permission: permission.to_string(),
        description: format!("{tool_name}: {resource}"),
        options: vec![],
    });

    let timeout = tokio::time::Duration::from_secs(timeout_secs);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        // A zero remaining timeout resolves `tokio::time::timeout` to `Err`
        // on the first pass, so no separate deadline check is needed here.
        let recv_timeout = deadline.saturating_duration_since(tokio::time::Instant::now());

        match tokio::time::timeout(recv_timeout, rx.recv()).await {
            Ok(Ok(Event::PermissionReplied {
                request_id: rid,
                allowed,
                decision,
                ..
            })) if rid == request_id => {
                // If user chose 'Always', record the grant.
                if allowed && decision == crate::permission::PermissionDecision::Always {
                    let mut c = checker.write();
                    c.record_always(permission, resource);
                    debug!(
                        "Recorded always-grant for permission={permission}, resource={resource}"
                    );
                }
                return Ok(if allowed {
                    PermissionAction::Allow
                } else {
                    PermissionAction::Deny
                });
            }
            Ok(Err(RecvError::Lagged(_))) => {
                // Idle-CPU fix: yield briefly so a lagged subscriber resyncs
                // without pinning a core (see the main prompt loop below).
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Ok(Err(_)) => {
                bail!("Event bus closed during permission check");
            }
            Err(_) => {
                // Timeout counts as denial (FR-015).
                debug!("Permission request timeout for {tool_name}");
                return Ok(PermissionAction::Deny);
            }
            _ => continue,
        }
    }
}

/// Tools that are always available to an agent even when a skill's
/// `allowed_tools` list is being enforced. These are essential control and
/// introspection tools that must not be removed from the LLM's tool list.
pub(crate) const SKILL_ALWAYS_ALLOWED_TOOLS: &[&str] =
    &["think", "ask_user", "agent_complete", "model_info"];

/// Check permission for a tool execution, prompting the user if necessary.
///
/// Returns `Allow` or `Deny`. If the policy says `Ask`, this publishes a
/// `PermissionRequested` event and waits for a user reply up to the
/// checkpoint timeout; a timeout is treated as denial.
///
/// `auto_approve` distinguishes the plain auto-approve flag from a loop
/// destructive-action checkpoint (FR-015, FR-024, T-010):
///
/// - `Some(true)` — the plain `--yes`/autopilot auto-approval path: prompts
///   are answered automatically and `Allow` is returned without prompting.
/// - `Some(false)` — interactive mode: a `Deny` verdict denies without
///   prompting and an `Ask` verdict prompts the user with the standard
///   120-second timeout.
/// - `None` — a loop run with checkpoints enabled: `checkpoint_forced` is
///   `true` for destructive calls and forces the human-approval prompt
///   with the reply window controlled by `checkpoint_timeout_secs`, even
///   under allow rules and in auto-approve mode.
///
/// # Errors
///
/// Returns an error if the event bus closes during the wait.
pub async fn check_permission_with_prompt(
    checker: &Arc<parking_lot::RwLock<PermissionChecker>>,
    event_bus: &Arc<EventBus>,
    session_id: &str,
    permission: &str,
    resource: &str,
    tool_name: &str,
    auto_approve: Option<bool>,
    canonical_cache: Option<&ragent_tools_core::CanonicalPathCache>,
    checkpoint_forced: bool,
    checkpoint_timeout_secs: u64,
) -> Result<PermissionAction> {
    // Short-circuit if --yes / --no-prompt flag is set. A forced
    // destructive-action checkpoint (FR-015) suppresses this: auto-approval
    // may answer prompts automatically, but loop checkpoints are still
    // enforced (FR-024).
    if auto_approve == Some(true) && !checkpoint_forced {
        return Ok(PermissionAction::Allow);
    }

    // YOLO mode bypasses interactive permission prompts for all tools. A
    // forced destructive-action checkpoint still applies (FR-024).
    if ragent_config::yolo::is_enabled() && !checkpoint_forced {
        return Ok(PermissionAction::Allow);
    }

    // Codeindex tools, team tools, and task_* tools are hardwired helpers
    // and must never trigger interactive permission prompts.
    if is_hardwired_auto_approved_tool(tool_name) {
        return Ok(PermissionAction::Allow);
    }

    // FR-004/FR-017: Check in-memory rules (dir_lists + PermissionChecker)
    // BEFORE any filesystem I/O.  When an explicit rule already grants or
    // denies the request, the blocking `canonicalize()` in the file:read
    // auto-grant below is never reached.
    //
    // FR-015 (T-010): a forced destructive-action checkpoint suppresses the
    // allowlist auto-approve so the request reaches the checkpoint prompt.
    if (permission.starts_with("file:")
        || permission == "read"
        || permission == "edit"
        || permission == "write")
        && !checkpoint_forced
    {
        use ragent_config::dir_lists::{get_compiled_allowlist, get_compiled_denylist};

        let denylist = get_compiled_denylist();
        let allowlist = get_compiled_allowlist();

        // Denylist takes precedence - immediately reject
        if denylist.is_match(resource) {
            return Ok(PermissionAction::Deny);
        }

        // Allowlist - immediately approve
        if allowlist.is_match(resource) {
            return Ok(PermissionAction::Allow);
        }
    }

    // Check PermissionChecker (in-memory rule lookup — no I/O).
    let action = {
        let c = checker.read();
        c.check(permission, resource)
    };

    match action {
        PermissionAction::Allow | PermissionAction::Deny => {
            // FR-015 (T-010): a forced destructive-action checkpoint
            // overrides an allow verdict — the call goes to a human
            // checkpoint prompt even when a rule would auto-approve it
            // (FR-024: the permission layer is always in the path).
            if checkpoint_forced && action == PermissionAction::Allow {
                return prompt_for_permission(
                    checker,
                    event_bus,
                    session_id,
                    permission,
                    resource,
                    tool_name,
                    checkpoint_timeout_secs,
                )
                .await;
            }
            // Explicit policy decision — no prompt needed, no I/O performed.
            Ok(action)
        }
        PermissionAction::Ask => {
            // No explicit rule matched.  Try the file:read auto-grant
            // before falling through to an interactive prompt.  This is
            // the only path that may perform a blocking `canonicalize()`.
            //
            // FR-004/FR-017: because the in-memory checks above already
            // short-circuited, the canonicalise syscall fires only when no
            // rule exists for the resource — i.e. on the first access to a
            // new file, not on every call.
            //
            // FR-015 (T-010): with a forced destructive-action checkpoint the
            // auto-grant is skipped — an allow rule never auto-approves a
            // destructive call; the request goes to a human checkpoint
            // prompt whose timeout counts as denial (FR-024).
            if !checkpoint_forced && (permission == "file:read" || permission == "read") {
                if let Ok(cwd) = std::env::current_dir() {
                    // FR-017: use the per-step canonical path cache when
                    // available to avoid a blocking canonicalize syscall.
                    let resource_canonical = match canonical_cache {
                        Some(cache) => cache.get_or_canonicalize(std::path::Path::new(resource)),
                        None => std::path::Path::new(resource).canonicalize().ok(),
                    };
                    if let Some(resource_path) = resource_canonical {
                        if resource_path.starts_with(&cwd) {
                            return Ok(PermissionAction::Allow);
                        }
                    } else if !resource.starts_with('/') {
                        // Relative path within project, not yet created.
                        // Reject embedded `..` components: after lexical
                        // normalisation the path could resolve outside the
                        // project (e.g. `foo/../../etc/target`), and
                        // canonicalise could not verify containment because
                        // the file does not exist yet.
                        let escapes = std::path::Path::new(resource)
                            .components()
                            .any(|c| c == std::path::Component::ParentDir);
                        if !escapes {
                            return Ok(PermissionAction::Allow);
                        }
                    }
                }
            }

            return prompt_for_permission(
                checker,
                event_bus,
                session_id,
                permission,
                resource,
                tool_name,
                if checkpoint_forced {
                    checkpoint_timeout_secs
                } else {
                    120
                },
            )
            .await;
        }
    }
}
// ---------------------------------------------------------------------------
// Tool-repeat guard (FR-044)
// ---------------------------------------------------------------------------

/// Consecutive identical tool calls allowed before the guard intervenes.
///
/// Five identical calls pass through untouched; the **sixth** call with the
/// same `(tool, args)` key prompts the user (primary runs) or is denied with
/// a corrective observation (subagent / auto-approve runs).
pub const TOOL_REPEAT_LIMIT: u32 = 5;

/// Per-session tracker for consecutive identical tool calls.
///
/// A call whose `(tool_name, hash(args))` key matches the previous recorded
/// call bumps `repeat_count`; any different call resets the tracker. State is
/// shared per session id behind a lock on [`SessionProcessor`].
#[derive(Debug, Default)]
pub struct RepeatTracker {
    /// Hash of the last call's `(tool_name, canonical args)` pair.
    last_key: Option<u64>,
    /// How many consecutive calls have carried that key.
    repeat_count: u32,
}

/// Return the guard key hash for a tool call: a stable hash of the tool name
/// plus the canonical serialised arguments. Serialisation (rather than the
/// raw `args_json` text) is hashed so provider-side key reordering or
/// whitespace differences do not split one logical repetition in two.
fn tool_call_key(tool_name: &str, tool_input: &Value) -> u64 {
    use std::hash::{Hash, Hasher};

    let canonical = serde_json::to_string(tool_input).unwrap_or_else(|_| tool_input.to_string());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tool_name.hash(&mut hasher);
    canonical.hash(&mut hasher);
    hasher.finish()
}

/// Record a tool call against the per-session tracker.
///
/// Returns the number of consecutive identical calls ending with this one
/// (1 for a first/different call).
fn record_tool_call(tracker: &mut RepeatTracker, key: u64) -> u32 {
    match tracker.last_key {
        Some(prev) if prev == key => {
            tracker.repeat_count = tracker.repeat_count.saturating_add(1);
        }
        _ => {
            tracker.last_key = Some(key);
            tracker.repeat_count = 1;
        }
    }
    tracker.repeat_count
}

/// Guard against an agent loop stuck replaying the same tool call with the
/// same arguments.
///
/// Every call is recorded on the session tracker. Once the same
/// `(tool, args)` key is seen more than [`TOOL_REPEAT_LIMIT`] times
/// **consecutively**:
///
/// - subagent / auto-approve (`--yes`) / YOLO runs **fail closed**: the call
///   is denied with a corrective observation instead of prompting (an
///   unattended prompt would stall the run — the v1.0.91 lesson);
/// - interactive primary runs raise a `PermissionRequested` prompt and wait
///   for the user's `PermissionReplied` up to `timeout_secs`; timeout counts
///   as denial (FR-015 semantics).
///
/// A denial lets the model see the reason and change approach; an allowance
/// resets the consecutive counter so the next burst can prompt again.
#[allow(clippy::implicit_hasher)]
pub async fn check_tool_repeat_guard(
    trackers: &Arc<parking_lot::Mutex<HashMap<String, RepeatTracker>>>,
    event_bus: &Arc<EventBus>,
    session_id: &str,
    tool_name: &str,
    tool_input: &Value,
    is_subagent: bool,
    auto_approve: bool,
    timeout_secs: u64,
) -> Option<String> {
    let key = tool_call_key(tool_name, tool_input);
    let count = {
        let mut trackers = trackers.lock();
        let tracker = trackers.entry(session_id.to_string()).or_default();
        record_tool_call(tracker, key)
    };
    if count <= TOOL_REPEAT_LIMIT {
        return None;
    }

    let reason = format!(
        "tool '{tool_name}' has been called with identical arguments \
         {count} times in a row"
    );

    // Unattended runs must not wait on a user: deny with a corrective
    // observation the model can act on (fail closed).
    if is_subagent || auto_approve || ragent_config::yolo::is_enabled() {
        tracing::info!(
            session_id = %session_id,
            tool = %tool_name,
            repeats = count,
            "tool-repeat guard denied a repeated identical call in an unattended run"
        );
        return Some(format!(
            "{reason} - repeated calls were auto-denied to break a \
             possible loop; change the approach or arguments"
        ));
    }

    // Interactive primary run: ask the user whether to continue.
    let request_id = Uuid::new_v4().to_string();
    let mut rx = event_bus.subscribe();

    event_bus.publish(Event::PermissionRequested {
        session_id: session_id.to_string(),
        request_id: request_id.clone(),
        permission: "tool:repeat".to_string(),
        description: format!("{tool_name}: {reason}. Continue?"),
        options: vec![],
    });

    let timeout = tokio::time::Duration::from_secs(timeout_secs);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let recv_timeout = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(recv_timeout, rx.recv()).await {
            Ok(Ok(Event::PermissionReplied {
                request_id: rid,
                allowed,
                ..
            })) if rid == request_id => {
                if allowed {
                    // Reset so a further run of identical calls can prompt
                    // again rather than deny forever.
                    let mut trackers = trackers.lock();
                    if let Some(tracker) = trackers.get_mut(session_id) {
                        tracker.repeat_count = 0;
                    }
                    tracing::info!(
                        session_id = %session_id,
                        tool = %tool_name,
                        "tool-repeat guard: user allowed the repeated call"
                    );
                    return None;
                }
                tracing::info!(
                    session_id = %session_id,
                    tool = %tool_name,
                    "tool-repeat guard: user declined the repeated call"
                );
                return Some(format!(
                    "{reason} - user declined to continue with this \
                     repeated call; change the approach or arguments"
                ));
            }
            Ok(Err(RecvError::Lagged(_))) => {
                // Idle-CPU fix: yield briefly so a lagged subscriber resyncs
                // (same pattern as `prompt_for_permission`).
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Ok(Err(_)) => {
                return Some(format!(
                    "{reason} - event bus closed while waiting for the \
                     user; the repeated call was not executed"
                ));
            }
            Err(_) => {
                // Timeout counts as denial (FR-015: the safe default).
                tracing::info!(
                    session_id = %session_id,
                    tool = %tool_name,
                    "tool-repeat guard prompt timed out; treating as denial"
                );
                return Some(format!(
                    "{reason} - no user reply arrived in time; the \
                     repeated call was not executed"
                ));
            }
            _ => continue,
        }
    }
}
