//! Configuration loading, merging, and types for ragent.
//!
//! [`Config`] is loaded via [`Config::load`] with a layered precedence:
//! compiled defaults → global file → project file → `RAGENT_CONFIG` env →
//! `RAGENT_CONFIG_CONTENT` env. Provider, agent, MCP server, and permission
//! settings are all configured here.

use crate::compaction::{CompactionConfig, LegacyCompressionConfig};
use anyhow::bail;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
    #[serde(default = "default_agent_name", alias = "defaultAgent")]
    pub default_agent: String,
    #[serde(skip)]
    specified_default_agent: bool,
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
    /// Tavily search API key for the websearch tool.
    ///
    /// Can also be set via the `TAVILY_API_KEY` environment variable. The
    /// environment variable takes precedence over this config field.
    #[serde(default)]
    pub tavily_api_key: Option<String>,
    /// LangSearch API key for the `mf_search` tool.
    ///
    /// Stored in `ragent.json` (global or project). When present, `mf_search`
    /// will query the LangSearch Web Search API as an additional backend. The
    /// key is masked in diagnostics and never logged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub langsearch_api_key: Option<String>,
    /// Code index configuration (codebase indexing & search).
    #[serde(default)]
    pub code_index: CodeIndexConfig,
    /// LLM streaming configuration (timeouts, retries).
    #[serde(default)]
    pub stream: StreamConfig,
    /// Memory system configuration (blocks, structured store, retrieval).
    #[serde(default)]
    pub memory: MemoryConfig,
    /// Context compaction configuration (OpenCode-derived summarisation).
    ///
    /// When `auto` is `true`, the agent summarises conversation history before
    /// sending a request that would exceed the context window minus the
    /// configured buffer. This replaces the older Headroom-based `compression`
    /// section.
    #[serde(default)]
    pub compaction: CompactionConfig,
    /// Deprecated alias for the context compaction configuration.
    ///
    /// If present, `compression.enabled` is mapped to `compaction.auto` when the
    /// new `compaction` section does not explicitly set `auto`. This one-
    /// release shim eases migration from the Headroom compression scheme.
    #[serde(default, alias = "compression")]
    pub compression: LegacyCompressionConfig,
    /// GitLab integration configuration.
    #[serde(default)]
    pub gitlab: GitLabIntegrationConfig,
    /// Tool-family visibility switches.
    /// When a switch is `false`, all tools in that family are hidden from the LLM.
    #[serde(default)]
    pub tool_visibility: ToolVisibilityConfig,
    /// Agent-loop performance configuration (AgentPerf FR-027).
    ///
    /// Controls the per-step wall-clock budget, the per-stream stall
    /// timeout, the maximum number of concurrent tool calls, and the
    /// master enable / profiling switches.  See
    /// `specs/AgentPerf/SPEC.md` for the full schema.
    #[serde(default)]
    pub agent_perf: AgentPerfConfig,
    /// Telemetry configuration for the OpenTelemetry metrics export subsystem.
    ///
    /// See `specs/otel/SPEC.md` for the `telemetry.otel` schema. Defaults to
    /// disabled; the legacy `experimental.open_telemetry` flag is mapped as a
    /// deprecated alias via [`TelemetryConfig::apply_legacy_flag`](crate::telemetry::TelemetryConfig::apply_legacy_flag).
    #[serde(default)]
    pub telemetry: crate::telemetry::TelemetryConfig,
    /// Tool names to hide from the LLM (excluded from tool definitions and system-prompt listings).
    /// Hidden tools remain registered and executable; they are simply not advertised to the model.
    ///
    /// Example — suppress all GitHub and GitLab tools:
    /// ```json
    /// { "hidden_tools": ["github_list_issues", "github_get_issue", "gitlab_list_mrs"] }
    /// ```
    #[serde(default)]
    pub hidden_tools: Vec<String>,
    /// YOLO mode — bypass command validation and tool restrictions.
    #[serde(default)]
    pub yolo: bool,
    /// User-defined price overrides for cost estimation (FR-011).
    ///
    /// Each entry overrides the built-in price table for a specific model.
    /// Prices are in USD per 1,000,000 tokens.
    #[serde(default)]
    pub prices: Vec<PriceEntry>,
    /// Browser automation configuration (CDP endpoint, headless launch).
    #[serde(default)]
    pub browser: BrowserConfig,
    /// External messaging channel configuration (JCODEPLAN M7).
    ///
    /// Used by the `send_channel_message` tool to post notifications to
    /// Telegram chats and Discord webhooks.
    #[serde(default, skip_serializing_if = "ChannelsConfig::is_empty")]
    pub channels: ChannelsConfig,
    /// Gmail tool configuration (JCODEPLAN M7).
    ///
    /// Optional OAuth2 client credentials for the `gmail` tool. The OAuth
    /// access/refresh tokens themselves are stored encrypted in `ragent-storage`
    /// (never in this file).
    #[serde(default, skip_serializing_if = "GmailConfig::is_empty")]
    pub gmail: GmailConfig,
    /// Paths of configuration files that were loaded during [`Config::load`].
    #[serde(skip)]
    pub config_paths: Vec<PathBuf>,
}

/// Tool-family visibility configuration.
///
/// Controls which tool families are advertised to the LLM. Each switch
/// corresponds to a group of related tools. When a switch is `false`,
/// all tools in that family are suppressed from `ToolRegistry::definitions()`.
/// Tools remain registered and executable regardless of visibility.
///
/// All fields are public so that callers which have a [`ToolVisibilityConfig`]
/// in hand (e.g. the TUI `/tools` handler) can mark a switch as explicitly
/// user-set before persisting the config.
#[derive(Debug, Clone, Default)]
pub struct ToolVisibilitySpecified {
    /// `true` when `office` was explicitly set in the source JSON or via a setter.
    pub office: bool,
    /// `true` when `github` was explicitly set in the source JSON or via a setter.
    pub github: bool,
    /// `true` when `gitlab` was explicitly set in the source JSON or via a setter.
    pub gitlab: bool,
    /// `true` when `teams` was explicitly set in the source JSON or via a setter.
    pub teams: bool,
    /// `true` when `agents` was explicitly set in the source JSON or via a setter.
    pub agents: bool,
    /// `true` when `plan` was explicitly set in the source JSON or via a setter.
    pub plan: bool,
    /// `true` when `codeindex` was explicitly set in the source JSON or via a setter.
    pub codeindex: bool,
    /// `true` when `masterfetch` was explicitly set in the source JSON or via a setter.
    pub masterfetch: bool,
    /// `true` when `browser` was explicitly set in the source JSON or via a setter.
    pub browser: bool,
}

