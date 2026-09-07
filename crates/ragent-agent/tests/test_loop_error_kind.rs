//! Integration tests for `ErrorKind::{Recoverable, Unrecoverable}`
//! classification (spec `agentloop`, task T-006; FR-011, FR-012):
//! stage mapping, message sniffing, and the tool-level recoverable default.

use ragent_agent::error::{ErrorKind, LoopStage, classify_message, classify_stage};

// --- Stage mapping (FR-011 / FR-012) ----------------------------------------

#[test]
fn test_classify_stage_transport_failure_is_unrecoverable() {
    assert_eq!(
        classify_stage(LoopStage::LlmTransport),
        ErrorKind::Unrecoverable
    );
}

#[test]
fn test_classify_stage_permission_hard_deny_is_unrecoverable() {
    assert_eq!(
        classify_stage(LoopStage::PermissionHardDeny),
        ErrorKind::Unrecoverable
    );
}

#[test]
fn test_classify_stage_tool_panic_is_unrecoverable() {
    assert_eq!(
        classify_stage(LoopStage::ToolPanic),
        ErrorKind::Unrecoverable
    );
}

#[test]
fn test_classify_stage_context_overflow_is_unrecoverable() {
    assert_eq!(
        classify_stage(LoopStage::ContextOverflow),
        ErrorKind::Unrecoverable
    );
}

#[test]
fn test_classify_stage_tool_failure_is_recoverable_by_default() {
    assert_eq!(
        classify_stage(LoopStage::ToolFailure),
        ErrorKind::Recoverable
    );
}

// --- Message sniffing --------------------------------------------------------

#[test]
fn test_classify_message_context_overflow_markers_are_unrecoverable() {
    for message in [
        "prompt token count 210000 exceeds the maximum of 200000",
        "context_length_exceeded",
        "maximum context length is 200000 tokens",
        "prompt is too long: 210000 tokens > 200000 maximum",
        "input too large",
    ] {
        assert_eq!(
            classify_message(message),
            ErrorKind::Unrecoverable,
            "{message}"
        );
    }
}

#[test]
fn test_classify_message_permission_hard_deny_is_unrecoverable() {
    for message in [
        "Permission denied by user or policy",
        "permission denied for one or more sub-commands",
        "Permission denied by hook: write-to-tests",
        "Blocked by hook: destructive git command",
    ] {
        assert_eq!(
            classify_message(message),
            ErrorKind::Unrecoverable,
            "{message}"
        );
    }
}

#[test]
fn test_classify_message_panic_and_watchdog_are_unrecoverable() {
    for message in [
        "tool task panicked: attempt to subtract with overflow",
        "agent run terminated: tool call stalled beyond the 120s watchdog timeout",
    ] {
        assert_eq!(
            classify_message(message),
            ErrorKind::Unrecoverable,
            "{message}"
        );
    }
}

#[test]
fn test_classify_message_plain_tool_failure_defaults_recoverable() {
    for message in [
        "file not found: src/main.rs",
        "git command exited with status 1",
        "json parse error in tool arguments",
        "",
    ] {
        assert_eq!(
            classify_message(message),
            ErrorKind::Recoverable,
            "{message}"
        );
    }
}

// --- Predicates and labels ----------------------------------------------------

#[test]
fn test_error_kind_predicates_and_display_are_consistent() {
    assert!(ErrorKind::Recoverable.is_recoverable());
    assert!(!ErrorKind::Recoverable.is_unrecoverable());
    assert_eq!(ErrorKind::Recoverable.to_string(), "recoverable");
    assert!(ErrorKind::Unrecoverable.is_unrecoverable());
    assert!(!ErrorKind::Unrecoverable.is_recoverable());
    assert_eq!(ErrorKind::Unrecoverable.to_string(), "unrecoverable");
}

#[test]
fn test_classify_stage_and_message_agree_on_overflow_markers() {
    // The message sniffer must agree with the static stage mapping for the
    // FR-011 marker set, so both classification paths terminate the loop.
    assert_eq!(
        classify_stage(LoopStage::ContextOverflow),
        classify_message("context_length_exceeded")
    );
    assert_eq!(
        classify_stage(LoopStage::PermissionHardDeny),
        classify_message("Permission denied by user or policy")
    );
}
