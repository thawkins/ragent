//! Agent orchestration, session runtime, and live tool registry for ragent.
//!
//! This crate owns the Milestone 7 extracted orchestration layer while keeping
//! compatibility re-exports for the shared config, storage, LLM, and runtime
//! primitives that the moved modules still reference through `crate::*`.

pub mod agent;
pub mod compaction;
pub mod error;
pub mod event;

pub use ragent_types::event::{Event, EventBus, FinishReason};
pub mod file_ops;
pub mod hooks;
pub mod id;
pub mod mcp;
pub mod memory;
pub mod message;
pub mod orchestrator;
pub mod perf;
pub mod permission;
pub mod reference;
/// Process resource limits — bounded concurrency for child process spawns
/// and tool execution.
///
/// Re-exported from `ragent_types::resource` (DUPPLAN.md Milestone E).
/// Previously duplicated as a local `resource.rs` file; now a single source of
/// truth lives in `ragent_types::resource`.
pub use ragent_types::resource;
/// Input sanitization and secret redaction utilities.
pub mod sanitize;
pub mod session;
pub mod skill;
pub mod snapshot;
pub mod storage;
pub mod task;
pub mod team;
pub mod telemetry;
pub mod tool;
pub mod updater;

/// Shared adapter that wires the agent tool registry into the research
/// system's web/local gatherers.
pub mod research_adapter;

pub use ragent_config::config::StreamConfig;
pub use ragent_config::{
    AgentConfig, Capabilities, Config, Cost, CrossProjectConfig, GitLabIntegrationConfig,
    McpServerConfig, MemoryConfig, ModelConfig, ProviderConfig, ToolVisibilityConfig, bash_lists,
    dir_lists, tool_family_names,
};
pub use ragent_llm::{llm, provider};
pub use ragent_tools_vcs::{github, gitlab};

pub use ragent_llm::{
    AnthropicProvider, CopilotProvider, GeminiProvider, GenericOpenAiProvider, HuggingFaceProvider,
    ModelInfo, OllamaCloudProvider, OllamaProvider, OpenAiProvider, Provider, ProviderInfo,
    ProviderRegistry, UsageInfo, create_default_registry,
};
