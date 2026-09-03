#![allow(clippy::assert_is_empty)]
//! Tests for recording tool-call and tool-result events (maka spec T-005,
//! FR-004).
//!
//! FR-004: "When a tool call completes, the system shall record both the tool
//! invocation and its result as ordered events, linked by a shared tool-call
//! identifier, before the next model invocation reads the result."
//!
//! NOTE: This file was accidentally overwritten during T-006 and is restored
//! here so the T-005 record_tool_call / record_tool_result /
//! find_tool_call_pair methods retain their dedicated test coverage.

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::EventKind;
use ragent_types::id::RunId;

#[test]
fn record_tool_call_persists_invocation() {
    // FR-004: the tool invocation is recorded with its tool, args, and the
    // shared tool-call id.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_tool_call(&run, "call-1", "read", r#"{"path":"README.md"}"#)
        .expect("record");
    assert_eq!(event.seq, 0);
    match &event.kind {
        EventKind::ToolCall {
            tool_call_id,
            tool,
            args,
        } => {
            assert_eq!(tool_call_id, "call-1");
            assert_eq!(tool, "read");
            assert_eq!(args, r#"{"path":"README.md"}"#);
        }
        other => panic!("expected ToolCall, got {other:?}"),
    }
}

#[test]
fn record_tool_result_persists_completion() {
    // FR-004: the tool result is recorded with success, content, and the
    // shared tool-call id.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_tool_call(&run, "call-1", "read", r#"{"path":"README.md"}"#)
        .expect("call");
    let result = log
        .record_tool_result(&run, "call-1", "read", true, "# ragent")
        .expect("result");
    match &result.kind {
        EventKind::ToolResult {
            tool_call_id,
            tool,
            success,
            content,
        } => {
            assert_eq!(tool_call_id, "call-1");
            assert_eq!(tool, "read");
            assert!(*success);
            assert_eq!(content, "# ragent");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[test]
fn tool_call_and_result_are_ordered_events() {
    // FR-004: the invocation and result are ordered events (invocation first).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let call = log
        .record_tool_call(&run, "call-7", "read", r#"{"path":"a"}"#)
        .expect("call");
    let result = log
        .record_tool_result(&run, "call-7", "read", true, "ok")
        .expect("result");
    assert!(call.seq < result.seq, "invocation precedes result");
}

#[test]
fn tool_call_and_result_are_linked_by_shared_id() {
    // FR-004: the two events are linked by a shared tool-call identifier.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_tool_call(&run, "link-1", "read", "{}")
        .expect("call");
    log.record_tool_result(&run, "link-1", "read", true, "content")
        .expect("result");

    let (call, result) = log.find_tool_call_pair(&run, "link-1").expect("find");
    let call = call.expect("call exists");
    let result = result.expect("result exists");
    assert!(call.seq < result.seq);
    let call_id = match &call.kind {
        EventKind::ToolCall { tool_call_id, .. } => tool_call_id.clone(),
        _ => unreachable!(),
    };
    let result_id = match &result.kind {
        EventKind::ToolResult { tool_call_id, .. } => tool_call_id.clone(),
        _ => unreachable!(),
    };
    assert_eq!(call_id, result_id, "linked by shared tool_call_id");
}

#[test]
fn find_tool_call_pair_returns_call_only_before_result() {
    // FR-004: before the result is recorded, only the invocation is found.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_tool_call(&run, "half", "read", "{}")
        .expect("call");
    let (call, result) = log.find_tool_call_pair(&run, "half").expect("find");
    assert!(call.is_some());
    assert!(result.is_none());
}

#[test]
fn find_tool_call_pair_returns_none_none_for_unknown_id() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let (call, result) = log.find_tool_call_pair(&run, "nope").expect("find");
    assert!(call.is_none());
    assert!(result.is_none());
}

#[test]
fn tool_result_records_failure_with_error_content() {
    // FR-004: a failed tool call is recorded with success=false and the error
    // as content.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_tool_call(&run, "fail-1", "read", r#"{"path":"missing"}"#)
        .expect("call");
    let result = log
        .record_tool_result(&run, "fail-1", "read", false, "file not found")
        .expect("result");
    match &result.kind {
        EventKind::ToolResult {
            success, content, ..
        } => {
            assert!(!*success);
            assert_eq!(content, "file not found");
        }
        other => panic!("expected ToolResult, got {other:?}"),
    }
}

#[test]
fn multiple_tool_calls_each_get_distinct_sequence_numbers() {
    // FR-004 + FR-002: multiple tool calls in a turn are separate ordered
    // events.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let c1 = log
        .record_tool_call(&run, "c1", "read", "{}")
        .expect("call");
    let r1 = log
        .record_tool_result(&run, "c1", "read", true, "a")
        .expect("result");
    let c2 = log
        .record_tool_call(&run, "c2", "read", "{}")
        .expect("call");
    let r2 = log
        .record_tool_result(&run, "c2", "read", true, "b")
        .expect("result");
    assert_eq!(c1.seq, 0);
    assert_eq!(r1.seq, 1);
    assert_eq!(c2.seq, 2);
    assert_eq!(r2.seq, 3);

    // Each pair resolves independently.
    let (call1, res1) = log.find_tool_call_pair(&run, "c1").expect("find");
    let (call2, res2) = log.find_tool_call_pair(&run, "c2").expect("find");
    assert_eq!(call1.unwrap().seq, 0);
    assert_eq!(res1.unwrap().seq, 1);
    assert_eq!(call2.unwrap().seq, 2);
    assert_eq!(res2.unwrap().seq, 3);
}

#[test]
fn tool_events_interleave_correctly_with_model_messages() {
    // FR-004: a turn is model-message -> tool-call -> tool-result, all ordered.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let m0 = log
        .record_model_message(&run, "assistant", "I'll read README.md", None)
        .expect("msg");
    let c1 = log
        .record_tool_call(&run, "c1", "read", r#"{"path":"README.md"}"#)
        .expect("call");
    let r1 = log
        .record_tool_result(&run, "c1", "read", true, "# ragent")
        .expect("result");
    let m1 = log
        .record_model_message(&run, "assistant", "Done.", None)
        .expect("msg");
    assert!(m0.seq < c1.seq);
    assert!(c1.seq < r1.seq);
    assert!(r1.seq < m1.seq);
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 4);
}
