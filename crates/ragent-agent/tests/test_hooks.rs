#![allow(clippy::assert_is_empty)]
//! Tests for PreToolUse and PostToolUse hook exit-code semantics (T-001, T-002, T-003, T-005).

use ragent_agent::hooks::{
    HookConfig, HookTrigger, PostToolUseResult, PreToolUseResult, run_post_tool_use_hooks,
    run_pre_tool_use_hooks,
};
use ragent_types::event::EventBus;
use std::path::Path;
use tempfile::TempDir;

// ── PreToolUse inline-command tests (T-001) ─────────────────────────────────

#[test]
fn test_pre_tool_use_exit_code_2_blocks_and_ignores_stdout_allow() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: "echo '{\"decision\":\"allow\"}' && exit 2".to_string(),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "write",
        r#"{"path":"/etc/passwd"}"#,
        "sess-001",
        Some(&bus),
    );

    match result {
        PreToolUseResult::Blocked { reason } => {
            assert!(
                reason.is_empty(),
                "empty stderr should produce empty reason"
            );
        }
        other => panic!("expected Blocked, got {:?}", other),
    }

    // No HookWarning should be published for exit code 2.
    assert!(rx.try_recv().is_err());
}

#[test]
fn test_pre_tool_use_exit_code_2_uses_stderr_as_reason() {
    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: "echo 'policy violation' >&2 && exit 2".to_string(),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "bash",
        r#"{"command":"rm -rf /"}"#,
        "sess-002",
        Some(&bus),
    );

    match result {
        PreToolUseResult::Blocked { reason } => {
            assert_eq!(reason, "policy violation");
        }
        other => panic!("expected Blocked, got {:?}", other),
    }
}

#[test]
fn test_pre_tool_use_exit_code_2_crops_long_stderr() {
    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!(
            "python3 -c 'print(\"x\"*1000, end=\"\"); import sys; sys.stderr.write(\"y\"*1000); sys.exit(2)'"
        ),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "bash",
        r#"{"command":"echo hi"}"#,
        "sess-003",
        Some(&bus),
    );

    match result {
        PreToolUseResult::Blocked { reason } => {
            assert_eq!(reason.len(), 500, "stderr should be capped at 500 chars");
            assert!(reason.chars().all(|c| c == 'y'));
        }
        other => panic!("expected Blocked, got {:?}", other),
    }
}

#[test]
fn test_pre_tool_use_exit_code_1_warns_and_returns_no_decision() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: "echo 'suspicious' >&2 && exit 1".to_string(),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "write",
        r#"{"path":"/tmp/test"}"#,
        "sess-004",
        Some(&bus),
    );

    assert!(
        matches!(result, PreToolUseResult::NoDecision),
        "expected NoDecision, got {:?}",
        result
    );

    let event = rx
        .try_recv()
        .expect("HookWarning event should be published");
    match event {
        ragent_types::event::Event::HookWarning {
            session_id,
            hook_command,
            tool,
            stderr,
        } => {
            assert_eq!(session_id, "sess-004");
            assert_eq!(hook_command, hooks[0].command);
            assert_eq!(tool, "write");
            assert_eq!(stderr, "suspicious");
        }
        other => panic!("expected HookWarning, got {:?}", other),
    }
}

#[test]
fn test_pre_tool_use_exit_code_3_falls_through_to_no_decision() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: "echo 'hook bug' >&2 && exit 3".to_string(),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "read",
        r#"{"path":"/tmp/foo"}"#,
        "sess-005",
        Some(&bus),
    );

    assert!(
        matches!(result, PreToolUseResult::NoDecision),
        "expected NoDecision for exit code >=3, got {:?}",
        result
    );
    assert!(
        rx.try_recv().is_err(),
        "no event should be published for exit code >=3"
    );
}

#[test]
fn test_pre_tool_use_exit_code_0_allow_still_parses_stdout() {
    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: "echo '{\"decision\":\"allow\"}'".to_string(),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "read",
        r#"{"path":"/tmp/foo"}"#,
        "sess-006",
        Some(&bus),
    );

    assert!(
        matches!(result, PreToolUseResult::Allow),
        "expected Allow, got {:?}",
        result
    );
}

