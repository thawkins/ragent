use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default = "default_agent_name")]
    pub default_agent: String,
    #[serde(default)]
    pub provider: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub permission: Vec<crate::permission::PermissionRule>,
    #[serde(default)]
    pub agent: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub command: HashMap<String, CommandDef>,
    #[serde(default)]
    pub mcp: HashMap<String, McpServerConfig>,
    #[serde(default)]
    pub instructions: Vec<String>,
    #[serde(default)]
    pub experimental: ExperimentalFlags,
}

fn default_agent_name() -> String {
    "general".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    #[serde(default)]
    pub env: Vec<String>,
    pub api: Option<ApiConfig>,
    #[serde(default)]
    pub models: HashMap<String, ModelConfig>,
    #[serde(default)]
    pub options: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiConfig {
    pub base_url: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelConfig {
    pub name: Option<String>,
    pub cost: Option<Cost>,
    pub capabilities: Option<Capabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cost {
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default = "default_true")]
    pub streaming: bool,
    #[serde(default)]
    pub vision: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentConfig {
    pub name: Option<String>,
    pub model: Option<String>,
    pub variant: Option<String>,
    pub prompt: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub mode: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub permission: Vec<crate::permission::PermissionRule>,
    pub max_steps: Option<u32>,
    #[serde(default)]
    pub options: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    pub command: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    #[serde(rename = "type")]
    pub type_: McpTransport,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub url: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    Stdio,
    Sse,
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentalFlags {
    #[serde(default)]
    pub open_telemetry: bool,
}

impl Default for ExperimentalFlags {
    fn default() -> Self {
        Self {
            open_telemetry: false,
        }
    }
}

impl Config {
    /// Load configuration with precedence:
    /// compiled defaults → global → project → env var → inline content
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
            || base.default_agent == default_agent_name()
        {
            if overlay.default_agent != default_agent_name() {
                base.default_agent = overlay.default_agent;
            }
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
