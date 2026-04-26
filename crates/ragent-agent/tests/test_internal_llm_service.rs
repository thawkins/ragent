//! Integration tests for the internal embedded-LLM service wrapper.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ragent_agent::config::InternalLlmConfig;
use ragent_agent::embedded::{
    EmbeddedRuntimeLifecycle, EmbeddedRuntimeSettings, EmbeddedRuntimeStatus, RuntimeAvailability,
};
use ragent_agent::internal_llm::{
    InternalLlmError, InternalLlmExecutionRequest, InternalLlmExecutor, InternalLlmQueueStatus,
    InternalLlmService, InternalLlmTaskKind, InternalTaskLimits,
};

struct FakeExecutor {
    response: FakeResponse,
    queue_status: Option<InternalLlmQueueStatus>,
    runtime_status: Option<EmbeddedRuntimeStatus>,
}

enum FakeResponse {
    Ok(&'static str),
    Unavailable(&'static str),
    Timeout(u64),
}

#[derive(Default)]
struct CapturingExecutor {
    requests: Mutex<Vec<InternalLlmExecutionRequest>>,
    response: &'static str,
}

#[async_trait]
impl InternalLlmExecutor for FakeExecutor {
    async fn execute(
        &self,
        _request: InternalLlmExecutionRequest,
    ) -> Result<String, InternalLlmError> {
        match self.response {
            FakeResponse::Ok(text) => Ok(text.to_string()),
            FakeResponse::Unavailable(message) => Err(InternalLlmError::Unavailable {
                message: message.to_string(),
            }),
            FakeResponse::Timeout(timeout_ms) => Err(InternalLlmError::Timeout { timeout_ms }),
        }
    }

    fn queue_status(&self) -> Option<InternalLlmQueueStatus> {
        self.queue_status.clone()
    }

    fn status(&self) -> Option<EmbeddedRuntimeStatus> {
        self.runtime_status.clone()
    }
}

#[async_trait]
impl InternalLlmExecutor for CapturingExecutor {
    async fn execute(
        &self,
        request: InternalLlmExecutionRequest,
    ) -> Result<String, InternalLlmError> {
        self.requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(request);
        Ok(self.response.to_string())
    }
}

fn enabled_config() -> InternalLlmConfig {
    let mut config = InternalLlmConfig::default();
    config.enabled = true;
    config
}

#[tokio::test]
async fn test_internal_llm_service_rejects_non_allowlisted_tasks() {
    let mut config = enabled_config();
    config.allowed_tasks = vec!["session_title".to_string()];

    let service = InternalLlmService::with_executor(
        config,
        Arc::new(FakeExecutor {
            response: FakeResponse::Ok("Short title"),
            queue_status: None,
            runtime_status: None,
        }),
    );

    let err = service
        .run_internal_task(
            InternalLlmTaskKind::PromptCompaction,
            "keep this compact",
            InternalTaskLimits::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        InternalLlmError::TaskNotAllowed {
            task: "prompt_compaction"
        }
    ));
}

#[tokio::test]
async fn test_internal_llm_service_surfaces_unavailable_executor() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Unavailable("backend offline"),
            queue_status: None,
            runtime_status: None,
        }),
    );

    let err = service
        .run_internal_task(
            InternalLlmTaskKind::SessionTitle,
            "conversation snippet",
            InternalTaskLimits::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, InternalLlmError::Unavailable { .. }));
}

#[tokio::test]
async fn test_internal_llm_service_rejects_invalid_session_title_output() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Ok("```bad```"),
            queue_status: None,
            runtime_status: None,
        }),
    );

    let err = service
        .run_internal_task(
            InternalLlmTaskKind::SessionTitle,
            "conversation snippet",
            InternalTaskLimits::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, InternalLlmError::InvalidOutput { .. }));
}

#[tokio::test]
async fn test_internal_llm_service_validates_memory_prefilter_json() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Ok("[\"keep this\", \"and this\"]"),
            queue_status: None,
            runtime_status: None,
        }),
    );

    let result = service
        .run_internal_task(
            InternalLlmTaskKind::MemoryPrefilter,
            "memory candidates",
            InternalTaskLimits::default(),
        )
        .await
        .expect("memory prefilter should succeed");

    assert_eq!(result.output, "[\"keep this\", \"and this\"]");
    assert_eq!(result.task_kind, InternalLlmTaskKind::MemoryPrefilter);
}

#[tokio::test]
async fn test_internal_llm_service_allows_chat_task() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Ok("Internal reply"),
            queue_status: None,
            runtime_status: None,
        }),
    );

    let result = service
        .run_internal_task(
            InternalLlmTaskKind::Chat,
            "hello internal model",
            InternalTaskLimits::default(),
        )
        .await
        .expect("chat task should succeed");

    assert_eq!(result.output, "Internal reply");
    assert_eq!(result.task_kind, InternalLlmTaskKind::Chat);
}

#[tokio::test]
async fn test_internal_llm_service_tracks_counters_and_fallbacks() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Unavailable("backend offline"),
            queue_status: None,
            runtime_status: None,
        }),
    );

    let _ = service
        .run_internal_task(
            InternalLlmTaskKind::SessionTitle,
            "conversation snippet",
            InternalTaskLimits::default(),
        )
        .await;
    service.record_fallback(
        InternalLlmTaskKind::PromptCompaction,
        "fell back to provider compaction",
    );

    let snapshot = service.status_snapshot();
    assert_eq!(snapshot.metrics.attempts, 1);
    assert_eq!(snapshot.metrics.successes, 0);
    assert_eq!(snapshot.metrics.failures, 1);
    assert_eq!(snapshot.metrics.timeouts, 0);
    assert_eq!(snapshot.metrics.fallbacks, 1);
    assert!(
        snapshot
            .metrics
            .last_error
            .unwrap_or_default()
            .contains("backend offline")
    );
    assert!(
        snapshot
            .metrics
            .last_fallback
            .unwrap_or_default()
            .contains("provider compaction")
    );
}

