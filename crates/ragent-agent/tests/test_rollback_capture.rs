//! Regression coverage for the FR-020/T-013 race behind the CI failure in
//! `test_rollback_accept_restores_snapshot`.
//!
//! The TUI runs the actual rollback on a spawned tokio task; that task removes
//! the session's capture from `active_loop_captures` **before** the snapshot
//! restore's filesystem writes complete, then deposits the outcome into the
//! `rollback_result` mailbox drained by `App::poll_rollback_result`. A test
//! (or a UI tick) that polls on the capture map emptiness therefore observed
//! "capture dropped" while the workspace file still held the mutated
//! contents. The production-code fix in `SessionProcessor::rollback_loop`
//! removes the capture only *after* the restore has fully completed (and
//! reinserts it on failure).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_capture::LoopCapture;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_storage::snapshot::take_snapshot;

fn make_processor(storage: Arc<Storage>) -> Arc<SessionProcessor> {
    let event_bus = Arc::new(EventBus::default());
    let provider_registry = Arc::new(ProviderRegistry::new());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    Arc::new(SessionProcessor {
        session_manager,
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
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
        team_context_cache: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new())),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
    })
}

/// After `rollback_loop` returns, the file contents must already be the
/// pre-loop contents — regression cover for the FR-020 race where the
/// capture was removed BEFORE the blocking fs writes completed, letting an
/// observer read the mutated file the moment the map went empty.
#[tokio::test]
async fn test_rollback_loop_file_is_restored_when_the_call_returns() -> Result<()> {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let processor = make_processor(storage);

    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("existing.txt");
    std::fs::write(&target, "original").expect("write original");
    let snapshot = take_snapshot("sess-rollback", "msg-1", std::slice::from_ref(&target))?;
    std::fs::write(&target, "mutated by the loop").expect("mutate");
    processor.active_loop_captures.write().await.insert(
        "sess-rollback".to_string(),
        LoopCapture {
            snapshot: Some(snapshot),
            git: None,
        },
    );

    let outcome = processor.rollback_loop("sess-rollback").await?;
    assert!(outcome, "the pending capture was restored");
    assert!(
        processor.active_loop_captures.read().await.is_empty(),
        "the capture is dropped after the restore"
    );
    let content = std::fs::read_to_string(&target).expect("read restored");
    assert_eq!(
        content, "original",
        "FR-020: the pre-loop contents are back by the time rollback_loop returns"
    );
    Ok(())
}

/// A failing restore keeps the capture pending so the caller can retry.
#[tokio::test]
async fn test_rollback_loop_failure_keeps_the_capture() -> Result<()> {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let processor = make_processor(storage);

    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("existing.txt");
    std::fs::write(&target, "original").expect("write original");
    let snapshot = take_snapshot("sess-rollback", "msg-1", std::slice::from_ref(&target))?;
    std::fs::remove_file(&target).expect("remove mutated");
    // Turn the target directory read-only so the restore fails.
    let mut perms = std::fs::metadata(dir.path())?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o500);
    }
    std::fs::set_permissions(dir.path(), perms.clone())?;
    processor.active_loop_captures.write().await.insert(
        "sess-rollback".to_string(),
        LoopCapture {
            snapshot: Some(snapshot),
            git: None,
        },
    );

    let result = processor.rollback_loop("sess-rollback").await;
    #[cfg(unix)]
    {
        assert!(result.is_err(), "restore into a read-only dir fails");
        assert!(
            !processor.active_loop_captures.read().await.is_empty(),
            "the capture is kept for retry when the restore fails"
        );
        // Restore permissions so the tempdir can be cleaned up.
        use std::os::unix::fs::PermissionsExt;
        let mut writable = std::fs::metadata(dir.path())?.permissions();
        writable.set_mode(0o700);
        std::fs::set_permissions(dir.path(), writable)?;
    }
    #[cfg(not(unix))]
    {
        let _ = result;
    }
    Ok(())
}
