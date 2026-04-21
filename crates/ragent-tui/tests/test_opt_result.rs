//! Tests for `/opt` result handling and Mutex poisoning recovery (Section 4.F).

use std::sync::Arc;

use ragent_core::{
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
        stream_config: ragent_core::config::StreamConfig::default(),
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
        false,
    )
}

// =========================================================================
// poll_pending_opt — no result pending
// =========================================================================

#[test]
fn test_poll_pending_opt_noop_when_empty() {
    let mut app = make_app();
    let status_before = app.status.clone();
    app.poll_pending_opt();
    // Status shouldn't change when there's no pending result.
    assert_eq!(app.status, status_before);
}

// =========================================================================
// poll_pending_opt — Ok result
// =========================================================================

#[test]
fn test_poll_pending_opt_ok_updates_status_and_messages() {
    let mut app = make_app();

    // Create a session so append_assistant_text can push messages.
    app.session_id = Some("test-session".to_string());

    // Deposit an Ok result.
    {
        let mut guard = app.opt_result.lock().unwrap();
        *guard = Some(Ok("Optimized prompt output\nLine two".to_string()));
    }

    app.poll_pending_opt();

    assert_eq!(app.status, "opt: done");
    // The mutex should now be empty.
    assert!(app.opt_result.lock().unwrap().is_none());
}

// =========================================================================
// poll_pending_opt — Err result
// =========================================================================

#[test]
fn test_poll_pending_opt_err_updates_status() {
    let mut app = make_app();

    {
        let mut guard = app.opt_result.lock().unwrap();
        *guard = Some(Err("API rate limit exceeded".to_string()));
    }

    app.poll_pending_opt();

    assert!(
        app.status.contains("opt failed"),
        "status should mention failure: {}",
        app.status
    );
    assert!(
        app.status.contains("rate limit"),
        "status should contain error message: {}",
        app.status
    );
    // The mutex should now be empty.
    assert!(app.opt_result.lock().unwrap().is_none());
}

// =========================================================================
// poll_pending_opt — Mutex poisoned
// =========================================================================

#[test]
fn test_poll_pending_opt_recovers_from_poisoned_mutex() {
    let mut app = make_app();

    // Poison the mutex by panicking inside a lock.
    let opt_result_clone = Arc::clone(&app.opt_result);
    let _ = std::thread::spawn(move || {
        let _guard = opt_result_clone.lock().unwrap();
        panic!("intentional poison");
    })
    .join();

    // The mutex is now poisoned. poll_pending_opt should recover gracefully.
    app.poll_pending_opt();

    // Should not have panicked — verify the app is still functional.
    assert!(
        !app.status.contains("panic"),
        "should recover without propagating panic"
    );
}

#[test]
fn test_poll_pending_opt_poisoned_mutex_with_result() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    // Deposit a result, then poison the mutex.
    let opt_result_clone = Arc::clone(&app.opt_result);
    let _ = std::thread::spawn(move || {
        let mut guard = opt_result_clone.lock().unwrap();
        *guard = Some(Ok("poisoned but valid result".to_string()));
        panic!("intentional poison after storing result");
    })
    .join();

    // poll should recover the result despite the poison.
    app.poll_pending_opt();

    assert_eq!(app.status, "opt: done");
    assert!(
        app.opt_result
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none()
    );
}

// =========================================================================
// opt_result — concurrent deposit and poll
// =========================================================================

#[test]
fn test_opt_result_deposit_and_poll_cycle() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    // Simulate multiple cycles of deposit → poll.
    for i in 0..5 {
        {
            let mut guard = app.opt_result.lock().unwrap();
            *guard = Some(Ok(format!("result_{i}")));
        }
        app.poll_pending_opt();
        assert_eq!(app.status, "opt: done");
        assert!(
            app.opt_result.lock().unwrap().is_none(),
            "result should be consumed"
        );
    }
}
