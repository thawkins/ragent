//! Compaction-summary labelling in the message window / output overlay.
//!
//! A sub-agent's mid-run auto-compaction summary ("## Objective / ## Work
//! State / ## Next Move") used to render with the same magenta bullet as
//! assistant text, so users read it as the sub-agent's completion report and
//! concluded the agent was failing to terminate. These tests pin the
//! distinct `[compaction]` labelling.

use std::sync::Arc;

use ratatui::{Terminal, backend::TestBackend};

use ragent_agent::{
    event::EventBus,
    message::{Message, Role},
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use ragent_tui::app::{OutputViewState, OutputViewTarget, ScreenMode};

fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let agent_info = ragent_agent::agent::resolve_agent("general", &Default::default())
        .expect("resolve general agent");
    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        true,
        std::path::PathBuf::new(),
    )
}

/// Render the output overlay for a child session containing one compaction
/// summary and one assistant message, and return the overlay text.
fn render_overlay(child_session: &str, app: &mut App) -> String {
    app.current_screen = ScreenMode::Chat;
    app.session_id = Some("lead-s1".to_string());
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: child_session.to_string(),
            label: "explore [a1b2c3d4]".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 0,
        line_cache: ragent_tui::app::OutputViewLineCache {
            lines: Vec::new(),
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            cache_width: 0,
            source_generation: 0,
        },
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");

    let area = app.output_view_area;
    let buffer = terminal.backend().buffer().clone();
    let mut overlay_text = String::new();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buffer.cell((x, y)) {
                overlay_text.push_str(cell.symbol());
            }
        }
        overlay_text.push('\n');
    }
    overlay_text
}

#[test]
fn test_compaction_summary_renders_with_distinct_label() {
    let mut app = make_app();
    app.storage
        .create_session("child-s1", "/tmp")
        .expect("create child session");
    let mut compaction = Message::new("child-s1", Role::Compaction, Vec::new());
    compaction
        .parts
        .push(ragent_agent::message::MessagePart::Text {
            text: "## Objective\n- review the files\n\n## Next Move\n- (none)".to_string(),
        });
    app.storage.create_message(&compaction).expect("seed");
    let text = render_overlay("child-s1", &mut app);

    assert!(
        text.contains("[compaction]"),
        "compaction summary must carry the [compaction] label, got:\n{text}"
    );
    // The compaction summary content is still visible.
    assert!(
        text.contains("review the files"),
        "compaction summary content must render, got:\n{text}"
    );
}

#[test]
fn test_assistant_text_keeps_bullet_marker_without_compaction_label() {
    let mut app = make_app();
    app.storage
        .create_session("child-s2", "/tmp")
        .expect("create child session");
    app.storage
        .create_message(&Message::assistant_text(
            "child-s2",
            "## Objective\n- the real completion report",
        ))
        .expect("seed");
    let text = render_overlay("child-s2", &mut app);

    assert!(
        text.contains("the real completion report"),
        "assistant text must render, got:\n{text}"
    );
    assert!(
        !text.contains("[compaction]"),
        "assistant text must not be labelled as compaction, got:\n{text}"
    );
}
