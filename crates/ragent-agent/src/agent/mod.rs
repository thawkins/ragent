//! Agent definitions, built-in agent registry, and prompt construction.
//!
//! This module defines the [`AgentInfo`] type that describes an agent's
//! identity, model binding, permissions, and system prompt. It also provides
//! [`create_builtin_agents`] for the default agent roster and
//! [`resolve_agent`] for merging built-in definitions with user config.
//!
//! Custom agents defined using the OASF standard are loaded via
//! [`load_all_agents`], which combines built-ins with agents discovered from
//! `.ragent/agents/` directories.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::permission::{Permission, PermissionAction, PermissionRule, PermissionRuleset};
use ragent_types::{ThinkingConfig, ThinkingLevel};

pub mod custom;
pub mod oasf;

pub use custom::CustomAgentDef;

#[derive(Debug, Clone, Default)]
struct PromptContextCache {
    git: String,
    readme: String,
    agents_md: String,
    file_tree: String,
    cached_at: Option<std::time::Instant>,
}

static PROMPT_CONTEXT_CACHE: OnceLock<Mutex<HashMap<String, PromptContextCache>>> = OnceLock::new();
static NO_GIT_CONTEXT: AtomicBool = AtomicBool::new(false);
static NO_README_CONTEXT: AtomicBool = AtomicBool::new(false);

fn prompt_context_cache() -> &'static Mutex<HashMap<String, PromptContextCache>> {
    PROMPT_CONTEXT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn prompt_context_cache_key(working_dir: &Path) -> String {
    let cwd = working_dir
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());
    format!(
        "{}|git:{}|readme:{}",
        cwd.display(),
        NO_GIT_CONTEXT.load(Ordering::Relaxed),
        NO_README_CONTEXT.load(Ordering::Relaxed)
    )
}

/// Clear cached prompt-context snippets.
pub fn clear_prompt_context_cache() {
    if let Ok(mut cache) = prompt_context_cache().lock() {
        cache.clear();
    }
}

/// Disable automatic git prompt context injection.
pub fn disable_git_prompt_context() {
    NO_GIT_CONTEXT.store(true, Ordering::Relaxed);
}

/// Disable automatic README prompt context injection.
pub fn disable_readme_prompt_context() {
    NO_README_CONTEXT.store(true, Ordering::Relaxed);
}

fn truncate_lines(text: &str, max_lines: usize) -> String {
    let mut lines = text.lines();
    let mut out = Vec::new();
    for _ in 0..max_lines {
        if let Some(line) = lines.next() {
            out.push(line);
        } else {
            return text.to_string();
        }
    }
    if lines.next().is_some() {
        out.push("... (truncated)");
    }
    out.join("\n")
}

async fn run_command_with_timeout(
    working_dir: &Path,
    program: &str,
    args: &[&str],
) -> Option<String> {
    use tokio::process::Command;
    use tokio::time::{Duration, timeout};

    let output = timeout(
        Duration::from_secs(1),
        Command::new(program)
            .args(args)
            .current_dir(working_dir)
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

async fn collect_git_context(working_dir: &Path) -> String {
    if NO_GIT_CONTEXT.load(Ordering::Relaxed) {
        return String::new();
    }

    let branch = run_command_with_timeout(working_dir, "git", &["branch", "--show-current"]);
    let origin_head = run_command_with_timeout(
        working_dir,
        "git",
        &["symbolic-ref", "refs/remotes/origin/HEAD"],
    );
    let status = run_command_with_timeout(working_dir, "git", &["status", "--short"]);
    let recent = run_command_with_timeout(working_dir, "git", &["log", "--oneline", "-n5"]);
    let authors = run_command_with_timeout(
        working_dir,
        "git",
        &["shortlog", "-sn", "--all", "--no-merges"],
    );

    let (branch, origin_head, status, recent, authors) =
        tokio::join!(branch, origin_head, status, recent, authors);

    let mut output = String::new();
    if let Some(branch) = branch {
        output.push_str(&format!("**Branch:** {branch}\n"));
    }
    if let Some(origin_head) = origin_head {
        let cleaned = origin_head
            .trim()
            .strip_prefix("refs/remotes/origin/")
            .unwrap_or(origin_head.trim());
        output.push_str(&format!("**Origin HEAD:** {cleaned}\n"));
    }
    if let Some(status) = status {
        output.push_str("**Status:**\n```\n");
        output.push_str(&status);
        output.push_str("\n```\n");
    }
    if let Some(recent) = recent {
        output.push_str("**Recent Commits:**\n```\n");
        output.push_str(&recent);
        output.push_str("\n```\n");
    }
    if let Some(authors) = authors {
        output.push_str("**Top Authors:**\n```\n");
        output.push_str(&authors);
        output.push_str("\n```\n");
    }

    truncate_lines(&output, 200)
}

fn find_readme_path(working_dir: &Path) -> Option<std::path::PathBuf> {
    let wanted = ["readme.md", "readme.txt", "readme.rst"];
    let mut current = Some(working_dir);
    for _ in 0..=3 {
        let dir = current?;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if wanted
                    .iter()
                    .any(|needle| name.eq_ignore_ascii_case(needle))
                {
                    return Some(path);
                }
            }
        }
        current = dir.parent();
    }
    None
}