#[test]
fn test_pre_tool_use_no_hooks_returns_no_decision() {
    let bus = EventBus::default();
    let hooks: Vec<HookConfig> = vec![];

    let result = run_pre_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "read",
        r#"{"path":"/tmp/foo"}"#,
        "sess-007",
        Some(&bus),
    );

    assert!(
        matches!(result, PreToolUseResult::NoDecision),
        "expected NoDecision, got {:?}",
        result
    );
}

// ── PostToolUse inline-command tests (T-002) ───────────────────────────────

#[tokio::test]
async fn test_post_tool_use_exit_code_0_parses_modified_output() {
    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: r#"echo '{"modified_output":{"content":"replaced"}}'"#.to_string(),
        timeout_secs: 30,
    }];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "write",
        r#"{"path":"/tmp/foo"}"#,
        r#"{"content":"original"}"#,
        true,
        "sess-post-001",
        Some(&bus),
    )
    .await;

    match result {
        PostToolUseResult::Ok { modified_output } => {
            let modified = modified_output.expect("should have modified output");
            assert_eq!(
                modified.get("content").unwrap().as_str().unwrap(),
                "replaced"
            );
        }
        other => panic!("expected Ok, got {:?}", other),
    }
}

#[tokio::test]
async fn test_post_tool_use_exit_code_1_warns_and_publishes_hook_warning() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: "echo 'suspicious output' >&2 && exit 1".to_string(),
        timeout_secs: 30,
    }];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "write",
        r#"{"path":"/tmp/foo"}"#,
        r#"{"content":"original"}"#,
        true,
        "sess-post-002",
        Some(&bus),
    )
    .await;

    match result {
        PostToolUseResult::Warn { message } => {
            assert_eq!(message, "suspicious output");
        }
        other => panic!("expected Warn, got {:?}", other),
    }

    let event = rx
        .try_recv()
        .expect("HookWarning event should be published");
    match event {
        ragent_types::event::Event::HookWarning {
            session_id,
            hook_command,
            tool,
            stderr,
        } => {
            assert_eq!(session_id, "sess-post-002");
            assert_eq!(hook_command, hooks[0].command);
            assert_eq!(tool, "write");
            assert_eq!(stderr, "suspicious output");
        }
        other => panic!("expected HookWarning, got {:?}", other),
    }
}

#[tokio::test]
async fn test_post_tool_use_exit_code_2_flags_and_publishes_tool_result_flagged() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: "echo 'policy violation' >&2 && exit 2".to_string(),
        timeout_secs: 30,
    }];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "bash",
        r#"{"command":"rm -rf /"}"#,
        r#"{"content":"done"}"#,
        true,
        "sess-post-003",
        Some(&bus),
    )
    .await;

    match result {
        PostToolUseResult::Flagged { reason } => {
            assert_eq!(reason, "policy violation");
        }
        other => panic!("expected Flagged, got {:?}", other),
    }

    let event = rx
        .try_recv()
        .expect("ToolResultFlagged event should be published");
    match event {
        ragent_types::event::Event::ToolResultFlagged {
            session_id,
            tool,
            hook_command,
            reason,
        } => {
            assert_eq!(session_id, "sess-post-003");
            assert_eq!(tool, "bash");
            assert_eq!(hook_command, hooks[0].command);
            assert_eq!(reason, "policy violation");
        }
        other => panic!("expected ToolResultFlagged, got {:?}", other),
    }
}

#[tokio::test]
async fn test_post_tool_use_exit_code_3_falls_through_to_ok() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: "echo 'hook bug' >&2 && exit 3".to_string(),
        timeout_secs: 30,
    }];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "read",
        r#"{"path":"/tmp/foo"}"#,
        r#"{"content":"done"}"#,
        true,
        "sess-post-004",
        Some(&bus),
    )
    .await;

    match result {
        PostToolUseResult::Ok { modified_output } => {
            assert!(modified_output.is_none(), "no modified output expected");
        }
        other => panic!("expected Ok, got {:?}", other),
    }

    assert!(
        rx.try_recv().is_err(),
        "no event should be published for exit code >=3"
    );
}