/// Tool-family visibility configuration.
///
/// The config loader tracks which switches were explicitly present in the
/// source JSON so merge operations can preserve base values for omitted fields.
///
/// The `codeindex` switch is serialised only when the user has *explicitly* set
/// it (tracked by [`ToolVisibilitySpecified::codeindex`]). This lets the default
/// config omit the key so code-level default changes propagate, while ensuring
/// that an explicit user toggle (e.g. `/tools codeindex on|off`) is written to
/// disk and survives a restart — even when a global config disagrees.
#[derive(Debug, Clone)]
pub struct ToolVisibilityConfig {
    /// Office document tools (office_read, office_write, office_info, libre_read, etc.).
    pub office: bool,
    /// GitHub tools (github_list_issues, github_get_issue, github_create_issue, etc.).
    pub github: bool,
    /// GitLab tools (gitlab_list_issues, gitlab_get_issue, gitlab_create_mr, etc.).
    pub gitlab: bool,
    /// Team coordination tools (team_create, team_spawn, team_message, etc.).
    pub teams: bool,
    /// Autonomous agent task tools (new_task, list_tasks, cancel_task, etc.).
    pub agents: bool,
    /// Plan-mode tools (plan_enter, plan_exit).
    pub plan: bool,
    /// Code-index tools (codeindex_search, codeindex_status, codeindex_symbols, etc.).
    /// Default `true` — codeindex tools are visible when the subsystem is enabled.
    /// When serialised, this field is only written if the user explicitly set it
    /// (via [`ToolVisibilityConfig::set_codeindex`] or by having it present in the
    /// loaded JSON).
    pub codeindex: bool,
    /// Masterfetch web-access tools (mf_fetch, mf_crawl, mf_search,
    /// mf_screenshot, mf_cache_clear, mf_version).
    /// Default `true` — masterfetch tools are visible by default.
    /// When serialised, this field is only written if the user explicitly set it
    /// (tracked by [`ToolVisibilitySpecified::masterfetch`]).
    pub masterfetch: bool,
    /// Browser automation tool (`browser`).
    /// Default `true` — the browser tool is visible by default.
    /// When serialised, this field is only written if the user explicitly set it
    /// (tracked by [`ToolVisibilitySpecified::browser`]).
    pub browser: bool,
    /// Tracks which switches were explicitly set, so merge/serialise can
    /// distinguish "user set this" from "this is just the default".
    pub specified: ToolVisibilitySpecified,
}

impl ToolVisibilityConfig {
    /// Iterate over every tool-visibility switch and its enabled state.
    pub fn iter_switches(&self) -> impl Iterator<Item = (&'static str, bool)> {
        [
            ("office", self.office),
            ("github", self.github),
            ("gitlab", self.gitlab),
            ("teams", self.teams),
            ("agents", self.agents),
            ("plan", self.plan),
            ("codeindex", self.codeindex),
            ("masterfetch", self.masterfetch),
            ("browser", self.browser),
        ]
        .into_iter()
    }

    /// Explicitly set the `codeindex` switch and mark it as user-specified.
    ///
    /// Use this (rather than direct field assignment) when persisting a user
    /// toggle such as `/tools codeindex on` / `/tools codeindex off`, so the
    /// value is written to the config file and overrides any global-config
    /// default on reload.
    pub fn set_codeindex(&mut self, enabled: bool) {
        self.codeindex = enabled;
        self.specified.codeindex = true;
    }
}

impl Serialize for ToolVisibilityConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        // Non-codeindex switches are always serialised (they default to false
        // and have no skip). `codeindex` is serialised only when explicitly set.
        let mut count = 6; // office, github, gitlab, teams, agents, plan
        if self.specified.codeindex {
            count += 1;
        }
        if self.specified.masterfetch {
            count += 1;
        }
        if self.specified.browser {
            count += 1;
        }
        let mut s = serializer.serialize_struct("ToolVisibilityConfig", count)?;
        s.serialize_field("office", &self.office)?;
        s.serialize_field("github", &self.github)?;
        s.serialize_field("gitlab", &self.gitlab)?;
        s.serialize_field("teams", &self.teams)?;
        s.serialize_field("agents", &self.agents)?;
        s.serialize_field("plan", &self.plan)?;
        if self.specified.codeindex {
            s.serialize_field("codeindex", &self.codeindex)?;
        }
        if self.specified.masterfetch {
            s.serialize_field("masterfetch", &self.masterfetch)?;
        }
        if self.specified.browser {
            s.serialize_field("browser", &self.browser)?;
        }
        s.end()
    }
}

/// Agent-loop performance configuration (AgentPerf FR-027).
///
/// All fields are optional and have sensible defaults; the agent
/// action loop consults this struct at startup and on every step.
///
/// # Example
///
/// ```jsonc
/// {
///   "agent_perf": {
///     "enabled": true,
///     "profiling": true,
///     "step_budget_secs": 300,
///     "stall_timeout_secs": 60,
///     "max_concurrent_tools": 4,
///     "parallel_independent_tools": true
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPerfConfig {
    /// Master switch for the entire perf subsystem.
    /// When `false`, every performance optimisation short-circuits.
    #[serde(default = "agent_perf_default_true")]
    pub enabled: bool,
    /// Emit detailed per-scope timing logs at `info` level.
    #[serde(default)]
    pub profiling: bool,
    /// Maximum wall-clock seconds per agent step.
    #[serde(default = "default_step_budget_secs")]
    pub step_budget_secs: u64,
    /// Maximum seconds without a stream delta before stall recovery
    /// fires.
    #[serde(default = "default_stall_timeout_secs")]
    pub stall_timeout_secs: u64,
    /// Maximum parallel tool calls per turn.
    #[serde(default = "default_max_concurrent_tools")]
    pub max_concurrent_tools: u32,
    /// Execute independent tool calls in parallel.
    #[serde(default = "agent_perf_default_true")]
    pub parallel_independent_tools: bool,
}