async fn collect_readme_context(working_dir: &Path) -> String {
    if NO_README_CONTEXT.load(Ordering::Relaxed) {
        return String::new();
    }

    let Some(path) = find_readme_path(working_dir) else {
        return String::new();
    };

    let path_for_read = path.clone();

    tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(&path_for_read).ok().map(|content| {
            let mut lines = content.lines();
            let mut preview = Vec::new();
            for _ in 0..500 {
                if let Some(line) = lines.next() {
                    preview.push(line);
                } else {
                    break;
                }
            }
            let truncated = lines.next().is_some();
            let mut output = format!(
                "**File:** {}\n```\n{}\n```",
                path.display(),
                preview.join("\n")
            );
            if truncated {
                output.push_str("\n*(truncated to first 500 lines)*");
            }
            output
        })
    })
    .await
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// Collect git, README, and agents-md context snippets for prompt injection.
pub async fn collect_prompt_context(working_dir: &Path) -> (String, String, String, String) {
    const TTL: std::time::Duration = std::time::Duration::from_secs(30);
    let key = prompt_context_cache_key(working_dir);
    if let Ok(cache) = prompt_context_cache().lock()
        && let Some(entry) = cache.get(&key)
        && entry.cached_at.is_some_and(|t| t.elapsed() < TTL)
    {
        return (
            entry.git.clone(),
            entry.readme.clone(),
            entry.agents_md.clone(),
            entry.file_tree.clone(),
        );
    }

    let git = collect_git_context(working_dir);
    let readme = collect_readme_context(working_dir);
    let (git, readme) = tokio::join!(git, readme);

    let wd = working_dir.to_path_buf();
    let agents_md = tokio::task::spawn_blocking(move || collect_agents_md_content(&wd))
        .await
        .unwrap_or_default();

    let wd2 = working_dir.to_path_buf();
    let file_tree = tokio::task::spawn_blocking(move || build_file_tree(&wd2, 2))
        .await
        .unwrap_or_default();

    if let Ok(mut cache) = prompt_context_cache().lock() {
        cache.insert(
            key,
            PromptContextCache {
                git: git.clone(),
                readme: readme.clone(),
                agents_md: agents_md.clone(),
                file_tree: file_tree.clone(),
                cached_at: Some(std::time::Instant::now()),
            },
        );
    }

    (git, readme, agents_md, file_tree)
}

fn build_file_tree(dir: &Path, max_depth: usize) -> String {
    let mut lines = Vec::new();
    build_tree_recursive(dir, "", 0, max_depth, &mut lines);
    lines.join("\n")
}

fn build_tree_recursive(
    dir: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    lines: &mut Vec<String>,
) {
    if depth >= max_depth {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut entries: Vec<_> = entries.filter_map(std::result::Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);

    entries.retain(|e| {
        let name = e.file_name();
        let name_str = name.to_string_lossy();
        !name_str.starts_with('.')
            && !matches!(
                name_str.as_ref(),
                "node_modules" | "target" | "__pycache__" | "dist" | "build" | ".git"
            )
    });

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let path = entry.path();

        if path.is_dir() {
            lines.push(format!("{prefix}{connector}{name_str}/"));
            let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
            build_tree_recursive(&path, &new_prefix, depth + 1, max_depth, lines);
        } else {
            lines.push(format!("{prefix}{connector}{name_str}"));
        }
    }
}

/// Determines when an agent is available for use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    /// Agent can be used as the top-level (primary) agent.
    Primary,
    /// Agent runs as a child of another agent.
    Subagent,
    /// Agent may be used in either role.
    All,
}

impl fmt::Display for AgentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primary => write!(f, "primary"),
            Self::Subagent => write!(f, "subagent"),
            Self::All => write!(f, "all"),
        }
    }
}

/// Reference to a specific model offered by a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRef {
    /// Identifier of the LLM provider (e.g. `"anthropic"`).
    pub provider_id: String,
    /// Model identifier within the provider (e.g. `"claude-sonnet-4-20250514"`).
    pub model_id: String,
}

/// Complete definition of an agent, including its model, prompt, and permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Unique name used to select this agent.
    pub name: String,
    /// Human-readable description of the agent's purpose.
    pub description: String,
    /// Whether this agent runs as primary, subagent, or both.
    pub mode: AgentMode,
    /// If `true`, the agent is omitted from user-facing listings.
    pub hidden: bool,
    /// Sampling temperature override for the model.
    pub temperature: Option<f32>,
    /// Top-p (nucleus) sampling override.
    pub top_p: Option<f32>,
    /// Model binding for this agent.
    pub model: Option<ModelRef>,
    /// System prompt injected at the start of conversations.
    pub prompt: Option<String>,
    /// Permission rules governing tool access.
    pub permission: PermissionRuleset,
    /// Maximum number of agentic loop iterations.
    pub max_steps: Option<u32>,
    /// Skill names this agent should preload into its prompt context.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Persistent memory scope for this agent.
    #[serde(default)]
    pub memory: crate::team::config::MemoryScope,
    /// Default thinking configuration for this agent's LLM requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    /// Arbitrary key-value options forwarded to the provider.
    // TODO: Replace `Value` with typed agent option structs.
    pub options: HashMap<String, Value>,
    /// When `true` the `model` field was explicitly set by a custom agent
    /// profile and should not be overridden by the user's global provider
    /// selection.  Built-in agents set this to `false` so `/provider` works.
    #[serde(default)]
    pub model_pinned: bool,
}

impl AgentInfo {
    /// Creates a new agent with the given name and description, using default values.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::agent::AgentInfo;
    ///
    /// let agent = AgentInfo::new("my-agent", "A custom coding assistant");
    /// assert_eq!(agent.name, "my-agent");
    /// assert_eq!(agent.description, "A custom coding assistant");
    /// assert!(agent.model.is_none());
    /// ```
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            mode: AgentMode::Primary,
            hidden: false,
            temperature: None,
            top_p: None,
            model: None,
            prompt: None,
            permission: Vec::new(),
            max_steps: None,
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        }
    }
}

impl Default for AgentInfo {
    fn default() -> Self {
        Self::new("", "")
    }
}

