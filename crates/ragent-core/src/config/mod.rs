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
    /// LSP server definitions keyed by language id (e.g. `"rust"`, `"typescript"`).
    #[serde(default)]
    pub lsp: HashMap<String, LspServerConfig>,
    /// Additional instruction strings appended to agent prompts.
    #[serde(default)]
    pub instructions: Vec<String>,
    /// Additional directories to scan for skill definitions.
    #[serde(default)]
    pub skill_dirs: Vec<String>,
    /// Feature flags for experimental functionality.
    #[serde(default)]
    pub experimental: ExperimentalFlags,
    /// Lifecycle hooks. See [`crate::hooks::HookConfig`].
    #[serde(default)]
    pub hooks: Vec<crate::hooks::HookConfig>,
    /// User-defined bash command allowlist and denylist additions.
    #[serde(default)]
    pub bash: BashConfig,
    /// Code index configuration (codebase indexing & search).
    #[serde(default)]
    pub code_index: CodeIndexConfig,
    /// LLM streaming configuration (timeouts, retries).
    #[serde(default)]
    pub stream: StreamConfig,
    /// Memory system configuration (blocks, structured store, retrieval).
    #[serde(default)]
    pub memory: MemoryConfig,
    /// GitLab integration configuration.
    #[serde(default)]
    pub gitlab: GitLabIntegrationConfig,
}
/// Configuration for LLM streaming behaviour (timeouts, retries).
///
/// Override in `ragent.json`:
/// ```json
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        }
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

// ── LSP configuration ────────────────────────────────────────────────────────

/// Configuration for a single Language Server Protocol server.
///
/// Servers communicate over stdio JSON-RPC. The server is started as a child
/// process and the standard LSP initialize handshake is performed.
///
/// Example `ragent.json` entry:
/// ```json
/// {
///   "lsp": {
///     "rust": {
///       "command": "rust-analyzer",
///       "extensions": ["rs"]
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// Executable name or full path (e.g. `"rust-analyzer"`).
    pub command: Option<String>,
    /// Command-line arguments (e.g. `["--stdio"]` for some servers).
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variable overrides injected into the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// File extensions this server handles (e.g. `["rs"]` for Rust).
    #[serde(default)]
    pub extensions: Vec<String>,
    /// If `true`, this server is configured but will not be started.
    #[serde(default)]
    pub disabled: bool,
    /// Maximum milliseconds to wait for an LSP response (default: 10 000 ms).
    #[serde(default = "LspServerConfig::default_timeout_ms")]
    pub timeout_ms: u64,
}

impl LspServerConfig {
    /// Default LSP response timeout in milliseconds.
    #[must_use]
    pub const fn default_timeout_ms() -> u64 {
        10_000
    }
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            command: None,
            args: Vec::new(),
            env: HashMap::new(),
            extensions: Vec::new(),
            disabled: false,
            timeout_ms: Self::default_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Flags for experimental features that are not yet stable.
pub struct ExperimentalFlags {
    /// Enable OpenTelemetry trace export.
    #[serde(default)]
    pub open_telemetry: bool,
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
    /// use ragent_core::config::Config;
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
            let overlay: Self = serde_json::from_str(&content)?;
            config = Self::merge(config, overlay);
        }

        Ok(config)
    }

    fn load_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Deep merge two configs, with overlay taking precedence for set fields.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::config::Config;
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
        for (k, v) in overlay.lsp {
            base.lsp.insert(k, v);
        }
        // Permissions, instructions, and skill dirs append
        base.permission.extend(overlay.permission);
        base.instructions.extend(overlay.instructions);
        base.skill_dirs.extend(overlay.skill_dirs);

        if overlay.experimental.open_telemetry {
            base.experimental.open_telemetry = true;
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

        base
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