fn agent_perf_default_true() -> bool {
    true
}

fn default_step_budget_secs() -> u64 {
    300
}

fn default_stall_timeout_secs() -> u64 {
    60
}

fn default_max_concurrent_tools() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
        .min(4)
}

impl Default for AgentPerfConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            profiling: false,
            step_budget_secs: default_step_budget_secs(),
            stall_timeout_secs: default_stall_timeout_secs(),
            max_concurrent_tools: default_max_concurrent_tools(),
            parallel_independent_tools: true,
        }
    }
}

impl AgentPerfConfig {
    /// Validate the configuration and return a list of any problems.
    ///
    /// The agent loop calls this on startup and refuses to start
    /// if `validate()` returns a non-empty list.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.step_budget_secs < 5 {
            problems.push(format!(
                "agent_perf.step_budget_secs must be >= 5 (got {})",
                self.step_budget_secs
            ));
        }
        if self.stall_timeout_secs < 5 {
            problems.push(format!(
                "agent_perf.stall_timeout_secs must be >= 5 (got {})",
                self.stall_timeout_secs
            ));
        }
        if self.max_concurrent_tools < 1 {
            problems.push(format!(
                "agent_perf.max_concurrent_tools must be >= 1 (got {})",
                self.max_concurrent_tools
            ));
        }
        problems
    }
}

impl Default for ToolVisibilityConfig {
    fn default() -> Self {
        Self {
            office: false,
            github: false,
            gitlab: false,
            teams: false,
            agents: false,
            plan: false,
            codeindex: true,
            masterfetch: true,
            browser: true,
            specified: ToolVisibilitySpecified::default(),
        }
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
            github: Option<bool>,
            gitlab: Option<bool>,
            teams: Option<bool>,
            agents: Option<bool>,
            plan: Option<bool>,
            codeindex: Option<bool>,
            masterfetch: Option<bool>,
            browser: Option<bool>,
        }

        let raw = RawToolVisibilityConfig::deserialize(deserializer)?;
        Ok(Self {
            office: raw.office.unwrap_or_else(default_false),
            github: raw.github.unwrap_or_else(default_false),
            gitlab: raw.gitlab.unwrap_or_else(default_false),
            teams: raw.teams.unwrap_or_else(default_false),
            agents: raw.agents.unwrap_or_else(default_false),
            plan: raw.plan.unwrap_or_else(default_false),
            codeindex: raw.codeindex.unwrap_or_else(default_true),
            masterfetch: raw.masterfetch.unwrap_or_else(default_true),
            browser: raw.browser.unwrap_or_else(default_true),
            specified: ToolVisibilitySpecified {
                office: raw.office.is_some(),
                github: raw.github.is_some(),
                gitlab: raw.gitlab.is_some(),
                teams: raw.teams.is_some(),
                agents: raw.agents.is_some(),
                plan: raw.plan.is_some(),
                codeindex: raw.codeindex.is_some(),
                masterfetch: raw.masterfetch.is_some(),
                browser: raw.browser.is_some(),
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
        "teams" => Some(&[
            "team_approve_plan",
            "team_assign_task",
            "team_broadcast",
            "team_cleanup",
            "team_create",
            "team_idle",
            "team_memory_read",
            "team_memory_write",
            "team_message",
            "team_read_messages",
            "team_shutdown_ack",
            "team_shutdown_teammate",
            "team_spawn",
            "team_status",
            "team_submit_plan",
            "team_task_claim",
            "team_task_complete",
            "team_task_create",
            "team_task_list",
            "team_wait",
        ]),
        "agents" => Some(&[
            "cancel_task",
            "list_tasks",
            "new_task",
            "task_complete",
            "wait_tasks",
        ]),
        "plan" => Some(&["plan_enter", "plan_exit"]),
        "codeindex" => Some(&[
            "codeindex_search",
            "codeindex_status",
            "codeindex_symbols",
            "codeindex_references",
            "codeindex_dependencies",
            "codeindex_reindex",
        ]),
        "masterfetch" => Some(&[
            "mf_fetch",
            "mf_crawl",
            "mf_search",
            "mf_screenshot",
            "mf_cache_clear",
            "mf_version",
        ]),
        "browser" => Some(&["browser"]),
        _ => None,
    }
}
/// Configuration for LLM streaming behaviour (timeouts, retries).
///
/// The two timeouts serve distinct purposes:
///
/// - `initial_response_timeout_secs` (default 300) bounds how long the
///   HTTP client will wait for the **first byte** from the provider after
///   sending the request.  This covers network RTT plus provider-side
///   cold-start / queue wait and must be generous for cloud-hosted
///   models that may spend tens of seconds preparing the model before
///   the first token streams.
/// - `timeout_secs` (default 120) bounds the gap between **subsequent
///   stream deltas** during an in-flight response.  Once tokens are
///   flowing, an inter-delta gap larger than this is treated as a
///   stream stall and triggers the retry/recovery path.
///
/// `initial_response_timeout_secs` must be greater than or equal to
/// `timeout_secs` to avoid aborting an in-flight stream before it
/// produces any data.
///
/// ```json
/// {
///   "stream": {
///     "initial_response_timeout_secs": 300,
///     "timeout_secs": 120,
///     "max_retries": 4,
///     "retry_backoff_secs": 2
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamConfig {
    /// Seconds the HTTP client will wait for the provider to return the
    /// **first byte** of a streaming response (default: 300).
    ///
    /// This is forwarded to each provider as `ChatRequest::stream_timeout_secs`
    /// and governs the initial-response / connection-establishment timeout,
    /// not the per-event stall timeout (see [`Self::timeout_secs`]).
    #[serde(default = "default_initial_response_timeout_secs")]
    pub initial_response_timeout_secs: u64,
    /// Seconds of silence between stream deltas before a stream is
    /// considered stalled (default: 120).
    ///
    /// Used by the session processor's per-event stall detection.  The
    /// `agent_perf.stall_timeout_secs` knob is a separate budget used by
    /// the agent-perf profiler and should be ≤ this value.
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

const fn default_initial_response_timeout_secs() -> u64 {
    300
}

const fn default_stream_timeout_secs() -> u64 {
    120
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
            initial_response_timeout_secs: default_initial_response_timeout_secs(),
            timeout_secs: default_stream_timeout_secs(),
            max_retries: default_stream_max_retries(),
            retry_backoff_secs: default_stream_retry_backoff_secs(),
        }
    }
}

impl StreamConfig {
    /// Validate the configuration and return a list of any problems.
    ///
    /// Returns an empty `Vec` when all values are within their supported
    /// ranges.  Callers (e.g. the session processor startup path) can
    /// surface the messages and refuse to start the runtime.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.initial_response_timeout_secs < 5 {
            problems.push(format!(
                "stream.initial_response_timeout_secs must be >= 5 (got {})",
                self.initial_response_timeout_secs
            ));
        }
        if self.timeout_secs < 5 {
            problems.push(format!(
                "stream.timeout_secs must be >= 5 (got {})",
                self.timeout_secs
            ));
        }
        if self.initial_response_timeout_secs < self.timeout_secs {
            problems.push(format!(
                "stream.initial_response_timeout_secs ({}) must be >= stream.timeout_secs ({})",
                self.initial_response_timeout_secs, self.timeout_secs
            ));
        }
        if self.max_retries > 32 {
            problems.push(format!(
                "stream.max_retries must be <= 32 (got {})",
                self.max_retries
            ));
        }
        problems
    }
}

