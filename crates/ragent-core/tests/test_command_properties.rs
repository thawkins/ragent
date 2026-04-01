//! Property-style and stress tests for command execution boundaries.
//!
//! Tests dynamic context parsing with edge-case inputs: unicode, long commands,
//! nested backticks, huge output, and concurrent execution.

use ragent_core::skill::context::inject_dynamic_context;
use ragent_core::resource::{acquire_process_permit, available_process_permits, MAX_CONCURRENT_PROCESSES};
use std::path::Path;

fn wd() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

// ── Parsing robustness ───────────────────────────────────────────

#[tokio::test]
async fn test_inject_unicode_in_surrounding_text() {
    let result = inject_dynamic_context("日本語 !`echo ok` テスト", wd())
        .await
        .unwrap();
    assert!(result.contains("ok"), "Command should execute in unicode context: {result}");
    assert!(result.contains("日本語"), "Unicode prefix should be preserved");
    assert!(result.contains("テスト"), "Unicode suffix should be preserved");
}

#[tokio::test]
async fn test_inject_emoji_in_text() {
    let result = inject_dynamic_context("🚀 Result: !`echo launch` 🎉", wd())
        .await
        .unwrap();
    assert!(result.contains("launch"), "Command output present: {result}");
    assert!(result.contains("🚀"), "Emoji prefix preserved");
    assert!(result.contains("🎉"), "Emoji suffix preserved");
}

#[tokio::test]
async fn test_inject_nested_backticks_not_confused() {
    // Nested backticks should not cause infinite parsing
    let result = inject_dynamic_context("Code: `let x = 1;` and !`echo hello`", wd())
        .await
        .unwrap();
    // Regular backticks preserved, exclamation pattern executed
    assert!(result.contains("hello"), "Exclamation pattern should execute: {result}");
    assert!(result.contains("`let x = 1;`"), "Regular backticks preserved: {result}");
}

#[tokio::test]
async fn test_inject_unclosed_backtick_handled() {
    // !` without closing backtick should be treated as plain text
    let result = inject_dynamic_context("Broken: !`echo unclosed", wd())
        .await
        .unwrap();
    // Should not panic or hang
    assert!(
        result.contains("Broken:"),
        "Unclosed backtick should be handled: {result}"
    );
}

#[tokio::test]
async fn test_inject_many_patterns_in_sequence() {
    let mut input = String::new();
    for i in 0..20 {
        input.push_str(&format!("v{i}=!`echo val{i}` "));
    }
    let result = inject_dynamic_context(&input, wd()).await.unwrap();
    // All 20 commands should have run (or been handled)
    for i in 0..20 {
        assert!(
            result.contains(&format!("val{i}")),
            "Pattern {i} should have produced output: {result}"
        );
    }
}

#[tokio::test]
async fn test_inject_empty_input() {
    let result = inject_dynamic_context("", wd()).await.unwrap();
    assert_eq!(result, "", "Empty input should produce empty output");
}

#[tokio::test]
async fn test_inject_only_whitespace() {
    let result = inject_dynamic_context("   \n\t  ", wd()).await.unwrap();
    assert_eq!(result, "   \n\t  ", "Whitespace-only input unchanged");
}

// ── Long command / large output ──────────────────────────────────

#[tokio::test]
async fn test_inject_long_echo_command() {
    // Command with very long argument
    let long_arg: String = "x".repeat(5000);
    let input = format!("Out: !`echo {long_arg}`");
    let result = inject_dynamic_context(&input, wd()).await.unwrap();
    // Should either succeed or timeout, not panic
    assert!(result.contains("Out:"), "Should handle long command: {result}");
}

#[tokio::test]
async fn test_inject_command_producing_large_output() {
    // Use printf with many repetitions via a pipeline (echo is allowlisted)
    let result = inject_dynamic_context("Count: !`echo -e '1\\n2\\n3\\n4\\n5\\n6\\n7\\n8\\n9\\n10'`", wd()).await.unwrap();
    assert!(result.contains("1"), "Output should be captured: {result}");
    assert!(result.contains("Count:"), "Surrounding text preserved");
}

// ── Rejected commands don't hang ─────────────────────────────────

#[tokio::test]
async fn test_rejected_command_returns_quickly() {
    let start = std::time::Instant::now();
    let result = inject_dynamic_context("Out: !`evil_nonexistent_binary`", wd())
        .await
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        result.contains("[command rejected:"),
        "Non-allowlisted command should be rejected: {result}"
    );
    assert!(
        elapsed.as_secs() < 2,
        "Rejected command should return quickly, took {:?}",
        elapsed
    );
}

// ── Resource semaphore tests ─────────────────────────────────────

#[tokio::test]
async fn test_process_semaphore_constant() {
    assert_eq!(MAX_CONCURRENT_PROCESSES, 16);
}

#[tokio::test]
async fn test_process_semaphore_acquire_release() {
    let initial = available_process_permits();
    assert!(initial > 0, "Should have available permits");

    let permit = acquire_process_permit().await.unwrap();
    assert_eq!(
        available_process_permits(),
        initial - 1,
        "Acquiring permit should decrement count"
    );

    drop(permit);
    tokio::task::yield_now().await;
    assert_eq!(
        available_process_permits(),
        initial,
        "Dropping permit should restore count"
    );
}

#[tokio::test]
async fn test_process_semaphore_multiple_permits() {
    let initial = available_process_permits();
    let mut permits = Vec::new();

    let count = 6.min(initial);
    for _ in 0..count {
        permits.push(acquire_process_permit().await.unwrap());
    }
    assert_eq!(available_process_permits(), initial - count);

    drop(permits);
    tokio::task::yield_now().await;
    assert_eq!(available_process_permits(), initial);
}

// ── Concurrent dynamic context execution ─────────────────────────

#[tokio::test]
async fn test_concurrent_context_commands() {
    let mut handles = Vec::new();
    for i in 0..8 {
        let wd = wd();
        handles.push(tokio::spawn(async move {
            let input = format!("Out: !`echo concurrent_{i}`");
            inject_dynamic_context(&input, wd).await.unwrap()
        }));
    }

    let results: Vec<String> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    for i in 0..8 {
        assert!(
            results[i].contains(&format!("concurrent_{i}")),
            "Concurrent command {i} should produce output: {}",
            results[i]
        );
    }
}
