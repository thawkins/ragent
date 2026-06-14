//! Configuration system for ragent
//!
//! This crate handles:
//! - Configuration loading from ragent.json / ragent.jsonc
//! - Config merging (global + project + CLI overrides)
//! - Permission rules and checking
//! - Runtime allowlists and denylists (bash, directories)
//! - YOLO mode configuration

pub mod bash_lists;
pub mod compression;
pub mod config;
pub mod dir_lists;
pub mod permission;
pub mod yolo;

// Re-export commonly used types
pub use compression::{
    CcrConfig, CompressionConfig, CompressorConfig, RelevanceConfig, TokenizerConfig,
};
pub use config::{
    AgentConfig, AutoExtractConfig, Capabilities, Config, Cost, CrossProjectConfig,
    InternalLlmConfig, InternalLlmDownloadPolicy, McpServerConfig, McpTransport, MemoryConfig,
    ModelConfig, ProviderConfig, ToolVisibilityConfig, tool_family_names,
};
pub use permission::{PermissionAction, PermissionChecker, PermissionRule};
