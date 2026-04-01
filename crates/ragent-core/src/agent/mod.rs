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
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::permission::{Permission, PermissionAction, PermissionRule, PermissionRuleset};

pub mod custom;
pub mod oasf;

pub use custom::CustomAgentDef;

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
            AgentMode::Primary => write!(f, "primary"),
            AgentMode::Subagent => write!(f, "subagent"),
            AgentMode::All => write!(f, "all"),
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
            max_steps: Some(1),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
            options: HashMap::from([("thinking".to_string(), json!("disabled"))]),
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
                 shell commands, and searching codebases. Always prefer using tools to verify \
                 your assumptions rather than guessing."
                    .to_string(),
            ),
            permission: default_permissions(),
            max_steps: Some(500),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
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
            max_steps: Some(20),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
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
            max_steps: Some(1),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
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
            max_steps: Some(1),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
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
            max_steps: Some(1),
            skills: Vec::new(),
            memory: crate::team::config::MemoryScope::None,
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
        pattern: pattern.into(),
        action,
    }
}

/// Returns the default permission ruleset applied when a custom agent does not
/// specify its own `permissions` array.
pub fn default_permissions() -> PermissionRuleset {
    vec![
        rule(Permission::Read, "**", PermissionAction::Allow),
        rule(Permission::Edit, "**", PermissionAction::Ask),
        rule(Permission::Bash, "*", PermissionAction::Ask),
        rule(Permission::Web, "*", PermissionAction::Ask),
        rule(Permission::PlanEnter, "*", PermissionAction::Ask),
        rule(Permission::Todo, "*", PermissionAction::Allow),
    ]
}