/// Returns the full set of built-in agents shipped with ragent.
///
/// Includes `chat`, `general`, `build`, `plan`, `explore`, `title`, `summary`,
/// and `compaction` agents.
///
/// # Examples
///
/// ```
/// use ragent_core::agent::create_builtin_agents;
///
/// let agents = create_builtin_agents();
/// assert!(!agents.is_empty());
///
/// let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
/// assert!(names.contains(&"general"));
/// assert!(names.contains(&"explore"));
/// ```
#[must_use]
pub fn create_builtin_agents() -> Vec<AgentInfo> {
    vec![
        AgentInfo {
            name: "ask".to_string(),
            description: "Quick Q&A — answers questions without tools".to_string(),
            mode: AgentMode::Primary,
            hidden: false,
            temperature: None,
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4-20250514".to_string(),
            }),
            prompt: Some(
                "You are a helpful AI assistant. Answer the user's questions clearly and \
                 concisely. You do not have access to any tools — just respond with your \
                 best knowledge."
                    .to_string(),
            ),
            permission: read_only_permissions(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: Some(ThinkingConfig::off()),
            options: HashMap::new(),
            model_pinned: false,
        },
        AgentInfo {
            name: "general".to_string(),
            description: "General-purpose coding agent".to_string(),
            mode: AgentMode::Primary,
            hidden: false,
            temperature: None,
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4-20250514".to_string(),
            }),
            prompt: Some(
                "You are a powerful AI coding assistant. You help users with software development \
                 tasks including writing code, debugging, reviewing, and explaining code. \
                 You have access to tools for reading, writing, and editing files, executing \
                 shell commands, and searching codebases. \
                 Use 'grep' or 'search' to find text/code patterns, 'glob' to find files by name, \
                 'list' to view directory contents, and 'read' to view file contents. \
                 Always prefer using tools to verify your assumptions rather than guessing."
                    .to_string(),
            ),
            permission: default_permissions(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        },
        AgentInfo {
            name: "build".to_string(),
            description: "Build and test agent with full tool access".to_string(),
            mode: AgentMode::Subagent,
            hidden: false,
            temperature: None,
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4-20250514".to_string(),
            }),
            prompt: Some(
                "You are a build agent specializing in compiling, testing, and debugging \
                 software projects. Focus on running builds, fixing compilation errors, \
                 running tests, and ensuring code quality. Use bash commands to interact \
                 with build systems and test frameworks."
                    .to_string(),
            ),
            permission: default_permissions(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        },
        AgentInfo {
            name: "plan".to_string(),
            description: "Planning agent that creates implementation plans".to_string(),
            mode: AgentMode::Subagent,
            hidden: false,
            temperature: Some(0.7),
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4-20250514".to_string(),
            }),
            prompt: Some(
                "You are a planning agent. Your job is to analyze requirements and create \
                 detailed implementation plans. Read the codebase to understand existing patterns \
                 and architecture. Output a structured plan with clear steps. Do NOT make any \
                 changes yourself — only plan and document."
                    .to_string(),
            ),
            permission: read_only_permissions(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        },
        AgentInfo {
            name: "explore".to_string(),
            description: "Exploration agent for understanding codebases".to_string(),
            mode: AgentMode::Subagent,
            hidden: false,
            temperature: None,
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-haiku-latest".to_string(),
            }),
            prompt: Some(
                "You are an exploration agent specializing in understanding codebases. \
                 Use read, grep, glob, and list tools to navigate and understand code. \
                 Provide concise, accurate answers about code structure, patterns, and logic. \
                 Do NOT modify any files."
                    .to_string(),
            ),
            permission: read_only_permissions(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        },
        AgentInfo {
            name: "title".to_string(),
            description: "Generate session titles".to_string(),
            mode: AgentMode::Subagent,
            hidden: true,
            temperature: Some(0.3),
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-haiku-latest".to_string(),
            }),
            prompt: Some(
                "Generate a short, descriptive title (3-6 words) for a coding session \
                 based on the conversation. Output ONLY the title, nothing else."
                    .to_string(),
            ),
            permission: Vec::new(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        },
        AgentInfo {
            name: "summary".to_string(),
            description: "Summarize sessions".to_string(),
            mode: AgentMode::Subagent,
            hidden: true,
            temperature: Some(0.3),
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-haiku-latest".to_string(),
            }),
            prompt: Some(
                "Summarize the conversation so far into a concise paragraph that captures \
                 the key topics discussed, decisions made, and work completed."
                    .to_string(),
            ),
            permission: Vec::new(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        },
        AgentInfo {
            name: "compaction".to_string(),
            description: "Compact conversation history".to_string(),
            mode: AgentMode::Subagent,
            hidden: true,
            temperature: Some(0.2),
            top_p: None,
            model: Some(ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-3-5-haiku-latest".to_string(),
            }),
            prompt: Some(
                "You are a compaction agent. Summarize the conversation into a shorter \
                 representation that preserves all important context, decisions, and state. \
                 Include file paths, key code changes, and outstanding tasks."
                    .to_string(),
            ),
            permission: Vec::new(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            thinking: None,
            options: HashMap::new(),
            model_pinned: false,
        },
    ]
}

/// Helper to create a permission rule with the given parameters.
fn rule(
    permission: Permission,
    pattern: impl Into<String>,
    action: PermissionAction,
) -> PermissionRule {
    PermissionRule {
        permission,
        pattern: Some(pattern.into()),
        action,
    }
}

/// Returns the default permission ruleset applied when a custom agent does not
/// specify its own `permissions` array.
#[must_use]
pub fn default_permissions() -> PermissionRuleset {
    vec![
        rule(Permission::Read, "**", PermissionAction::Allow),
        rule(Permission::Edit, "**", PermissionAction::Ask),
        rule(Permission::Bash, "*", PermissionAction::Ask),
        rule(Permission::Web, "*", PermissionAction::Ask),
        rule(Permission::PlanEnter, "*", PermissionAction::Ask),
        rule(Permission::Todo, "*", PermissionAction::Allow),
        // Auto-approve all codeindex tools
        rule(
            Permission::Custom("tool:codeindex_search".to_string()),
            "*",
            PermissionAction::Allow,
        ),
        rule(
            Permission::Custom("tool:codeindex_symbols".to_string()),
            "*",
            PermissionAction::Allow,
        ),
        rule(
            Permission::Custom("tool:codeindex_references".to_string()),
            "*",
            PermissionAction::Allow,
        ),
        rule(
            Permission::Custom("tool:codeindex_dependencies".to_string()),
            "*",
            PermissionAction::Allow,
        ),
        rule(
            Permission::Custom("tool:codeindex_status".to_string()),
            "*",
            PermissionAction::Allow,
        ),
        rule(
            Permission::Custom("tool:codeindex_reindex".to_string()),
            "*",
            PermissionAction::Allow,
        ),
    ]
}

fn read_only_permissions() -> PermissionRuleset {
    vec![
        rule(Permission::Read, "**", PermissionAction::Allow),
        rule(Permission::Edit, "**", PermissionAction::Deny),
        rule(Permission::Bash, "*", PermissionAction::Deny),
    ]
}