/// Persistent configuration for the code-index subsystem.
///
/// Runtime-derived fields like `project_root` and `index_dir` are
/// resolved at startup, not stored in the config file.
///
/// All fields are public so callers which have a [`CodeIndexConfig`] in hand
/// can mark a field as explicitly user-set before persisting the config.
#[derive(Debug, Clone, Default)]
pub struct CodeIndexSpecified {
    /// `true` when `enabled` was explicitly set in the source JSON or via a setter.
    pub enabled: bool,
    /// `true` when `max_file_size` was explicitly set in the source JSON or via a setter.
    pub max_file_size: bool,
    /// `true` when `extra_exclude_dirs` was explicitly set in the source JSON or via a setter.
    pub extra_exclude_dirs: bool,
    /// `true` when `extra_exclude_patterns` was explicitly set in the source JSON or via a setter.
    pub extra_exclude_patterns: bool,
}

/// Persistent configuration for the code-index subsystem.
///
/// The `enabled` field is serialised only when the user has *explicitly* set it
/// (tracked by [`CodeIndexSpecified::enabled`]). This lets the default config
/// omit the key so code-level default changes propagate, while ensuring that an
/// explicit user toggle (e.g. `/codeindex on`/`/codeindex off`) is written to disk
/// and survives a restart — even when a global config disagrees.
#[derive(Debug, Clone)]
pub struct CodeIndexConfig {
    /// Whether code indexing is enabled.
    ///
    /// Defaults to `true`. When serialised, this field is only written if the
    /// user explicitly set it (via [`CodeIndexConfig::set_enabled`] or by
    /// having it present in the loaded JSON).
    pub enabled: bool,
    /// Maximum file size in bytes to index (default: 1 MB).
    pub max_file_size: u64,
    /// Additional directory names to exclude from scanning.
    pub extra_exclude_dirs: Vec<String>,
    /// Additional glob patterns to exclude from scanning.
    pub extra_exclude_patterns: Vec<String>,
    /// Tracks which fields were explicitly present in the source JSON or set
    /// via a setter, so merge/serialise operations can distinguish "user set
    /// this" from "this is just the default".
    pub specified: CodeIndexSpecified,
}

impl CodeIndexConfig {
    /// Explicitly set `enabled` and mark it as user-specified.
    ///
    /// Use this (rather than direct field assignment) when persisting a user
    /// toggle such as `/codeindex on` / `/codeindex off`, so the value is written
    /// to the config file and overrides any global-config default on reload.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.specified.enabled = true;
    }
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

impl Serialize for CodeIndexConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        // Count fields actually written: `enabled` only when explicitly set.
        let mut count = 3; // max_file_size, extra_exclude_dirs, extra_exclude_patterns
        if self.specified.enabled {
            count += 1;
        }
        let mut s = serializer.serialize_struct("CodeIndexConfig", count)?;
        if self.specified.enabled {
            s.serialize_field("enabled", &self.enabled)?;
        }
        s.serialize_field("max_file_size", &self.max_file_size)?;
        s.serialize_field("extra_exclude_dirs", &self.extra_exclude_dirs)?;
        s.serialize_field("extra_exclude_patterns", &self.extra_exclude_patterns)?;
        s.end()
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
    /// Optional HTTP headers sent with every request for HTTP/SSE transports.
    #[serde(default)]
    pub headers: HashMap<String, String>,
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
            headers: HashMap::new(),
            disabled: false,
        }
    }
}

