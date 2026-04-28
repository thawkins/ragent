//! Configuration loading, merging, and types for ragent.
//!
//! [`Config`] is loaded via [`Config::load`] with a layered precedence:
//! compiled defaults → global file → project file → `RAGENT_CONFIG` env →
//! `RAGENT_CONFIG_CONTENT` env. Provider, agent, MCP server, and permission
//! settings are all configured here.

use serde::{Deserialize, Deserializer, Serialize};
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
    /// Additional directories to scan for skill definitions.
    #[serde(default)]
    pub skill_dirs: Vec<String>,
    /// Feature flags for experimental functionality.
    #[serde(default)]
    pub experimental: ExperimentalFlags,
    /// Lifecycle hooks (placeholder - hooks module not yet extracted).
    #[serde(default)]
    pub hooks: Vec<serde_json::Value>,
    /// User-defined bash command allowlist and denylist additions.
    #[serde(default)]
    pub bash: BashConfig,
    /// User-defined directory/file path allowlist and denylist additions.
    #[serde(default)]
    pub dirs: DirsConfig,
    /// Code index configuration (codebase indexing & search).
    #[serde(default)]
    pub code_index: CodeIndexConfig,
    /// LLM streaming configuration (timeouts, retries).
    #[serde(default)]
    pub stream: StreamConfig,
    /// Embedded internal LLM configuration for local helper tasks.
    #[serde(default)]
    pub internal_llm: InternalLlmConfig,
    /// Memory system configuration (blocks, structured store, retrieval).
    #[serde(default)]
    pub memory: MemoryConfig,
    /// GitLab integration configuration.
    #[serde(default)]
    pub gitlab: GitLabIntegrationConfig,
    /// Tool-family visibility switches.
    /// When a switch is `false`, all tools in that family are hidden from the LLM.
    #[serde(default)]
    pub tool_visibility: ToolVisibilityConfig,
    /// Tool names to hide from the LLM (excluded from tool definitions and system-prompt listings).
    /// Hidden tools remain registered and executable; they are simply not advertised to the model.
    ///
    /// Example — suppress all GitHub and GitLab tools:
    /// ```json
    /// { "hidden_tools": ["github_list_issues", "github_get_issue", "gitlab_list_mrs"] }
    /// ```
    #[serde(default)]
    pub hidden_tools: Vec<String>,
}

/// Tool-family visibility configuration.
///
/// Controls which tool families are advertised to the LLM. Each switch
/// corresponds to a group of related tools. When a switch is `false`,
/// all tools in that family are suppressed from `ToolRegistry::definitions()`.
/// Tools remain registered and executable regardless of visibility.
#[derive(Debug, Clone, Default)]
struct ToolVisibilitySpecified {
    office: bool,
    journal: bool,
    github: bool,
    gitlab: bool,
    codeindex: bool,
}

/// Tool-family visibility configuration.
///
/// The config loader tracks which switches were explicitly present in the
/// source JSON so merge operations can preserve base values for omitted fields.
#[derive(Debug, Clone, Serialize)]
pub struct ToolVisibilityConfig {
    /// Office document tools (office_read, office_write, office_info, libre_read, etc.).
    #[serde(default = "default_false")]
    pub office: bool,
    /// Journal tools (journal_write, journal_search, journal_read).
    #[serde(default = "default_false")]
    pub journal: bool,
    /// GitHub tools (github_list_issues, github_get_issue, github_create_issue, etc.).
    #[serde(default = "default_false")]
    pub github: bool,
    /// GitLab tools (gitlab_list_issues, gitlab_get_issue, gitlab_create_mr, etc.).
    #[serde(default = "default_false")]
    pub gitlab: bool,
    /// Code-index tools (codeindex_search, codeindex_status, codeindex_symbols, etc.).
    /// Default `true` — codeindex tools are visible when the subsystem is enabled.
    #[serde(default = "default_true")]
    pub codeindex: bool,
    #[serde(skip)]
    specified: ToolVisibilitySpecified,
}

impl Default for ToolVisibilityConfig {
    fn default() -> Self {
        Self {
            office: false,
            journal: false,
            github: false,
            gitlab: false,
            codeindex: true,
            specified: ToolVisibilitySpecified::default(),
        }
    }
}

impl ToolVisibilityConfig {
    /// Iterate over every tool-visibility switch and its enabled state.
    pub fn iter_switches(&self) -> impl Iterator<Item = (&'static str, bool)> {
        [
            ("office", self.office),
            ("journal", self.journal),
            ("github", self.github),
            ("gitlab", self.gitlab),
            ("codeindex", self.codeindex),
        ]
        .into_iter()
    }
}

impl<'de> Deserialize<'de> for ToolVisibilityConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        struct RawToolVisibilityConfig {
            office: Option<bool>,
            journal: Option<bool>,
            github: Option<bool>,
            gitlab: Option<bool>,
            codeindex: Option<bool>,
        }

        let raw = RawToolVisibilityConfig::deserialize(deserializer)?;
        Ok(Self {
            office: raw.office.unwrap_or_else(default_false),
            journal: raw.journal.unwrap_or_else(default_false),
            github: raw.github.unwrap_or_else(default_false),
            gitlab: raw.gitlab.unwrap_or_else(default_false),
            codeindex: raw.codeindex.unwrap_or_else(default_true),
            specified: ToolVisibilitySpecified {
                office: raw.office.is_some(),
                journal: raw.journal.is_some(),
                github: raw.github.is_some(),
                gitlab: raw.gitlab.is_some(),
                codeindex: raw.codeindex.is_some(),
            },
        })
    }
}

const fn default_false() -> bool {
    false
}

