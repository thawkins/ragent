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
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        // Strip timeout prefix before adding to parts
                        parts.push(strip_timeout_prefix(trimmed).to_string());
                    }
                    current.clear();
                } else if c == ';' {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        // Strip timeout prefix before adding to parts
                        parts.push(strip_timeout_prefix(trimmed).to_string());
                    }
                    current.clear();
                } else {
                    // Single & or | - add to current command
                    current.push(c);
                }
            }
            _ => current.push(c),
        }
    }

    // Add the final part
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        // Strip timeout prefix before adding to parts
        parts.push(strip_timeout_prefix(trimmed).to_string());
    }

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
    const AUTO_APPROVED_CODEINDEX_TOOLS: &[&str] = &[
        "codeindex_search",
        "codeindex_symbols",
        "codeindex_references",
        "codeindex_dependencies",
        "codeindex_status",
        "codeindex_reindex",
    ];
    const AUTO_APPROVED_AGENT_TOOLS: &[&str] = &[
        "new_agent",
        "cancel_agent",
        "list_agents",
        "wait_agents",
        "agent_complete",
    ];
    AUTO_APPROVED_CODEINDEX_TOOLS.contains(&tool_name)
        || tool_name.starts_with("team_")
        || AUTO_APPROVED_AGENT_TOOLS.contains(&tool_name)
        || tool_name.starts_with("task_")
        || tool_name == "ask_user"
}

/// Check permission for a tool execution, prompting the user if necessary.
///
/// Returns `Allow` or `Deny`. If the policy says `Ask`, this publishes a
/// `PermissionRequested` event and waits up to 2 minutes for a user reply.
///
/// If `auto_approve` is true, always returns `Allow` without checking rules
/// or prompting.
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
    auto_approve: bool,
    canonical_cache: Option<&ragent_tools_core::CanonicalPathCache>,
) -> Result<PermissionAction> {
    // Short-circuit if --yes / --no-prompt flag is set
    if auto_approve {
        return Ok(PermissionAction::Allow);
    }

    // YOLO mode bypasses interactive permission prompts for all tools
    if ragent_config::yolo::is_enabled() {
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
    if permission.starts_with("file:")
        || permission == "read"
        || permission == "edit"
        || permission == "write"
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
            if permission == "file:read" || permission == "read" {
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
                    } else if !resource.starts_with('/') && !resource.starts_with("..") {
                        // Relative path within project, not yet created
                        return Ok(PermissionAction::Allow);
                    }
                }
            }

            // Need user interaction
            let request_id = Uuid::new_v4().to_string();
            let mut rx = event_bus.subscribe();

            // Publish request
            event_bus.publish(Event::PermissionRequested {
                session_id: session_id.to_string(),
                request_id: request_id.clone(),
                permission: permission.to_string(),
                description: format!("{tool_name}: {resource}"),
                options: vec![],
            });

            // Wait for reply with 120s timeout
            let timeout = tokio::time::Duration::from_mins(2);
            let deadline = tokio::time::Instant::now() + timeout;

            loop {
                let recv_timeout = deadline.saturating_duration_since(tokio::time::Instant::now());
                if recv_timeout.is_zero() {
                    debug!("Permission request timeout for {tool_name}");
                    return Ok(PermissionAction::Deny);
                }

                match tokio::time::timeout(recv_timeout, rx.recv()).await {
                    Ok(Ok(Event::PermissionReplied {
                        request_id: rid,
                        allowed,
                        decision,
                        ..
                    })) if rid == request_id => {
                        // If user chose 'Always', record the grant
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
                    Ok(Err(RecvError::Lagged(_))) => continue,
                    Ok(Err(_)) => {
                        bail!("Event bus closed during permission check");
                    }
                    Err(_) => {
                        // Timeout
                        debug!("Permission request timeout for {tool_name}");
                        return Ok(PermissionAction::Deny);
                    }
                    _ => continue,
                }
            }
        }
    }
}
