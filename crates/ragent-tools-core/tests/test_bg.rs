#![allow(clippy::assert_is_empty)]
//! Integration tests for the `bg` background command runner (M3).
//!
//! These tests exercise [`ragent_tools_core::bg::BackgroundCommand`] directly:
//! spawning shell commands, waiting for completion, inspecting captured output,
//! progress parsing, tail, and cancellation.

use std::sync::Arc;

use ragent_tools_core::bg::BackgroundCommand;
use ragent_types::event::{Event, EventBus};

/// `spawn` + `wait` + `output` for a simple stdout-producing command.
#[tokio::test]
async fn test_background_command_captures_stdout() {
    let cmd = BackgroundCommand::spawn(
        "test-1".to_string(),
        "echo hello-bg && echo done".to_string(),
        std::env::current_dir().unwrap(),
        None,
        String::new(),
    )
    .await
    .expect("spawn should succeed");

    cmd.wait(5)
        .await
        .expect("wait should finish before timeout");
    let (stdout, stderr) = cmd.output();
    assert!(
        stdout.contains("hello-bg\n"),
        "stdout missing expected line: {stdout}"
    );
    assert!(
        stdout.contains("done\n"),
        "stdout missing expected line: {stdout}"
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");
    assert_eq!(cmd.status(), "completed");
    assert_eq!(cmd.exit_code(), Some(0));
    assert!(cmd.is_done());
}

/// A failing command should be reported as `failed` with a non-zero exit code.
#[tokio::test]
async fn test_background_command_failed_exit_code() {
    let cmd = BackgroundCommand::spawn(
        "test-fail".to_string(),
        "exit 7".to_string(),
        std::env::current_dir().unwrap(),
        None,
        String::new(),
    )
    .await
    .expect("spawn should succeed");

    cmd.wait(5).await.expect("wait should finish");
    assert_eq!(cmd.status(), "failed");
    assert_eq!(cmd.exit_code(), Some(7));
    assert!(cmd.is_done());
}

/// `tail` should return only the last `n` lines of combined output.
#[tokio::test]
async fn test_background_command_tail() {
    let cmd = BackgroundCommand::spawn(
        "test-tail".to_string(),
        "printf 'line1\\nline2\\nline3\\nline4\\nline5\\n'".to_string(),
        std::env::current_dir().unwrap(),
        None,
        String::new(),
    )
    .await
    .expect("spawn should succeed");

    cmd.wait(5).await.expect("wait should finish");
    let tail = cmd.tail(2);
    assert!(tail.contains("line4"), "tail missing line4: {tail}");
    assert!(tail.contains("line5"), "tail missing line5: {tail}");
    assert!(
        !tail.contains("line1"),
        "tail should not contain line1: {tail}"
    );
    assert!(
        !tail.contains("line2"),
        "tail should not contain line2: {tail}"
    );
}

/// `JCODE_PROGRESS` JSON lines should be parsed into the progress object and
/// stripped from the normal stdout buffer.
#[tokio::test]
async fn test_background_command_progress_parsing() {
    let script = r#"printf 'JCODE_PROGRESS {"percent": 42, "step": "build"}\nJCODE_PROGRESS {"percent": 100, "step": "done"}\n'"#;
    let cmd = BackgroundCommand::spawn(
        "test-progress".to_string(),
        script.to_string(),
        std::env::current_dir().unwrap(),
        None,
        String::new(),
    )
    .await
    .expect("spawn should succeed");

    cmd.wait(5).await.expect("wait should finish");
    let progress = cmd.progress();
    assert_eq!(
        progress["percent"],
        serde_json::json!(100),
        "progress should merge to latest percent: {progress}"
    );
    assert_eq!(
        progress["step"],
        serde_json::json!("done"),
        "progress should merge to latest step: {progress}"
    );

    let (stdout, _stderr) = cmd.output();
    assert!(
        !stdout.contains("JCODE_PROGRESS"),
        "JCODE_PROGRESS marker should not appear in stdout: {stdout}"
    );
}

/// The event bus should receive `BackgroundTaskSpawned` and
/// `BackgroundTaskCompleted` events.
#[tokio::test]
async fn test_background_command_events() {
    let bus = Arc::new(EventBus::new(64));
    let mut rx = bus.subscribe();

    let cmd = BackgroundCommand::spawn(
        "test-events".to_string(),
        "echo event-test".to_string(),
        std::env::current_dir().unwrap(),
        Some(Arc::clone(&bus)),
        "event-session".to_string(),
    )
    .await
    .expect("spawn should succeed");

    cmd.wait(5).await.expect("wait should finish");

    // Drain a few events; we expect at least a Completed event.
    let mut saw_completed = false;
    for _ in 0..16 {
        match rx.try_recv() {
            Ok(Event::BackgroundTaskCompleted { .. }) => {
                saw_completed = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    assert!(saw_completed, "expected a BackgroundTaskCompleted event");
}

/// Cancelling a long-running command should stop it and set status to
/// `cancelled`.
#[tokio::test]
async fn test_background_command_cancel() {
    let cmd = BackgroundCommand::spawn(
        "test-cancel".to_string(),
        "sleep 30".to_string(),
        std::env::current_dir().unwrap(),
        None,
        String::new(),
    )
    .await
    .expect("spawn should succeed");

    // Give the process a moment to start.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    cmd.cancel().await.expect("cancel should succeed");
    cmd.wait(5).await.expect("cancelled task should exit");

    assert_eq!(cmd.status(), "cancelled");
    assert!(cmd.is_done());
}

/// `wait` should time out (return an error) when the command runs longer than
/// the timeout without completing.
#[tokio::test]
async fn test_background_command_wait_timeout() {
    let cmd = BackgroundCommand::spawn(
        "test-timeout".to_string(),
        "sleep 5".to_string(),
        std::env::current_dir().unwrap(),
        None,
        String::new(),
    )
    .await
    .expect("spawn should succeed");

    let res = cmd.wait(1).await;
    assert!(res.is_err(), "wait should time out");
    // Clean up so the test process does not linger.
    let _ = cmd.cancel().await;
}
