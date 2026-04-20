//! Shell command execution tool.
//!
//! Provides [`BashTool`], which runs shell commands via `bash -c` in the
//! agent's working directory with configurable timeouts.
//!
//! Shell state (current directory and exported environment variables) is
//! persisted across invocations using a per-session state file so that
//! `cd subdir` and `export FOO=bar` survive between tool calls.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::time::Instant;
use tokio::process::Command;

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;

/// Derive a filesystem-safe identifier from a session ID.
///
/// Replaces any character that is not alphanumeric or `-` with `_` so that
/// the result is safe to embed directly in a file path.
fn safe_session_id(session_id: &str) -> String {
    session_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Return the path of the persistent state file for the given session.
#[must_use]
pub fn state_file_path(session_id: &str) -> String {
    format!("/tmp/ragent_shell_{}.state", safe_session_id(session_id))
}

/// Parse the current working directory from a state file's contents.
///
/// The state file may contain lines written by `export -p` (e.g.
/// `declare -x PWD="/some/dir"`) **and** an explicit trailing line in the
/// form `RAGENT_PWD=/some/dir` which we prefer because it is unambiguous.
fn parse_cwd_from_state(state: &str) -> Option<String> {
    // Prefer the explicit marker we append after every command.
    for line in state.lines().rev() {
        if let Some(val) = line.strip_prefix("RAGENT_PWD=") {
            let v = val.trim_matches('"').trim_matches('\'');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Executes shell commands via `bash -c` and returns combined stdout/stderr output.
///
/// Output is truncated to 100 KB to avoid overwhelming the agent context.
/// Commands that exceed the configured timeout (default 120 s) are terminated.
pub struct BashTool;

const DEFAULT_TIMEOUT_SECS: u64 = 120;

// Safe commands: only these exact prefixes are auto-approved without user prompting.
// The check is prefix-based: a command is safe if it equals the entry exactly OR starts
// with the entry followed by a space (so "ls" matches "ls -la", "git" matches "git status", etc.).
#[allow(dead_code)]
const SAFE_COMMANDS: &[&str] = &[
    // --- File management ---
    "ls",
    "cd",
    "pwd",
    "mkdir",
    "touch",
    "cp",
    "mv",
    // NOTE: "rm" is intentionally excluded — prefix matching cannot distinguish
    // safe "rm file.txt" from destructive "rm -rf /". DENIED_PATTERNS blocks the
    // destructive variants; individual rm calls go through normal permission flow.
    // --- File reading & search ---
    "cat",
    "head",
    "tail",
    "grep",
    "egrep",
    "fgrep",
    "find",
    "rg", // ripgrep
    "wc",
    // --- Version control ---
    "git", // covers all git subcommands (clone, add, commit, push, pull, status, diff, log …)
    "gh",  // GitHub CLI
    // --- Build / package management ---
    "cargo",
    "rustc",
    "rustfmt",
    "clippy-driver",
    "npm",
    "yarn",
    "pnpm",
    "node",
    "npx",
    "python3",
    "python",
    "pip",
    "pip3",
    "make",
    "docker-compose",
    // --- Text / data utilities ---
    "echo",
    "printf",
    "chmod",
    "jq", // JSON query/processing
    "yq", // YAML query/processing
    "sed",
    "awk",
    "sort",
    "uniq",
    "cut",
    "tr",
    "xargs",
    "date",
    "which",
    "tree",
    "diff",
    "patch",
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
    // Attack and exploitation tools
    "nmap",
    "masscan",
    "nikto",
    "sqlmap",
    "hydra",
    "john",
    "hashcat",
    "aircrack",
    "metasploit",
    "msfconsole",
    "msfvenom",
    "burpsuite",
    "ettercap",
    "arpspoof",
    // tcpdump and wireshark are blocked by default but can be enabled via YOLO mode
    "tcpdump",
    "wireshark",
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
    // Privilege escalation commands
    "sudo ",
    "sudo\t",
    "su -",
    "su root",
    "doas ",
    // User/group manipulation
    "useradd",
    "usermod",
    "groupadd",
    "passwd ",
    // System configuration
    "visudo",
    "crontab -",
    "systemctl disable",
    "systemctl mask",
    "chattr +i",
    // Destructive git operations
    "git push --force",
    "git push -f ",
    "git push origin --delete",
    // Boot/firmware
    "grub-install",
    "efibootmgr",
    // More destructive patterns
    "rm -rf ~",
    "rm -rf $HOME",
    "rm -rf .",
    "chmod 000 /",
    "chmod -R 000",
    // Data exfiltration via pipes
    "> /dev/tcp",
    "bash -i >&",
    "/dev/tcp/",
    "/dev/udp/",
];

/// Check if command is in the safe whitelist (exact match or with allowed args).
#[must_use]
pub fn is_safe_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    SAFE_COMMANDS
        .iter()
        .any(|safe| trimmed == *safe || trimmed.starts_with(&format!("{safe} ")))
}

/// Extract the bare heredoc delimiter from a line that contains `<<`.
///
/// Handles `<<EOF`, `<< EOF`, `<<'EOF'`, `<<"EOF"`, and `<<-EOF` variants.
/// Returns `None` if no heredoc marker is found.
fn extract_heredoc_delimiter(line: &str) -> Option<String> {
    let pos = line.find("<<")?;
    // <<- is allowed (strip leading tabs from body); skip the optional '-'
    let rest = line[pos + 2..].trim_start_matches('-').trim_start();
    let delimiter = if let Some(inner) = rest.strip_prefix('\'') {
        inner.split('\'').next()?.to_string()
    } else if let Some(inner) = rest.strip_prefix('"') {
        inner.split('"').next()?.to_string()
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | ')'))
            .unwrap_or(rest.len());
        rest[..end].to_string()
    };
    if delimiter.is_empty() {
        None
    } else {
        Some(delimiter)
    }
}

/// Return a copy of `cmd` with heredoc bodies removed.
///
/// The line containing the `<<` marker and the closing delimiter line are
/// kept so that the structural shell command is still present for subsequent
/// checks; only the body lines (the literal data) are dropped.  This
/// prevents heredoc content (e.g. Rust string literals containing `\nc\n`)
/// from producing false positives in the banned-command scan.
fn strip_heredoc_bodies(cmd: &str) -> String {
    let mut result = String::with_capacity(cmd.len());
    let mut iter = cmd.split('\n');
    'outer: while let Some(line) = iter.next() {
        if let Some(delimiter) = extract_heredoc_delimiter(line) {
            result.push_str(line);
            result.push('\n');
            // Skip body lines until the closing delimiter.
            for body_line in iter.by_ref() {
                if body_line.trim_end() == delimiter {
                    result.push_str(body_line);
                    result.push('\n');
                    continue 'outer;
                }
                // body content intentionally omitted
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

/// Check if command uses a banned tool (e.g., curl, wget).
fn contains_banned_command(cmd: &str) -> bool {
    // Strip heredoc bodies first so that literal data inside a heredoc
    // (e.g. Rust string escapes like `\nc\n`) cannot trigger false positives.
    let cmd_stripped = strip_heredoc_bodies(cmd);
    let cmd_lower = cmd_stripped.trim().to_lowercase();
    let bytes = cmd_lower.as_bytes();
    let clen = bytes.len();

    BANNED_COMMANDS.iter().any(|banned| {
        let banned_bytes = banned.as_bytes();
        let blen = banned_bytes.len();
        if clen < blen {
            return false;
        }
        // Require word boundaries: banned name must not be part of a longer identifier.
        // Characters that delimit command tokens: whitespace, |, ;, &, (, ), `, ', "
        let is_boundary = |b: u8| !b.is_ascii_alphanumeric() && b != b'_' && b != b'-';
        for i in 0..=(clen - blen) {
            if &bytes[i..i + blen] == banned_bytes {
                let before_ok = i == 0 || is_boundary(bytes[i - 1]);
                let after_ok = i + blen == clen || is_boundary(bytes[i + blen]);
                if before_ok && after_ok {
                    return true;
                }
            }
        }
        false
    })
}

/// Check if command tries to escape the working directory.
/// Rejects cd/pushd with .., /, ~, $HOME, or ${HOME}.
fn is_directory_escape_attempt(cmd: &str, working_dir: &std::path::Path) -> bool {
    let canonical_wd = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    for token in &["cd ", "pushd "] {
        // Find each occurrence of the token in the command
        let mut search_start = 0;
        while let Some(pos) = cmd[search_start..].find(token) {
            let abs_pos = search_start + pos;
            // Only treat it as a cd if it's at the start or after a shell separator
            let before = if abs_pos == 0 {
                b';'
            } else {
                cmd.as_bytes()[abs_pos - 1]
            };
            let is_after_separator = matches!(before, b';' | b'&' | b'|' | b'(' | b'\n' | b' ');
            if abs_pos == 0 || is_after_separator {
                let arg_start = abs_pos + token.len();
                // Extract the argument (up to next whitespace or ; & | )
                let arg = cmd[arg_start..]
                    .split([';', '&', '|', ')', '\n'])
                    .next()
                    .unwrap_or("")
                    .trim();

                if arg.starts_with("..") {
                    return true;
                }
                if arg.starts_with('~') || arg.starts_with("$HOME") || arg.starts_with("${HOME}") {
                    return true;
                }
                if arg.starts_with('/') {
                    // D1 fix: Single-segment slash-prefixed tokens (e.g., /help, /start)
                    // are likely commands, not file paths - exclude from directory escape check.
                    // Only check as path if it contains a directory separator (e.g., /etc/passwd).
                    if arg.len() > 1 && !arg[1..].contains('/') {
                        // Single segment after / - treat as command, not a file path
                        continue;
                    }
                    // Allow if the absolute path resolves to the working directory or a subdirectory of it
                    let target = std::path::Path::new(arg);
                    let canonical_target = target
                        .canonicalize()
                        .unwrap_or_else(|_| target.to_path_buf());
                    if !canonical_target.starts_with(&canonical_wd) {
                        return true;
                    }
                }
            }
            search_start = abs_pos + 1;
        }
    }
    false
}

/// Pre-check command syntax using `sh -n -c` without executing.
/// Returns error if syntax is invalid.
async fn validate_bash_syntax(cmd: &str) -> Result<()> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        Command::new("sh").arg("-n").arg("-c").arg(cmd).output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                bail!("Bash syntax error: {stderr}");
            }
            Ok(())
        }
        Ok(Err(e)) => bail!("Failed to check bash syntax: {e}"),
        Err(_) => bail!("Bash syntax check timed out"),
    }
}

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
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

    fn permission_category(&self) -> &'static str {
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

        if is_safe_command(command) {
            tracing::info!("Safe bash command auto-approved");
        }

        // CC1-T4: Check for banned commands (curl, wget, nc, etc.)
        // A user-defined allowlist entry (via /bash add allow <cmd>) exempts the command.
        if contains_banned_command(command) {
            if crate::yolo::is_enabled() {
                tracing::warn!("YOLO mode: allowing banned command tool");
            } else if crate::bash_lists::is_allowlisted(command) {
                tracing::info!("Banned command allowed by user allowlist");
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

        // Check user-defined denylist (from ragent.json `bash.denylist`)
        if !crate::yolo::is_enabled()
            && let Some(pattern) = crate::bash_lists::matches_denylist(command)
        {
            bail!(
                "Command rejected: matches user-defined deny pattern '{pattern}'. \
                    Use `/bash remove deny \"{pattern}\"` to remove this restriction."
            );
        }

        // Reject commands that use encoding/eval tricks to bypass the denylist.
        if !crate::yolo::is_enabled() {
            validate_no_obfuscation(command)?;
        }

        // Acquire a process-spawn permit to bound concurrency.
        let _permit = crate::resource::acquire_process_permit().await?;

        // ── Persistent shell state ────────────────────────────────────────────
        // Write the user command to a temporary script file so that we can
        // source the persisted environment before running it without any
        // quoting issues.
        let state_file = state_file_path(&ctx.session_id);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        let script_file = format!(
            "/tmp/ragent_cmd_{}_{}.sh",
            safe_session_id(&ctx.session_id),
            timestamp
        );
        std::fs::write(&script_file, command)
            .context("Failed to write command to temporary script file")?;

        // Wrapper: restore state → run user script → save new state.
        // RAGENT_PWD is appended as an unambiguous marker for the cwd.
        let wrapper = format!(
            "STATE_FILE=\"{state_file}\"\n\
             if [ -f \"$STATE_FILE\" ]; then\n\
               . \"$STATE_FILE\" 2>/dev/null\n\
               cd \"${{RAGENT_PWD:-}}\" 2>/dev/null || true\n\
             fi\n\
             bash \"{script_file}\"\n\
             EXIT_CODE=$?\n\
             export -p 2>/dev/null > \"$STATE_FILE\" || true\n\
             printf 'RAGENT_PWD=%s\\n' \"$(pwd)\" >> \"$STATE_FILE\"\n\
             rm -f \"{script_file}\"\n\
             exit $EXIT_CODE\n"
        );

        let start = Instant::now();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            Command::new("bash")
                .arg("-c")
                .arg(&wrapper)
                .current_dir(&ctx.working_dir)
                .output(),
        )
        .await;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // After execution, read the saved cwd and publish ShellCwdChanged.
        if let Ok(state_content) = std::fs::read_to_string(&state_file)
            && let Some(cwd) = parse_cwd_from_state(&state_content)
        {
            ctx.event_bus.publish(Event::ShellCwdChanged {
                session_id: ctx.session_id.clone(),
                cwd,
            });
        }

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
                    // Find valid UTF-8 char boundaries near the target split points
                    let first_end = {
                        let mut i = FIRST_CHARS.min(content.len());
                        while i > 0 && !content.is_char_boundary(i) {
                            i -= 1;
                        }
                        i
                    };
                    let first_part = &content[..first_end];
                    let remainder_len = content.len() - first_end;
                    let last_part = if remainder_len > LAST_CHARS {
                        let mut j = content.len() - LAST_CHARS;
                        while j < content.len() && !content.is_char_boundary(j) {
                            j += 1;
                        }
                        &content[j..]
                    } else {
                        &content[first_end..]
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
                        "Exit code: {exit_code}\nDuration: {elapsed_ms}ms\n\n{content}"
                    ),
                    metadata: Some(json!({
                        "exit_code": exit_code,
                        "duration_ms": elapsed_ms,
                        "line_count": line_count,
                    })),
                })
            }
            Ok(Err(e)) => Err(anyhow::anyhow!(
                "Failed to execute command: {e}. Check that the command exists and is accessible."
            )),
            Err(_) => Ok(ToolOutput {
                content: format!("Command timed out after {timeout_secs} seconds"),
                metadata: Some(json!({
                    "timed_out": true,
                    "duration_ms": timeout_secs * 1000,
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
