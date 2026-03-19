//! Tests for session processor error propagation paths (Task 5.2).

use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

use tokio::time::timeout;

use ragent_core::agent::{AgentInfo, ModelRef};
use ragent_core::event::{Event, EventBus};
use ragent_core::permission::PermissionChecker;
use ragent_core::provider::ProviderRegistry;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::session::SessionManager;
use ragent_core::storage::Storage;
use ragent_core::tool::ToolRegistry;

#[tokio::test]
async fn test_session_processor_errors_when_model_missing() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let provider_registry = Arc::new(ProviderRegistry::new());
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));

    let processor = SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
    };

    let tempdir = tempfile::tempdir().unwrap();
    let session = session_manager
        .create_session(PathBuf::from(tempdir.path()))
        .unwrap();

    let mut rx = event_bus.subscribe();

    let agent = AgentInfo::new("test", "Test agent");
    let cancel = Arc::new(AtomicBool::new(false));

    let err = processor
        .process_message(&session.id, "hi", &agent, cancel)
        .await;
    assert!(err.is_err());
    assert!(err
        .unwrap_err()
        .to_string()
        .contains("has no model configured"));

    let ev = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("failed to receive event");
    match ev {
        Event::AgentError { session_id, error } => {
            assert_eq!(session_id, session.id);
            assert!(error.contains("has no model configured"));
        }
        _ => panic!("expected AgentError event"),
    }
}

#[tokio::test]
async fn test_session_processor_errors_when_provider_missing() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let provider_registry = Arc::new(ProviderRegistry::new());
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));

    let processor = SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
    };

    let tempdir = tempfile::tempdir().unwrap();
    let session = session_manager
        .create_session(PathBuf::from(tempdir.path()))
        .unwrap();

    let mut rx = event_bus.subscribe();

    let mut agent = AgentInfo::new("test", "Test agent");
    agent.model = Some(ModelRef {
        provider_id: "missing".to_string(),
        model_id: "m".to_string(),
    });

    let cancel = Arc::new(AtomicBool::new(false));

    let err = processor
        .process_message(&session.id, "hi", &agent, cancel)
        .await;
    assert!(err.is_err());
    assert!(err
        .unwrap_err()
        .to_string()
        .contains("Provider 'missing' not found"));

    let ev = timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("failed to receive event");
    match ev {
        Event::AgentError { session_id, error } => {
            assert_eq!(session_id, session.id);
            assert!(error.contains("Provider 'missing' not found"));
        }
        _ => panic!("expected AgentError event"),
    }
}
