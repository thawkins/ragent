//! Agent orchestration, session runtime, and live tool registry for ragent.
//!
//! This crate owns the Milestone 7 extracted orchestration layer while keeping
//! compatibility re-exports for the shared config, storage, LLM, and runtime
//! primitives that the moved modules still reference through `crate::*`.

pub mod agent;
/// Public compatibility re-export for callers that still import `ragent_agent::config::*`.
pub mod config {
    pub use ragent_config::config::StreamConfig;
    pub use ragent_config::*;
}
#[cfg(feature = "compression")]
pub mod compression;
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

pub use ragent_config::config::StreamConfig;
pub use ragent_config::{
    AgentConfig, Capabilities, Config, Cost, CrossProjectConfig, GitLabIntegrationConfig,
    MemoryConfig, ModelConfig, ProviderConfig, ToolVisibilityConfig, bash_lists, dir_lists,
    tool_family_names,
};
pub use ragent_llm::{llm, provider};
pub use ragent_tools_vcs::{github, gitlab};

pub use ragent_llm::{
    AnthropicProvider, CopilotProvider, GeminiProvider, GenericOpenAiProvider, HuggingFaceProvider,
    ModelInfo, OllamaCloudProvider, OllamaProvider, OpenAiProvider, Provider, ProviderInfo,
    ProviderRegistry, UsageInfo, create_default_registry,
};
