//! Tests for context.rs (M8/T8.4).
//! Compiled as a submodule via #[path], super::* resolves to the source module.

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
    let result = inject_dynamic_context("Count: !`echo -e 'a\\nb\\nc' | wc -l`", Path::new("/tmp"))
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