#[tokio::test]
async fn test_internal_llm_service_records_executor_timeouts() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Timeout(250),
            queue_status: None,
            runtime_status: None,
        }),
    );

    let err = service
        .run_internal_task(
            InternalLlmTaskKind::SessionTitle,
            "conversation snippet",
            InternalTaskLimits::default(),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, InternalLlmError::Timeout { timeout_ms: 250 }));

    let snapshot = service.status_snapshot();
    assert_eq!(snapshot.metrics.attempts, 1);
    assert_eq!(snapshot.metrics.successes, 0);
    assert_eq!(snapshot.metrics.failures, 1);
    assert_eq!(snapshot.metrics.timeouts, 1);
    assert!(
        snapshot
            .metrics
            .last_error
            .unwrap_or_default()
            .contains("timed out")
    );
}

#[tokio::test]
async fn test_internal_llm_service_surfaces_queue_status() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Ok("Internal reply"),
            queue_status: Some(InternalLlmQueueStatus {
                capacity: 3,
                in_flight: 2,
                queued: 1,
                worker_busy: true,
            }),
            runtime_status: None,
        }),
    );

    let snapshot = service.status_snapshot();
    assert_eq!(
        snapshot.queue,
        Some(InternalLlmQueueStatus {
            capacity: 3,
            in_flight: 2,
            queued: 1,
            worker_busy: true,
        })
    );
}

#[tokio::test]
async fn test_internal_llm_service_surfaces_runtime_settings() {
    let service = InternalLlmService::with_executor(
        enabled_config(),
        Arc::new(FakeExecutor {
            response: FakeResponse::Ok("Internal reply"),
            queue_status: None,
            runtime_status: Some(EmbeddedRuntimeStatus {
                model_id: "smollm2-360m-instruct-q4".to_string(),
                availability: RuntimeAvailability::Available,
                lifecycle: EmbeddedRuntimeLifecycle::Uninitialized,
                backend_name: None,
                detail: None,
                cache_root: "/tmp/cache".into(),
                model_dir: "/tmp/cache/model".into(),
                settings: EmbeddedRuntimeSettings {
                    execution_device: "cpu".to_string(),
                    quantized_runtime:
                        "gguf via candle_transformers::models::quantized_llama".to_string(),
                    requested_threads: 2,
                    effective_threads: 2,
                    threading: "rayon global pool initialized for internal LLM CPU execution with 2 threads"
                        .to_string(),
                    requested_gpu_layers: 4,
                    effective_gpu_layers: 0,
                    gpu_offload: "requested 4 GPU layers, but this internal Candle runtime does not implement GGUF layer offload; forcing 0 effective layers".to_string(),
                },
            }),
        }),
    );

    let snapshot = service.status_snapshot();
    let runtime = snapshot.runtime.expect("runtime status should exist");
    assert_eq!(runtime.settings.execution_device, "cpu");
    assert_eq!(runtime.settings.requested_threads, 2);
    assert_eq!(runtime.settings.effective_threads, 2);
    assert_eq!(runtime.settings.requested_gpu_layers, 4);
    assert_eq!(runtime.settings.effective_gpu_layers, 0);
    assert!(
        runtime
            .settings
            .gpu_offload
            .contains("forcing 0 effective layers")
    );
}

#[tokio::test]
async fn test_internal_llm_service_uses_tuned_session_title_limits() {
    let executor = Arc::new(CapturingExecutor {
        requests: Mutex::new(Vec::new()),
        response: "Short title",
    });
    let service = InternalLlmService::with_executor(enabled_config(), executor.clone());

    let result = service
        .run_internal_task(
            InternalLlmTaskKind::SessionTitle,
            "conversation snippet",
            InternalTaskLimits::default(),
        )
        .await
        .expect("session title should succeed");

    let requests = executor.requests.lock().unwrap_or_else(|e| e.into_inner());
    let request = requests.last().expect("captured request");
    assert_eq!(request.max_output_tokens, 16);
    assert_eq!(result.effective_limits.max_output_tokens, Some(16));
    assert!(request.system_prompt.contains("max 8 words"));
}

#[tokio::test]
async fn test_internal_llm_service_uses_tuned_memory_prefilter_limits() {
    let executor = Arc::new(CapturingExecutor {
        requests: Mutex::new(Vec::new()),
        response: "[\"keep this\"]",
    });
    let service = InternalLlmService::with_executor(enabled_config(), executor.clone());

    let result = service
        .run_internal_task(
            InternalLlmTaskKind::MemoryPrefilter,
            "memory candidates",
            InternalTaskLimits::default(),
        )
        .await
        .expect("memory prefilter should succeed");

    let requests = executor.requests.lock().unwrap_or_else(|e| e.into_inner());
    let request = requests.last().expect("captured request");
    assert_eq!(request.max_output_tokens, 96);
    assert_eq!(result.effective_limits.max_output_tokens, Some(96));
    assert!(request.system_prompt.contains("JSON array"));
}
