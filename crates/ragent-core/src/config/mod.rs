//! Configuration loading, merging, and types for ragent.
//!
//! [`Config`] is loaded via [`Config::load`] with a layered precedence:
//! compiled defaults → global file → project file → `RAGENT_CONFIG` env →
//! `RAGENT_CONFIG_CONTENT` env. Provider, agent, MCP server, and permission
//! settings are all configured here.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Top-level ragent configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Display name of the user.
    #[serde(default)]
    pub username: Option<String>,
    /// Name of the agent to use when none is specified.
    #[serde(default = "default_agent_name")]
    pub default_agent: String,
    /// LLM provider configurations keyed by provider id.
    #[serde(default)]
    pub provider: HashMap<String, ProviderConfig>,
    /// Global permission rules applied to all agents.
    #[serde(default)]
    pub permission: Vec<crate::permission::PermissionRule>,
    /// Per-agent configuration overrides keyed by agent name.
    #[serde(default)]
    pub agent: HashMap<String, AgentConfig>,
    /// User-defined slash-command shortcuts.
    #[serde(default)]
    pub command: HashMap<String, CommandDef>,
    /// MCP server definitions keyed by server id.
    #[serde(default)]
    pub mcp: HashMap<String, McpServerConfig>,
    /// Additional instruction strings appended to agent prompts.
    #[serde(default)]
    pub instructions: Vec<String>,
    /// Feature flags for experimental functionality.
    #[serde(default)]
    pub experimental: ExperimentalFlags,
}

fn default_agent_name() -> String {
    "general".to_string()
}

/// Configuration for a single LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    /// Environment variable names required by this provider (e.g. API keys).
    #[serde(default)]
    pub env: Vec<String>,
    /// Optional API endpoint and header overrides.
    pub api: Option<ApiConfig>,
    /// Model definitions available through this provider.
    #[serde(default)]
    pub models: HashMap<String, ModelConfig>,
    /// Arbitrary provider-specific options.
    // TODO: Replace `Value` with typed provider option structs per-provider.
    #[serde(default)]
    pub options: HashMap<String, Value>,
}

/// API endpoint configuration for a provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiConfig {
    /// Base URL for API requests (overrides the provider default).
    pub base_url: Option<String>,
    /// Extra HTTP headers sent with every request.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

/// Metadata and pricing for a single model within a provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelConfig {
    /// Human-readable display name for the model.
    pub name: Option<String>,
    /// Token pricing information.
    pub cost: Option<Cost>,
    /// Feature capabilities of this model.
    pub capabilities: Option<Capabilities>,
}

/// Per-token cost for a model (USD per million tokens).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cost {
    /// Cost per million input tokens.
    pub input: f64,
    /// Cost per million output tokens.
    pub output: f64,
}

/// Feature flags describing what a model supports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    /// Whether the model supports chain-of-thought reasoning.
    #[serde(default)]
    pub reasoning: bool,
    /// Whether the model supports streaming responses.
    #[serde(default = "default_true")]
    pub streaming: bool,
    /// Whether the model can process image inputs.
    #[serde(default)]
    pub vision: bool,
    /// Whether the model supports tool/function calling.
    #[serde(default = "default_true")]
    pub tool_use: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            reasoning: false,
            streaming: true,
            vision: false,
            tool_use: true,
        }
    }
}

/// Per-agent configuration overrides applied on top of built-in defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    /// Display name override.
    pub name: Option<String>,
    /// Model identifier in `"provider:model"` format.
    pub model: Option<String>,
    /// Agent variant selector.
    pub variant: Option<String>,
    /// System prompt override.
    pub prompt: Option<String>,
    /// Sampling temperature override.
    pub temperature: Option<f32>,
    /// Top-p (nucleus) sampling override.
    pub top_p: Option<f32>,
    /// Agent mode override (`"primary"`, `"subagent"`, or `"all"`).
    pub mode: Option<String>,
    /// Whether to hide this agent from user-facing listings.
    #[serde(default)]
    pub hidden: bool,
    /// Permission rules specific to this agent.
    #[serde(default)]
    pub permission: Vec<crate::permission::PermissionRule>,
    /// Maximum agentic loop iterations.
    pub max_steps: Option<u32>,
    /// Arbitrary agent-specific options.
    // TODO: Replace `Value` with typed agent option structs.
    #[serde(default)]
    pub options: HashMap<String, Value>,
}

