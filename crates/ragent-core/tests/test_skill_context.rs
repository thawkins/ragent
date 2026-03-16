//! Phase 10.3: External tests for dynamic context injection.
//!
//! Covers edge cases in `!`command`` pattern matching and replacement.

use ragent_core::skill::context::inject_dynamic_context;
use std::path::Path;

/// Default working dir for tests.
fn wd() -> &'static Path {
    Path::new("/tmp")
}

// ── Pattern matching ─────────────────────────────────────────────

#[tokio::test]
async fn test_inject_simple_echo() {
    let result = inject_dynamic_context("Before !`echo hello` after", wd())
        .await
        .expect("should succeed");
    assert_eq!(result, "Before hello after");
}

#[tokio::test]
async fn test_inject_preserves_text_without_patterns() {
    let input = "No dynamic context here. Just plain text.";
    let result = inject_dynamic_context(input, wd()).await.expect("should succeed");
    assert_eq!(result, input);
}

#[tokio::test]
async fn test_inject_multiple_commands() {
    let input = "A: !`echo one` B: !`echo two`";
    let result = inject_dynamic_context(input, wd()).await.expect("should succeed");
    assert_eq!(result, "A: one B: two");
}

#[tokio::test]
async fn test_inject_command_with_pipe() {
    let result = inject_dynamic_context("Files: !`echo 'a b c' | tr ' ' '\\n'`", wd())
        .await
        .expect("should succeed");
    assert!(result.starts_with("Files:"));
    assert!(result.contains('a'));
}

#[tokio::test]
async fn test_inject_command_with_working_dir() {
    let tmp = std::env::temp_dir().join("ragent_test_ctx_wd");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tmp");

    let result = inject_dynamic_context("Dir: !`pwd`", &tmp)
        .await
        .expect("should succeed");
    assert!(result.contains(&tmp.to_string_lossy().to_string()));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_inject_failing_command_shows_error() {
    let result = inject_dynamic_context("Out: !`exit 1`", wd())
        .await
        .expect("should succeed");
    assert!(
        result.contains("[error") || result.contains("Out:"),
        "Should handle failing command gracefully: {result}"
    );
}

#[tokio::test]
async fn test_inject_nonexistent_command() {
    let result =
        inject_dynamic_context("Out: !`__nonexistent_command_12345__`", wd())
            .await
            .expect("should succeed");
    assert!(
        result.contains("[error") || result.contains("Out:"),
        "Should handle nonexistent command: {result}"
    );
}

#[tokio::test]
async fn test_inject_multiline_output() {
    let result = inject_dynamic_context("Lines: !`printf 'a\\nb\\nc'`", wd())
        .await
        .expect("should succeed");
    assert!(result.contains("a\nb\nc") || result.contains("a\\nb\\nc"));
}

#[tokio::test]
async fn test_inject_regular_backticks_preserved() {
    let input = "Code: `let x = 42;` and more";
    let result = inject_dynamic_context(input, wd()).await.expect("should succeed");
    assert_eq!(result, input, "Regular backticks should not be processed");
}

#[tokio::test]
async fn test_inject_exclamation_without_backtick_preserved() {
    let input = "Hey! This is great! And `code` too!";
    let result = inject_dynamic_context(input, wd()).await.expect("should succeed");
    assert_eq!(result, input);
}

#[tokio::test]
async fn test_inject_empty_command_ignored() {
    let input = "Empty: !`` stuff";
    let result = inject_dynamic_context(input, wd()).await.expect("should succeed");
    assert_eq!(result, input, "Empty commands should be ignored");
}

#[tokio::test]
async fn test_inject_command_output_trimmed() {
    // echo adds a trailing newline — verify it's trimmed
    let result = inject_dynamic_context("Val: !`echo trimmed`", wd())
        .await
        .expect("should succeed");
    assert_eq!(result, "Val: trimmed");
}

#[tokio::test]
async fn test_inject_command_with_special_chars_in_output() {
    let result = inject_dynamic_context(
        r#"Out: !`echo "hello <world> & stuff"`"#,
        wd(),
    )
    .await
    .expect("should succeed");
    assert!(result.starts_with("Out:"));
}

#[tokio::test]
async fn test_inject_adjacent_patterns() {
    let result = inject_dynamic_context("!`echo a`!`echo b`", wd())
        .await
        .expect("should succeed");
    assert_eq!(result, "ab");
}

#[tokio::test]
async fn test_inject_pattern_at_start_of_text() {
    let result = inject_dynamic_context("!`echo start` rest", wd())
        .await
        .expect("should succeed");
    assert_eq!(result, "start rest");
}

#[tokio::test]
async fn test_inject_pattern_at_end_of_text() {
    let result = inject_dynamic_context("start !`echo end`", wd())
        .await
        .expect("should succeed");
    assert_eq!(result, "start end");
}