/// Map a visibility switch to the list of tool names it governs.
pub fn tool_family_names(switch: &str) -> Option<&'static [&'static str]> {
    match switch {
        "office" => Some(&[
            "office_read",
            "office_write",
            "office_info",
            "libre_read",
            "libre_write",
            "libre_info",
            "pdf_read",
            "pdf_write",
        ]),
        "journal" => Some(&["journal_write", "journal_search", "journal_read"]),
        "github" => Some(&[
            "github_list_issues",
            "github_get_issue",
            "github_create_issue",
            "github_comment_issue",
            "github_close_issue",
            "github_list_prs",
            "github_get_pr",
            "github_create_pr",
            "github_merge_pr",
            "github_review_pr",
        ]),
        "gitlab" => Some(&[
            "gitlab_list_issues",
            "gitlab_get_issue",
            "gitlab_create_issue",
            "gitlab_comment_issue",
            "gitlab_close_issue",
            "gitlab_list_mrs",
            "gitlab_get_mr",
            "gitlab_create_mr",
            "gitlab_merge_mr",
            "gitlab_approve_mr",
            "gitlab_list_pipelines",
            "gitlab_get_pipeline",
            "gitlab_cancel_pipeline",
            "gitlab_retry_pipeline",
            "gitlab_list_jobs",
            "gitlab_get_job",
            "gitlab_get_job_log",
            "gitlab_cancel_job",
            "gitlab_retry_job",
        ]),
        "codeindex" => Some(&[
            "codeindex_search",
            "codeindex_status",
            "codeindex_symbols",
            "codeindex_references",
            "codeindex_dependencies",
            "codeindex_reindex",
        ]),
        _ => None,
    }
}

/// Configuration for LLM streaming behaviour (timeouts, retries)./// ```json
/// {
///   "stream": {
///     "timeout_secs": 600,
///     "max_retries": 4,
///     "retry_backoff_secs": 2
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Seconds of silence before a stream is considered stalled (default: 600).
    #[serde(default = "default_stream_timeout_secs")]
    pub timeout_secs: u64,
    /// Maximum number of retry attempts after a stall or connection failure (default: 4).
    #[serde(default = "default_stream_max_retries")]
    pub max_retries: u32,
    /// Backoff multiplier per retry attempt in seconds (default: 2).
    /// Attempt N waits `N * retry_backoff_secs` seconds before retrying.
    #[serde(default = "default_stream_retry_backoff_secs")]
    pub retry_backoff_secs: u64,
}

const fn default_stream_timeout_secs() -> u64 {
    600
}

const fn default_stream_max_retries() -> u32 {
    4
}

const fn default_stream_retry_backoff_secs() -> u64 {
    2
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_stream_timeout_secs(),
            max_retries: default_stream_max_retries(),
            retry_backoff_secs: default_stream_retry_backoff_secs(),
        }
    }
}

/// Persistent configuration for the code-index subsystem.
///
/// Runtime-derived fields like `project_root` and `index_dir` are
/// resolved at startup, not stored in the config file.
#[derive(Debug, Clone, Default)]
struct CodeIndexSpecified {
    enabled: bool,
    max_file_size: bool,
    extra_exclude_dirs: bool,
    extra_exclude_patterns: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeIndexConfig {
    /// Whether code indexing is enabled.
    #[serde(default = "default_code_index_enabled")]
    pub enabled: bool,
    /// Maximum file size in bytes to index (default: 1 MB).
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// Additional directory names to exclude from scanning.
    #[serde(default)]
    pub extra_exclude_dirs: Vec<String>,
    /// Additional glob patterns to exclude from scanning.
    #[serde(default)]
    pub extra_exclude_patterns: Vec<String>,
    #[serde(skip_serializing, default)]
    specified: CodeIndexSpecified,
}

const fn default_code_index_enabled() -> bool {
    true
}

const fn default_max_file_size() -> u64 {
    1_048_576 // 1 MB
}

impl Default for CodeIndexConfig {
    fn default() -> Self {
        Self {
            enabled: default_code_index_enabled(),
            max_file_size: default_max_file_size(),
            extra_exclude_dirs: Vec::new(),
            extra_exclude_patterns: Vec::new(),
            specified: CodeIndexSpecified::default(),
        }
    }
}

impl<'de> Deserialize<'de> for CodeIndexConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawCodeIndexConfig {
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default)]
            max_file_size: Option<u64>,
            #[serde(default)]
            extra_exclude_dirs: Option<Vec<String>>,
            #[serde(default)]
            extra_exclude_patterns: Option<Vec<String>>,
        }

        let raw = RawCodeIndexConfig::deserialize(deserializer)?;
        let mut config = Self::default();

        if let Some(enabled) = raw.enabled {
            config.enabled = enabled;
            config.specified.enabled = true;
        }
        if let Some(max_file_size) = raw.max_file_size {
            config.max_file_size = max_file_size;
            config.specified.max_file_size = true;
        }
        if let Some(extra_exclude_dirs) = raw.extra_exclude_dirs {
            config.extra_exclude_dirs = extra_exclude_dirs;
            config.specified.extra_exclude_dirs = true;
        }
        if let Some(extra_exclude_patterns) = raw.extra_exclude_patterns {
            config.extra_exclude_patterns = extra_exclude_patterns;
            config.specified.extra_exclude_patterns = true;
        }

        Ok(config)
    }
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
    /// Default thinking/reasoning configuration for models under this provider.
    /// Used when a per-model `thinking` override is not present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ragent_types::ThinkingConfig>,
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
    /// Default thinking/reasoning configuration for this model.
    /// When set, this overrides any provider-level default and acts as
    /// the fallback if no user-level choice is made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ragent_types::ThinkingConfig>,
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
    /// Which thinking/reasoning levels this model supports.
    /// Empty vec means no thinking support. Populated from built-in model
    /// definitions and may be extended by provider discovery APIs.
    #[serde(default)]
    pub thinking_levels: Vec<ragent_types::ThinkingLevel>,
}

const fn default_true() -> bool {
    true
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            reasoning: false,
            streaming: true,
            vision: false,
            tool_use: true,
            thinking_levels: Vec::new(),
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
    /// Skill names to preload into this agent's prompt context.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Arbitrary agent-specific options.
    // TODO: Replace `Value` with typed agent option structs.
    #[serde(default)]
    pub options: HashMap<String, Value>,
}

/// User-defined additions to the bash command allowlist and denylist.
///
/// Entries in `allowlist` are command prefixes that bypass the built-in
/// banned-command check (e.g. `"curl"` to allow curl).  Entries in
/// `denylist` are substring patterns that always reject a command (e.g.
/// `"git push --force"`).  Both global and project configs are merged —
/// the union of all entries is used.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BashConfig {
    /// Command prefixes exempted from the banned-command check.
    #[serde(default)]
    pub allowlist: Vec<String>,
    /// Patterns that unconditionally reject a command.
    #[serde(default)]
    pub denylist: Vec<String>,
}