/// Returns the default thinking configuration for a model's supported levels.
#[must_use]
pub fn default_thinking_config_for_levels(levels: &[ThinkingLevel]) -> ThinkingConfig {
    let _ = levels;
    ThinkingConfig::off()
}

/// Returns the fallback thinking configuration for a resolved provider/model.
///
/// Precedence: config per-model → config per-provider → model metadata → built-in default.
#[must_use]
pub fn fallback_thinking_for_model_ref(
    config: &crate::config::Config,
    provider_registry: &crate::provider::ProviderRegistry,
    model_ref: &ModelRef,
) -> Option<ThinkingConfig> {
    config
        .thinking_config_for_model(&model_ref.provider_id, &model_ref.model_id)
        .or_else(|| {
            provider_registry
                .resolve_model(&model_ref.provider_id, &model_ref.model_id)
                .map(|model| {
                    model.thinking_config.unwrap_or_else(|| {
                        default_thinking_config_for_levels(&model.capabilities.thinking_levels)
                    })
                })
        })
}

/// Applies fallback thinking to an agent when it has a resolved model but no explicit default.
pub fn apply_fallback_thinking(
    agent: &mut AgentInfo,
    config: &crate::config::Config,
    provider_registry: &crate::provider::ProviderRegistry,
) {
    if agent.thinking.is_none()
        && let Some(model_ref) = agent.model.as_ref()
    {
        agent.thinking = fallback_thinking_for_model_ref(config, provider_registry, model_ref);
    }
}

/// Resolve an agent by name, merging built-in definition with config overrides.
///
/// # Errors
///
/// Returns an error if config overlay parsing fails (e.g. invalid model string format).
///
/// # Examples
///
/// ```
/// use ragent_core::agent::resolve_agent;
/// use ragent_core::config::Config;
///
/// let config = Config::default();
/// let agent = resolve_agent("general", &config).unwrap();
/// assert_eq!(agent.name, "general");
/// ```
pub fn resolve_agent(name: &str, config: &crate::config::Config) -> anyhow::Result<AgentInfo> {
    let builtins = create_builtin_agents();
    let mut agent = builtins
        .into_iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| AgentInfo::new(name, format!("Custom agent: {name}")));

    // Apply config overrides
    if let Some(agent_config) = config.agent.get(name) {
        if let Some(ref prompt) = agent_config.prompt {
            agent.prompt = Some(prompt.clone());
        }
        if let Some(temp) = agent_config.temperature {
            agent.temperature = Some(temp);
        }
        if let Some(top_p) = agent_config.top_p {
            agent.top_p = Some(top_p);
        }
        if let Some(ref model_str) = agent_config.model {
            // Parse "provider:model" format
            if let Some((provider, model)) = model_str.split_once(':') {
                agent.model = Some(ModelRef {
                    provider_id: provider.to_string(),
                    model_id: model.to_string(),
                });
            }
        }
        if let Some(max_steps) = agent_config.max_steps {
            agent.max_steps = Some(max_steps);
        }
        if !agent_config.permission.is_empty() {
            agent.permission = agent_config.permission.clone();
        }
        agent.hidden = agent_config.hidden;
        if !agent_config.skills.is_empty() {
            agent.skills = agent_config.skills.clone();
        }
        for (k, v) in &agent_config.options {
            agent.options.insert(k.clone(), v.clone());
        }
    }

    Ok(agent)
}

/// Like [`resolve_agent`] but also searches custom OASF agents loaded from
/// `[PROJECT]/.ragent/agents/` and `~/.ragent/agents/`.
///
/// Lookup order:
/// 1. Project-local custom agents (highest priority)
/// 2. User-global custom agents
/// 3. Built-in agents with config overrides
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ragent_core::agent::resolve_agent_with_customs;
/// use ragent_core::config::Config;
///
/// let config = Config::default();
/// let agent = resolve_agent_with_customs("my-custom-agent", &config, Path::new(".")).unwrap();
/// assert_eq!(agent.name, "my-custom-agent");
/// ```
pub fn resolve_agent_with_customs(
    name: &str,
    config: &crate::config::Config,
    working_dir: &Path,
) -> anyhow::Result<AgentInfo> {
    let (custom_defs, _) = custom::load_custom_agents(working_dir);
    if let Some(def) = custom_defs.into_iter().find(|d| d.agent_info.name == name) {
        return Ok(def.agent_info);
    }
    resolve_agent(name, config)
}

/// Load every available agent: built-ins plus custom OASF-defined agents.
///
/// Custom agents are discovered from `~/.ragent/agents/` (user-global) and
/// `[PROJECT]/.ragent/agents/` (project-local). Project-local definitions
/// take precedence over user-global ones when names collide. If a custom
/// agent name collides with a built-in, the custom agent is renamed to
/// `custom:<name>` and a diagnostic warning is added.
///
/// Returns `(agents, diagnostics)`. Diagnostics are non-fatal strings
/// suitable for display in the TUI log panel.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ragent_core::agent::load_all_agents;
///
/// let (agents, warnings) = load_all_agents(Path::new("."));
/// println!("{} agents loaded, {} warnings", agents.len(), warnings.len());
/// ```
#[must_use]
pub fn load_all_agents(working_dir: &Path) -> (Vec<AgentInfo>, Vec<String>) {
    let builtins = create_builtin_agents();
    let builtin_names: std::collections::HashSet<String> =
        builtins.iter().map(|a| a.name.clone()).collect();

    let (custom_defs, mut diagnostics) = custom::load_custom_agents(working_dir);

    let mut all = builtins;

    for mut def in custom_defs {
        if builtin_names.contains(&def.agent_info.name) {
            let new_name = format!("custom:{}", def.agent_info.name);
            diagnostics.push(format!(
                "custom agent '{}' collides with a built-in — loaded as '{}'",
                def.agent_info.name, new_name
            ));
            def.agent_info.name = new_name;
        }
        all.push(def.agent_info);
    }

    (all, diagnostics)
}

