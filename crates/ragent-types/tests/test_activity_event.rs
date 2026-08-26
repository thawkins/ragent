//! Tests for the activity-log event schema (maka spec T-001).
//!
//! Covers FR-001 (every execution fact is a recorded event), FR-002 (each
//! event has a monotonic sequence number and immutable id), and NFR-003
//! (events are self-describing: type, schema version, and run id travel
//! with every record).

#![forbid(unsafe_code)]

use ragent_types::activity::{
    ACTIVITY_EVENT_SCHEMA_VERSION, ActivityEvent, BoundaryTarget, EventKind, Principal, RunStatus,
    TerminationReason,
};
use ragent_types::id::{EventId, RunId};

#[test]
fn event_roundtrips_through_serde() {
    let event = ActivityEvent::new(
        RunId::from("run-1"),
        3,
        EventKind::ToolCall {
            tool_call_id: "call-7".to_string(),
            tool: "read".to_string(),
            args: r#"{"path":"README.md"}"#.to_string(),
        },
    );
    let json = serde_json::to_string(&event).expect("serialize");
    let back: ActivityEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(event, back);
}

#[test]
fn event_is_self_describing() {
    let event = ActivityEvent::new(
        RunId::from("run-1"),
        1,
        EventKind::Termination {
            reason: TerminationReason::Completed,
            seq: 1,
        },
    );
    // NFR-003: type, schema version, and run id all travel with the event.
    let json = serde_json::to_value(&event).expect("serialize");
    assert!(json.get("id").is_some(), "event carries its id");
    assert!(json.get("run_id").is_some(), "event carries its run id");
    assert!(
        json.get("schema_version").is_some(),
        "event carries schema version"
    );
    assert!(
        json.get("kind").is_some(),
        "event carries its kind discriminator"
    );
    assert_eq!(
        json.get("schema_version").and_then(|v| v.as_u64()),
        Some(u64::from(ACTIVITY_EVENT_SCHEMA_VERSION))
    );
}

#[test]
fn event_kind_discriminates_via_tag() {
    let json = serde_json::to_value(EventKind::Checkpoint {
        name: "after-turn-1".to_string(),
        seq: 4,
    })
    .expect("serialize");
    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("checkpoint")
    );
}

#[test]
fn run_status_serializes_snake_case() {
    assert_eq!(
        serde_json::to_string(&RunStatus::Interrupted).expect("serialize"),
        "\"interrupted\""
    );
    assert_eq!(
        serde_json::to_string(&RunStatus::Completed).expect("serialize"),
        "\"completed\""
    );
}

#[test]
fn tool_call_and_result_share_tool_call_id() {
    // FR-004: a tool call and its result are linked by a shared tool-call id.
    let call = ActivityEvent::new(
        RunId::from("run-1"),
        1,
        EventKind::ToolCall {
            tool_call_id: "call-9".to_string(),
            tool: "read".to_string(),
            args: r#"{"path":"README.md"}"#.to_string(),
        },
    );
    let result = ActivityEvent::new(
        RunId::from("run-1"),
        2,
        EventKind::ToolResult {
            tool_call_id: "call-9".to_string(),
            tool: "read".to_string(),
            success: true,
            content: "# ragent".to_string(),
        },
    );
    match (&call.kind, &result.kind) {
        (
            EventKind::ToolCall {
                tool_call_id: c, ..
            },
            EventKind::ToolResult {
                tool_call_id: r, ..
            },
        ) => {
            assert_eq!(c, r, "tool call and result share a tool_call_id");
        }
        _ => panic!("expected ToolCall then ToolResult"),
    }
}

#[test]
fn permission_decision_carries_principal_and_boundary() {
    // FR-005: a permission decision records the principal, the tool, and the
    // boundary-crossing target.
    let event = ActivityEvent::new(
        RunId::from("run-1"),
        5,
        EventKind::PermissionDecision {
            tool: "bash".to_string(),
            principal: Principal::Operator,
            boundary: BoundaryTarget::Shell,
            granted: true,
        },
    );
    let json = serde_json::to_value(&event.kind).expect("serialize");
    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("permission_decision")
    );
    assert_eq!(json.get("granted").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn event_id_is_immutable_unique_per_event() {
    // FR-002: each event has an immutable event identifier. Two events with
    // the same payload must still carry distinct ids.
    let a = ActivityEvent::new(
        RunId::from("run-1"),
        1,
        EventKind::Lifecycle { event: "x".into() },
    );
    let b = ActivityEvent::new(
        RunId::from("run-1"),
        2,
        EventKind::Lifecycle { event: "x".into() },
    );
    assert_ne!(a.id, b.id);
    assert_ne!(a.seq, b.seq);
}

#[test]
fn branch_origin_event_carries_source() {
    // FR-018: a branch records its origin in both runs.
    let event = ActivityEvent::new(
        RunId::from("run-2"),
        0,
        EventKind::BranchOrigin {
            source_run_id: RunId::from("run-1"),
            source_seq: 4,
        },
    );
    let json = serde_json::to_value(&event.kind).expect("serialize");
    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("branch_origin")
    );
    assert_eq!(json.get("source_seq").and_then(|v| v.as_u64()), Some(4));
}

#[test]
fn mutation_rejected_event_carries_target() {
    // FR-010: a rejected mutation records the target sequence number.
    let event = ActivityEvent::new(
        RunId::from("run-1"),
        9,
        EventKind::MutationRejected {
            target_seq: 3,
            attempted: "edit committed event".to_string(),
        },
    );
    let json = serde_json::to_value(&event.kind).expect("serialize");
    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("mutation_rejected")
    );
    assert_eq!(json.get("target_seq").and_then(|v| v.as_u64()), Some(3));
}

#[test]
fn event_id_round_trips_as_string() {
    let id = EventId::new();
    let s = serde_json::to_string(&id).expect("serialize");
    let back: EventId = serde_json::from_str(&s).expect("deserialize");
    assert_eq!(id, back);
}
