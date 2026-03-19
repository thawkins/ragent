use ragent_core::event::{Event, EventBus, FinishReason};

// ── All event variants serialize/deserialize ─────────────────────

#[test]
fn test_event_serialization_roundtrip_all_variants() {
    let events = vec![
        Event::SessionCreated {
            session_id: "s1".into(),
        },
        Event::SessionUpdated {
            session_id: "s1".into(),
        },
        Event::MessageStart {
            session_id: "s1".into(),
            message_id: "m1".into(),
        },
        Event::TextDelta {
            session_id: "s1".into(),
            text: "hello".into(),
        },
        Event::ReasoningDelta {
            session_id: "s1".into(),
            text: "thinking...".into(),
        },
        Event::ToolCallStart {
            session_id: "s1".into(),
            call_id: "c1".into(),
            tool: "read".into(),
        },
        Event::ToolCallEnd {
            session_id: "s1".into(),
            call_id: "c1".into(),
            tool: "read".into(),
            error: None,
            duration_ms: 42,
        },
        Event::ToolCallEnd {
            session_id: "s1".into(),
            call_id: "c2".into(),
            tool: "bash".into(),
            error: Some("command failed".into()),
            duration_ms: 100,
        },
        Event::MessageEnd {
            session_id: "s1".into(),
            message_id: "m1".into(),
            reason: FinishReason::Stop,
        },
        Event::MessageEnd {
            session_id: "s1".into(),
            message_id: "m2".into(),
            reason: FinishReason::ToolUse,
        },
        Event::MessageEnd {
            session_id: "s1".into(),
            message_id: "m3".into(),
            reason: FinishReason::Length,
        },
        Event::MessageEnd {
            session_id: "s1".into(),
            message_id: "m4".into(),
            reason: FinishReason::ContentFilter,
        },
        Event::PermissionRequested {
            session_id: "s1".into(),
            request_id: "r1".into(),
            permission: "file:write".into(),
            description: "Write to foo.txt".into(),
        },
        Event::PermissionReplied {
            session_id: "s1".into(),
            request_id: "r1".into(),
            allowed: true,
        },
        Event::AgentSwitched {
            session_id: "s1".into(),
            from: "general".into(),
            to: "build".into(),
        },
        Event::AgentSwitchRequested {
            session_id: "s1".into(),
            to: "plan".into(),
            task: "Analyze codebase".into(),
            context: String::new(),
        },
        Event::AgentRestoreRequested {
            session_id: "s1".into(),
            summary: "Plan complete".into(),
        },
        Event::AgentError {
            session_id: "s1".into(),
            error: "something broke".into(),
        },
        Event::McpStatusChanged {
            server_id: "github".into(),
            status: "connected".into(),
        },
        Event::TokenUsage {
            session_id: "s1".into(),
            input_tokens: 100,
            output_tokens: 200,
        },
        Event::ToolsSent {
            session_id: "s1".into(),
            tools: vec!["read".into(), "write".into()],
        },
        Event::ModelResponse {
            session_id: "s1".into(),
            text: "Here is the answer".into(),
            elapsed_ms: 1234,
        },
        Event::ToolCallArgs {
            session_id: "s1".into(),
            call_id: "c1".into(),
            tool: "read".into(),
            args: r#"{"path":"foo.txt"}"#.into(),
        },
        Event::ToolResult {
            session_id: "s1".into(),
            call_id: "c1".into(),
            tool: "read".into(),
            content: "file contents here".into(),
            content_line_count: 1,
            metadata: None,
            success: true,
        },
        Event::CopilotDeviceFlowComplete {
            token: "ghu_test123".into(),
            api_base: "https://api.individual.githubcopilot.com".into(),
        },
        Event::SessionAborted {
            session_id: "s1".into(),
            reason: "user_requested".into(),
        },
    ];

    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let deserialized: Event = serde_json::from_str(&json).unwrap();
        // Re-serialize and compare JSON to verify roundtrip
        let json2 = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(json, json2, "Event roundtrip failed for: {:?}", event);
    }
}

// ── Subscriber ordering ──────────────────────────────────────────

#[test]
fn test_event_bus_ordering() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    for i in 0..5 {
        bus.publish(Event::TextDelta {
            session_id: "s1".into(),
            text: format!("chunk-{}", i),
        });
    }

    for i in 0..5 {
        let event = rx.try_recv().unwrap();
        match event {
            Event::TextDelta { text, .. } => {
                assert_eq!(text, format!("chunk-{}", i));
            }
            _ => panic!("Expected TextDelta"),
        }
    }
}

// ── Late subscriber misses earlier events ────────────────────────

#[test]
fn test_event_bus_late_subscriber() {
    let bus = EventBus::new(64);

    bus.publish(Event::SessionCreated {
        session_id: "early".into(),
    });

    let mut rx = bus.subscribe();

    bus.publish(Event::SessionCreated {
        session_id: "late".into(),
    });

    let event = rx.try_recv().unwrap();
    match event {
        Event::SessionCreated { session_id } => assert_eq!(session_id, "late"),
        _ => panic!("Expected SessionCreated with 'late'"),
    }
}

// ── Multiple publishers ──────────────────────────────────────────

#[test]
fn test_event_bus_multiple_publishers() {
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    let bus_clone = bus.clone();

    bus.publish(Event::TextDelta {
        session_id: "s1".into(),
        text: "from original".into(),
    });
    bus_clone.publish(Event::TextDelta {
        session_id: "s1".into(),
        text: "from clone".into(),
    });

    let e1 = rx.try_recv().unwrap();
    let e2 = rx.try_recv().unwrap();

    match (&e1, &e2) {
        (Event::TextDelta { text: t1, .. }, Event::TextDelta { text: t2, .. }) => {
            assert_eq!(t1, "from original");
            assert_eq!(t2, "from clone");
        }
        _ => panic!("Expected two TextDeltas"),
    }
}

// ── No subscribers doesn't panic ─────────────────────────────────

#[test]
fn test_event_bus_no_subscribers() {
    let bus = EventBus::new(64);
    // Should not panic
    bus.publish(Event::SessionCreated {
        session_id: "s1".into(),
    });
}

// ── FinishReason display ─────────────────────────────────────────

#[test]
fn test_finish_reason_display() {
    assert_eq!(FinishReason::Stop.to_string(), "stop");
    assert_eq!(FinishReason::ToolUse.to_string(), "tool_use");
    assert_eq!(FinishReason::Length.to_string(), "length");
    assert_eq!(FinishReason::ContentFilter.to_string(), "content_filter");
}

// ── FinishReason serde ───────────────────────────────────────────

#[test]
fn test_finish_reason_serde() {
    let reasons = vec![
        FinishReason::Stop,
        FinishReason::ToolUse,
        FinishReason::Length,
        FinishReason::ContentFilter,
    ];
    for reason in &reasons {
        let json = serde_json::to_string(reason).unwrap();
        let deserialized: FinishReason = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, reason);
    }
}

// ── Default EventBus capacity ────────────────────────────────────

#[test]
fn test_event_bus_default_capacity() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();

    // Publish and receive to verify default works
    bus.publish(Event::SessionCreated {
        session_id: "s1".into(),
    });
    assert!(rx.try_recv().is_ok());
}