#[tokio::test]
async fn test_post_tool_use_no_hooks_returns_ok() {
    let bus = EventBus::default();
    let hooks: Vec<HookConfig> = vec![];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "read",
        r#"{"path":"/tmp/foo"}"#,
        r#"{"content":"done"}"#,
        true,
        "sess-post-005",
        Some(&bus),
    )
    .await;

    match result {
        PostToolUseResult::Ok { modified_output } => {
            assert!(modified_output.is_none());
        }
        other => panic!("expected Ok, got {:?}", other),
    }
}

#[tokio::test]
async fn test_post_tool_use_flagged_takes_priority_over_warn() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![
        HookConfig {
            trigger: HookTrigger::PostToolUse,
            command: "echo 'warn' >&2 && exit 1".to_string(),
            timeout_secs: 30,
        },
        HookConfig {
            trigger: HookTrigger::PostToolUse,
            command: "echo 'flagged' >&2 && exit 2".to_string(),
            timeout_secs: 30,
        },
    ];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "write",
        r#"{"path":"/tmp/foo"}"#,
        r#"{"content":"done"}"#,
        true,
        "sess-post-006",
        Some(&bus),
    )
    .await;

    match result {
        PostToolUseResult::Flagged { reason } => {
            assert_eq!(reason, "flagged");
        }
        other => panic!("expected Flagged, got {:?}", other),
    }

    // Both events should have been published.
    let mut got_warning = false;
    let mut got_flagged = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ragent_types::event::Event::HookWarning { .. } => got_warning = true,
            ragent_types::event::Event::ToolResultFlagged { .. } => got_flagged = true,
            _ => {}
        }
    }
    assert!(got_warning, "HookWarning should have been published");
    assert!(got_flagged, "ToolResultFlagged should have been published");
}

// ── Hook spawn failure / timeout tests (T-003, FR-016) ─────────────────────

#[tokio::test]
async fn test_post_tool_use_timeout_falls_through_to_ok() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PostToolUse,
        command: "sleep 10".to_string(),
        timeout_secs: 1,
    }];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "read",
        r#"{"path":"/tmp/foo"}"#,
        r#"{"content":"done"}"#,
        true,
        "sess-timeout-001",
        Some(&bus),
    )
    .await;

    match result {
        PostToolUseResult::Ok { modified_output } => {
            assert!(
                modified_output.is_none(),
                "timeout should not produce modified output"
            );
        }
        other => panic!("expected Ok on timeout, got {:?}", other),
    }

    assert!(
        rx.try_recv().is_err(),
        "no event should be published on timeout (hook error, not policy decision)"
    );
}

#[tokio::test]
async fn test_post_tool_use_timeout_does_not_override_warn() {
    let bus = EventBus::default();
    let hooks = vec![
        HookConfig {
            trigger: HookTrigger::PostToolUse,
            command: "echo 'warn' >&2 && exit 1".to_string(),
            timeout_secs: 30,
        },
        HookConfig {
            trigger: HookTrigger::PostToolUse,
            command: "sleep 10".to_string(),
            timeout_secs: 1,
        },
    ];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "write",
        r#"{"path":"/tmp/foo"}"#,
        r#"{"content":"done"}"#,
        true,
        "sess-timeout-002",
        Some(&bus),
    )
    .await;

    // The warn from hook 1 should survive; the timeout from hook 2 is a hook
    // error (exit >=3) and should NOT override the warn.
    match result {
        PostToolUseResult::Warn { message } => {
            assert_eq!(message, "warn");
        }
        other => panic!("expected Warn, got {:?}", other),
    }
}