/// User-defined price override for a single model (FR-011).
///
/// Prices are in USD per 1,000,000 tokens. When a `PriceEntry` matches a
/// model id in the built-in price table, the entry's values take precedence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceEntry {
    /// Model identifier as returned by the provider (e.g. `"gpt-4o"`).
    pub model: String,
    /// Price per 1M input/prompt tokens in USD.
    pub input_per_1m: f64,
    /// Price per 1M output/completion tokens in USD.
    pub output_per_1m: f64,
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
    8
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
        let mut loaded = false;

        // Global config: ~/.config/ragent/ragent.json
        if let Some(config_dir) = dirs::config_dir() {
            let global_path = config_dir.join("ragent").join("ragent.json");
            if global_path.exists() {
                let overlay = Self::load_file(&global_path)?;
                config = Self::merge(config, overlay);
                config.config_paths.push(global_path);
                loaded = true;
            }
        }

        // Project config: ./.ragent/ragent.json
        let project_path = PathBuf::from(".ragent").join("ragent.json");
        if project_path.exists() {
            let overlay = Self::load_file(&project_path)?;
            config = Self::merge(config, overlay);
            config.config_paths.push(project_path.clone());
            loaded = true;
        }
        // If neither global nor project config exists, create a default
        // project-level config so the user has a starting point.
        if !loaded {
            let ragent_dir = PathBuf::from(".ragent");
            std::fs::create_dir_all(&ragent_dir).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create project config directory '{}': {}",
                    ragent_dir.display(),
                    e
                )
            })?;
            let default_json = serde_json::to_string_pretty(&config)
                .map_err(|e| anyhow::anyhow!("Failed to serialise default config: {}", e))?;
            std::fs::write(&project_path, &default_json).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to write default config file '{}': {}",
                    project_path.display(),
                    e
                )
            })?;
            config.config_paths.push(project_path);
        }

        // Environment variable pointing to config file
        if let Ok(env_path) = std::env::var("RAGENT_CONFIG") {
            let path = PathBuf::from(&env_path);
            if path.exists() {
                let overlay = Self::load_file(&path)?;
                config = Self::merge(config, overlay);
                config.config_paths.push(path);
            }
        }

        // Inline config from environment variable
        if let Ok(content) = std::env::var("RAGENT_CONFIG_CONTENT") {
            let mut overlay: Self = serde_json::from_str(&content).map_err(|e| {
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
            let overlay_value: serde_json::Value =
                serde_json::from_str(&content).expect("valid JSON already parsed into Config");
            overlay.specified_default_agent = overlay_value.get("defaultAgent").is_some()
                || overlay_value.get("default_agent").is_some();
            config = Self::merge(config, overlay);
        }

        Ok(config)
    }

    pub(crate) fn load_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Failed to read config file '{}': {}", path.display(), e)
        })?;

        let mut config: Self = serde_json::from_str(&content).map_err(|e| {
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
        let config_value: serde_json::Value =
            serde_json::from_str(&content).expect("valid JSON already parsed into Config");
        config.specified_default_agent = config_value.get("defaultAgent").is_some()
            || config_value.get("default_agent").is_some();

        Ok(config)
    }

    /// Save the config back to a file.
    ///
    /// Writes the current config as pretty-printed JSON. The path is the
    /// project-local config file (`.ragent/ragent.json`) when
    /// `prefer_project` is true, creating the `.ragent` directory if needed;
    /// otherwise the global config file (`~/.config/ragent/ragent.json`).
    ///
    /// The file is only touched when the serialised JSON differs from the
    /// existing content, avoiding unnecessary writes and timestamp churn.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save(&self, prefer_project: bool) -> anyhow::Result<()> {
        let path = if prefer_project {
            let project = PathBuf::from(".ragent/ragent.json");
            // Ensure the project config directory exists so project-local saves
            // do not silently fall back to the global file.
            if let Some(parent) = project.parent() {
                std::fs::create_dir_all(parent)?;
            }
            project
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

        Self::write_config_if_changed(&path, &json)
    }

    /// Save the config back to its original source file.
    ///
    /// If a project-local config (`.ragent/ragent.json`) was loaded during
    /// [`Config::load`], this writes back to that path. Otherwise it writes
    /// to the global config file (`~/.config/ragent/ragent.json`).
    ///
    /// The file is only touched when the serialised JSON differs from the
    /// existing content, avoiding unnecessary writes and timestamp churn.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn save_to_source(&self) -> anyhow::Result<()> {
        let project_path = PathBuf::from(".ragent/ragent.json");
        let was_loaded_from_project = self.config_paths.iter().any(|p| {
            p.file_name().is_some_and(|f| f == "ragent.json")
                && p.parent()
                    .is_some_and(|parent| parent.file_name().is_some_and(|f| f == ".ragent"))
        });
        let path = if was_loaded_from_project {
            project_path
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

        Self::write_config_if_changed(&path, &json)
    }

    /// Resolve the global config directory (`<config_dir>/ragent`) using the
    /// same platform-aware logic as [`Config::load`] and [`Config::save`].
    ///
    /// Returns `None` when the platform config directory cannot be determined
    /// (e.g. `XDG_CONFIG_HOME` unset on a headless Linux box).
    #[must_use]
    pub fn global_config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("ragent"))
    }

    /// Resolve the path to the global `ragent.json` file.
    ///
    /// This is the canonical path used by [`Config::load`], [`Config::save`],
    /// and the `/config show` / `/config save` / `/config list` slash commands,
    /// satisfying FR-001 (single, consistent path resolution).
    #[must_use]
    pub fn global_config_path() -> Option<PathBuf> {
        Self::global_config_dir().map(|d| d.join("ragent.json"))
    }

    /// Snapshot the current global `ragent.json` into a timestamped backup
    /// file inside a `saves/` subdirectory of the global config directory.
    ///
    /// The backup is named `ragent.json.[date].[time]` where `[date]` is
    /// `YYYY-MM-DD` and `[time]` is `HH-MM-SS` (hyphens in the time portion
    /// for Windows NTFS compatibility, where colons are illegal in file names).
    /// The `saves/` directory is created if it does not already exist. The
    /// backup is written via a temp-file-then-rename so a crash mid-write never
    /// leaves a partial backup (FR-003). Each call produces a new, uniquely
    /// named file — existing backups are never overwritten (FR-011).
    ///
    /// # Arguments
    ///
    /// * `config_dir` — the global config directory (e.g.
    ///   `~/.config/ragent`). When `None`, the directory is resolved via
    ///   [`global_config_dir`](Self::global_config_dir); an error is returned
    ///   if it cannot be determined. Tests may pass a temp directory so the
    ///   real global config is not touched.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the global config directory cannot be determined and `config_dir`
    ///   is `None`,
    /// - the global `ragent.json` does not exist or cannot be read,
    /// - the `saves/` directory cannot be created,
    /// - the backup file cannot be written atomically.
    ///
    /// On success, returns the path to the newly created backup file.
    pub fn backup_global_config(config_dir: Option<&Path>) -> anyhow::Result<PathBuf> {
        let dir = match config_dir {
            Some(d) => d.to_path_buf(),
            None => Self::global_config_dir()
                .ok_or_else(|| anyhow::anyhow!("no global config directory found"))?,
        };
        let source = dir.join("ragent.json");

        // Read the current global config. If it does not exist there is nothing
        // to back up — surface a clear error rather than creating an empty file.
        let content = std::fs::read_to_string(&source).map_err(|e| {
            anyhow::anyhow!("Failed to read global config '{}': {}", source.display(), e)
        })?;

        // Timestamp: YYYY-MM-DD.HH-MM-SS (hyphens in time for Windows).
        let timestamp = chrono::Utc::now().format("%Y-%m-%d.%H-%M-%S").to_string();
        let mut backup_name = format!("ragent.json.{timestamp}");

        let saves_dir = dir.join("saves");
        std::fs::create_dir_all(&saves_dir).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create saves directory '{}': {}",
                saves_dir.display(),
                e
            )
        })?;

        // FR-011: if multiple saves happen in the same second, append a
        // hyphenated counter so existing backups are never overwritten.
        let mut backup_path = saves_dir.join(&backup_name);
        let mut counter = 1u32;
        while backup_path.exists() {
            backup_name = format!("ragent.json.{timestamp}-{counter}");
            backup_path = saves_dir.join(&backup_name);
            counter += 1;
            if counter > 1000 {
                bail!(
                    "Too many backup collisions for timestamp '{}'; cannot create a unique backup name",
                    timestamp
                );
            }
        }

        // Atomic write: write to a sibling temp file, then rename. A crash
        // between the write and the rename leaves the temp file behind (which
        // does not match the `ragent.json.*` backup pattern) but never a
        // partial backup.
        let tmp_path = saves_dir.join(format!("{backup_name}.tmp"));
        std::fs::write(&tmp_path, &content).map_err(|e| {
            anyhow::anyhow!(
                "Failed to write backup temp file '{}': {}",
                tmp_path.display(),
                e
            )
        })?;
        std::fs::rename(&tmp_path, &backup_path).map_err(|e| {
            // Best-effort cleanup of the orphaned temp file on rename failure.
            let _ = std::fs::remove_file(&tmp_path);
            anyhow::anyhow!(
                "Failed to rename temp file to backup '{}': {}",
                backup_path.display(),
                e
            )
        })?;

        Ok(backup_path)
    }

    /// Restore a saved backup over the global `ragent.json` atomically.
    ///
    /// The `backup` argument may be either a file name (e.g.
    /// `ragent.json.2024-01-01.12-00-00`) or an absolute path. The destination is
    /// always the global `ragent.json` inside `config_dir` (or the resolved global
    /// config directory when `config_dir` is `None`). The restore validates that:
    ///
    /// - the backup file exists and is a regular file,
    /// - the resolved destination path is exactly `<config_dir>/ragent.json`
    ///   (FR-012: never write to an arbitrary path),
    /// - the backup content is valid JSON (defence against restoring a corrupt
    ///   file).
    ///
    /// The write uses a temp-file-then-rename so readers never see a partial
    /// config.
    ///
    /// # Arguments
    ///
    /// * `config_dir` — the global config directory (e.g. `~/.config/ragent`).
    ///   When `None`, resolved via [`global_config_dir`].
    /// * `backup` — backup file name or full path inside the `saves/` subfolder.
    ///
    /// # Errors
    ///
    /// Returns an error if the global config directory cannot be determined, the
    /// backup cannot be read, the backup content is not valid JSON, or the
    /// atomic rename fails.
    pub fn restore_global_config(
        config_dir: Option<&Path>,
        backup: &Path,
    ) -> anyhow::Result<PathBuf> {
        let dir = match config_dir {
            Some(d) => d.to_path_buf(),
            None => Self::global_config_dir()
                .ok_or_else(|| anyhow::anyhow!("no global config directory found"))?,
        };
        let target = dir.join("ragent.json");

        // Resolve the backup path. If a bare file name is supplied, look inside
        // the saves/ subdirectory.
        let backup_path = if backup.file_name().is_some() && backup.components().count() == 1 {
            dir.join("saves").join(backup)
        } else {
            backup.to_path_buf()
        };

        if !backup_path.exists() || !backup_path.is_file() {
            bail!(
                "Backup file '{}' does not exist or is not a file",
                backup_path.display()
            );
        }

        let content = std::fs::read_to_string(&backup_path).map_err(|e| {
            anyhow::anyhow!("Failed to read backup '{}': {}", backup_path.display(), e)
        })?;

        // Guard against restoring corrupt/non-JSON backups.
        let _: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            anyhow::anyhow!(
                "Backup '{}' is not valid JSON: {}",
                backup_path.display(),
                e
            )
        })?;

        // Atomic write to the canonical global config path.
        let tmp_path = target.with_extension("json.tmp");
        std::fs::write(&tmp_path, &content).map_err(|e| {
            anyhow::anyhow!(
                "Failed to write restored config temp file '{}': {}",
                tmp_path.display(),
                e
            )
        })?;
        std::fs::rename(&tmp_path, &target).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            anyhow::anyhow!(
                "Failed to rename restored config to '{}': {}",
                target.display(),
                e
            )
        })?;

        Ok(target)
    }

    /// Write `json` to `path` only if the file does not already contain the same
    /// JSON value.
    ///
    /// Comparing parsed [`serde_json::Value`]s prevents spurious rewrites when
    /// map key ordering differs between serialisations.
    fn write_config_if_changed(path: &Path, json: &str) -> anyhow::Result<()> {
        let changed = match std::fs::read_to_string(path) {
            Ok(existing) => {
                let existing_value: serde_json::Value =
                    serde_json::from_str(&existing).unwrap_or(serde_json::Value::Null);
                let new_value: serde_json::Value =
                    serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
                existing_value != new_value
            }
            Err(_) => true,
        };

        if changed {
            std::fs::write(path, json).map_err(|e| {
                anyhow::anyhow!("Failed to write config file '{}': {}", path.display(), e)
            })?;
        }

        Ok(())
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
        if overlay.specified_default_agent
            || overlay.default_agent != default_agent_name()
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
        // Price overrides: append entries from the overlay, replacing any
        // existing entries with the same model id (last-wins per model).
        for entry in overlay.prices {
            if let Some(existing) = base.prices.iter_mut().find(|e| e.model == entry.model) {
                *existing = entry;
            } else {
                base.prices.push(entry);
            }
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

        // Telemetry: merge the overlay telemetry config into the base. The
        // overlay takes precedence when it explicitly enables telemetry.
        base.telemetry =
            crate::telemetry::TelemetryConfig::merge(&base.telemetry, &overlay.telemetry);
        // Legacy experimental.open_telemetry flag support: if the overlay has
        // the legacy flag set, apply it after merging the new telemetry block.
        if overlay.experimental.open_telemetry {
            base.telemetry.apply_legacy_flag(true);
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

        // tavily_api_key: overlay overrides base
        if overlay.tavily_api_key.is_some() {
            base.tavily_api_key = overlay.tavily_api_key;
        }

        // channels: overlay fields override base (M7)
        if overlay.channels.enabled {
            base.channels.enabled = true;
        }
        if overlay.channels.telegram.is_some() {
            base.channels.telegram = overlay.channels.telegram;
        }
        if overlay.channels.discord.is_some() {
            base.channels.discord = overlay.channels.discord;
        }

        // gmail: overlay fields override base (M7)
        if overlay.gmail.client_id.is_some() {
            base.gmail.client_id = overlay.gmail.client_id;
        }
        if overlay.gmail.client_secret.is_some() {
            base.gmail.client_secret = overlay.gmail.client_secret;
        }
        if overlay.gmail.base_url.is_some() {
            base.gmail.base_url = overlay.gmail.base_url;
        }

        // langsearch_api_key: overlay overrides base
        if overlay.langsearch_api_key.is_some() {
            base.langsearch_api_key = overlay.langsearch_api_key;
        }

        // hidden_tools: union of base and overlay (both lists are honoured)
        for name in overlay.hidden_tools {
            if !base.hidden_tools.contains(&name) {
                base.hidden_tools.push(name);
            }
        }

        // code_index: overlay takes precedence only for explicitly set fields.
        // The `specified` flags from the overlay are propagated onto the base so
        // that a subsequent serialise (e.g. save_to_source) writes the field
        // explicitly and the value survives a reload even when a global config
        // disagrees.
        if overlay.code_index.specified.enabled {
            base.code_index.enabled = overlay.code_index.enabled;
            base.code_index.specified.enabled = true;
        }
        if overlay.code_index.specified.max_file_size {
            base.code_index.max_file_size = overlay.code_index.max_file_size;
            base.code_index.specified.max_file_size = true;
        }
        if overlay.code_index.specified.extra_exclude_dirs {
            base.code_index.extra_exclude_dirs = overlay.code_index.extra_exclude_dirs;
            base.code_index.specified.extra_exclude_dirs = true;
        }
        if overlay.code_index.specified.extra_exclude_patterns {
            base.code_index.extra_exclude_patterns = overlay.code_index.extra_exclude_patterns;
            base.code_index.specified.extra_exclude_patterns = true;
        }

        // tool_visibility: overlay takes precedence only for explicitly set fields.
        // Propagate `specified` flags so an explicit toggle persists on save.
        if overlay.tool_visibility.specified.office {
            base.tool_visibility.office = overlay.tool_visibility.office;
            base.tool_visibility.specified.office = true;
        }
        if overlay.tool_visibility.specified.github {
            base.tool_visibility.github = overlay.tool_visibility.github;
            base.tool_visibility.specified.github = true;
        }
        if overlay.tool_visibility.specified.gitlab {
            base.tool_visibility.gitlab = overlay.tool_visibility.gitlab;
            base.tool_visibility.specified.gitlab = true;
        }
        if overlay.tool_visibility.specified.teams {
            base.tool_visibility.teams = overlay.tool_visibility.teams;
            base.tool_visibility.specified.teams = true;
        }
        if overlay.tool_visibility.specified.agents {
            base.tool_visibility.agents = overlay.tool_visibility.agents;
            base.tool_visibility.specified.agents = true;
        }
        if overlay.tool_visibility.specified.plan {
            base.tool_visibility.plan = overlay.tool_visibility.plan;
            base.tool_visibility.specified.plan = true;
        }
        if overlay.tool_visibility.specified.codeindex {
            base.tool_visibility.codeindex = overlay.tool_visibility.codeindex;
            base.tool_visibility.specified.codeindex = true;
        }
        if overlay.tool_visibility.specified.masterfetch {
            base.tool_visibility.masterfetch = overlay.tool_visibility.masterfetch;
            base.tool_visibility.specified.masterfetch = true;
        }
        if overlay.tool_visibility.specified.browser {
            base.tool_visibility.browser = overlay.tool_visibility.browser;
            base.tool_visibility.specified.browser = true;
        }

        // Compaction: overlay takes precedence.        base.compaction = overlay.compaction;
        base.compression = LegacyCompressionConfig::default();

        // Apply legacy `compression.enabled` -> `compaction.auto` migration when
        // the overlay config file contains `compression` but not `compaction`.
        for path in &overlay.config_paths {
            if let Ok(content) = std::fs::read_to_string(path)
                && let Ok(value) = serde_json::from_str::<serde_json::Value>(&content)
            {
                let has_compaction = value.get("compaction").is_some();
                let has_compression = value.get("compression").is_some();
                if !has_compaction && has_compression {
                    if let Some(enabled) = value
                        .get("compression")
                        .and_then(|c| c.get("enabled"))
                        .and_then(|v| v.as_bool())
                    {
                        base.compaction.auto = enabled;
                    }
                    // Carry the configured trigger point across so compaction
                    // fires at the user's chosen percentage (e.g. 0.8 = 80%)
                    // instead of the buffer-based default.
                    if let Some(threshold) = value
                        .get("compression")
                        .and_then(|c| c.get("auto_threshold"))
                        .and_then(|v| v.as_f64())
                    {
                        base.compaction.threshold = Some(threshold);
                    }
                }
            }
        }

        if overlay.yolo {
            base.yolo = overlay.yolo;
        }

        base
    }

    fn merge_provider_config(mut base: ProviderConfig, overlay: ProviderConfig) -> ProviderConfig {
        if overlay.thinking.is_some() {
            base.thinking = overlay.thinking;
        }
        // Provider-level api/env/models/options come from the overlay if it
        // specifies them; the base value is preserved otherwise.  Deep
        // model-level merge happens at the `provider.<id>.models.<model>` level
        // when callers go through `Config::merge`.
        if overlay.api.is_some() {
            base.api = overlay.api;
        }
        if !overlay.env.is_empty() {
            base.env = overlay.env;
        }
        if !overlay.models.is_empty() {
            base.models = overlay.models;
        }
        if !overlay.options.is_empty() {
            base.options = overlay.options;
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
            cross_project: CrossProjectConfig::default(),
        }
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
/// When enabled, structured memories are embedded using a local
/// sentence-transformer model for similarity-based retrieval. This extends
/// the existing FTS5 keyword search with cosine-similarity ranking.
///
/// # Feature flag
///
/// The `embeddings` Cargo feature must be enabled for the local ONNX-based
/// embedding provider. When the feature is disabled, memory search falls back
/// to FTS5-only mode regardless of this config.
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
    /// When `false` (default), memory search uses FTS5 keyword search only.
    /// When `true` and the `embeddings` feature is compiled in, entries are
    /// embedded and searched by cosine similarity.
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

/// Stale memory eviction configuration.
///
/// Controls how memories that have decayed below the minimum confidence
/// threshold are evicted from the store.
///
/// ```json
/// {
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
    /// When `true` (default when cross_project is enabled), `memory_recall`
    /// searches across both global and project scopes. The legacy `memory_search`
    /// tool has been removed; `memory_recall` is the current structured-memory
    /// search tool.
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

/// Browser automation configuration (JCODEPLAN M4).
///
/// Controls the CDP (Chrome DevTools Protocol) endpoint used by the `browser`
/// tool. When `cdp_endpoint` is `None` or empty, the tool defaults to
/// `http://127.0.0.1:9222`.
///
/// ```json
/// {
///   "browser": {
///     "cdp_endpoint": "http://127.0.0.1:9222",
///     "default_headless": true
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    /// CDP HTTP endpoint URL (e.g. `"http://127.0.0.1:9222"`).
    ///
    /// When `None` or empty, the `browser` tool defaults to
    /// `http://127.0.0.1:9222`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdp_endpoint: Option<String>,
    /// Default headless mode for the `setup` action (default: `true`).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub default_headless: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            cdp_endpoint: None,
            default_headless: true,
        }
    }
}

