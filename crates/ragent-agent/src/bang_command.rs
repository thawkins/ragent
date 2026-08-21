//! Shared helpers for the "bang command" (`!`-prefixed) feature.
//!
//! Both the CLI (`src/main.rs`) and the TUI (`ragent-tui`) support running a
//! shell command prefixed with `!` and asking the model to review its output.
//! This module centralises the output-combining and prompt-building logic so
//! the two entry points stay in sync.

use std::process::Output;

/// Maximum combined output length before truncation is applied.
///
/// Matches the `bash` tool's limit: keep the first 15k chars + last 15k chars
/// with a separator.  This prevents a single command with huge output from
/// blowing the model's context window.
const MAX_OUTPUT: usize = 30_000 + 1_000; // first + last + separator allowance
const FIRST_CHARS: usize = 15_000;
const LAST_CHARS: usize = 15_000;

/// Combine the stdout and stderr of a completed command into a single string.
///
/// stdout is emitted first, followed by stderr (separated by a newline when
/// both are non-empty). When both are empty the placeholder `"(no output)"` is
/// returned so the model always has something to review.
pub fn combine_command_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::new();
    if !stdout.is_empty() {
        text.push_str(&String::from_utf8_lossy(stdout));
    }
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&String::from_utf8_lossy(stderr));
    }
    if text.is_empty() {
        text.push_str("(no output)");
    }
    text
}

/// Truncate very long command output to fit within the context window.
///
/// Keeps the first [`FIRST_CHARS`] and last [`LAST_CHARS`] characters, joining
/// them with an omission marker that reports how many characters were dropped.
/// Output at or below [`MAX_OUTPUT`] is returned unchanged.
fn truncate_command_output(content: &str) -> String {
    if content.len() <= MAX_OUTPUT {
        return content.to_string();
    }

    // Find valid UTF-8 char boundary near the head split point.
    let first_end = {
        let mut i = FIRST_CHARS.min(content.len());
        while i > 0 && !content.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let first_part = &content[..first_end];

    let last_start = {
        let mut j = content.len().saturating_sub(LAST_CHARS);
        while j < content.len() && !content.is_char_boundary(j) {
            j += 1;
        }
        j
    };
    let last_part = &content[last_start..];

    let omitted = last_start.saturating_sub(first_end);
    format!(
        "{first_part}\n\n\
         ... ({omitted} characters omitted) ...\n\n\
         {last_part}"
    )
}

/// Build the review prompt sent to the model after a bang command runs.
///
/// The prompt presents the command and its combined output and asks the model
/// to review for errors and resolve them.  Output longer than [`MAX_OUTPUT`]
/// is truncated (head + tail) to protect the context window.
pub fn build_bang_command_prompt(command: &str, combined_output: &str) -> String {
    let truncated = truncate_command_output(combined_output);
    format!(
        "I ran the following shell command:\n\n\
         $ {command}\n\n\
         Output:\n```\n{truncated}\n```\n\n\
         Please review the output for any errors and resolve them as required."
    )
}

/// Convenience wrapper: combine a [`Result<Output>`] into the review prompt.
///
/// Maps execution failures (`Err`) to a human-readable error string so the
/// model can still be asked to diagnose the failure.
pub fn bang_command_prompt_from_output(
    command: &str,
    result: &Result<Output, std::io::Error>,
) -> String {
    let combined = match result {
        Ok(out) => combine_command_output(&out.stdout, &out.stderr),
        Err(e) => format!("failed to execute command: {e}"),
    };
    build_bang_command_prompt(command, &combined)
}