#[tokio::test]
async fn test_post_tool_use_timeout_does_not_override_flagged() {
    let bus = EventBus::default();
    let hooks = vec![
        HookConfig {
            trigger: HookTrigger::PostToolUse,
            command: "echo 'flagged' >&2 && exit 2".to_string(),
            timeout_secs: 30,
        },
        HookConfig {
            trigger: HookTrigger::PostToolUse,
            command: "sleep 10".to_string(),
            timeout_secs: 1,
        },
    ];

    let result = run_post_tool_use_hooks(
        &hooks,
        Path::new("/tmp"),
        "bash",
        r#"{"command":"echo hi"}"#,
        r#"{"content":"done"}"#,
        true,
        "sess-timeout-003",
        Some(&bus),
    )
    .await;

    // The flag from hook 1 should survive; the timeout from hook 2 is a hook
    // error (exit >=3) and should NOT override the flag.
    match result {
        PostToolUseResult::Flagged { reason } => {
            assert_eq!(reason, "flagged");
        }
        other => panic!("expected Flagged, got {:?}", other),
    }
}

// ── PreToolUse fixture-script tests (T-005) ────────────────────────────────
//
// These tests create actual shell scripts in a temp dir and invoke them via
// HookConfig, validating the full path from file-based hook scripts through
// the exit-code parsing logic.

/// Write a shell script to a temp dir and return its path.
fn write_hook_script(dir: &TempDir, name: &str, body: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, body).expect("failed to write hook script");
    // Make the script executable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("set_permissions");
    }
    path.to_string_lossy().to_string()
}

#[test]
fn test_fixture_pre_tool_use_exit_0_allow() {
    let dir = TempDir::new().expect("tempdir");
    let script = write_hook_script(
        &dir,
        "allow.sh",
        "#!/bin/sh\necho '{\"decision\":\"allow\"}'\nexit 0\n",
    );

    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {script}"),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "read",
        r#"{"path":"src/main.rs"}"#,
        "sess-fixture-001",
        Some(&bus),
    );

    assert!(
        matches!(result, PreToolUseResult::Allow),
        "expected Allow, got {:?}",
        result
    );
}

#[test]
fn test_fixture_pre_tool_use_exit_0_deny() {
    let dir = TempDir::new().expect("tempdir");
    let script = write_hook_script(
        &dir,
        "deny.sh",
        "#!/bin/sh\necho '{\"decision\":\"deny\",\"reason\":\"forbidden by policy\"}'\nexit 0\n",
    );

    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {script}"),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "write",
        r#"{"path":"/etc/passwd"}"#,
        "sess-fixture-002",
        Some(&bus),
    );

    match result {
        PreToolUseResult::Deny { reason } => {
            assert_eq!(reason, "forbidden by policy");
        }
        other => panic!("expected Deny, got {:?}", other),
    }
}

#[test]
fn test_fixture_pre_tool_use_exit_0_modified_input() {
    let dir = TempDir::new().expect("tempdir");
    let script = write_hook_script(
        &dir,
        "modify.sh",
        "#!/bin/sh\necho '{\"modified_input\":{\"path\":\"src/lib.rs\"}}'\nexit 0\n",
    );

    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {script}"),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "read",
        r#"{"path":"src/main.rs"}"#,
        "sess-fixture-003",
        Some(&bus),
    );

    match result {
        PreToolUseResult::ModifiedInput { input } => {
            assert_eq!(
                input.get("path").and_then(|v| v.as_str()),
                Some("src/lib.rs")
            );
        }
        other => panic!("expected ModifiedInput, got {:?}", other),
    }
}

#[test]
fn test_fixture_pre_tool_use_exit_1_warns_and_allows() {
    let dir = TempDir::new().expect("tempdir");
    let script = write_hook_script(
        &dir,
        "warn.sh",
        "#!/bin/sh\necho 'suspicious tool call' >&2\nexit 1\n",
    );

    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {script}"),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "bash",
        r#"{"command":"curl http://example.com"}"#,
        "sess-fixture-004",
        Some(&bus),
    );

    assert!(
        matches!(result, PreToolUseResult::NoDecision),
        "expected NoDecision (fall through), got {:?}",
        result
    );

    let event = rx
        .try_recv()
        .expect("HookWarning event should be published");
    match event {
        ragent_types::event::Event::HookWarning {
            session_id,
            tool,
            stderr,
            ..
        } => {
            assert_eq!(session_id, "sess-fixture-004");
            assert_eq!(tool, "bash");
            assert_eq!(stderr, "suspicious tool call");
        }
        other => panic!("expected HookWarning, got {:?}", other),
    }
}