/// Return `true` if the value is `true` — used for `skip_serializing_if`.
fn is_true(v: &bool) -> bool {
    *v
}

/// External messaging channel configuration for the `send_channel_message` tool
/// (JCODEPLAN M7, T-061).
///
/// Configured under the `channels` key in `ragent.json`:
///
/// ```json
/// {
///   "channels": {
///     "enabled": true,
///     "telegram": { "bot_token": "123:abc", "chat_id": "-100123" },
///     "discord": { "webhook_url": "https://discord.com/api/webhooks/..." }
///   }
/// }
/// ```
///
/// Token values support a `env:VAR_NAME` prefix — the value is then read from
/// the named environment variable at use time, so secrets do not need to live
/// in the config file (e.g. `"bot_token": "env:TELEGRAM_BOT_TOKEN"`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelsConfig {
    /// Master switch for the channel messaging tool (default: `false`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub enabled: bool,
    /// Telegram bot channel configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telegram: Option<TelegramChannelConfig>,
    /// Discord webhook channel configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord: Option<DiscordChannelConfig>,
}

impl ChannelsConfig {
    /// Returns `true` when no channels settings are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.enabled && self.telegram.is_none() && self.discord.is_none()
    }
}

/// Telegram channel settings for `send_channel_message`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelegramChannelConfig {
    /// Bot token from BotFather. Supports the `env:VAR_NAME` indirection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_token: Option<String>,
    /// Chat identifier (`chat_id`) that messages are sent to. Supports the
    /// `env:VAR_NAME` indirection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    /// Optional HTTP(S) endpoint override (used by tests; defaults to
    /// `https://api.telegram.org`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// Discord channel settings for `send_channel_message`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscordChannelConfig {
    /// Full webhook URL (`https://discord.com/api/webhooks/<id>/<token>`).
    /// Supports the `env:VAR_NAME` indirection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
}

/// Gmail tool configuration (JCODEPLAN M7, T-060).
///
/// Configured under the `gmail` key in `ragent.json`:
///
/// ```json
/// {
///   "gmail": {
///     "client_id": "...apps.googleusercontent.com",
///     "client_secret": "env:GMAIL_CLIENT_SECRET"
///   }
/// }
/// ```
///
/// The OAuth2 access/refresh tokens used to call the Gmail API are managed by
/// the `gmail` tool itself (`auth`/`status`/`logout` actions) and are stored
/// encrypted in `ragent-storage` — never in this file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GmailConfig {
    /// OAuth2 client ID used for refresh-token exchange. Supports the
    /// `env:VAR_NAME` indirection, and falls back to the
    /// `GMAIL_CLIENT_ID` environment variable when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// OAuth2 client secret used for refresh-token exchange. Supports the
    /// `env:VAR_NAME` indirection, and falls back to the
    /// `GMAIL_CLIENT_SECRET` environment variable when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// Optional HTTP(S) endpoint override (used by tests; defaults to
    /// `https://gmail.googleapis.com`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl GmailConfig {
    /// Returns `true` when no gmail settings are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.client_id.is_none() && self.client_secret.is_none() && self.base_url.is_none()
    }
}