/// Configuration for directory/file path allow and deny lists.
///
/// Entries in `allowlist` are glob patterns (e.g. `"src/**"`, `"*.rs"`) that
/// automatically grant permission for read/edit operations without prompting.
/// Entries in `denylist` are glob patterns that unconditionally reject access
/// (e.g. `"secrets/**"`, `"/etc/**"`). Both global and project configs are merged —
/// the union of all entries is used. Denylist takes precedence over allowlist.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirsConfig {
    /// Glob patterns for paths that are automatically allowed (no prompt).
    #[serde(default)]
    pub allowlist: Vec<String>,
    /// Glob patterns for paths that are unconditionally rejected.
    #[serde(default)]
    pub denylist: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Flags for experimental features that are not yet stable.
pub struct ExperimentalFlags {
    /// Enable OpenTelemetry trace export.
    #[serde(default)]
    pub open_telemetry: bool,
    /// Allow multiple tool calls from a single model turn to execute in parallel.
    ///
    /// Disabled by default so tool calls execute sequentially and each follow-up
    /// prompt is based on the completed result of the previous call.
    #[serde(default)]
    pub parallel_tool_calls: bool,
    /// Maximum number of concurrent background sub-agent tasks (F14).
    #[serde(default = "default_max_background_agents")]
    pub max_background_agents: usize,
    /// Timeout in seconds for background sub-agent tasks (F14).
    #[serde(default = "default_background_agent_timeout")]
    pub background_agent_timeout: u64,
}

impl Default for ExperimentalFlags {
    fn default() -> Self {
        Self {
            open_telemetry: false,
            parallel_tool_calls: false,
            max_background_agents: default_max_background_agents(),
            background_agent_timeout: default_background_agent_timeout(),
        }
    }
}

const fn default_max_background_agents() -> usize {
    4
}

const fn default_background_agent_timeout() -> u64 {
    3600
}

impl Config {
    /// Load configuration with precedence:
    /// compiled defaults → global → project → env var → inline content
    ///
    /// # Errors
    ///
    /// Returns an error if a config file cannot be read or contains invalid JSON.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_config::Config;
    ///
    /// let config = Config::load().expect("failed to load config");
    /// println!("default agent: {}", config.default_agent);
    /// ```
    pub fn load() -> anyhow::Result<Self> {
        let mut config = Self::default();

        // Global config: ~/.config/ragent/ragent.json
        if let Some(config_dir) = dirs::config_dir() {
            let global_path = config_dir.join("ragent").join("ragent.json");
            if global_path.exists() {
                let overlay = Self::load_file(&global_path)?;
                config = Self::merge(config, overlay);
            }
        }

        // Project config: ./.ragent/ragent.json
        let project_path = PathBuf::from(".ragent").join("ragent.json");
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
            let overlay: Self = serde_json::from_str(&content).map_err(|e| {
                let line = e.line();
                let column = e.column();
                let problematic_line = content
                    .lines()
                    .nth(line.saturating_sub(1))
                    .unwrap_or("<line not found>");

                anyhow::anyhow!(
                    "Failed to parse RAGENT_CONFIG_CONTENT environment variable:\n\
                     Error at line {}, column {}:\n\
                     {}\n\
                     Problematic line:\n\
                     {}\n\
                     {}^\n\
                     Parse error: {}",
                    line,
                    column,
                    "─".repeat(80),
                    problematic_line,
                    " ".repeat(column.saturating_sub(1)),
                    e
                )
            })?;
            config = Self::merge(config, overlay);
        }

