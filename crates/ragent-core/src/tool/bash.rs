//! Shell command execution tool.
//!
//! Provides [`BashTool`], which runs shell commands via `bash -c` in the
//! agent's working directory with configurable timeouts.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::time::Instant;
use tokio::process::Command;

use super::{Tool, ToolContext, ToolOutput};

/// Executes shell commands via `bash -c` and returns combined stdout/stderr output.
///
/// Output is truncated to 100 KB to avoid overwhelming the agent context.
/// Commands that exceed the configured timeout (default 120 s) are terminated.
pub struct BashTool;

const DEFAULT_TIMEOUT_SECS: u64 = 120;

// Safe commands: only these exact commands (or with specific args) are allowed without restrictions.
// Used when "safe mode" is enabled in session config.
#[allow(dead_code)]
const SAFE_COMMANDS: &[&str] = &[
    "git status",
    "git diff",
    "git log",
    "git branch",
    "pwd",
    "tree",
    "date",
    "which",
];

// Banned commands: these are never allowed (unless YOLO mode enabled).
// High-risk tools that could exfiltrate data or connect to external systems.
const BANNED_COMMANDS: &[&str] = &[
    "curl",
    "wget",
    "nc",
    "netcat",
    "telnet",
    "axel",
    "aria2c",
    "lynx",
    "w3m",
];

const DENIED_PATTERNS: &[&str] = &[
    // Destructive filesystem operations
    "rm -rf /",
    "rm -r -f /",
    "rm -fr /",
    "rm -Rf /",
    "rmdir /",
    // Disk / partition destruction
    "mkfs",
    "dd if=",
    "wipefs",
    "shred /dev",
    // Device writes
    "> /dev/sd",
    "> /dev/nvme",
    "> /dev/vd",
    // Fork bomb
    ":(){ :|:&};:",
    // Privilege escalation
    "chmod -R 777 /",
    "chown -R",
    // Network exfiltration of sensitive files
    "curl.*etc/shadow",
    "wget.*etc/shadow",
    // History / credential file theft
    ".bash_history",
    ".ssh/id_",
    // Kernel modifications
    "insmod",
    "modprobe -r",
    "sysctl -w",
];

/// Check if command is in the safe whitelist (exact match or with allowed args).
#[allow(dead_code)]
fn is_safe_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    SAFE_COMMANDS.iter().any(|safe| {
        trimmed == *safe || trimmed.starts_with(&format!("{} ", safe))
    })
}

/// Check if command uses a banned tool (e.g., curl, wget).
fn contains_banned_command(cmd: &str) -> bool {
    let trimmed = cmd.trim().to_lowercase();
    BANNED_COMMANDS.iter().any(|banned| {
        trimmed.contains(banned)
    })
}

/// Check if command tries to escape the working directory (e.g., cd ../).
/// This is a basic guard; determined by checking if cd command contains `..` or `/`.
fn is_directory_escape_attempt(cmd: &str, _working_dir: &std::path::Path) -> bool {
    let cmd_lower = cmd.to_lowercase();
    if !cmd_lower.contains("cd ") {
        return false;
    }

    // Simple heuristic: reject "cd" with "..", "/", or paths starting with /
    // (absolute paths would try to escape the working directory)
    if cmd.contains("cd ..") {
        return true;
    }
    if cmd.contains("cd /") {
        return true;
    }
    // If cd argument doesn't look relative or is a special path, assume escape attempt
    // For now, allow cd with relative paths (cd ./foo, cd foo)
    false
}

