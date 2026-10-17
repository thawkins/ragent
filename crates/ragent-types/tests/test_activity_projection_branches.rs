//! Branch coverage for [`Projection`]: `apply` over every `EventKind`, the
//! derived-status mapping, the accessor helpers, and log-consistency validation
//! (maka FR-003, FR-011, FR-012, FR-013).
//!
//! The existing `test_activity_projection.rs` exercises only
//! `pruned_tool_results`; this file drives every remaining branch of the
//! projection state machine.

use ragent_types::activity::{
    ActivityEvent, BoundaryTarget, ConsistencyError, EventKind, LIFECYCLE_RESUMED, Principal,
    Projection, RunStatus, TerminationReason, validate_event_log_consistency,
};
use ragent_types::id::RunId;

fn ev(seq: u64, kind: EventKind) -> ActivityEvent {
    ActivityEvent::new(RunId::from("run-1"), seq, kind)
}

#[test]
fn derived_status_maps_completed_interrupted_and_aborted() {
    assert_eq!(
        TerminationReason::Completed.derived_status(),
        RunStatus::Completed
    );
    assert_eq!(
        TerminationReason::Interrupted.derived_status(),
        RunStatus::Interrupted
    );
    assert_eq!(
        TerminationReason::Aborted.derived_status(),
        RunStatus::Interrupted
    );
}