        Ok(config)
    }

    pub(crate) fn load_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Failed to read config file '{}': {}", path.display(), e)
        })?;

        let config: Self = serde_json::from_str(&content).map_err(|e| {
            // Extract line and column from serde_json error
            let line = e.line();
            let column = e.column();

            // Get the problematic line from the content
            let problematic_line = content
                .lines()
                .nth(line.saturating_sub(1))
                .unwrap_or("<line not found>");

            anyhow::anyhow!(
                "Failed to parse config file '{}':\n\
                 Error at line {}, column {}:\n\
                 {}\n\
                 Problematic line:\n\
                 {}\n\
                 {}^\n\
                 Parse error: {}",
                path.display(),
                line,
                column,
                "─".repeat(80),
                problematic_line,
                " ".repeat(column.saturating_sub(1)),
                e
            )
        })?;

        Ok(config)
    }

    /// Save the config back to a file.
    ///
    /// Writes the current config as pretty-printed JSON. The path is
    /// the project-local config file (`.ragent/ragent.json`) if
    /// `prefer_project` is true and the project directory exists,
    /// otherwise the global config file (`~/.config/ragent/ragent.json`).
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, prefer_project: bool) -> anyhow::Result<()> {
        let path = if prefer_project {
            let project = PathBuf::from(".ragent/ragent.json");
            if project.parent().map_or(false, |p| p.exists()) {
                project
            } else {
                dirs::config_dir()
                    .map(|d| d.join("ragent").join("ragent.json"))
                    .ok_or_else(|| anyhow::anyhow!("no config directory found"))?
            }
        } else {
            dirs::config_dir()
                .map(|d| d.join("ragent").join("ragent.json"))
                .ok_or_else(|| anyhow::anyhow!("no config directory found"))?
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialise config: {}", e))?;

        std::fs::write(&path, json)
            .map_err(|e| anyhow::anyhow!("Failed to write config file '{}': {}", path.display(), e))
    }

    /// Compute the complete hidden-tool set from both legacy per-tool overrides
    /// and the tool-family visibility switches.
    #[must_use]
    pub fn effective_hidden_tools(&self) -> Vec<String> {
        let mut hidden: std::collections::HashSet<String> =
            self.hidden_tools.iter().cloned().collect();

        for (switch, enabled) in self.tool_visibility.iter_switches() {
            if enabled {
                continue;
            }
            if let Some(names) = tool_family_names(switch) {
                hidden.extend(names.iter().map(|name| (*name).to_string()));
            }
        }

        let mut hidden: Vec<String> = hidden.into_iter().collect();
        hidden.sort();
        hidden
    }

    /// Deep merge two configs, with overlay taking precedence for set fields.    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_config::Config;
    ///
    /// let base = Config::default();
    /// let mut overlay = Config::default();
    /// overlay.username = Some("alice".to_string());
    ///
    /// let merged = Config::merge(base, overlay);
    /// assert_eq!(merged.username.as_deref(), Some("alice"));
    /// ```
    #[must_use]
    pub fn merge(mut base: Self, overlay: Self) -> Self {
        if overlay.username.is_some() {
            base.username = overlay.username;
        }
        if overlay.default_agent != default_agent_name()
            && (overlay.default_agent != default_agent_name()
                || base.default_agent == default_agent_name())
        {
            base.default_agent = overlay.default_agent;
        }
        // Merge provider config deeply so partial overlays do not discard model,
        // API, or thinking defaults from lower-precedence config files.
        for (k, v) in overlay.provider {
            let merged = if let Some(existing) = base.provider.remove(&k) {
                Self::merge_provider_config(existing, v)
            } else {
                v
            };
            base.provider.insert(k, merged);
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
        // Permissions, instructions, and skill dirs append
        base.permission.extend(overlay.permission);
        base.instructions.extend(overlay.instructions);
        base.skill_dirs.extend(overlay.skill_dirs);

        if overlay.experimental.open_telemetry {
            base.experimental.open_telemetry = true;
        }
        if overlay.experimental.parallel_tool_calls {
            base.experimental.parallel_tool_calls = true;
        }

        // Hooks append (overlay hooks are added on top of base hooks)
        base.hooks.extend(overlay.hooks);

        // Bash lists are unioned across global + project configs
        for entry in overlay.bash.allowlist {
            if !base.bash.allowlist.contains(&entry) {
                base.bash.allowlist.push(entry);
            }
        }
        for entry in overlay.bash.denylist {
            if !base.bash.denylist.contains(&entry) {
                base.bash.denylist.push(entry);
            }
        }

        // GitLab: overlay fields override base
        if overlay.gitlab.instance_url.is_some() {
            base.gitlab.instance_url = overlay.gitlab.instance_url;
        }
        if overlay.gitlab.token.is_some() {
            base.gitlab.token = overlay.gitlab.token;
        }
        if overlay.gitlab.username.is_some() {
            base.gitlab.username = overlay.gitlab.username;
        }

        // hidden_tools: union of base and overlay (both lists are honoured)
        for name in overlay.hidden_tools {
            if !base.hidden_tools.contains(&name) {
                base.hidden_tools.push(name);
            }
        }

        // code_index: overlay takes precedence only for explicitly set fields
        if overlay.code_index.specified.enabled {
            base.code_index.enabled = overlay.code_index.enabled;
        }
        if overlay.code_index.specified.max_file_size {
            base.code_index.max_file_size = overlay.code_index.max_file_size;
        }
        if overlay.code_index.specified.extra_exclude_dirs {
            base.code_index.extra_exclude_dirs = overlay.code_index.extra_exclude_dirs;
        }
        if overlay.code_index.specified.extra_exclude_patterns {
            base.code_index.extra_exclude_patterns = overlay.code_index.extra_exclude_patterns;
        }

        // internal_llm: overlay takes precedence only for explicitly set fields
        if overlay.internal_llm.specified.enabled {
            base.internal_llm.enabled = overlay.internal_llm.enabled;
        }
        if overlay.internal_llm.specified.backend {
            base.internal_llm.backend = overlay.internal_llm.backend;
        }
        if overlay.internal_llm.specified.model_id {
            base.internal_llm.model_id = overlay.internal_llm.model_id;
        }
        if overlay.internal_llm.specified.artifact_max_bytes {
            base.internal_llm.artifact_max_bytes = overlay.internal_llm.artifact_max_bytes;
        }
        if overlay.internal_llm.specified.threads {
            base.internal_llm.threads = overlay.internal_llm.threads;
        }
        if overlay.internal_llm.specified.gpu_layers {
            base.internal_llm.gpu_layers = overlay.internal_llm.gpu_layers;
        }
        if overlay.internal_llm.specified.context_window {
            base.internal_llm.context_window = overlay.internal_llm.context_window;
        }
        if overlay.internal_llm.specified.max_output_tokens {
            base.internal_llm.max_output_tokens = overlay.internal_llm.max_output_tokens;
        }
        if overlay.internal_llm.specified.timeout_ms {
            base.internal_llm.timeout_ms = overlay.internal_llm.timeout_ms;
        }
        if overlay.internal_llm.specified.max_parallel_requests {
            base.internal_llm.max_parallel_requests = overlay.internal_llm.max_parallel_requests;
        }
        if overlay.internal_llm.specified.download_policy {
            base.internal_llm.download_policy = overlay.internal_llm.download_policy;
        }
        if overlay.internal_llm.specified.allowed_tasks {
            base.internal_llm.allowed_tasks = overlay.internal_llm.allowed_tasks;
        }
        if overlay.internal_llm.specified.session_title_enabled {
            base.internal_llm.session_title_enabled = overlay.internal_llm.session_title_enabled;
        }
        if overlay.internal_llm.specified.prompt_context_enabled {
            base.internal_llm.prompt_context_enabled = overlay.internal_llm.prompt_context_enabled;
        }
        if overlay.internal_llm.specified.memory_extraction_enabled {
            base.internal_llm.memory_extraction_enabled =
                overlay.internal_llm.memory_extraction_enabled;
        }

        // tool_visibility: overlay takes precedence only for explicitly set fields
        if overlay.tool_visibility.specified.office {
            base.tool_visibility.office = overlay.tool_visibility.office;
        }
        if overlay.tool_visibility.specified.journal {
            base.tool_visibility.journal = overlay.tool_visibility.journal;
        }
        if overlay.tool_visibility.specified.github {
            base.tool_visibility.github = overlay.tool_visibility.github;
        }
        if overlay.tool_visibility.specified.gitlab {
            base.tool_visibility.gitlab = overlay.tool_visibility.gitlab;
        }
        if overlay.tool_visibility.specified.codeindex {
            base.tool_visibility.codeindex = overlay.tool_visibility.codeindex;
        }

        base
    }

    fn merge_provider_config(mut base: ProviderConfig, overlay: ProviderConfig) -> ProviderConfig {
        if !overlay.env.is_empty() {
            base.env = overlay.env;
        }
        if let Some(overlay_api) = overlay.api {
            if let Some(base_api) = base.api.as_mut() {
                if overlay_api.base_url.is_some() {
                    base_api.base_url = overlay_api.base_url;
                }
                base_api.headers.extend(overlay_api.headers);
            } else {
                base.api = Some(overlay_api);
            }
        }
        if overlay.thinking.is_some() {
            base.thinking = overlay.thinking;
        }
        for (model_id, overlay_model) in overlay.models {
            let merged = if let Some(existing) = base.models.remove(&model_id) {
                Self::merge_model_config(existing, overlay_model)
            } else {
                overlay_model
            };
            base.models.insert(model_id, merged);
        }
        base.options.extend(overlay.options);
        base
    }

    fn merge_model_config(mut base: ModelConfig, overlay: ModelConfig) -> ModelConfig {
        if overlay.name.is_some() {
            base.name = overlay.name;
        }
        if overlay.cost.is_some() {
            base.cost = overlay.cost;
        }
        if overlay.capabilities.is_some() {
            base.capabilities = overlay.capabilities;
        }
        if overlay.thinking.is_some() {
            base.thinking = overlay.thinking;
        }
        base
    }

    /// Returns the configured thinking default for the given provider/model.
    ///
    /// Model-level configuration overrides provider-level configuration.
    #[must_use]
    pub fn thinking_config_for_model(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Option<ragent_types::ThinkingConfig> {
        self.provider.get(provider_id).and_then(|provider| {
            provider
                .models
                .get(model_id)
                .and_then(|model| model.thinking.clone())
                .or_else(|| provider.thinking.clone())
        })
    }
}
// ── GitLab integration configuration ─────────────────────────────────────────

/// GitLab integration configuration.
///
/// Provides connection details for a GitLab instance. Values set here
/// override those stored in the ragent database (set via `/gitlab setup`).
/// Environment variables (`GITLAB_TOKEN`, `GITLAB_URL`, `GITLAB_USERNAME`)
/// take the highest priority.
///
/// Override in `ragent.json`:
/// ```json
/// {
///   "gitlab": {
///     "instance_url": "https://gitlab.example.com",
///     "token": "glpat-xxxxxxxxxxxx",
///     "username": "myuser"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitLabIntegrationConfig {
    /// GitLab instance base URL, e.g. `https://gitlab.com`.
    pub instance_url: Option<String>,
    /// Personal Access Token for the GitLab API.
    pub token: Option<String>,
    /// GitLab username / identity.
    pub username: Option<String>,
}

// ── Embedded internal LLM configuration ───────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct InternalLlmSpecified {
    enabled: bool,
    backend: bool,
    model_id: bool,
    artifact_max_bytes: bool,
    threads: bool,
    gpu_layers: bool,
    context_window: bool,
    max_output_tokens: bool,
    timeout_ms: bool,
    max_parallel_requests: bool,
    download_policy: bool,
    allowed_tasks: bool,
    session_title_enabled: bool,
    prompt_context_enabled: bool,
    memory_extraction_enabled: bool,
}

