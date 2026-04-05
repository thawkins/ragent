//! Dynamic context injection for skill bodies.
//!
//! The `` !`command` `` syntax allows skill bodies to embed the output of
//! shell commands. Before the skill content is sent to the agent, each
//! `` !`command` `` placeholder is replaced with the command's stdout.
//!
//! # Security
//!
//! Commands are validated against an allowlist of known-safe executables.
//! Only executables on the allowlist can be run. Pipes and shell operators
//! are supported for allowlisted commands by falling through to `sh -c`,
//! but the first command in any pipeline must still be on the allowlist.
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
/// This list covers common development tools used in skill bodies for
/// gathering project context (VCS info, file listings, etc.).
const ALLOWED_EXECUTABLES: &[&str] = &[
    // Version control
    "git", "gh", "svn", "hg", // File inspection (read-only)
    "cat", "head", "tail", "less", "wc", "ls", "find", "tree", "file", "stat", "du", "df",
    "readlink", "realpath", "basename", "dirname", // Text processing (read-only)
    "grep", "rg", "awk", "sed", "cut", "sort", "uniq", "tr", "diff", "comm", "paste", "column",
    "fmt", "fold", "jq", "yq", "xargs", // Shell builtins / utilities
    "echo", "printf", "date", "env", "printenv", "whoami", "hostname", "uname", "id", "pwd",
    "which", "test", "[", "true", "false", // Build tool queries (read-only)
    "cargo", "rustc", "npm", "node", "python", "python3", "pip", "make", "cmake", "go", "java",
    "javac", "dotnet", // Package / project info
    "dpkg", "rpm", "brew", "apt", // Networking (read-only queries)
    "curl", "wget", "dig", "nslookup", "ping", // Docker / container inspection
    "docker", "podman", "kubectl",
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
/// * `body` — The skill body text potentially containing `` !`command` `` patterns.
/// * `working_dir` — The directory in which to execute commands.
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
/// use ragent_core::skill::context::inject_dynamic_context;
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

/// Shell metacharacters that indicate the command needs `sh -c` wrapping
/// rather than direct execution.
const SHELL_OPERATORS: &[&str] = &["|", "&&", "||", ";", ">", "<", ">>", "$(", "`"];

/// Validate a command string against the allowlist.
///
/// Returns `Ok(())` if the first executable in the command is on the
/// allowlist, or an error message explaining the rejection.
fn validate_command(command: &str) -> Result<(), String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("empty command".to_string());
    }

    // Extract the first token (executable name) from the command.
    // For pipelines like `git diff | grep foo`, validate the first command.
    let first_token = trimmed
        .split(|c: char| c.is_whitespace() || c == '|' || c == ';' || c == '&')
        .next()
        .unwrap_or("");

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
/// If the command contains shell operators (pipes, redirects, etc.), it
/// is executed via `sh -c` for proper interpretation. Simple commands are
/// executed directly without a shell intermediary.
async fn execute_command(command: &str, working_dir: &Path) -> String {
    use crate::sanitize::redact_secrets;
    let safe_cmd = redact_secrets(command);

    // Validate against allowlist before execution (skipped in YOLO mode).
    if !crate::yolo::is_enabled()
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

    let needs_shell = SHELL_OPERATORS.iter().any(|op| command.contains(op));

    let result = if needs_shell {
        // Pipeline / redirect — must use shell, but first command is allowlisted.
        tokio::time::timeout(
            COMMAND_TIMEOUT,
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(working_dir)
                .output(),
        )
        .await
    } else {
        // Simple command — execute directly without shell.
        let tokens = tokenize_command(command);
        if tokens.is_empty() {
            return "[command error: empty command]".to_string();
        }
        tokio::time::timeout(
            COMMAND_TIMEOUT,
            tokio::process::Command::new(&tokens[0])
                .args(&tokens[1..])
                .current_dir(working_dir)
                .output(),
        )
        .await
    };

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
/// Handles single and double-quoted strings. Does not handle shell
/// expansions (those go through `sh -c` via the `needs_shell` path).
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
mod tests {
    use super::*;

    // --- Pattern finding tests ---

    #[test]
    fn test_find_no_patterns() {
        let patterns = find_command_patterns("Just plain text");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_find_single_pattern() {
        let patterns = find_command_patterns("Output: !`echo hello`");
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].command, "echo hello");
    }

    #[test]
    fn test_find_multiple_patterns() {
        let text = "A: !`cmd1` B: !`cmd2` C: !`cmd3`";
        let patterns = find_command_patterns(text);
        assert_eq!(patterns.len(), 3);
        assert_eq!(patterns[0].command, "cmd1");
        assert_eq!(patterns[1].command, "cmd2");
        assert_eq!(patterns[2].command, "cmd3");
    }

    #[test]
    fn test_find_pattern_with_pipes() {
        let patterns = find_command_patterns("!`cat file.txt | grep error | wc -l`");
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].command, "cat file.txt | grep error | wc -l");
    }

    #[test]
    fn test_find_pattern_multiline_text() {
        let text = "Line 1\n- Diff: !`git diff`\n- Status: !`git status`\nDone";
        let patterns = find_command_patterns(text);
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0].command, "git diff");
        assert_eq!(patterns[1].command, "git status");
    }

    #[test]
    fn test_find_ignores_empty_command() {
        let patterns = find_command_patterns("!``");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_find_ignores_unclosed() {
        let patterns = find_command_patterns("!`no closing backtick");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_find_ignores_regular_backticks() {
        let patterns = find_command_patterns("Use `code` and `more code` here");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_find_ignores_exclamation_without_backtick() {
        let patterns = find_command_patterns("This is great! And exciting!");
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_pattern_offsets() {
        let text = "prefix !`echo hi` suffix";
        let patterns = find_command_patterns(text);
        assert_eq!(patterns.len(), 1);
        assert_eq!(&text[patterns[0].start..patterns[0].end], "!`echo hi`");
    }

    // --- Command execution tests (require tokio runtime) ---

    #[tokio::test]
    async fn test_inject_no_patterns() {
        let result = inject_dynamic_context("Just text", Path::new("/tmp"))
            .await
            .expect("should succeed");
        assert_eq!(result, "Just text");
    }

    #[tokio::test]
    async fn test_inject_echo_command() {
        let result = inject_dynamic_context("Output: !`echo hello`", Path::new("/tmp"))
            .await
            .expect("should succeed");
        assert_eq!(result, "Output: hello");
    }

    #[tokio::test]
    async fn test_inject_multiple_commands() {
        let result = inject_dynamic_context("A: !`echo alpha` B: !`echo beta`", Path::new("/tmp"))
            .await
            .expect("should succeed");
        assert_eq!(result, "A: alpha B: beta");
    }

    #[tokio::test]
    async fn test_inject_failing_command() {
        // `false` is on the allowlist and always exits with status 1.
        let result = inject_dynamic_context("Result: !`false`", Path::new("/tmp"))
            .await
            .expect("should succeed even with failed command");
        assert!(
            result.starts_with("Result: [command failed:"),
            "Expected failure placeholder, got: {result}"
        );
    }

    #[tokio::test]
    async fn test_inject_preserves_surrounding_text() {
        let result = inject_dynamic_context("Before\n!`echo injected`\nAfter", Path::new("/tmp"))
            .await
            .expect("should succeed");
        assert_eq!(result, "Before\ninjected\nAfter");
    }

    #[tokio::test]
    async fn test_inject_working_dir() {
        let tmp = std::env::temp_dir().join("ragent_test_context_wd");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create temp dir");
        std::fs::write(tmp.join("marker.txt"), "found").expect("write marker");

        let result = inject_dynamic_context("Content: !`cat marker.txt`", &tmp)
            .await
            .expect("should succeed");
        assert_eq!(result, "Content: found");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_inject_command_with_pipes() {
        let result =
            inject_dynamic_context("Count: !`echo -e 'a\\nb\\nc' | wc -l`", Path::new("/tmp"))
                .await
                .expect("should succeed");
        // wc -l output may have leading spaces on some systems
        let count: String = result.replace("Count: ", "").trim().to_string();
        assert_eq!(count, "3");
    }

    #[tokio::test]
    async fn test_inject_pr_summary_pattern() {
        // Simulates the SPEC example with git-like commands
        let body = "## Context\n- Files: !`echo 'src/main.rs'`\n- Branch: !`echo 'feature/test'`\n\n## Task\nSummarize changes";
        let result = inject_dynamic_context(body, Path::new("/tmp"))
            .await
            .expect("should succeed");
        assert_eq!(
            result,
            "## Context\n- Files: src/main.rs\n- Branch: feature/test\n\n## Task\nSummarize changes"
        );
    }

    #[tokio::test]
    async fn test_inject_nonexistent_command() {
        // Command not on the allowlist should be rejected.
        let result = inject_dynamic_context("!`ragent_nonexistent_cmd_12345`", Path::new("/tmp"))
            .await
            .expect("should succeed with error placeholder");
        assert!(
            result.contains("[command rejected:"),
            "Expected rejection placeholder, got: {result}"
        );
    }

    // --- Allowlist validation tests ---

    #[test]
    fn test_validate_allowed_command() {
        assert!(validate_command("echo hello").is_ok());
        assert!(validate_command("git status").is_ok());
        assert!(validate_command("cat file.txt").is_ok());
        assert!(validate_command("grep -r pattern .").is_ok());
        assert!(validate_command("curl https://example.com").is_ok());
    }

    #[test]
    fn test_validate_rejected_command() {
        assert!(validate_command("rm -rf /").is_err());
        assert!(validate_command("bash -c 'evil'").is_err());
        assert!(validate_command("sh -c 'evil'").is_err());
        assert!(validate_command("nc -l 4444").is_err());
        assert!(validate_command("unknown_program").is_err());
    }

    #[test]
    fn test_validate_command_with_path() {
        // Absolute path to allowed executable should still work.
        assert!(validate_command("/usr/bin/echo hello").is_ok());
        assert!(validate_command("/usr/bin/git status").is_ok());
    }

    #[test]
    fn test_validate_empty_command() {
        assert!(validate_command("").is_err());
        assert!(validate_command("   ").is_err());
    }

    #[test]
    fn test_validate_pipeline_first_cmd() {
        // First command in a pipeline must be allowed.
        assert!(validate_command("echo hello | wc -l").is_ok());
        assert!(validate_command("ncat foo | grep bar").is_err());
    }

    // --- Tokenizer tests ---

    #[test]
    fn test_tokenize_simple() {
        assert_eq!(tokenize_command("echo hello"), vec!["echo", "hello"]);
    }

    #[test]
    fn test_tokenize_quoted() {
        assert_eq!(
            tokenize_command(r#"echo "hello world""#),
            vec!["echo", "hello world"]
        );
        assert_eq!(
            tokenize_command("echo 'hello world'"),
            vec!["echo", "hello world"]
        );
    }

    #[test]
    fn test_tokenize_escaped_space() {
        assert_eq!(
            tokenize_command(r"echo hello\ world"),
            vec!["echo", "hello world"]
        );
    }
}