#[test]
fn event_kind_discriminators_are_the_serde_tags() {
    assert_eq!(EventKind::tool_call_discriminator(), "tool_call");
    assert_eq!(EventKind::tool_result_discriminator(), "tool_result");
    assert_eq!(EventKind::checkpoint_discriminator(), "checkpoint");
    assert_eq!(EventKind::termination_discriminator(), "termination");
    assert_eq!(EventKind::lifecycle_resumed_discriminator(), "lifecycle");

    // Each discriminator must match the actual `kind` tag serde emits.
    let tag_of = |kind: &EventKind| {
        serde_json::to_value(kind)
            .expect("serialize")
            .get("kind")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    assert_eq!(
        tag_of(&EventKind::ToolCall {
            tool_call_id: "c".into(),
            tool: "read".into(),
            args: "{}".into(),
        }),
        Some(EventKind::tool_call_discriminator().to_string())
    );
    assert_eq!(
        tag_of(&EventKind::ToolResult {
            tool_call_id: "c".into(),
            tool: "read".into(),
            success: true,
            content: "x".into(),
        }),
        Some(EventKind::tool_result_discriminator().to_string())
    );
    assert_eq!(
        tag_of(&EventKind::Checkpoint {
            name: "cp".into(),
            seq: 1,
        }),
        Some(EventKind::checkpoint_discriminator().to_string())
    );
    assert_eq!(
        tag_of(&EventKind::Termination {
            reason: TerminationReason::Completed,
            seq: 1,
        }),
        Some(EventKind::termination_discriminator().to_string())
    );
    assert_eq!(
        tag_of(&EventKind::Lifecycle {
            event: LIFECYCLE_RESUMED.into(),
        }),
        Some(EventKind::lifecycle_resumed_discriminator().to_string())
    );
}

#[test]
fn apply_project_accumulates_every_kind() {
    let seq = std::cell::Cell::new(0u64);
    let next = || {
        let s = seq.get();
        seq.set(s + 1);
        s
    };
    let events = vec![
        ev(
            next(),
            EventKind::ModelMessage {
                role: "user".into(),
                content: "hello".into(),
                message_id: Some("m-1".into()),
            },
        ),
        ev(
            next(),
            EventKind::ToolCall {
                tool_call_id: "c-1".into(),
                tool: "read".into(),
                args: r#"{"path":"a"}"#.into(),
            },
        ),
        ev(
            next(),
            EventKind::ToolResult {
                tool_call_id: "c-1".into(),
                tool: "read".into(),
                success: false,
                content: "boom".into(),
            },
        ),
        ev(
            next(),
            EventKind::PermissionDecision {
                tool: "bash".into(),
                principal: Principal::Policy,
                boundary: BoundaryTarget::Other("custom".into()),
                granted: false,
            },
        ),
        ev(
            next(),
            EventKind::Checkpoint {
                name: "cp-a".into(),
                seq: 5,
            },
        ),
        // Audit-only variants must not affect the projection.
        ev(
            next(),
            EventKind::BranchOrigin {
                source_run_id: RunId::from("run-0"),
                source_seq: 3,
            },
        ),
        ev(
            next(),
            EventKind::MutationRejected {
                target_seq: 2,
                attempted: "edit".into(),
            },
        ),
        ev(
            next(),
            EventKind::Lifecycle {
                event: "retention".into(),
            },
        ),
    ];
    let proj = Projection::replay(&events);

    assert_eq!(proj.last_seq, 7);
    assert_eq!(proj.messages.len(), 1);
    assert_eq!(proj.messages[0].role, "user");
    assert_eq!(proj.messages[0].content, "hello");
    assert_eq!(proj.messages[0].message_id.as_deref(), Some("m-1"));
    assert_eq!(proj.tool_calls.len(), 1);
    assert_eq!(proj.tool_calls[0].tool_call_id, "c-1");
    assert_eq!(proj.tool_results.len(), 1);
    assert!(!proj.tool_results[0].success);
    assert_eq!(proj.permissions.len(), 1);
    assert_eq!(proj.permissions[0].principal, Principal::Policy);
    assert_eq!(
        proj.permissions[0].boundary,
        BoundaryTarget::Other("custom".into())
    );
    assert!(!proj.permissions[0].granted);
    assert_eq!(proj.checkpoints.len(), 1);
    // The audit-only kinds left no trace and no termination.
    assert!(proj.termination.is_none());
    assert_eq!(proj.status, RunStatus::Active);

    // `model_messages` returns the accumulated messages slice.
    assert_eq!(proj.model_messages().len(), 1);
}

#[test]
fn termination_sets_status_and_derives_it() {
    let events = [
        ev(0, EventKind::Lifecycle { event: "x".into() }),
        ev(
            1,
            EventKind::Termination {
                reason: TerminationReason::Aborted,
                seq: 1,
            },
        ),
    ];
    let proj = Projection::replay(&events);
    assert_eq!(proj.termination, Some((TerminationReason::Aborted, 1)));
    assert_eq!(proj.status, RunStatus::Interrupted);
    assert!(proj.is_resumable());
    assert_eq!(proj.resume_from_seq(), 2);
}

#[test]
fn completed_run_is_not_resumable() {
    let events = [ev(
        0,
        EventKind::Termination {
            reason: TerminationReason::Completed,
            seq: 0,
        },
    )];
    let proj = Projection::replay(&events);
    assert_eq!(proj.status, RunStatus::Completed);
    assert!(!proj.is_resumable());
}

#[test]
fn lifecycle_resumed_transitions_back_to_active() {
    let events = [
        ev(
            0,
            EventKind::Termination {
                reason: TerminationReason::Interrupted,
                seq: 0,
            },
        ),
        ev(
            1,
            EventKind::Lifecycle {
                event: LIFECYCLE_RESUMED.into(),
            },
        ),
    ];
    let proj = Projection::replay(&events);
    assert_eq!(proj.status, RunStatus::Active);
    assert!(!proj.is_resumable());
    assert_eq!(proj.resume_from_seq(), 2);
}

#[test]
fn replay_upto_stops_after_the_target_sequence() {
    let events = [
        ev(0, EventKind::Lifecycle { event: "a".into() }),
        ev(
            1,
            EventKind::ModelMessage {
                role: "user".into(),
                content: "first".into(),
                message_id: None,
            },
        ),
        ev(
            2,
            EventKind::ModelMessage {
                role: "assistant".into(),
                content: "second".into(),
                message_id: None,
            },
        ),
    ];
    let proj = Projection::replay_upto(&events, 1);
    assert_eq!(proj.last_seq, 1);
    assert_eq!(proj.messages.len(), 1);
    assert_eq!(proj.messages[0].content, "first");
}

#[test]
fn pending_tool_calls_excludes_answered_calls() {
    let events = [
        ev(
            0,
            EventKind::ToolCall {
                tool_call_id: "answered".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(
            1,
            EventKind::ToolCall {
                tool_call_id: "in-flight".into(),
                tool: "bash".into(),
                args: "{}".into(),
            },
        ),
        ev(
            2,
            EventKind::ToolResult {
                tool_call_id: "answered".into(),
                tool: "read".into(),
                success: true,
                content: "ok".into(),
            },
        ),
    ];
    let proj = Projection::replay(&events);
    let pending = proj.pending_tool_calls();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool_call_id, "in-flight");
}

#[test]
fn checkpoint_seq_returns_the_most_recent_duplicate() {
    let events = [
        ev(
            0,
            EventKind::Checkpoint {
                name: "cp".into(),
                seq: 0,
            },
        ),
        ev(
            1,
            EventKind::Checkpoint {
                name: "cp".into(),
                seq: 1,
            },
        ),
        ev(
            2,
            EventKind::Checkpoint {
                name: "other".into(),
                seq: 2,
            },
        ),
    ];
    let proj = Projection::replay(&events);
    assert_eq!(proj.checkpoint_seq("cp"), Some(1));
    assert_eq!(proj.checkpoint_seq("other"), Some(2));
    assert_eq!(proj.checkpoint_seq("missing"), None);
}

#[test]
fn empty_and_default_projection_match() {
    let empty = Projection::empty();
    let default = Projection::default();
    assert_eq!(empty, default);
    assert_eq!(empty.last_seq, 0);
    assert_eq!(empty.status, RunStatus::Active);
    assert_eq!(empty.messages.len(), 0);
    assert!(empty.termination.is_none());
}

#[test]
fn validate_accepts_a_consistent_log() {
    let events = [
        ev(
            0,
            EventKind::ToolCall {
                tool_call_id: "c-1".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(
            1,
            EventKind::ToolResult {
                tool_call_id: "c-1".into(),
                tool: "read".into(),
                success: true,
                content: "ok".into(),
            },
        ),
    ];
    assert_eq!(validate_event_log_consistency(&events), Ok(()));
}

#[test]
fn validate_reports_the_first_sequence_gap() {
    let events = [
        ev(0, EventKind::Lifecycle { event: "a".into() }),
        ev(2, EventKind::Lifecycle { event: "b".into() }),
    ];
    assert_eq!(
        validate_event_log_consistency(&events),
        Err(ConsistencyError::SeqGap {
            expected: 1,
            found: 2
        })
    );
}

#[test]
fn validate_reports_an_orphaned_tool_result() {
    let events = [ev(
        0,
        EventKind::ToolResult {
            tool_call_id: "ghost".into(),
            tool: "read".into(),
            success: true,
            content: "no call".into(),
        },
    )];
    assert_eq!(
        validate_event_log_consistency(&events),
        Err(ConsistencyError::OrphanedToolResult {
            tool_call_id: "ghost".into()
        })
    );
}

#[test]
fn consistency_error_messages_name_the_problem() {
    let gap = ConsistencyError::SeqGap {
        expected: 4,
        found: 9,
    };
    assert!(gap.to_string().contains("expected 4"));
    assert!(gap.to_string().contains("found 9"));
    let orphan = ConsistencyError::OrphanedToolResult {
        tool_call_id: "c-9".into(),
    };
    assert!(orphan.to_string().contains("c-9"));
}

#[test]
fn run_status_serializes_every_variant() {
    for (status, label) in [
        (RunStatus::Active, "active"),
        (RunStatus::Interrupted, "interrupted"),
        (RunStatus::Completed, "completed"),
        (RunStatus::Unrecoverable, "unrecoverable"),
        (RunStatus::Rebuilding, "rebuilding"),
        (RunStatus::RolledBack, "rolled_back"),
    ] {
        assert_eq!(
            serde_json::to_string(&status).expect("serialize"),
            format!("\"{label}\"")
        );
    }
}

#[test]
fn termination_reason_roundtrips_snake_case() {
    for (reason, label) in [
        (TerminationReason::Completed, "completed"),
        (TerminationReason::Interrupted, "interrupted"),
        (TerminationReason::Aborted, "aborted"),
    ] {
        let json = serde_json::to_string(&reason).expect("serialize");
        assert_eq!(json, format!("\"{label}\""));
        let back: TerminationReason = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, reason);
    }
}

#[test]
fn boundary_target_other_roundtrips_its_payload() {
    let boundary = BoundaryTarget::Other("named-edge".into());
    let json = serde_json::to_string(&boundary).expect("serialize");
    let back: BoundaryTarget = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, boundary);
    // Every non-payload variant is a bare snake_case string.
    for (target, label) in [
        (BoundaryTarget::FileSystem, "file_system"),
        (BoundaryTarget::Shell, "shell"),
        (BoundaryTarget::Network, "network"),
        (BoundaryTarget::Mcp, "mcp"),
    ] {
        assert_eq!(
            serde_json::to_string(&target).expect("serialize"),
            format!("\"{label}\"")
        );
    }
}