/// Download policy for embedded internal-LLM assets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InternalLlmDownloadPolicy {
    /// Download missing artifacts only when the runtime first needs them.
    #[default]
    OnDemand,
    /// Never download automatically; require the files to already exist locally.
    Never,
    /// Download proactively during future startup/setup flows.
    Prefetch,
}

/// Embedded internal-LLM configuration.
///
/// Controls the dormant local-runtime scaffold used for internal helper tasks.
/// The runtime stays disabled unless `enabled` is set to `true`, and later
/// phases decide which internal call sites may consume it.
///
/// ```json
/// {
///   "internal_llm": {
///     "enabled": false,
///     "backend": "embedded",
///     "model_id": "smollm2-360m-instruct-q4",
///     "artifact_max_bytes": 1073741824,
///     "threads": 4,
///     "gpu_layers": 0,
///     "context_window": 4096,
///     "max_output_tokens": 1024,
///     "timeout_ms": 300000,
///     "max_parallel_requests": 2,
///     "download_policy": "on_demand",
///     "allowed_tasks": ["session_title", "summarize_tool_output"],
///     "session_title_enabled": false,
///     "prompt_context_enabled": false,
///     "memory_extraction_enabled": false
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InternalLlmConfig {
    /// Whether the embedded internal LLM is enabled.
    #[serde(default = "default_internal_llm_enabled")]
    pub enabled: bool,
    /// Backend identifier for the embedded runtime.
    #[serde(default = "default_internal_llm_backend")]
    pub backend: String,
    /// Model identifier used by the embedded runtime.
    #[serde(default = "default_internal_llm_model_id")]
    pub model_id: String,
    /// Maximum allowed artifact size in bytes.
    #[serde(default = "default_internal_llm_artifact_max_bytes")]
    pub artifact_max_bytes: u64,
    /// Requested CPU thread count for Candle's Rayon-backed CPU execution path.
    ///
    /// The effective value is reported at startup and in `/internal-llm show`
    /// because the process-global Rayon pool can only be configured once.
    #[serde(default = "default_internal_llm_threads")]
    pub threads: usize,
    /// Requested number of model layers to place on the GPU.
    ///
    /// The current internal Candle runtime does not implement GGUF layer
    /// offload, so non-zero values are surfaced in status output but forced to
    /// `0` effective GPU layers.
    #[serde(default)]
    pub gpu_layers: u32,
    /// Maximum context window reserved for the internal model.
    #[serde(default = "default_internal_llm_context_window")]
    pub context_window: usize,
    /// Maximum tokens the internal model may generate.
    #[serde(default = "default_internal_llm_max_output_tokens")]
    pub max_output_tokens: u32,
    /// Per-request timeout in milliseconds.
    #[serde(default = "default_internal_llm_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum active + queued requests allowed against the single-worker runtime.
    #[serde(default = "default_internal_llm_max_parallel_requests")]
    pub max_parallel_requests: usize,
    /// Policy for obtaining model artifacts.
    #[serde(default)]
    pub download_policy: InternalLlmDownloadPolicy,
    /// Allowlisted internal tasks that may use the embedded model.
    #[serde(default = "default_internal_llm_allowed_tasks")]
    pub allowed_tasks: Vec<String>,
    /// Whether session titles may be generated with the embedded model.
    #[serde(default = "default_internal_llm_session_title_enabled")]
    pub session_title_enabled: bool,
    /// Whether prompt/context compaction may use the embedded model.
    #[serde(default = "default_internal_llm_prompt_context_enabled")]
    pub prompt_context_enabled: bool,
    /// Whether memory extraction prefiltering may use the embedded model.
    #[serde(default = "default_internal_llm_memory_extraction_enabled")]
    pub memory_extraction_enabled: bool,
    #[serde(skip_serializing, default)]
    specified: InternalLlmSpecified,
}

