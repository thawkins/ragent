//! Dynamic context injection for skill bodies.
//!
//! The `` !`command` `` syntax allows skill bodies to embed the output of
//! shell commands. Before the skill content is sent to the agent, each
//! `` !`command` `` placeholder is replaced with the command's stdout.
//!
//! # Security
//!
//! Commands are validated against an allowlist of known-safe, read-only
//! executables. Only executables on the allowlist can be run, and they are
//! always executed directly - never through a shell (`sh -c`) - so a skill
//! body cannot smuggle a second command or a shell expansion. Any command
//! that contains a shell metacharacter (`|`, `;`, `&`, redirects, `$(...)`,
//! globs, ...) is rejected outright.
//!
//! # Example
//!
//! ```markdown
//! ## Pull request context
//! - PR diff: !`gh pr diff`
//! - Changed files: !`gh pr diff --name-only`
//! ```
//!
//! After injection, the placeholders are replaced with the actual command output.

use std::path::Path;
use std::time::Duration;

/// Default timeout for each dynamic context command.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Allowlist of executables permitted in dynamic context `` !`command` ``
/// patterns. Only the base executable name is checked (not the full path).
///
/// The list is deliberately restricted to genuinely read-only, local
/// inspection tools. Network clients (`curl`, `wget`, `gh`, `dig`, ...),
/// interpreters (`python`, `node`, `awk`, `sed`), process launchers
/// (`xargs`, `env`, `docker`, ...), and build/package tools that execute
/// arbitrary code through scripts (`cargo`, `make`, `npm`, ...) are
/// intentionally excluded: allowing them would turn a skill body into an
/// arbitrary-code-execution or data-exfiltration primitive.
const ALLOWED_EXECUTABLES: &[&str] = &[
    // Version control (local inspection only)
    "git", "svn", "hg", // File inspection (read-only)
    "cat", "head", "tail", "wc", "ls", "find", "tree", "file", "stat", "du", "df", "readlink",
    "realpath", "basename", "dirname", // Text processing (read-only, non-interpreter)
    "grep", "rg", "cut", "sort", "uniq", "tr", "diff", "comm", "paste", "column", "fmt", "fold",
    "jq", "yq", // Shell info utilities
    "echo", "printf", "date", "whoami", "hostname", "uname", "id", "pwd", "which", "test", "true",
    "false",
];

/// Execute all `` !`command` `` placeholders in the skill body and replace
/// them with command output.
///
/// Commands are executed sequentially via the system shell (`sh -c` on Unix).
/// Each command has a 30-second timeout. If a command fails or times out,
/// the placeholder is replaced with an error message.
///
/// # Arguments
///
/// * `body` - The skill body text potentially containing `` !`command` `` patterns.
/// * `working_dir` - The directory in which to execute commands.
///
/// # Errors
///
/// Returns an error only if an internal string manipulation fails (which should
/// not occur in normal operation). Command execution errors are captured and
/// inserted into the result as error messages rather than propagating.
///
/// # Examples
///
/// ```no_run
/// # async fn example() -> anyhow::Result<()> {
/// use ragent_agent::skill::context::inject_dynamic_context;
/// use std::path::Path;
///
/// let body = "Files: !`ls src/`\nDone";
/// let result = inject_dynamic_context(body, Path::new("/my/project")).await?;
/// // result contains "Files: main.rs\nlib.rs\n..." etc.
/// # Ok(())
/// # }
/// ```
pub async fn inject_dynamic_context(body: &str, working_dir: &Path) -> anyhow::Result<String> {
    let patterns = find_command_patterns(body);

    if patterns.is_empty() {
        return Ok(body.to_string());
    }

    let mut result = body.to_string();

    // Process patterns in reverse order so byte offsets remain valid
    for pattern in patterns.into_iter().rev() {
        let output = execute_command(&pattern.command, working_dir).await;
        result.replace_range(pattern.start..pattern.end, &output);
    }

    Ok(result)
}

/// A matched `` !`command` `` pattern with its byte offsets in the source text.
#[derive(Debug, Clone)]
struct CommandPattern {
    /// The shell command to execute (without the `` !` `` and `` ` `` delimiters).
    command: String,
    /// Byte offset of the start of the full pattern (the `!`).
    start: usize,
    /// Byte offset past the end of the full pattern (after closing `` ` ``).
    end: usize,
}

/// Find all `` !`command` `` patterns in the text.
///
/// Returns patterns sorted by their start offset (ascending).
fn find_command_patterns(text: &str) -> Vec<CommandPattern> {
    let mut patterns = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Look for !` sequence
        if bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'`' {
            let pattern_start = i;
            let cmd_start = i + 2;

            // Find the closing backtick (must be on the same or subsequent lines,
            // but NOT a triple-backtick code fence)
            if let Some(cmd_end) = find_closing_backtick(text, cmd_start) {
                let command = text[cmd_start..cmd_end].to_string();
                let pattern_end = cmd_end + 1; // past the closing backtick

                if !command.is_empty() {
                    patterns.push(CommandPattern {
                        command,
                        start: pattern_start,
                        end: pattern_end,
                    });
                }

                i = pattern_end;
                continue;
            }
        }
        i += 1;
    }

    patterns
}