/// Build the system prompt for an agent invocation.
///
/// Assembles the system prompt in the order specified by the SPEC:
/// 1. Agent role definition
/// 2. Working directory context
/// 3. Project structure (file tree)
/// 4. AGENTS.md project guidelines
/// 5. Available skills (agent-invocable skills from the registry)
/// 6. Tool usage guidelines
///
/// When `skills` is `Some`, agent-invocable skills are listed so the model
/// can invoke them automatically. If the agent has specific skills configured
/// in its `skills` field, only those are shown; otherwise all agent-invocable
/// skills from the registry are included.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ragent_core::agent::{AgentInfo, build_system_prompt};
///
/// let mut agent = AgentInfo::new("helper", "A helpful agent");
/// agent.prompt = Some("You are a helpful assistant.".to_string());
/// agent.max_steps = Some(10);
///
/// let prompt = build_system_prompt(&agent, Path::new("/tmp/project"), "src/\n  main.rs", None);
/// assert!(prompt.contains("You are a helpful assistant."));
/// assert!(prompt.contains("/tmp/project"));
/// ```

/// Read git status from the working directory.
/// Returns a formatted string with branch, status, and recent commits, or empty string on error.
fn read_git_status(working_dir: &Path) -> String {
    use std::process::Command;

    let mut output = String::new();

    // Get current branch
    if let Ok(result) = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(working_dir)
        .output()
        && result.status.success()
        && let Ok(branch) = String::from_utf8(result.stdout)
    {
        let branch = branch.trim();
        if !branch.is_empty() {
            output.push_str(&format!("**Branch:** {branch}\n"));
        }
    }

    // Get git status (short format)
    if let Ok(result) = Command::new("git")
        .args(["status", "--short"])
        .current_dir(working_dir)
        .output()
        && result.status.success()
        && let Ok(status) = String::from_utf8(result.stdout)
    {
        let status = status.trim();
        if !status.is_empty() {
            output.push_str("**Status:**\n```\n");
            output.push_str(status);
            output.push_str("\n```\n");
        }
    }

    // Get recent commits (5 most recent, one line each)
    if let Ok(result) = Command::new("git")
        .args(["log", "--oneline", "-n", "5"])
        .current_dir(working_dir)
        .output()
        && result.status.success()
        && let Ok(commits) = String::from_utf8(result.stdout)
    {
        let commits = commits.trim();
        if !commits.is_empty() {
            output.push_str("**Recent Commits:**\n```\n");
            output.push_str(commits);
            output.push_str("\n```\n");
        }
    }

    output
}

/// Read README.md from the working directory.
/// Returns file contents or empty string if not found.
fn read_readme(working_dir: &Path) -> String {
    let readme_path = working_dir.join("README.md");
    std::fs::read_to_string(&readme_path).unwrap_or_default()
}