impl Default for InternalLlmConfig {
    fn default() -> Self {
        Self {
            enabled: default_internal_llm_enabled(),
            backend: default_internal_llm_backend(),
            model_id: default_internal_llm_model_id(),
            artifact_max_bytes: default_internal_llm_artifact_max_bytes(),
            threads: default_internal_llm_threads(),
            gpu_layers: 0,
            context_window: default_internal_llm_context_window(),
            max_output_tokens: default_internal_llm_max_output_tokens(),
            timeout_ms: default_internal_llm_timeout_ms(),
            max_parallel_requests: default_internal_llm_max_parallel_requests(),
            download_policy: InternalLlmDownloadPolicy::default(),
            allowed_tasks: default_internal_llm_allowed_tasks(),
            session_title_enabled: default_internal_llm_session_title_enabled(),
            prompt_context_enabled: default_internal_llm_prompt_context_enabled(),
            memory_extraction_enabled: default_internal_llm_memory_extraction_enabled(),
            specified: InternalLlmSpecified::default(),
        }
    }
}

impl InternalLlmConfig {
    /// Returns `true` when the named internal task is allowlisted.
    #[must_use]
    pub fn allows_task(&self, task: &str) -> bool {
        self.allowed_tasks.iter().any(|allowed| allowed == task)
    }
}

impl<'de> Deserialize<'de> for InternalLlmConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        struct RawInternalLlmConfig {
            enabled: Option<bool>,
            backend: Option<String>,
            model_id: Option<String>,
            artifact_max_bytes: Option<u64>,
            threads: Option<usize>,
            gpu_layers: Option<u32>,
            context_window: Option<usize>,
            max_output_tokens: Option<u32>,
            timeout_ms: Option<u64>,
            max_parallel_requests: Option<usize>,
            download_policy: Option<InternalLlmDownloadPolicy>,
            allowed_tasks: Option<Vec<String>>,
            session_title_enabled: Option<bool>,
            prompt_context_enabled: Option<bool>,
            memory_extraction_enabled: Option<bool>,
        }

        let raw = RawInternalLlmConfig::deserialize(deserializer)?;
        let specified = InternalLlmSpecified {
            enabled: raw.enabled.is_some(),
            backend: raw.backend.is_some(),
            model_id: raw.model_id.is_some(),
            artifact_max_bytes: raw.artifact_max_bytes.is_some(),
            threads: raw.threads.is_some(),
            gpu_layers: raw.gpu_layers.is_some(),
            context_window: raw.context_window.is_some(),
            max_output_tokens: raw.max_output_tokens.is_some(),
            timeout_ms: raw.timeout_ms.is_some(),
            max_parallel_requests: raw.max_parallel_requests.is_some(),
            download_policy: raw.download_policy.is_some(),
            allowed_tasks: raw.allowed_tasks.is_some(),
            session_title_enabled: raw.session_title_enabled.is_some(),
            prompt_context_enabled: raw.prompt_context_enabled.is_some(),
            memory_extraction_enabled: raw.memory_extraction_enabled.is_some(),
        };
        Ok(Self {
            enabled: raw.enabled.unwrap_or_else(default_internal_llm_enabled),
            backend: raw.backend.unwrap_or_else(default_internal_llm_backend),
            model_id: raw.model_id.unwrap_or_else(default_internal_llm_model_id),
            artifact_max_bytes: raw
                .artifact_max_bytes
                .unwrap_or_else(default_internal_llm_artifact_max_bytes),
            threads: raw.threads.unwrap_or_else(default_internal_llm_threads),
            gpu_layers: raw.gpu_layers.unwrap_or_default(),
            context_window: raw
                .context_window
                .unwrap_or_else(default_internal_llm_context_window),
            max_output_tokens: raw
                .max_output_tokens
                .unwrap_or_else(default_internal_llm_max_output_tokens),
            timeout_ms: raw
                .timeout_ms
                .unwrap_or_else(default_internal_llm_timeout_ms),
            max_parallel_requests: raw
                .max_parallel_requests
                .unwrap_or_else(default_internal_llm_max_parallel_requests),
            download_policy: raw.download_policy.unwrap_or_default(),
            allowed_tasks: raw
                .allowed_tasks
                .unwrap_or_else(default_internal_llm_allowed_tasks),
            session_title_enabled: raw
                .session_title_enabled
                .unwrap_or_else(default_internal_llm_session_title_enabled),
            prompt_context_enabled: raw
                .prompt_context_enabled
                .unwrap_or_else(default_internal_llm_prompt_context_enabled),
            memory_extraction_enabled: raw
                .memory_extraction_enabled
                .unwrap_or_else(default_internal_llm_memory_extraction_enabled),
            specified,
        })
    }
}

fn default_internal_llm_enabled() -> bool {
    false
}

fn default_internal_llm_backend() -> String {
    "embedded".to_string()
}

fn default_internal_llm_model_id() -> String {
    "smollm2-360m-instruct-q4".to_string()
}

const fn default_internal_llm_artifact_max_bytes() -> u64 {
    1_073_741_824
}

const fn default_internal_llm_threads() -> usize {
    4
}

const fn default_internal_llm_context_window() -> usize {
    4096
}

const fn default_internal_llm_max_output_tokens() -> u32 {
    1_024
}

const fn default_internal_llm_timeout_ms() -> u64 {
    // 300 seconds: must cover cold model load plus slower local helper tasks on
    // constrained CPUs.
    300_000
}

const fn default_internal_llm_max_parallel_requests() -> usize {
    2
}

fn default_internal_llm_allowed_tasks() -> Vec<String> {
    vec![
        "session_title".to_string(),
        "summarize_tool_output".to_string(),
        "prompt_compaction".to_string(),
        "memory_prefilter".to_string(),
        "chat".to_string(),
    ]
}

const fn default_internal_llm_session_title_enabled() -> bool {
    false
}

const fn default_internal_llm_prompt_context_enabled() -> bool {
    false
}

const fn default_internal_llm_memory_extraction_enabled() -> bool {
    false
}

// ── Memory configuration ─────────────────────────────────────────────────────