#[test]
fn test_fixture_pre_tool_use_exit_2_blocks() {
    let dir = TempDir::new().expect("tempdir");
    // Exit 2 with stdout allow — stdout JSON must be ignored (FR-003).
    let script = write_hook_script(
        &dir,
        "block.sh",
        "#!/bin/sh\necho '{\"decision\":\"allow\"}'\necho 'blocked by security policy' >&2\nexit 2\n",
    );

    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {script}"),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "write",
        r#"{"path":"/etc/shadow"}"#,
        "sess-fixture-005",
        Some(&bus),
    );

    match result {
        PreToolUseResult::Blocked { reason } => {
            assert_eq!(reason, "blocked by security policy");
        }
        other => panic!("expected Blocked, got {:?}", other),
    }

    // No HookWarning should be published for exit code 2.
    assert!(rx.try_recv().is_err());
}

#[test]
fn test_fixture_pre_tool_use_exit_3_falls_through() {
    let dir = TempDir::new().expect("tempdir");
    let script = write_hook_script(
        &dir,
        "error.sh",
        "#!/bin/sh\necho 'hook crashed' >&2\nexit 3\n",
    );

    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {script}"),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "read",
        r#"{"path":"/tmp/foo"}"#,
        "sess-fixture-006",
        Some(&bus),
    );

    assert!(
        matches!(result, PreToolUseResult::NoDecision),
        "expected NoDecision (fall through), got {:?}",
        result
    );

    // No event should be published for exit code >=3.
    assert!(rx.try_recv().is_err());
}

#[test]
fn test_fixture_pre_tool_use_exit_2_takes_precedence_over_allow_in_same_stdout() {
    let dir = TempDir::new().expect("tempdir");
    // The hook writes allow JSON to stdout but exits with code 2.
    // FR-003: exit code 2 takes absolute precedence over stdout JSON.
    let script = write_hook_script(
        &dir,
        "block_with_allow.sh",
        "#!/bin/sh\necho '{\"decision\":\"allow\",\"reason\":\"should be ignored\"}'\nexit 2\n",
    );

    let bus = EventBus::default();
    let hooks = vec![HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: format!("sh {script}"),
        timeout_secs: 30,
    }];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "write",
        r#"{"path":"/etc/shadow"}"#,
        "sess-fixture-007",
        Some(&bus),
    );

    match result {
        PreToolUseResult::Blocked { reason } => {
            // stderr was empty, so reason should be empty.
            assert!(reason.is_empty());
        }
        other => panic!("expected Blocked (exit 2 precedence), got {:?}", other),
    }
}

#[test]
fn test_fixture_pre_tool_use_multiple_hooks_first_blocks() {
    let dir = TempDir::new().expect("tempdir");
    let allow_script = write_hook_script(
        &dir,
        "allow.sh",
        "#!/bin/sh\necho '{\"decision\":\"allow\"}'\nexit 0\n",
    );
    let block_script =
        write_hook_script(&dir, "block.sh", "#!/bin/sh\necho 'blocked' >&2\nexit 2\n");

    let bus = EventBus::default();
    let hooks = vec![
        HookConfig {
            trigger: HookTrigger::PreToolUse,
            command: format!("sh {block_script}"),
            timeout_secs: 30,
        },
        HookConfig {
            trigger: HookTrigger::PreToolUse,
            command: format!("sh {allow_script}"),
            timeout_secs: 30,
        },
    ];

    let result = run_pre_tool_use_hooks(
        &hooks,
        dir.path(),
        "write",
        r#"{"path":"/etc/shadow"}"#,
        "sess-fixture-008",
        Some(&bus),
    );

    // The blocking hook runs first and returns immediately; the allow hook
    // is never reached.
    match result {
        PreToolUseResult::Blocked { reason } => {
            assert_eq!(reason, "blocked");
        }
        other => panic!("expected Blocked, got {:?}", other),
    }
}