/// Pre-check command syntax using `sh -n -c` without executing.
/// Returns error if syntax is invalid.
async fn validate_bash_syntax(cmd: &str) -> Result<()> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(cmd)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                bail!("Bash syntax error: {}", stderr);
            }
            Ok(())
        }
        Ok(Err(e)) => bail!("Failed to check bash syntax: {}", e),
        Err(_) => bail!("Bash syntax check timed out"),
    }
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

        /// Returns a human-readable description of what the tool does.
        fn description(&self) -> &str {
            "Execute a shell command and return stdout and stderr. \
               Commands are run with bash -c in the working directory."
        }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 120)"
                }
            },
            "required": ["command"]
        })
    }

    fn permission_category(&self) -> &str {
        "bash:execute"
    }

    /// Executes a shell command via `bash -c`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `command` parameter is missing or invalid
    /// - The command contains a dangerous pattern (e.g., `rm -rf /`, `mkfs`, `dd if=`)
    /// - The command fails to execute (command not found, permission denied, etc.)
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let command = input["command"]
            .as_str()
            .context("Missing required 'command' parameter")?;
        let timeout_secs = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        tracing::info!(
            command = %crate::sanitize::redact_secrets(command),
            working_dir = %ctx.working_dir.display(),
            "Executing bash command"
        );

        // CC1-T4: Check for banned commands (curl, wget, nc, etc.)
        if contains_banned_command(command) {
            if crate::yolo::is_enabled() {
                tracing::warn!("YOLO mode: allowing banned command tool");
            } else {
                bail!(
                    "Command rejected: uses banned external tool (curl, wget, nc, telnet, axel, aria2c, lynx, w3m). \
                    These tools could exfiltrate data or connect to external systems."
                );
            }
        }

        // CC1-T5: Check for directory escape attempts (cd to parent or absolute paths)
        if is_directory_escape_attempt(command, &ctx.working_dir) {
            bail!(
                "Command rejected: attempts to escape working directory {}. \
                Use only relative paths (cd ./subdir, cd subdir).",
                ctx.working_dir.display()
            );
        }

        // CC1-T6: Pre-check bash syntax
        validate_bash_syntax(command).await?;

        for pattern in DENIED_PATTERNS {
            if command.contains(pattern) {
                if crate::yolo::is_enabled() {
                    tracing::warn!(pattern, "YOLO mode: allowing denied pattern");
                } else {
                    bail!(
                        "Command rejected: contains dangerous pattern '{pattern}'. This pattern could cause irreversible damage to the system."
                    );
                }
            }
        }

        // Reject commands that use encoding/eval tricks to bypass the denylist.
        if !crate::yolo::is_enabled() {
            validate_no_obfuscation(command)?;
        }

        // Acquire a process-spawn permit to bound concurrency.
        let _permit = crate::resource::acquire_process_permit().await?;

        let start = Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(&ctx.working_dir)
                .output(),
        )
        .await;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

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

                // CC1-T7: Truncate very long output, keeping first 15k + last 15k chars
                const FIRST_CHARS: usize = 15_000;
                const LAST_CHARS: usize = 15_000;
                const MAX_OUTPUT: usize = FIRST_CHARS + LAST_CHARS + 1000; // allow for separator
                
                if content.len() > MAX_OUTPUT {
                    let first_part = &content[..FIRST_CHARS.min(content.len())];
                    let remainder_len = content.len() - FIRST_CHARS;
                    let last_part = if remainder_len > LAST_CHARS {
                        &content[content.len() - LAST_CHARS..]
                    } else {
                        &content[FIRST_CHARS..]
                    };
                    
                    let omitted = remainder_len.saturating_sub(LAST_CHARS);
                    content = format!(
                        "{}\n\n... ({} lines omitted) ...\n\n{}",
                        first_part,
                        omitted / content.lines().count().max(1), // rough line count
                        last_part
                    );
                }

                let line_count = content.lines().count();
                Ok(ToolOutput {
                    content: format!(
                        "Exit code: {}\nDuration: {}ms\n\n{}",
                        exit_code, elapsed_ms, content
                    ),
                    metadata: Some(json!({
                        "exit_code": exit_code,
                        "duration_ms": elapsed_ms,
                        "lines": line_count,
                    })),
                })
            }
            Ok(Err(e)) => Err(anyhow::anyhow!(
                "Failed to execute command: {}. Check that the command exists and is accessible.",
                e
            )),
            Err(_) => Ok(ToolOutput {
                content: format!("Command timed out after {} seconds", timeout_secs),
                metadata: Some(json!({
                    "timeout": true,
                    "timeout_secs": timeout_secs,
                })),
            }),
        }
    }
}

/// Rejects commands that attempt to bypass the denylist via encoding,
/// eval, or dynamic variable expansion tricks.
fn validate_no_obfuscation(command: &str) -> Result<()> {
    // base64 decode piped into shell
    if command.contains("base64") && (command.contains("| bash") || command.contains("| sh")) {
        bail!("Command rejected: base64-decode-to-shell pattern detected.");
    }

    // Python/perl one-liners executing encoded payloads
    if (command.contains("python") || command.contains("perl"))
        && (command.contains("exec(") || command.contains("eval("))
    {
        bail!("Command rejected: dynamic eval/exec in scripting language.");
    }

    // $'\xNN' hex escape sequences used to build commands
    if command.contains("$'\\x") {
        bail!("Command rejected: hex escape sequence obfuscation detected.");
    }

    // Prevent `eval` with variable expansion that could hide intent
    if command.contains("eval ") && command.contains("$(") {
        bail!("Command rejected: eval with command substitution detected.");
    }

    Ok(())
}
