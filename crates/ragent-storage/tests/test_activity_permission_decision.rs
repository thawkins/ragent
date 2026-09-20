//! Tests for recording permission-decision events (maka spec T-006, FR-005).
//!
//! FR-005: "When a permission decision is made (grant or deny) for a tool that
//! crosses a sandbox boundary, the system shall record the decision, the
//! principal, the tool, and the boundary-crossing target as an event."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{BoundaryTarget, EventKind, Principal};
use ragent_types::id::RunId;

#[test]
fn record_permission_decision_persists_grant() {
    // FR-005: a granted decision records the tool, principal, boundary, and
    // granted=true.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_permission_decision(
            &run,
            "bash",
            Principal::Operator,
            BoundaryTarget::Shell,
            true,
        )
        .expect("record");
    assert_eq!(event.seq, 0);
    match &event.kind {
        EventKind::PermissionDecision {
            tool,
            principal,
            boundary,
            granted,
        } => {
            assert_eq!(tool, "bash");
            assert_eq!(*principal, Principal::Operator);
            assert_eq!(*boundary, BoundaryTarget::Shell);
            assert!(*granted);
        }
        other => panic!("expected PermissionDecision, got {other:?}"),
    }
}

#[test]
fn record_permission_decision_persists_deny() {
    // FR-005: a denied decision records granted=false.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_permission_decision(
            &run,
            "write",
            Principal::Operator,
            BoundaryTarget::FileSystem,
            false,
        )
        .expect("record");
    match &event.kind {
        EventKind::PermissionDecision { granted, .. } => assert!(!*granted),
        other => panic!("expected PermissionDecision, got {other:?}"),
    }
}

#[test]
fn record_permission_decision_policy_principal() {
    // FR-005: the policy engine principal is recorded.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_permission_decision(
            &run,
            "http_request",
            Principal::Policy,
            BoundaryTarget::Network,
            true,
        )
        .expect("record");
    match &event.kind {
        EventKind::PermissionDecision { principal, .. } => {
            assert_eq!(*principal, Principal::Policy);
        }
        other => panic!("expected PermissionDecision, got {other:?}"),
    }
}

#[test]
fn record_permission_decision_other_boundary() {
    // FR-005: a named boundary not covered by the fixed variants is recorded.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_permission_decision(
            &run,
            "mcp_tool",
            Principal::Operator,
            BoundaryTarget::Mcp,
            true,
        )
        .expect("record");
    match &event.kind {
        EventKind::PermissionDecision { boundary, .. } => {
            assert_eq!(*boundary, BoundaryTarget::Mcp);
        }
        other => panic!("expected PermissionDecision, got {other:?}"),
    }
}

#[test]
fn recorded_permission_decision_is_durable_and_replayable() {
    // FR-001 + FR-005: the decision is persisted and survives a read-back.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let appended = log
        .record_permission_decision(
            &run,
            "bash",
            Principal::Operator,
            BoundaryTarget::Shell,
            true,
        )
        .expect("record");
    let read = log
        .get_event(&run, appended.seq)
        .expect("read")
        .expect("exists");
    assert_eq!(read, appended);
}

#[test]
fn multiple_permission_decisions_get_monotonic_sequence_numbers() {
    // FR-002 + FR-005: each decision is a separate ordered event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let d0 = log
        .record_permission_decision(
            &run,
            "bash",
            Principal::Operator,
            BoundaryTarget::Shell,
            true,
        )
        .expect("record");
    let d1 = log
        .record_permission_decision(
            &run,
            "write",
            Principal::Operator,
            BoundaryTarget::FileSystem,
            false,
        )
        .expect("record");
    assert_eq!(d0.seq, 0);
    assert_eq!(d1.seq, 1);
    assert_eq!(log.last_seq(&run).unwrap(), Some(1));
}

#[test]
fn permission_decision_interleaves_with_tool_call() {
    // FR-005: a permission decision precedes the tool call it gates, all
    // ordered in the log.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let perm = log
        .record_permission_decision(
            &run,
            "bash",
            Principal::Operator,
            BoundaryTarget::Shell,
            true,
        )
        .expect("perm");
    let call = log
        .record_tool_call(&run, "c1", "bash", r#"{"command":"ls"}"#)
        .expect("call");
    let result = log
        .record_tool_result(&run, "c1", "bash", true, "file.txt")
        .expect("result");
    assert!(perm.seq < call.seq);
    assert!(call.seq < result.seq);
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 3);
    assert!(matches!(
        events[0].kind,
        EventKind::PermissionDecision { .. }
    ));
    assert!(matches!(events[1].kind, EventKind::ToolCall { .. }));
    assert!(matches!(events[2].kind, EventKind::ToolResult { .. }));
}

#[test]
fn recorded_permission_decision_has_nonempty_event_id() {
    // FR-002: the recorded event carries a fresh immutable id.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_permission_decision(
            &run,
            "bash",
            Principal::Operator,
            BoundaryTarget::Shell,
            true,
        )
        .expect("record");
    assert!(!event.id.as_str().is_empty());
}