/// Memory system configuration.
///
/// Controls the behaviour of the persistent memory system including file-based
/// blocks, structured SQLite storage, semantic search (embeddings), and
/// context retrieval.
///
/// Override in `ragent.json`:
/// ```json
/// {
///   "memory": {
///     "enabled": true,
///     "tier": "semantic",
///     "semantic": {
///       "enabled": true,
///       "model": "all-MiniLM-L6-v2",
///       "dimensions": 384
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Whether the memory system is enabled.
    #[serde(default = "default_memory_enabled")]
    pub enabled: bool,
    /// Memory tier: "core" (file blocks only), "structured" (SQLite store),
    /// or "semantic" (with embeddings).
    #[serde(default = "default_memory_tier")]
    pub tier: String,
    /// Structured store configuration.
    #[serde(default)]
    pub structured: StructuredMemoryConfig,
    /// Retrieval configuration for prompt injection.
    #[serde(default)]
    pub retrieval: RetrievalConfig,
    /// Semantic search (embedding) configuration.
    #[serde(default)]
    pub semantic: SemanticConfig,
    /// Automatic memory extraction configuration.
    #[serde(default)]
    pub auto_extract: AutoExtractConfig,
    /// Confidence decay configuration.
    #[serde(default)]
    pub decay: DecayConfig,
    /// Compaction configuration.
    #[serde(default)]
    pub compaction: CompactionConfig,
    /// Eviction configuration.
    #[serde(default)]
    pub eviction: EvictionConfig,
    /// Cross-project memory sharing configuration.
    #[serde(default)]
    pub cross_project: CrossProjectConfig,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tier: default_memory_tier(),
            structured: StructuredMemoryConfig::default(),
            retrieval: RetrievalConfig::default(),
            semantic: SemanticConfig::default(),
            auto_extract: AutoExtractConfig::default(),
            decay: DecayConfig::default(),
            compaction: CompactionConfig::default(),
            eviction: EvictionConfig::default(),
            cross_project: CrossProjectConfig::default(),
        }
    }
}

impl MemoryConfig {
    /// Returns the block size limit in bytes.
    ///
    /// Default is 4096 bytes (4 KiB). Override in `ragent.json`:
    /// ```json
    /// { "memory": { "block_size_limit": 8192 } }
    /// ```
    pub fn block_size_limit(&self) -> usize {
        self.compaction.block_size_limit
    }
}

/// Structured memory store configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredMemoryConfig {
    /// Whether the structured store is enabled.
    #[serde(default = "default_structured_enabled")]
    pub enabled: bool,
}

impl Default for StructuredMemoryConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Semantic search (embedding) configuration.
///
/// When enabled, memories and journal entries are embedded using a local
/// sentence-transformer model for similarity-based retrieval. This extends
/// the existing FTS5 keyword search with cosine-similarity ranking.
///
/// # Feature flag
///
/// The `embeddings` Cargo feature must be enabled for the local ONNX-based
/// embedding provider. When the feature is disabled, `memory_search` and
/// `journal_search` fall back to FTS5-only mode regardless of this config.
///
/// ```json
/// {
///   "memory": {
///     "semantic": {
///       "enabled": true,
///       "model": "all-MiniLM-L6-v2",
///       "dimensions": 384
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticConfig {
    /// Whether semantic search via embeddings is enabled.
    ///
    /// When `false` (default), `memory_search` and `journal_search` use
    /// FTS5 keyword search only. When `true` and the `embeddings` feature
    /// is compiled in, entries are embedded and searched by cosine similarity.
    #[serde(default = "default_semantic_enabled")]
    pub enabled: bool,
    /// Name of the ONNX sentence-transformer model to use.
    ///
    /// Currently only `all-MiniLM-L6-v2` is supported. The model file is
    /// downloaded on first use to the ragent data directory.
    #[serde(default = "default_semantic_model")]
    pub model: String,
    /// Embedding vector dimensions (must match the model output).
    ///
    /// `all-MiniLM-L6-v2` produces 384-dimensional vectors.
    #[serde(default = "default_semantic_dimensions")]
    pub dimensions: usize,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            enabled: default_semantic_enabled(),
            model: default_semantic_model(),
            dimensions: default_semantic_dimensions(),
        }
    }
}

fn default_semantic_enabled() -> bool {
    false
}

fn default_semantic_model() -> String {
    "all-MiniLM-L6-v2".to_string()
}

fn default_semantic_dimensions() -> usize {
    384
}

/// Retrieval configuration for injecting memories into the system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    /// Maximum number of structured memories to inject into the system prompt.
    #[serde(default = "default_max_memories_per_prompt")]
    pub max_memories_per_prompt: usize,
    /// Weight for recency when ranking memories (0.0–1.0).
    #[serde(default = "default_recency_weight")]
    pub recency_weight: f64,
    /// Weight for relevance when ranking memories (0.0–1.0).
    #[serde(default = "default_relevance_weight")]
    pub relevance_weight: f64,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            max_memories_per_prompt: default_max_memories_per_prompt(),
            recency_weight: default_recency_weight(),
            relevance_weight: default_relevance_weight(),
        }
    }
}

fn default_memory_tier() -> String {
    "core".to_string()
}

fn default_max_memories_per_prompt() -> usize {
    5
}

fn default_recency_weight() -> f64 {
    0.3
}

fn default_relevance_weight() -> f64 {
    0.7
}

fn default_memory_enabled() -> bool {
    true
}

fn default_structured_enabled() -> bool {
    true
}

/// Automatic memory extraction configuration.
///
/// Controls whether the extraction engine observes tool usage and session
/// events to propose structured memories automatically.
///
/// ```json
/// {
///   "memory": {
///     "auto_extract": {
///       "enabled": true,
///       "require_confirmation": true
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoExtractConfig {
    /// Whether automatic memory extraction is enabled.
    ///
    /// When `true`, the extraction engine observes tool executions and
    /// session events, proposing memories for patterns, error resolutions,
    /// and session summaries. When `false`, no automatic extraction occurs.
    #[serde(default = "default_auto_extract_enabled")]
    pub enabled: bool,
    /// Whether extracted candidates require explicit confirmation before storage.
    ///
    /// When `true` (default), candidates are emitted as events but **not**
    /// automatically stored. The agent or user must explicitly call
    /// `memory_store` to persist them. When `false`, candidates are
    /// auto-stored directly.
    #[serde(default = "default_require_confirmation")]
    pub require_confirmation: bool,
}

impl Default for AutoExtractConfig {
    fn default() -> Self {
        Self {
            enabled: default_auto_extract_enabled(),
            require_confirmation: default_require_confirmation(),
        }
    }
}

