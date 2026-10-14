//! Coverage for the bang-command (`!`-prefixed) output/prompt helpers.
//!
//! Audit gap: `combine_command_output`, `build_bang_command_prompt` and
//! `bang_command_prompt_from_output` had no test references. The private
//! `truncate_command_output` (30k-char FIRST/LAST split) is exercised through
//! the public `build_bang_command_prompt`.

use ragent_agent::bang_command::{
    bang_command_prompt_from_output, build_bang_command_prompt, combine_command_output,
};
use std::process::{Command, Output};

#[test]
fn test_combine_command_output_stdout_only() {
    assert_eq!(combine_command_output(b"hello", b""), "hello");
}

#[test]
fn test_combine_command_output_stderr_only() {
    assert_eq!(combine_command_output(b"", b"boom"), "boom");
}

#[test]
fn test_combine_command_output_both_joined_by_newline() {
    assert_eq!(combine_command_output(b"out", b"err"), "out\nerr");
}

#[test]
fn test_combine_command_output_empty_placeholder() {
    assert_eq!(combine_command_output(b"", b""), "(no output)");
}

#[test]
fn test_combine_command_output_lossy_utf8() {
    // Invalid UTF-8 must not panic; it is replaced lossily.
    let out = combine_command_output(&[0xff, 0xfe], b"");
    assert_ne!(out, "");
}

#[test]
fn test_build_prompt_includes_command_and_output() {
    let prompt = build_bang_command_prompt("ls -la", "file-a\nfile-b");
    assert!(prompt.contains("$ ls -la"));
    assert!(prompt.contains("file-a\nfile-b"));
    assert!(prompt.contains("Please review the output"));
}

#[test]
fn test_build_prompt_truncates_long_output_head_and_tail() {
    // Build an input well past the 31k combined budget so the private
    // truncate_command_output head/tail split fires.
    let head_marker = "HEAD_MARKER";
    let tail_marker = "TAIL_MARKER";
    let middle_marker = "MIDDLE_MARKER_SHOULD_BE_DROPPED";
    let mut content = String::from(head_marker);
    content.push_str(&"y".repeat(20_000));
    content.push_str(middle_marker);
    content.push_str(&"z".repeat(20_000));
    content.push_str(tail_marker);

    let prompt = build_bang_command_prompt("cat big", &content);
    assert!(prompt.contains(head_marker), "head must be kept");
    assert!(prompt.contains(tail_marker), "tail must be kept");
    assert!(
        prompt.contains("characters omitted"),
        "omission marker shown"
    );
    // The middle of the input must have been dropped.
    assert!(
        !prompt.contains(middle_marker),
        "middle marker must be omitted"
    );
}

#[test]
fn test_prompt_from_output_ok_result() {
    let out = Command::new("sh")
        .arg("-c")
        .arg("printf 'from-stdout'")
        .output()
        .expect("spawn sh");
    let result: Result<Output, std::io::Error> = Ok(out);
    let prompt = bang_command_prompt_from_output("echo hi", &result);
    assert!(prompt.contains("from-stdout"));
}

#[test]
fn test_prompt_from_output_err_result_is_diagnosable() {
    let err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such command");
    let result: Result<Output, std::io::Error> = Err(err);
    let prompt = bang_command_prompt_from_output("definitely-not-a-cmd", &result);
    assert!(prompt.contains("failed to execute command"));
    assert!(prompt.contains("no such command"));
}