/// A user-defined slash-command shortcut.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    /// Shell command to execute.
    pub command: String,
    /// Human-readable description shown in help output.
    pub description: String,
}

/// Configuration for an MCP (Model Context Protocol) server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Transport mechanism used to communicate with the server.
    #[serde(rename = "type")]
    pub type_: McpTransport,
    /// Executable path or name (for stdio transport).
    pub command: Option<String>,
    /// Command-line arguments passed to the server process.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables injected into the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// URL endpoint (for SSE or HTTP transports).
    pub url: Option<String>,
    /// If `true`, this server is configured but will not be started.
    #[serde(default)]
    pub disabled: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            type_: McpTransport::Stdio,
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            url: None,
            disabled: false,
        }
    }
}

/// Transport protocol for MCP server communication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    /// Communicate over the server process's stdin/stdout.
    Stdio,
    /// Communicate via Server-Sent Events over HTTP.
    Sse,
    /// Communicate via plain HTTP request/response.
    Http,
}

/// Flags gating experimental features that may change or be removed.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExperimentalFlags {
    /// Enable OpenTelemetry trace export.
    #[serde(default)]
    pub open_telemetry: bool,
}

impl Config {
    /// Load configuration with precedence:
    /// compiled defaults → global → project → env var → inline content
    ///
    /// # Errors
    ///
    /// Returns an error if a config file cannot be read or contains invalid JSON.
    pub fn load() -> anyhow::Result<Self> {
        let mut config = Config::default();

        // Global config: ~/.config/ragent/ragent.json
        if let Some(config_dir) = dirs::config_dir() {
            let global_path = config_dir.join("ragent").join("ragent.json");
            if global_path.exists() {
                let overlay = Self::load_file(&global_path)?;
                config = Self::merge(config, overlay);
            }
        }

        // Project config: ./ragent.json
        let project_path = PathBuf::from("ragent.json");
        if project_path.exists() {
            let overlay = Self::load_file(&project_path)?;
            config = Self::merge(config, overlay);
        }

        // Environment variable pointing to config file
        if let Ok(env_path) = std::env::var("RAGENT_CONFIG") {
            let path = PathBuf::from(&env_path);
            if path.exists() {
                let overlay = Self::load_file(&path)?;
                config = Self::merge(config, overlay);
            }
        }

        // Inline config from environment variable
        if let Ok(content) = std::env::var("RAGENT_CONFIG_CONTENT") {
            let overlay: Config = serde_json::from_str(&content)?;
            config = Self::merge(config, overlay);
        }

        Ok(config)
    }

    fn load_file(path: &Path) -> anyhow::Result<Config> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Deep merge two configs, with overlay taking precedence for set fields.
    pub fn merge(mut base: Config, overlay: Config) -> Config {
        if overlay.username.is_some() {
            base.username = overlay.username;
        }
        if overlay.default_agent != default_agent_name()
            && (overlay.default_agent != default_agent_name()
                || base.default_agent == default_agent_name())
        {
            base.default_agent = overlay.default_agent;
        }
        // Merge hash maps by extending (overlay wins on conflicts)
        for (k, v) in overlay.provider {
            base.provider.insert(k, v);
        }
        for (k, v) in overlay.agent {
            base.agent.insert(k, v);
        }
        for (k, v) in overlay.command {
            base.command.insert(k, v);
        }
        for (k, v) in overlay.mcp {
            base.mcp.insert(k, v);
        }
        // Permissions and instructions append
        base.permission.extend(overlay.permission);
        base.instructions.extend(overlay.instructions);

        if overlay.experimental.open_telemetry {
            base.experimental.open_telemetry = true;
        }

        base
    }
}
