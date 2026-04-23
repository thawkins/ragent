//! Test for permission dialog countdown timer display

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ragent_core::{
    agent,
    config::StreamConfig,
    event::{Event, EventBus},
    permission::{PermissionChecker, PermissionRequest},
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig::default(),
        auto_approve: false,
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");
    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        true,
    )
}

fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render permission dialog");

    let backend = terminal.backend();
    let buffer = backend.buffer();
    let mut text = String::new();
    let area = buffer.area();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn test_permission_request_has_timeout_fields() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let request = PermissionRequest {
        id: "test-123".to_string(),
        session_id: "session-456".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["ls".to_string()],
        metadata: serde_json::json!({
            "command": "ls -la",
            "created_at": now,
            "timeout_secs": 120_u64
        }),
        tool_call_id: None,
    };

    assert_eq!(
        request
            .metadata
            .get("timeout_secs")
            .and_then(|v| v.as_u64()),
        Some(120)
    );
    assert_eq!(
        request.metadata.get("created_at").and_then(|v| v.as_u64()),
        Some(now)
    );
}

#[test]
fn test_countdown_calculation_logic() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simulate a request created 30 seconds ago
    let created_at = now - 30;
    let timeout_secs: u64 = 120;

    let elapsed = now.saturating_sub(created_at);
    let remaining = timeout_secs.saturating_sub(elapsed);

    assert_eq!(elapsed, 30);
    assert_eq!(remaining, 90);

    let remaining_mins = remaining / 60;
    let remaining_secs = remaining % 60;

    assert_eq!(remaining_mins, 1);
    assert_eq!(remaining_secs, 30);

    let title = format!(
        " Permission Required ({}:{:02} remaining) ",
        remaining_mins, remaining_secs
    );
    assert_eq!(title, " Permission Required (1:30 remaining) ");
}

#[test]
fn test_countdown_expired() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simulate a request created 130 seconds ago (expired)
    let created_at = now - 130;
    let timeout_secs: u64 = 120;

    let elapsed = now.saturating_sub(created_at);
    let remaining = timeout_secs.saturating_sub(elapsed);

    assert_eq!(remaining, 0);

    let title = if remaining == 0 {
        " Permission Required (EXPIRED) ".to_string()
    } else {
        let remaining_mins = remaining / 60;
        let remaining_secs = remaining % 60;
        format!(
            " Permission Required ({}:{:02} remaining) ",
            remaining_mins, remaining_secs
        )
    };

    assert_eq!(title, " Permission Required (EXPIRED) ");
}

#[test]
fn test_countdown_formats_correctly() {
    // Test various time formats
    let test_cases = vec![
        (120, 2, 0, "2:00"), // 2 minutes
        (90, 1, 30, "1:30"), // 1:30
        (60, 1, 0, "1:00"),  // 1 minute
        (59, 0, 59, "0:59"), // 59 seconds
        (5, 0, 5, "0:05"),   // 5 seconds
        (0, 0, 0, "0:00"),   // expired
    ];

    for (remaining_secs, expected_mins, expected_secs, expected_str) in test_cases {
        let mins = remaining_secs / 60;
        let secs = remaining_secs % 60;

        assert_eq!(mins, expected_mins);
        assert_eq!(secs, expected_secs);

        let formatted = format!("{}:{:02}", mins, secs);
        assert_eq!(formatted, expected_str);
    }
}

#[test]
fn test_permission_requested_event_queues_120_second_timeout() {
    let mut app = make_app();
    app.session_id = Some("session-1".to_string());

    app.handle_event(Event::PermissionRequested {
        session_id: "session-1".to_string(),
        request_id: "req-1".to_string(),
        permission: "file:write".to_string(),
        description: "create: crates/ragent-tui/tests/test_permission_countdown.rs".to_string(),
        options: vec![],
    });

    let request = app
        .permission_queue
        .front()
        .expect("permission request should be queued");

    assert_eq!(
        request
            .metadata
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64),
        Some(120)
    );
    assert!(
        request
            .metadata
            .get("created_at")
            .and_then(serde_json::Value::as_u64)
            .is_some()
    );
}

#[test]
fn test_permission_dialog_renders_initial_120_second_countdown() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut app = make_app();
    app.permission_queue.push_back(PermissionRequest {
        id: "req-1".to_string(),
        session_id: "session-1".to_string(),
        permission: "file:write".to_string(),
        patterns: vec!["create: crates/ragent-tui/tests/test_permission_countdown.rs".to_string()],
        metadata: serde_json::json!({
            "created_at": now,
            "timeout_secs": 120u64,
        }),
        tool_call_id: None,
    });

    let rendered = render_app_to_string(&mut app);

    assert!(rendered.contains("Permission Required (2:00 remaining)"));
}