fn default_auto_extract_enabled() -> bool {
    false
}

fn default_require_confirmation() -> bool {
    true
}

/// Memory confidence decay configuration.
///
/// Memories that are not accessed gradually lose confidence over time.
/// This keeps the memory store clean — stale, unconfirmed memories fade
/// while frequently recalled memories maintain high confidence.
///
/// ```json
/// {
///   "memory": {
///     "decay": {
///       "factor": 0.95,
///       "min_confidence": 0.1
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayConfig {
    /// Multiplicative decay factor per day since last access.
    ///
    /// A value of 0.95 means confidence is reduced by 5% per day.
    /// Set to 1.0 to disable decay entirely.
    #[serde(default = "default_decay_factor")]
    pub factor: f64,
    /// Minimum confidence threshold — memories never decay below this value.
    ///
    /// Once a memory's confidence reaches this floor, it stays there
    /// until explicitly deleted or re-confirmed.
    #[serde(default = "default_decay_min_confidence")]
    pub min_confidence: f64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            factor: default_decay_factor(),
            min_confidence: default_decay_min_confidence(),
        }
    }
}

fn default_decay_factor() -> f64 {
    0.95
}

fn default_decay_min_confidence() -> f64 {
    0.1
}
/// Compaction configuration.
///
/// Controls when and how memory blocks and structured memories are compacted
/// to prevent unbounded growth.
///
/// ```json
/// {
///   "memory": {
///     "compaction": {
///       "enabled": true,
///       "block_size_limit": 4096,
///       "memory_count_threshold": 500,
///       "min_interval_hours": 24
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Whether memory compaction is enabled.
    ///
    /// When `true` (default), block compaction, deduplication, and eviction
    /// run automatically based on trigger conditions.
    #[serde(default = "default_compaction_enabled")]
    pub enabled: bool,
    /// Maximum content size in bytes for a memory block.
    ///
    /// Blocks exceeding 90% of this limit are compacted (truncated with
    /// original content logged to the journal). Default: 4096 (4 KiB).
    #[serde(default = "default_block_size_limit")]
    pub block_size_limit: usize,
    /// Total memory count that triggers compaction.
    ///
    /// When the total number of stored memories exceeds this threshold,
    /// a compaction pass is triggered. Default: 500.
    #[serde(default = "default_memory_count_threshold")]
    pub memory_count_threshold: usize,
    /// Minimum hours between automatic compaction passes.
    ///
    /// Prevents excessive compaction on busy sessions. Default: 24.
    #[serde(default = "default_min_interval_hours")]
    pub min_interval_hours: u64,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            enabled: default_compaction_enabled(),
            block_size_limit: default_block_size_limit(),
            memory_count_threshold: default_memory_count_threshold(),
            min_interval_hours: default_min_interval_hours(),
        }
    }
}

fn default_compaction_enabled() -> bool {
    true
}

fn default_block_size_limit() -> usize {
    4096
}

fn default_memory_count_threshold() -> usize {
    500
}

fn default_min_interval_hours() -> u64 {
    24
}

/// Stale memory eviction configuration.
///
/// Controls how memories that have decayed below the minimum confidence
/// threshold are evicted from the store.
///
/// ```json
/// {
///   "memory": {
///     "eviction": {
///       "auto": false,
///       "stale_days": 30,
///       "min_confidence": 0.1
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionConfig {
    /// Whether to auto-evict stale memories without user confirmation.
    ///
    /// When `false` (default), stale memories are identified and logged to
    /// the journal but not deleted automatically. When `true`, they are
    /// deleted without confirmation.
    #[serde(default = "default_eviction_auto")]
    pub auto: bool,
    /// Number of days since last access before a memory is considered stale.
    ///
    /// Memories older than this that have decayed below `min_confidence`
    /// are candidates for eviction. Default: 30.
    #[serde(default = "default_eviction_stale_days")]
    pub stale_days: u32,
    /// Confidence threshold below which a stale memory is evicted.
    ///
    /// Only memories with confidence below this value AND older than
    /// `stale_days` are evicted. Default: 0.1.
    #[serde(default = "default_eviction_min_confidence")]
    pub min_confidence: f64,
}

impl Default for EvictionConfig {
    fn default() -> Self {
        Self {
            auto: default_eviction_auto(),
            stale_days: default_eviction_stale_days(),
            min_confidence: default_eviction_min_confidence(),
        }
    }
}

fn default_eviction_auto() -> bool {
    false
}

fn default_eviction_stale_days() -> u32 {
    30
}

fn default_eviction_min_confidence() -> f64 {
    0.1
}
/// Cross-project memory sharing configuration.
///
/// When enabled, global memory blocks are accessible from any project,
/// and search operations span both global and current project scopes.
/// Project-specific blocks override global blocks with the same label.
///
/// ```json
/// {
///   "memory": {
///     "cross_project": {
///       "enabled": true,
///       "search_global": true,
///       "project_override": true
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossProjectConfig {
    /// Whether cross-project memory sharing is enabled.
    ///
    /// When `true`, global memory blocks (stored under `~/.ragent/memory/`)
    /// are accessible from any project. Search operations include both
    /// global and project-scoped memories. When `false` (default), only
    /// the current project's memories are visible.
    #[serde(default = "default_cross_project_enabled")]
    pub enabled: bool,
    /// Whether search operations include global memories.
    ///
    /// When `true` (default when cross_project is enabled), `memory_search`
    /// and `memory_recall` search across both global and project scopes.
    /// When `false`, even if cross_project is enabled, searches are
    /// restricted to the current project scope.
    #[serde(default = "default_search_global")]
    pub search_global: bool,
    /// Whether project-specific blocks override global blocks with the same label.
    ///
    /// When `true` (default), if a project has a block with the same label
    /// as a global block, the project version takes precedence. When `false`,
    /// global and project blocks coexist and both appear in search results.
    #[serde(default = "default_project_override")]
    pub project_override: bool,
}

impl Default for CrossProjectConfig {
    fn default() -> Self {
        Self {
            enabled: default_cross_project_enabled(),
            search_global: default_search_global(),
            project_override: default_project_override(),
        }
    }
}

fn default_cross_project_enabled() -> bool {
    false
}

fn default_search_global() -> bool {
    true
}

fn default_project_override() -> bool {
    true
}
