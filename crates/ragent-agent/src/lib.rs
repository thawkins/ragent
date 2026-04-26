//! Agent orchestration, session runtime, and live tool registry for ragent.
//!
//! This crate owns the Milestone 7 extracted orchestration layer while keeping
//! compatibility re-exports for the shared config, storage, LLM, and runtime
//! primitives that the moved modules still reference through `crate::*`.

pub mod agent;
pub mod bash_lists;
pub mod config;
pub mod dir_lists;
pub mod error;
pub mod event;
pub mod file_ops;
pub mod hooks;
pub mod id;
pub mod internal_llm;
pub mod mcp;
pub mod memory;
pub mod message;
pub mod orchestrator;
pub mod permission;
pub mod predictive;
pub mod reference;
pub mod resource;
/// Input sanitization and secret redaction utilities.
pub mod sanitize;
pub mod session;
pub mod skill;
pub mod snapshot;
pub mod storage;
pub mod task;
pub mod team;
pub mod tool;
pub mod updater;
pub mod yolo;

pub use ragent_llm::{embedded, llm, provider};
pub use ragent_tools_vcs::{github, gitlab};

// Re-export config types that downstream crates (e.g. ragent-tui) need
pub use config::{ToolVisibilityConfig, tool_family_names};
pub use internal_llm::{
    InternalLlmError, InternalLlmExecutionRequest, InternalLlmExecutor, InternalLlmMetricsSnapshot,
    InternalLlmQueueStatus, InternalLlmResult, InternalLlmService, InternalLlmStatusSnapshot,
    InternalLlmTaskKind, InternalTaskLimits,
};

pub use ragent_llm::{
    AnthropicProvider, CopilotProvider, GeminiProvider, GenericOpenAiProvider, HuggingFaceProvider,
    ModelInfo, OllamaCloudProvider, OllamaProvider, OpenAiProvider, Provider, ProviderInfo,
    ProviderRegistry, UsageInfo, create_default_registry,
};
