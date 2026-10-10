//! Configuration system for ragent
//!
//! This crate handles:
//! - Configuration loading from ragent.json / ragent.jsonc
//! - Config merging (global + project + CLI overrides)
//! - Permission rules and checking
//! - Runtime allowlists and denylists (bash, directories)
//! - YOLO mode configuration

pub mod activity_log;
pub mod automation;
pub mod bash_lists;
pub mod compaction;
pub mod config;
pub mod connectors;
pub mod credential_env;
pub mod dir_lists;
pub mod edit_log;
pub mod gcf;
pub mod github;
pub mod i18n;
pub mod list_config;
pub mod openai;
pub mod permission;
pub mod plugins;
pub mod runtime_flag;
pub mod security_analyzer;
pub mod telemetry;
pub mod trigger;
pub mod user_dirs;
pub mod yolo;

// Re-export commonly used types
pub use automation::{
    AutomationConfig, AutomationDefinition, AutomationTriggerKind, DispatchTarget,
};
pub use compaction::{CompactionConfig, CompactionModelRef, KeepConfig};
pub use config::{
    AcpAgentConfig, AcpConfig, AgentConfig, AgentPerfConfig, AutoExtractConfig, BackendConfig,
    Capabilities, Config, Cost, CrossProjectConfig, DEFAULT_ERROR_RETRY_ALLOWANCE,
    DEFAULT_INPUT_QUEUE_CAPACITY, DEFAULT_LOOP_MAX_STEPS, ExecutionBackend, ExecutionBackendKind,
    GitLabIntegrationConfig, LoopConfig, McpServerConfig, McpTransport, MemoryConfig, ModelConfig,
    PieGapConfig, PriceEntry, ProviderConfig, ResearchConfig, ResearchEvaluateConfig,
    ResearchModelsConfig, ResearchSupervisorConfig, SddConfig, StreamConfig, ToolVisibilityConfig,
    default_acp_turn_timeout_secs, tool_family_names,
};
pub use connectors::{
    ConnectorCredentialsConfig, ConnectorStoreEndpoint, ConnectorStoresConfig, ConnectorsConfig,
};
pub use gcf::GcfConfig;
pub use i18n::{I18nConfig, LocaleCatalog, MessageKey};
pub use openai::OpenAiConfig;
pub use permission::{
    Permission, PermissionAction, PermissionChecker, PermissionDecision, PermissionRequest,
    PermissionRule,
};
pub use plugins::{PluginStoreEndpoint, PluginStoresConfig, PluginsConfig};
pub use security_analyzer::{AnalyzerModelRef, SecurityAnalyzerConfig};
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
    // UI i18n (spec `openhands` FR-027): resolve the locale + catalogue from the
    // loaded section (or the ambient locale when the section is absent) so the
    // status bar and help surface render the configured language immediately.
    i18n::sync_from_config(config);
}
