//! Tests for the slash-command status auto-expiry mechanism.
//!
//! After a slash command completes, the status indicator shows the command's
//! status (e.g. "help", "tools: office on") for a short grace period and then
//! auto-transitions to "ready" so the user can see the system is ready for the
//! next interaction. These tests verify the arming, polling, and guard logic.

use std::sync::Arc;

use ragent_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::default());
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
        task_manager: std::sync::OnceLock::new(),
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
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
        std::path::PathBuf::new(),
    )
}

/// A slash command that sets a specific status should keep it immediately after
/// execution (the grace period hasn't elapsed yet).
#[test]
fn test_slash_status_preserved_immediately_after_command() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help");

    // Right after the command, the status is the command-specific value.
    assert_eq!(app.status, "help");
    // The expiry timer should be armed.
    assert!(app.status_set_at.is_some());
}

/// After the grace period elapses (and the status hasn't changed), the status
/// should transition to "ready".
#[test]
fn test_slash_status_expires_to_ready() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help");
    assert_eq!(app.status, "help");

    // Simulate the grace period having elapsed by backdating the armed instant.
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );

    app.poll_status_expiry();

    assert_eq!(app.status, "ready");
    assert!(app.status_set_at.is_none());
}

/// If something else changes the status during the grace period, the timer
/// should NOT overwrite the new status.
#[test]
fn test_slash_status_expiry_skips_if_status_changed() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help");
    assert_eq!(app.status, "help");

    // Simulate the agent starting to process and setting a "busy" status.
    app.status = "busy - wait for the current turn to finish".to_string();
    // Backdate the timer so the grace period has elapsed.
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );

    app.poll_status_expiry();

    // The timer is cleared but the busy status is preserved.
    assert_eq!(app.status, "busy - wait for the current turn to finish");
    assert!(app.status_set_at.is_none());
}

/// The expiry timer should not be armed for async-in-progress (⏳) statuses.
#[test]
fn test_slash_status_expiry_not_armed_for_async() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Simulate a slash command that sets an async status.
    app.status = "⏳ opt/co_star: optimizing…".to_string();
    app.arm_status_expiry();

    assert!(app.status_set_at.is_none());
    assert_eq!(app.status, "⏳ opt/co_star: optimizing…");
}

/// The expiry timer should not be armed for error (⚠) statuses.
#[test]
fn test_slash_status_expiry_not_armed_for_error() {
    let mut app = make_app();

    app.status = "⚠ Please provide a task ID prefix: /cancel <id>".to_string();
    app.arm_status_expiry();

    assert!(app.status_set_at.is_none());
}

/// The expiry timer should not be armed when the status is already "ready".
#[test]
fn test_slash_status_expiry_not_armed_for_ready() {
    let mut app = make_app();

    app.status = "ready".to_string();
    app.arm_status_expiry();

    assert!(app.status_set_at.is_none());
}

/// Before the grace period elapses, polling should keep the timer armed and
/// the status unchanged.
#[test]
fn test_slash_status_expiry_keeps_timer_before_grace_period() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help");
    assert_eq!(app.status, "help");

    // Poll immediately — the grace period hasn't elapsed.
    app.poll_status_expiry();

    assert_eq!(app.status, "help");
    assert!(app.status_set_at.is_some());
}

/// A slash command that errors (⚠ status) should not arm the expiry timer.
#[test]
fn test_slash_error_status_not_armed() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // /cancel with no argument produces an error status.
    app.execute_slash_command("/cancel");

    assert!(app.status.starts_with('⚠'));
    assert!(
        app.status_set_at.is_none(),
        "error statuses should not arm the expiry timer"
    );
}

/// `/research create` runs its analysis in a background tokio task. Its status
/// must use the `⏳` async-in-progress prefix so [`App::arm_status_expiry`]
/// (called at the end of [`App::execute_slash_command`]) does NOT auto-clear it
/// to "ready" while the final analysis is still running.
///
/// This test reproduces the regression where the status flipped to "ready"
/// before the background research completed.
#[test]
fn test_research_create_status_is_async_in_progress() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Simulate the status the `/research create` handler sets.
    app.status = "⏳ research: my-topic…".to_string();
    // execute_slash_command calls arm_status_expiry after the inner handler
    // returns; replicate that here.
    app.arm_status_expiry();

    // The expiry timer must NOT be armed for an async-in-progress status.
    assert!(
        app.status_set_at.is_none(),
        "research create status should be async-in-progress (⏳) and not armed for auto-expiry"
    );
    assert_eq!(app.status, "⏳ research: my-topic…");

    // Even after the grace period elapses, the status must remain unchanged.
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );
    app.poll_status_expiry();
    assert_eq!(
        app.status, "⏳ research: my-topic…",
        "async-in-progress research status must not auto-expire to ready"
    );
}

/// The live per-phase status updated by the research progress event handler
/// also uses the `⏳` prefix, so it stays visible (and is not auto-cleared)
/// while the background research is still running.
#[test]
fn test_research_progress_status_is_async_in_progress() {
    let mut app = make_app();

    // Simulate the per-phase running status set by the AgentNotice progress
    // handler while the research session is mid-run.
    app.status = "⏳ research: my-topic — web (▶) — running".to_string();
    app.arm_status_expiry();
    assert!(
        app.status_set_at.is_none(),
        "running research phase status should not arm the auto-expiry timer"
    );

    // Once the final progress event arrives, the handler sets a terminal
    // (non-⏳) completion status and arms expiry — that one SHOULD expire.
    app.status = "research: my-topic complete — 7 sources".to_string();
    app.arm_status_expiry();
    assert!(
        app.status_set_at.is_some(),
        "terminal research completion status should arm the auto-expiry timer"
    );
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );
    app.poll_status_expiry();
    assert_eq!(app.status, "ready");
}
