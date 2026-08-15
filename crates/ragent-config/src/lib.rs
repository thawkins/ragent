//! Configuration system for ragent
//!
//! This crate handles:
//! - Configuration loading from ragent.json / ragent.jsonc
//! - Config merging (global + project + CLI overrides)
//! - Permission rules and checking
//! - Runtime allowlists and denylists (bash, directories)
//! - YOLO mode configuration

pub mod bash_lists;
pub mod compaction;
pub mod config;
pub mod dir_lists;
pub mod edit_log;
pub mod permission;
pub mod telemetry;
pub mod trigger;
pub mod yolo;

// Re-export commonly used types
pub use compaction::{CompactionConfig, KeepConfig};
pub use config::{
    AgentConfig, AgentPerfConfig, AutoExtractConfig, BrowserConfig, Capabilities, ChannelsConfig,
    Config, Cost, CrossProjectConfig, DiscordChannelConfig, GitLabIntegrationConfig, GmailConfig,
    McpServerConfig, McpTransport, MemoryConfig, ModelConfig, PieGapConfig, PriceEntry,
    ProviderConfig, ResearchConfig, SddConfig, StreamConfig, TelegramChannelConfig,
    ToolVisibilityConfig, tool_family_names,
};
pub use permission::{
    Permission, PermissionAction, PermissionChecker, PermissionDecision, PermissionRequest,
    PermissionRule,
};
pub use telemetry::{OtelConfig, OtelProtocol, TelemetryConfig};
pub use trigger::{McpNotificationMode, TriggerConfig};
