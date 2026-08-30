//! Integration tests for [`SessionProcessor::compact_session`] — the dedicated
//! compaction runner entry point used by the TUI `/compact` command and
//! pre-send auto-compaction.
//!
//! Verifies that:
//!
//! 1. `compact_session` makes exactly ONE summarisation LLM call with no tools
//!    (no agent loop, no init acknowledgement, no double summarisation).
//! 2. The persisted history is replaced with `[compaction, ...recent]` so the
//!    next turn loads from the compaction point forward.
//! 3. A cancelled compaction bails before any LLM call.
//! 4. An unresolvable provider produces a clear error.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::ModelRef;
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::message::{Message, Role};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Cost};

#[derive(Clone)]
struct CompactSessionMockProvider {
    captured_requests: Arc<Mutex<Vec<ChatRequest>>>,
}

struct CompactSessionMockClient {
    captured_requests: Arc<Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl LlmClient for CompactSessionMockClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured_requests
            .lock()
            .expect("captured requests lock")
            .push(request);
        Ok(Box::pin(stream::iter(vec![
            StreamEvent::TextDelta {
                text: "## Objective\n- proceed".to_string(),
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ])))
    }
}

#[async_trait::async_trait]
impl Provider for CompactSessionMockProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Compact Session Mock Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "qwen3:latest".to_string(),
            provider_id: "ollama".to_string(),
            name: "Qwen3".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 100_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(CompactSessionMockClient {
            captured_requests: Arc::clone(&self.captured_requests),
        }))
    }
}

/// Seed an alternating user/assistant history long enough that the compaction
/// runner keeps a verbatim recent tail and summarises a non-empty head.
fn seed_long_history(storage: &Storage, session_id: &str) {
    let pad = "x".repeat(4_000);
    for i in 0..10 {
        let role = if i % 2 == 0 {
            Role::User
        } else {
            Role::Assistant
        };
        let msg = Message::new(
            session_id,
            role,
            vec![Message::user_text("sess", format!("message {i} {pad}")).parts[0].clone()],
        );
        storage.create_message(&msg).expect("seed message");
    }
}

fn make_processor(
    provider_registry: ProviderRegistry,
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
) -> SessionProcessor {
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let mut config = ragent_config::Config::default();
    // Keep only 5% of the context verbatim so most of the seeded history is
    // summarised into the head and the compacted form is genuinely shorter.
    config.compaction.keep.tokens = Some(0.05);
    SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry,
        permission_checker,
        event_bus,
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
        cached_config: parking_lot::Mutex::new(Some(CachedConfig {
            config: Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        })),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
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
    }
}

#[tokio::test]
async fn test_compact_session_single_llm_call_and_history_replacement() {
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(CompactSessionMockProvider {
        captured_requests: Arc::clone(&captured_requests),
    }));

    let event_bus = Arc::new(EventBus::new(32));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let processor = make_processor(provider_registry, storage.clone(), event_bus);

    let working_dir = tempfile::tempdir().expect("tempdir");
    let session = processor
        .session_manager
        .create_session(working_dir.path().to_path_buf())
        .expect("session should be created");
    seed_long_history(&storage, &session.id);
    let pre_count = storage.get_messages(&session.id).expect("get").len();
    assert_eq!(pre_count, 10);

    let model_ref = ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    };
    let cancel = AtomicBool::new(false);
    let outcome = processor
        .compact_session(&session.id, &model_ref, "manual", &cancel)
        .await
        .expect("compact_session should succeed");

    // Exactly ONE LLM request was made — no agent loop, no init
    // acknowledgement, no second (in-loop trigger) summarisation.
    let captured = captured_requests.lock().expect("captured requests lock");
    assert_eq!(
        captured.len(),
        1,
        "compact_session must make exactly one LLM call (no agent loop)"
    );
    assert!(
        captured[0].tools.is_empty(),
        "summarisation request must carry no tools"
    );
    drop(captured);

    // The outcome starts with the synthetic compaction message.
    assert_eq!(outcome.compaction_message.role, Role::Compaction);
    assert_eq!(outcome.original_message_count, 10);
    assert!(outcome.kept_message_count >= 1, "recent tail is kept");

    // Persisted history was REPLACED: old rows deleted, compacted form stored.
    let stored = storage.get_messages(&session.id).expect("get_messages");
    assert_eq!(stored.len(), outcome.new_messages.len());
    assert_eq!(stored[0].role, Role::Compaction);
    assert_eq!(stored[0].id, outcome.compaction_message.id);
    assert!(
        stored.len() < pre_count,
        "compacted history must be shorter than the original"
    );
}

#[tokio::test]
async fn test_compact_session_cancelled_before_any_llm_call() {
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(CompactSessionMockProvider {
        captured_requests: Arc::clone(&captured_requests),
    }));

    let event_bus = Arc::new(EventBus::new(32));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let processor = make_processor(provider_registry, storage, event_bus);

    let working_dir = tempfile::tempdir().expect("tempdir");
    let session = processor
        .session_manager
        .create_session(working_dir.path().to_path_buf())
        .expect("session should be created");

    let model_ref = ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    };
    let cancel = AtomicBool::new(true);
    let result = processor
        .compact_session(&session.id, &model_ref, "manual", &cancel)
        .await;
    assert!(result.is_err(), "cancelled compaction must return an error");
    assert!(
        captured_requests
            .lock()
            .expect("captured requests lock")
            .is_empty(),
        "cancelled compaction must not call the LLM"
    );
}

#[tokio::test]
async fn test_compact_session_unknown_provider_errors() {
    let captured_requests: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
    // No provider registered under this id.
    let provider_registry = ProviderRegistry::new();
    let event_bus = Arc::new(EventBus::new(32));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let processor = make_processor(provider_registry, storage, event_bus);

    let working_dir = tempfile::tempdir().expect("tempdir");
    let session = processor
        .session_manager
        .create_session(working_dir.path().to_path_buf())
        .expect("session should be created");

    let model_ref = ModelRef {
        provider_id: "does_not_exist".to_string(),
        model_id: "model".to_string(),
    };
    let cancel = AtomicBool::new(false);
    let err = processor
        .compact_session(&session.id, &model_ref, "manual", &cancel)
        .await
        .expect_err("unknown provider must fail");
    assert!(
        err.to_string().contains("does_not_exist"),
        "error should name the missing provider: {err}"
    );
    assert!(
        captured_requests
            .lock()
            .expect("captured requests lock")
            .is_empty(),
        "provider resolution failure must not call the LLM"
    );
}

/// A cancel raised between the entry check and the LLM call still aborts
/// (or, if it lands too late, the run stays a single summarisation call —
/// never a double one).
#[tokio::test]
async fn test_compact_session_cancel_flag_raced_mid_run() {
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(CompactSessionMockProvider {
        captured_requests: Arc::clone(&captured_requests),
    }));

    let event_bus = Arc::new(EventBus::new(32));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let processor = make_processor(provider_registry, storage, event_bus);

    let working_dir = tempfile::tempdir().expect("tempdir");
    let session = processor
        .session_manager
        .create_session(working_dir.path().to_path_buf())
        .expect("session should be created");

    let model_ref = ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = Arc::clone(&cancel);
    let flip = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        cancel_for_task.store(true, Ordering::Relaxed);
    });
    let result = processor
        .compact_session(&session.id, &model_ref, "manual", cancel.as_ref())
        .await;
    let _ = flip.await;
    if result.is_ok() {
        assert_eq!(
            captured_requests
                .lock()
                .expect("captured requests lock")
                .len(),
            1,
            "at most one summarisation call regardless of race timing"
        );
    }
}