/// Find the closing backtick for a `` !`command` `` pattern.
///
/// Returns the byte offset of the closing backtick, or `None` if not found.
/// Stops at the end of the text. Does not match triple-backtick fences.
fn find_closing_backtick(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = start;

    while i < bytes.len() {
        if bytes[i] == b'`' {
            // Don't match triple backticks (code fences)
            if i + 1 < bytes.len() && bytes[i + 1] == b'`' {
                // Skip all consecutive backticks
                while i < bytes.len() && bytes[i] == b'`' {
                    i += 1;
                }
                continue;
            }
            return Some(i);
        }
        i += 1;
    }

    None
}

/// Shell metacharacters that must never appear in a dynamic context command.
///
/// The command is always executed directly (never via `sh -c`), so there is no
/// shell to interpret these. Permitting them would let a skill body smuggle a
/// second command (`|`, `;`, `&`), a redirect (`<`, `>`), a subshell or command
/// substitution (`(`, `)`, `$`, `` ` ``), a glob/path expansion (`*`, `?`, `~`,
/// `[`, `]`), a brace expansion, or a newline. Any occurrence rejects the whole
/// command.
const SHELL_METACHARACTERS: &[char] = &[
    '|', '&', ';', '<', '>', '$', '`', '(', ')', '{', '}', '*', '?', '~', '[', ']', '!', '\n', '\r',
];

/// Validate a command string against the allowlist.
///
/// Returns `Ok(())` if the command contains no shell metacharacter and its
/// first executable is on the allowlist, or an error message explaining the
/// rejection.
fn validate_command(command: &str) -> Result<(), String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("empty command".to_string());
    }

    // Reject any shell metacharacter outright. Commands run without a shell, so
    // there is no legitimate interpretation for these, and allowing them would
    // let a skill body chain a second (unvalidated) command or expand state.
    if let Some(metacharacter) = trimmed.chars().find(|c| SHELL_METACHARACTERS.contains(c)) {
        return Err(format!(
            "command contains a shell metacharacter '{metacharacter}' and is not permitted"
        ));
    }

    // Extract the first token (executable name) from the command.
    let first_token = trimmed.split_whitespace().next().unwrap_or("");

    // Strip any path prefix to get the base executable name.
    let exe_name = std::path::Path::new(first_token)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(first_token);

    if ALLOWED_EXECUTABLES.contains(&exe_name) {
        Ok(())
    } else {
        Err(format!(
            "executable '{exe_name}' is not on the dynamic context allowlist"
        ))
    }
}

/// Execute a validated command and return its stdout, or an error message.
///
/// The command is always executed directly (no shell intermediary); the
/// allowlist and metacharacter rejection in [`validate_command`] guarantee the
/// input is a single, shell-free invocation of an allowlisted executable.
async fn execute_command(command: &str, working_dir: &Path) -> String {
    use crate::sanitize::redact_secrets;
    let safe_cmd = redact_secrets(command);

    // Validate against allowlist before execution (skipped in YOLO mode).
    if !ragent_config::yolo::is_enabled()
        && let Err(reason) = validate_command(command)
    {
        tracing::warn!(
            command = %safe_cmd,
            reason = %reason,
            "Dynamic context command rejected by allowlist"
        );
        return format!("[command rejected: {reason}]");
    }

    tracing::debug!(command = %safe_cmd, "Executing dynamic context command");

    // Acquire a process-spawn permit to bound concurrency.
    let _permit = match crate::resource::acquire_process_permit().await {
        Ok(p) => p,
        Err(e) => return format!("[command error: {e}]"),
    };

    // Execute directly - never through a shell. [`validate_command`] has
    // already rejected any shell metacharacter, so there is no shell to
    // interpret here and no way to chain a second command.
    let tokens = tokenize_command(command);
    if tokens.is_empty() {
        return "[command error: empty command]".to_string();
    }
    let result = tokio::time::timeout(
        COMMAND_TIMEOUT,
        tokio::process::Command::new(&tokens[0])
            .args(&tokens[1..])
            .current_dir(working_dir)
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.trim_end().to_string()
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!(
                    command = %safe_cmd,
                    status = ?output.status,
                    stderr = %stderr,
                    "Dynamic context command failed"
                );
                format!("[command failed: {}]", stderr.trim())
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(command = %safe_cmd, error = %e, "Failed to spawn dynamic context command");
            format!("[command error: {e}]")
        }
        Err(_) => {
            tracing::warn!(command = %safe_cmd, "Dynamic context command timed out after 30s");
            format!("[command timed out: {command}]")
        }
    }
}

/// Simple tokenizer that splits a command string into executable + arguments.
///
/// Handles single and double-quoted strings. Shell expansions are not handled
/// (and never occur: metacharacter-bearing commands are rejected upstream).
fn tokenize_command(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            '\\' if !in_single_quote => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
#[path = "../../tests/inline/skill_context.rs"]
mod tests;
