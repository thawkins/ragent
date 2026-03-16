//! Dynamic context injection for skill bodies.
//!
//! The `` !`command` `` syntax allows skill bodies to embed the output of
//! shell commands. Before the skill content is sent to the agent, each
//! `` !`command` `` placeholder is replaced with the command's stdout.
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

/// Execute a shell command and return its stdout, or an error message.
async fn execute_command(command: &str, working_dir: &Path) -> String {
    tracing::debug!(command, "Executing dynamic context command");

    let result = tokio::time::timeout(
        COMMAND_TIMEOUT,
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
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
                    command,
                    status = ?output.status,
                    stderr = %stderr,
                    "Dynamic context command failed"
                );
                format!("[command failed: {}]", stderr.trim())
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(command, error = %e, "Failed to spawn dynamic context command");
            format!("[command error: {e}]")
        }
        Err(_) => {
            tracing::warn!(command, "Dynamic context command timed out after 30s");
            format!("[command timed out: {command}]")
        }
    }
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
        let result =
            inject_dynamic_context("Output: !`echo hello`", Path::new("/tmp"))
                .await
                .expect("should succeed");
        assert_eq!(result, "Output: hello");
    }

    #[tokio::test]
    async fn test_inject_multiple_commands() {
        let result = inject_dynamic_context(
            "A: !`echo alpha` B: !`echo beta`",
            Path::new("/tmp"),
        )
        .await
        .expect("should succeed");
        assert_eq!(result, "A: alpha B: beta");
    }

    #[tokio::test]
    async fn test_inject_failing_command() {
        let result = inject_dynamic_context(
            "Result: !`sh -c 'exit 1'`",
            Path::new("/tmp"),
        )
        .await
        .expect("should succeed even with failed command");
        assert!(result.starts_with("Result: [command failed:"));
    }

    #[tokio::test]
    async fn test_inject_preserves_surrounding_text() {
        let result = inject_dynamic_context(
            "Before\n!`echo injected`\nAfter",
            Path::new("/tmp"),
        )
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

        let result =
            inject_dynamic_context("Content: !`cat marker.txt`", &tmp)
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
        let result = inject_dynamic_context(
            "!`ragent_nonexistent_cmd_12345`",
            Path::new("/tmp"),
        )
        .await
        .expect("should succeed with error placeholder");
        assert!(
            result.contains("[command failed:") || result.contains("[command error:"),
            "Expected error placeholder, got: {result}"
        );
    }
}
