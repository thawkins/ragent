//! Configuration system for ragent
//!
//! This crate handles:
//! - Configuration loading from ragent.json / ragent.jsonc
//! - Config merging (global + project + CLI overrides)
//! - Permission rules and checking
//! - Runtime allowlists and denylists (bash, directories)
//! - YOLO mode configuration

pub mod activity_log;
pub mod bash_lists;
pub mod compaction;
pub mod config;
pub mod connectors;
pub mod dir_lists;
pub mod edit_log;
pub mod gcf;
pub mod github;
pub mod permission;
pub mod plugins;
pub mod runtime_flag;
pub mod telemetry;
pub mod trigger;
pub mod user_dirs;
pub mod yolo;

// Re-export commonly used types
pub use compaction::{CompactionConfig, CompactionModelRef, KeepConfig};
pub use config::{
    AgentConfig, AgentPerfConfig, AutoExtractConfig, BrowserConfig, Capabilities, ChannelsConfig,
    Config, Cost, CrossProjectConfig, DEFAULT_ERROR_RETRY_ALLOWANCE, DEFAULT_INPUT_QUEUE_CAPACITY,
    DEFAULT_LOOP_MAX_STEPS, DiscordChannelConfig, GitLabIntegrationConfig, GmailConfig, LoopConfig,
    McpServerConfig, McpTransport, MemoryConfig, ModelConfig, PieGapConfig, PriceEntry,
    ProviderConfig, ResearchConfig, ResearchEvaluateConfig, ResearchModelsConfig,
    ResearchSupervisorConfig, SddConfig, StreamConfig, TelegramChannelConfig, ToolVisibilityConfig,
    tool_family_names,
};
pub use connectors::{
    ConnectorCredentialsConfig, ConnectorStoreEndpoint, ConnectorStoresConfig, ConnectorsConfig,
};
pub use gcf::GcfConfig;
pub use permission::{
    Permission, PermissionAction, PermissionChecker, PermissionDecision, PermissionRequest,
    PermissionRule,
};
pub use plugins::{PluginStoreEndpoint, PluginStoresConfig, PluginsConfig};
pub use telemetry::{OtelConfig, OtelProtocol, TelemetryConfig};
pub use trigger::{McpNotificationMode, TriggerConfig};

/// Re-synchronise every process-wide runtime flag (`yolo`, `edit_log`,
/// `activity_log`, `gcf`) from an already-loaded [`Config`].
///
/// Callers that have just loaded or rewritten the config pass the values they
/// already hold here instead of reaching into each module, so the "which
/// config-backed flags exist" list lives in one place and a new flag cannot be
/// left un-synchronised at a second call site.
pub fn sync_runtime_flags(config: &Config) {
    yolo::sync_from_config_value(config.yolo);
    edit_log::sync_from_config_value(config.edit_log);
    activity_log::sync_from_config_value(config.activity_log);
    gcf::sync_from_config_value(config.gcf.enabled);
}