fn read_only_permissions() -> PermissionRuleset {
    vec![
        rule(Permission::Read, "**", PermissionAction::Allow),
        rule(Permission::Edit, "**", PermissionAction::Deny),
        rule(Permission::Bash, "*", PermissionAction::Deny),
    ]
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
        .unwrap_or_else(|| AgentInfo::new(name, &format!("Custom agent: {}", name)));

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
        .args(&["branch", "--show-current"])
        .current_dir(working_dir)
        .output()
    {
        if result.status.success() {
            if let Ok(branch) = String::from_utf8(result.stdout) {
                let branch = branch.trim();
                if !branch.is_empty() {
                    output.push_str(&format!("**Branch:** {}\n", branch));
                }
            }
        }
    }

    // Get git status (short format)
    if let Ok(result) = Command::new("git")
        .args(&["status", "--short"])
        .current_dir(working_dir)
        .output()
    {
        if result.status.success() {
            if let Ok(status) = String::from_utf8(result.stdout) {
                let status = status.trim();
                if !status.is_empty() {
                    output.push_str("**Status:**\n```\n");
                    output.push_str(status);
                    output.push_str("\n```\n");
                }
            }
        }
    }

    // Get recent commits (5 most recent, one line each)
    if let Ok(result) = Command::new("git")
        .args(&["log", "--oneline", "-n", "5"])
        .current_dir(working_dir)
        .output()
    {
        if result.status.success() {
            if let Ok(commits) = String::from_utf8(result.stdout) {
                let commits = commits.trim();
                if !commits.is_empty() {
                    output.push_str("**Recent Commits:**\n```\n");
                    output.push_str(commits);
                    output.push_str("\n```\n");
                }
            }
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

pub fn build_system_prompt(
    agent: &AgentInfo,
    working_dir: &Path,
    file_tree: &str,
    skills: Option<&crate::skill::SkillRegistry>,
) -> String {
    let mut prompt = String::new();

    // Read AGENTS.md once — used both for template substitution and appended
    // as a section for built-in agents that don't embed the variable.
    let agents_md_content = {
        let agents_md_path = working_dir.join("AGENTS.md");
        if agents_md_path.is_file() {
            std::fs::read_to_string(&agents_md_path).unwrap_or_default()
        } else {
            String::new()
        }
    };

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
        // Use chrono if available (it is a workspace dep of ragent-core).
        let date_str = {
            let dt = chrono::Utc::now();
            dt.format("%Y-%m-%d").to_string()
        };

        // Prepare git status and README for injection
        let git_status = read_git_status(working_dir);
        let readme = read_readme(working_dir);

        let expanded = agent_prompt
            .replace("{{WORKING_DIR}}", &working_dir.display().to_string())
            .replace("{{FILE_TREE}}", file_tree)
            .replace("{{AGENTS_MD}}", &agents_md_content)
            .replace("{{GIT_STATUS}}", &git_status)
            .replace("{{README}}", &readme)
            .replace("{{DATE}}", &date_str);
        let _ = today; // suppress unused warning from the fallback path

        prompt.push_str(&expanded);
        prompt.push_str("\n\n");
    }

    // Single-step agents (e.g. "ask") are tool-free; skip project context
    // so the model focuses on answering the user's question directly.
    let has_tools = agent.max_steps.map_or(true, |s| s > 1);
    if !has_tools {
        return prompt;
    }

    // Working directory context (skip if already embedded via template variable)
    if agent.prompt.as_deref().map_or(true, |p| !p.contains("{{WORKING_DIR}}")) {
        prompt.push_str(&format!(
            "## Working Directory\n\
             You are operating in: {}\n\n",
            working_dir.display()
        ));
    }

    // File tree context (skip if already embedded via template variable)
    if agent.prompt.as_deref().map_or(true, |p| !p.contains("{{FILE_TREE}}")) && !file_tree.is_empty() {
        prompt.push_str("## Project Structure\n");
        prompt.push_str("```\n");
        prompt.push_str(file_tree);
        prompt.push_str("\n```\n\n");
    }

    // AGENTS.md project guidelines (skip if already embedded via template variable)
    if agent.prompt.as_deref().map_or(true, |p| !p.contains("{{AGENTS_MD}}")) && !agents_md_content.is_empty() {
        prompt.push_str("## Project Guidelines (AGENTS.md)\n");
        prompt.push_str(&agents_md_content);
        prompt.push_str("\n\n");
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
            let model_tier = sa
                .model
                .as_ref()
                .map(|m| {
                    if m.model_id.contains("haiku") {
                        "fast / low-cost"
                    } else if m.model_id.contains("opus") {
                        "powerful / higher-cost"
                    } else {
                        "standard"
                    }
                })
                .unwrap_or("standard");

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
                let can_write = ca
                    .permission
                    .iter()
                    .any(|r| r.permission == Permission::Edit && r.action == PermissionAction::Allow);
                let can_bash = ca
                    .permission
                    .iter()
                    .any(|r| r.permission == Permission::Bash && r.action == PermissionAction::Allow);
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
             {\"agent\": \"explore\", \"task\": \"Summarise architecture in crates/ragent-core/src/agent/ and src/session/\", \"background\": true}\n\
             {\"agent\": \"explore\", \"task\": \"Summarise architecture in crates/ragent-core/src/tool/ listing every tool\", \"background\": true}\n\
             {\"agent\": \"explore\", \"task\": \"Summarise architecture in crates/ragent-core/src/provider/ and src/llm/\", \"background\": true}\n\
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
         - Example call (read lines 50-100 of src/main.rs):\n\
           {\"path\": \"src/main.rs\", \"start_line\": 50, \"end_line\": 100}\n\
         - Why this is required:\n\
           * Reduces token usage and response latency significantly\n\
           * Allows you to understand code structure piece-by-piece\n\
           * Prevents context overflow from large file dumps\n\
         - Strategy:\n\
           1. First, check the file size with a grep or head command if needed\n\
           2. Start by reading the first 50 lines to understand purpose\n\
           3. Then read specific sections based on what you discover\n\
           4. Never read an entire file >100 lines in a single call\n",
    );

    prompt
}