/// Discover and load all AGENTS.md-style instruction files from the project tree.
///
/// Searches recursively for `AGENTS.md`, `CLAUDE.md`, `.ragent.md`, and
/// `INSTRUCTIONS.md`, sorted by directory depth (root first). Returns a
/// combined string listing the discovered file paths and their concatenated
/// content.
fn collect_agents_md_content(working_dir: &Path) -> String {
    const AGENT_FILE_NAMES: &[&str] = &["AGENTS.md", "CLAUDE.md", ".ragent.md", "INSTRUCTIONS.md"];

    use ignore::WalkBuilder;

    let mut found: Vec<(usize, std::path::PathBuf)> = Vec::new();

    let walk = WalkBuilder::new(working_dir)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .ignore(true)
        .filter_entry(|e| e.file_name() != ".git")
        .build();

    for entry in walk.flatten() {
        let path = entry.path().to_path_buf();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && AGENT_FILE_NAMES.contains(&name)
        {
            let depth = path
                .strip_prefix(working_dir)
                .map(|rel| rel.components().count())
                .unwrap_or(usize::MAX);
            found.push((depth, path));
        }
    }

    if found.is_empty() {
        return String::new();
    }

    // Sort: root files first (depth=1), then deeper subdirectories
    found.sort_by_key(|(depth, path)| (*depth, path.clone()));

    let mut result = String::new();

    result.push_str("### Discovered Instruction Files\n");
    for (_, path) in &found {
        let rel = path
            .strip_prefix(working_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        result.push_str(&format!("- {rel}\n"));
    }
    result.push('\n');

    for (_, path) in &found {
        let rel = path
            .strip_prefix(working_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        if let Ok(content) = std::fs::read_to_string(path) {
            let content = content.trim();
            if !content.is_empty() {
                result.push_str(&format!("### From: {rel}\n\n"));
                result.push_str(content);
                result.push_str("\n\n");
            }
        }
    }

    result
}

/// Build a system prompt for the given agent using cached context.
#[must_use]
pub fn build_system_prompt(
    agent: &AgentInfo,
    working_dir: &Path,
    file_tree: &str,
    skills: Option<&crate::skill::SkillRegistry>,
) -> String {
    build_system_prompt_with_context(agent, working_dir, file_tree, skills, None, None, None)
}

/// Build a system prompt with explicitly supplied context snippets.
///
/// Passing `None` for any context field causes the function to read it
/// on-demand from the filesystem.
pub fn build_system_prompt_with_context(
    agent: &AgentInfo,
    working_dir: &Path,
    file_tree: &str,
    skills: Option<&crate::skill::SkillRegistry>,
    git_status: Option<&str>,
    readme: Option<&str>,
    agents_md: Option<&str>,
) -> String {
    build_system_prompt_with_storage(
        agent,
        working_dir,
        file_tree,
        skills,
        git_status,
        readme,
        agents_md,
        None,
        None,
    )
}

/// Build a system prompt with storage access for structured memory injection.
///
/// This is the full-featured variant that can load relevant structured memories
/// from SQLite when storage is provided.
pub fn build_system_prompt_with_storage(
    agent: &AgentInfo,
    working_dir: &Path,
    file_tree: &str,
    skills: Option<&crate::skill::SkillRegistry>,
    git_status: Option<&str>,
    readme: Option<&str>,
    agents_md: Option<&str>,
    storage: Option<&crate::storage::Storage>,
    memory_config: Option<&crate::config::MemoryConfig>,
) -> String {
    let mut prompt = String::new();

    // Use provided agents_md content or collect it from the project tree.
    let agents_md_content =
        agents_md.map_or_else(|| collect_agents_md_content(working_dir), ToOwned::to_owned);
    let git_status_text =
        git_status.map_or_else(|| read_git_status(working_dir), ToOwned::to_owned);
    let readme_text = readme.map_or_else(|| read_readme(working_dir), ToOwned::to_owned);

    // Agent identity and role — substitute template variables used by custom agents.
    if let Some(ref agent_prompt) = agent.prompt {
        let today = {
            // Use a simple date string; chrono is available transitively.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let days = now / 86400;
            // Approximate calendar date from Unix epoch (good enough for a hint).
            let year = 1970 + days / 365;
            format!("{}-{}", year, "xx-xx") // simple fallback; full chrono used below
        };
        // Use chrono if available (it is a workspace dependency in this workspace).
        let date_str = {
            let dt = chrono::Utc::now();
            dt.format("%Y-%m-%d").to_string()
        };

        let expanded = agent_prompt
            .replace("{{WORKING_DIR}}", &working_dir.display().to_string())
            .replace("{{FILE_TREE}}", file_tree)
            .replace("{{AGENTS_MD}}", &agents_md_content)
            .replace("{{GIT_STATUS}}", &git_status_text)
            .replace("{{README}}", &readme_text)
            .replace("{{DATE}}", &date_str);
        let _ = today; // suppress unused warning from the fallback path

        prompt.push_str(&expanded);
        prompt.push_str("\n\n");
    }

    // Single-step agents (e.g. "ask") are tool-free; skip project context
    // so the model focuses on answering the user's question directly.
    let has_tools = agent.max_steps.is_none_or(|s| s > 1);
    if !has_tools {
        return prompt;
    }

    // Working directory context (skip if already embedded via template variable)
    if agent
        .prompt
        .as_deref()
        .is_none_or(|p| !p.contains("{{WORKING_DIR}}"))
    {
        prompt.push_str(&format!(
            "## Working Directory\n\
             You are operating in: {}\n\n",
            working_dir.display()
        ));
    }

    // File tree context (skip if already embedded via template variable)
    if agent
        .prompt
        .as_deref()
        .is_none_or(|p| !p.contains("{{FILE_TREE}}"))
        && !file_tree.is_empty()
    {
        prompt.push_str("## Project Structure\n");
        prompt.push_str("```\n");
        prompt.push_str(file_tree);
        prompt.push_str("\n```\n\n");
    }

    // AGENTS.md project guidelines (skip if already embedded via template variable)
    if agent
        .prompt
        .as_deref()
        .is_none_or(|p| !p.contains("{{AGENTS_MD}}"))
        && !agents_md_content.is_empty()
    {
        prompt.push_str("## Project Guidelines (AGENTS.md)\n");
        prompt.push_str(&agents_md_content);
        prompt.push_str("\n\n");
    }

    if agent
        .prompt
        .as_deref()
        .is_none_or(|p| !p.contains("{{GIT_STATUS}}"))
        && !git_status_text.trim().is_empty()
    {
        prompt.push_str("## Git Context\n");
        prompt.push_str(&git_status_text);
        prompt.push_str("\n\n");
    }

    if agent
        .prompt
        .as_deref()
        .is_none_or(|p| !p.contains("{{README}}"))
        && !readme_text.trim().is_empty()
    {
        prompt.push_str("## README\n");
        prompt.push_str(&readme_text);
        prompt.push_str("\n\n");
    }

    // Auto-load project and user memory files into context
    {
        use crate::memory::block::BlockScope;
        use crate::memory::storage::{FileBlockStorage, load_all_blocks, load_legacy_memory};

        let block_storage = FileBlockStorage::new();
        let wd = working_dir.to_path_buf();

        // Load all structured memory blocks from both scopes.
        let blocks = load_all_blocks(&block_storage, &wd);
        if !blocks.is_empty() {
            prompt.push_str("## Memory Blocks\n");
            for (scope, block) in &blocks {
                let scope_label = match scope {
                    BlockScope::Global => "global",
                    BlockScope::Project => "project",
                };
                prompt.push_str(&format!("### {} ({})\n", block.label, scope_label));
                if !block.description.is_empty() {
                    prompt.push_str(&format!("*{}*\n\n", block.description));
                }
                if block.read_only {
                    prompt.push_str("*[read-only]*\n");
                }
                prompt.push_str(&block.content);
                prompt.push_str("\n\n");
                if block.limit > 0 {
                    let pct = (block.content.len() as f64 / block.limit as f64 * 100.0) as u32;
                    prompt.push_str(&format!(
                        "*[size: {}/{} bytes, {}%]*\n\n",
                        block.content.len(),
                        block.limit,
                        pct
                    ));
                }
            }
        }

        // Also load legacy MEMORY.md files that aren't already loaded as blocks.
        // This maintains backward compatibility with existing flat memory files.
        let has_project_memory_block = blocks
            .iter()
            .any(|(s, b)| *s == BlockScope::Project && b.label == "MEMORY");
        let has_global_memory_block = blocks
            .iter()
            .any(|(s, b)| *s == BlockScope::Global && b.label == "MEMORY");

        if !has_project_memory_block {
            if let Some(block) = load_legacy_memory(&BlockScope::Project, &wd) {
                prompt.push_str("## Project Memory\n");
                prompt.push_str(&block.content);
                prompt.push_str("\n\n");
            }
        }

        // Load PROJECT_ANALYSIS.md if present (legacy).
        let project_analysis = working_dir
            .join(".ragent")
            .join("memory")
            .join("PROJECT_ANALYSIS.md");
        if let Ok(content) = std::fs::read_to_string(&project_analysis) {
            if !content.trim().is_empty() {
                prompt.push_str("## Project Analysis\n");
                prompt.push_str(&content);
                prompt.push_str("\n\n");
            }
        }

        if !has_global_memory_block {
            if let Some(block) = load_legacy_memory(&BlockScope::Global, &wd) {
                prompt.push_str("## User Memory\n");
                prompt.push_str(&block.content);
                prompt.push_str("\n\n");
            }
        }
    }

    // Load relevant structured memories from SQLite.
    if let Some(sqlite_storage) = storage {
        let project = working_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let max = memory_config
            .map(|c| c.retrieval.max_memories_per_prompt)
            .unwrap_or(5);
        if let Ok(memories) = sqlite_storage.list_memories(project, max) {
            if !memories.is_empty() {
                prompt.push_str("## Relevant Memories\n");
                for mem in &memories {
                    let mem_tags = sqlite_storage.get_memory_tags(mem.id).unwrap_or_default();
                    prompt.push_str(&format!(
                        "- [{}] {} (confidence: {:.2})\n",
                        mem.category, mem.content, mem.confidence,
                    ));
                    if !mem_tags.is_empty() {
                        prompt.push_str(&format!("  tags: {}\n", mem_tags.join(", ")));
                    }
                }
                prompt.push('\n');
            }
        }
    }

    // Available skills (per SPEC §3.19 prompt assembly order)
    if let Some(registry) = skills {
        let skill_list = if agent.skills.is_empty() {
            // No agent-specific skills configured: show all agent-invocable skills
            registry.list_agent_invocable()
        } else {
            // Agent has specific skills configured: filter to those names
            registry
                .list_agent_invocable()
                .into_iter()
                .filter(|s| agent.skills.contains(&s.name))
                .collect()
        };

        if !skill_list.is_empty() {
            prompt.push_str("## Available Skills\n\n");
            prompt.push_str(
                "You can invoke the following skills by including `/skillname` \
                 (with optional arguments) in your response when contextually \
                 appropriate:\n\n",
            );
            for skill in &skill_list {
                let desc = skill.description.as_deref().unwrap_or("(no description)");
                let hint = skill
                    .argument_hint
                    .as_deref()
                    .map(|h| format!(" {h}"))
                    .unwrap_or_default();
                prompt.push_str(&format!("- `/{}{}`  — {}\n", skill.name, hint, desc));
            }
            prompt.push('\n');
        }
    }

    prompt.push_str(
        "## Reasoning Tool\n\n\
         When useful, use the `think` tool to record short reasoning notes before \
         making non-trivial decisions. Keep thoughts brief and focused on the next \
         action.\n\n",
    );

    // Sub-agent spawning guidance (new_task tool) — shown for primary agents only.
    // Agent list is generated dynamically from builtins + custom agents so it stays in sync.
    if agent.mode == AgentMode::Primary {
        let builtins = create_builtin_agents();
        let spawnable: Vec<&AgentInfo> = builtins
            .iter()
            .filter(|a| a.mode == AgentMode::Subagent && !a.hidden)
            .collect();
        let max_background_agents = crate::config::Config::load()
            .map(|c| c.experimental.max_background_agents)
            .unwrap_or(crate::task::DEFAULT_MAX_BACKGROUND_TASKS);

        // Load custom agents and collect the spawnable ones
        let (custom_defs, _) = custom::load_custom_agents(working_dir);
        let spawnable_custom: Vec<AgentInfo> = custom_defs
            .into_iter()
            .filter(|d| {
                (d.agent_info.mode == AgentMode::Subagent || d.agent_info.mode == AgentMode::All)
                    && !d.agent_info.hidden
            })
            .map(|d| d.agent_info)
            .collect();

        let mut section = String::from(
            "## Sub-Agent Spawning\n\n\
             **CRITICAL: Prefer using sub-agents over doing the work yourself.**\n\
             When sub-agents are available, your role shifts from a coder to a manager of \
             specialised agents. Delegate exploration, builds, and planning to them — they run \
             faster and cheaper than you would inline.\n\n\
             **Available agents:**\n",
        );

        for sa in &spawnable {
            // Derive key traits for the LLM to reason about
            let model_tier = sa.model.as_ref().map_or("standard", |m| {
                if m.model_id.contains("haiku") {
                    "fast / low-cost"
                } else if m.model_id.contains("opus") {
                    "powerful / higher-cost"
                } else {
                    "standard"
                }
            });

            let can_write = sa
                .permission
                .iter()
                .any(|r| r.permission == Permission::Edit && r.action == PermissionAction::Allow);
            let can_bash = sa
                .permission
                .iter()
                .any(|r| r.permission == Permission::Bash && r.action == PermissionAction::Allow);

            let mut traits = Vec::new();
            if !can_write {
                traits.push("read-only");
            }
            if can_bash {
                traits.push("can run shell commands");
            }
            traits.push(model_tier);

            section.push_str(&format!(
                "- `{}` — {} [{}]\n",
                sa.name,
                sa.description,
                traits.join(", "),
            ));
        }

        // Append any project/global custom agents so the LLM knows they're available
        if !spawnable_custom.is_empty() {
            section.push_str("\n**Custom agents (project/user defined):**\n");
            for ca in &spawnable_custom {
                let can_write = ca.permission.iter().any(|r| {
                    r.permission == Permission::Edit && r.action == PermissionAction::Allow
                });
                let can_bash = ca.permission.iter().any(|r| {
                    r.permission == Permission::Bash && r.action == PermissionAction::Allow
                });
                let mut traits = vec!["custom"];
                if !can_write {
                    traits.push("read-only");
                }
                if can_bash {
                    traits.push("can run shell commands");
                }
                section.push_str(&format!(
                    "- `{}` — {} [{}]\n",
                    ca.name,
                    ca.description,
                    traits.join(", "),
                ));
            }
        }

        section.push_str(
            "\n**Choosing an agent:**\n\
              - `explore` — fastest and cheapest; use for ANY codebase search, reading, or understanding.\n\
                Read-only. Stateless — loses all context between calls.\n\
                **Always prefer `explore` over doing file searches yourself.**\n\
             - `build`   — use when you need to compile, run tests, apply fixes, or execute shell commands.\n\
             - `plan`    — use to produce a structured implementation plan without making any changes.\n\
             - `general` — full-capability fallback; use when the task doesn't fit a specialist agent.\n\n\
             **CRITICAL — `background` mode rules:**\n\
             - **Use `background: true` for ALL tasks whenever you spawn more than one in the same response.**\n\
               `background: false` blocks the entire agent loop — every subsequent tool call in the same\n\
               response waits for it to finish. This makes parallel spawning impossible.\n\
              - Use `background: false` ONLY when you are spawning a single task and need its result\n\
                before you can continue reasoning (e.g. a quick targeted lookup).\n\
              - When in doubt, use `background: true`.\n\n\
              **CRITICAL — Concurrency limit for background tasks:**\n\
              - You can run at most **MAX_BG_TASKS** background tasks at once in this session.\n\
              - Never call `new_task` with `background: true` if it would exceed this limit.\n\
              - If the limit is reached, call `wait_tasks` (preferred) or `list_tasks`, then spawn only\n\
                after one finishes. Queue additional work in batches.\n\
              - Do not spam retries when you see \"Maximum concurrent background tasks reached\".\n\n\
              **CRITICAL — Parallel explore agents for large codebase reviews:**\n\
              When asked to review, understand, or analyse a codebase with multiple modules or\n\
              directories, DO NOT do the work yourself. Instead:\n\
              1. Identify independent areas (e.g. by top-level crate, directory, or concern).\n\
                 Spawn at most **MAX_BG_TASKS** background agents in one batch.\n\
              2. Spawn a separate `explore` agent for EACH area in the SAME response turn,\n\
                 ALL with `background: true`. They will run concurrently.\n\
              3. Use `list_tasks` to check progress. Synthesise results when all complete.\n\
              This is dramatically faster and cheaper than sequential exploration.\n\n\
             **CRITICAL — Batch all questions into one explore call:**\n\
             The explore agent is stateless — it loses ALL context between calls. Every call starts fresh.\n\
             - Batch ALL related questions into ONE explore call with a comprehensive prompt.\n\
             - If you have independent exploration questions, launch multiple agents IN PARALLEL.\n\
             - ANTI-PATTERN: Do NOT call explore, read the answer, then call explore again with a follow-up.\n\
               Anticipate what you need and ask for everything up-front.\n\
             - After an explore call, do NOT duplicate its work by reading files it already reported.\n\n\
             **When to spawn:**\n\
             - Large codebase review → multiple `explore` agents with `background: true`, one per area\n\
             - Any file search or code understanding task → `explore` (never do it inline)\n\
             - Slow build/test cycle → `build` with `background: true` while you keep reasoning\n\
             - Risky or speculative work → isolate in a focused `general` agent\n\n\
             **Example — parallel codebase review (ALL background: true):**\n\
             ```json\n\
             {\"agent\": \"explore\", \"task\": \"Summarise architecture in crates/ragent-agent/src/agent/ and crates/ragent-agent/src/session/\", \"background\": true}\n\
             {\"agent\": \"explore\", \"task\": \"Summarise architecture in crates/ragent-agent/src/tool/ listing every tool\", \"background\": true}\n\
             {\"agent\": \"explore\", \"task\": \"Summarise architecture in crates/ragent-llm/src/providers/ and crates/ragent-agent/src/llm/ if present\", \"background\": true}\n\
             ```\n\n\
             **Example — single blocking explore (background: false only when one task, result needed immediately):**\n\
             ```json\n\
             {\"agent\": \"explore\", \"task\": \"Find all usages of EventBus in src/ and explain how events flow\", \"background\": false}\n\
             ```\n\n\
              Use `wait_tasks` to block until background tasks finish (preferred — no polling).\n\
              Use `list_tasks` to check status without blocking. Use `cancel_task` to stop a task early.\n\n",
        );
        section = section.replace("MAX_BG_TASKS", &max_background_agents.to_string());

        prompt.push_str(&section);
    }

    // Tool usage guidelines
    prompt.push_str(
        "## Guidelines\n\
         - Use tools to verify information rather than guessing\n\
         - Read files before editing them to understand context\n\
         - Make precise, targeted changes\n\
         - Test changes when possible using the bash tool\n\
         - Explain what you're doing and why\n\n",
    );

    // Specific guidance on using line ranges for file reads
    prompt.push_str(
                  "## File Reading Best Practices\n\n\
                   When reading files with the `read` tool:\n\
                   - **REQUIRED for files larger than 100 lines**: Always use `start_line` and `end_line` parameters\n\
                     to read the file in focused sections rather than all at once\n\
                   - **CRITICAL**: `start_line` and `end_line` must NOT exceed the file's total line count.\n\
                     The tool will return an error if they do. The error message includes the total line count.\n\
                     When you read a file, the response metadata includes `total_lines` — use that value\n\
                     to stay within range on subsequent reads of the same file.\n\
                   - Strategy:\n\
                     1. Read the file without start_line/end_line first — for large files this returns\n\
                        the first 100 lines plus a section map with the total line count\n\
                     2. Use the total_lines from the response to plan your subsequent reads\n\
                     3. Then read specific sections using valid line ranges\n\
                     4. Never read an entire file >100 lines in a single call\n",
              );
        
              // Guidance on using edit / multiedit tools
              prompt.push_str(
                  "\n## Editing Files\n\n\
                   Use the `edit` tool for single surgical text replacements in one file.\n\
                   Use the `multiedit` tool when applying multiple edits across one or more files atomically.\n\
                   \n\
                   When using the `edit` tool:\n\
                   - You MUST always provide the `old_str` parameter containing the exact text to find\n\
                   - You MUST always provide the `new_str` parameter containing the replacement text\n\
                   - Calls to `edit` without `old_str` will fail with an error\n\
                   - The `old_str` must match exactly one location in the file\n\
                   - Read the relevant section of the file first to get the exact text for `old_str`\n\
                   \n\
                   When using the `multiedit` tool:\n\
                   - Provide an `edits` array, where each entry has `path`, `old_str`, and `new_str`\n\
                   - All edits are validated before any files are written\n\
                   - If any `old_str` match fails, no files are modified\n",
              );
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ask_agent_defaults_thinking_off() {
        let ask = create_builtin_agents()
            .into_iter()
            .find(|agent| agent.name == "ask")
            .expect("ask agent");

        assert_eq!(ask.thinking, Some(ThinkingConfig::off()));
        assert!(ask.options.is_empty());
    }
}
