//! Agent definitions, built-in agent registry, and prompt construction.
//!
//! This module defines the [`AgentInfo`] type that describes an agent's
//! identity, model binding, permissions, and system prompt. It also provides
//! [`create_builtin_agents`] for the default agent roster and
//! [`resolve_agent`] for merging built-in definitions with user config.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::permission::{PermissionAction, PermissionRule, PermissionRuleset, Permission};

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
    /// Arbitrary key-value options forwarded to the provider.
    // TODO: Replace `Value` with typed agent option structs.
    pub options: HashMap<String, Value>,
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
            options: HashMap::new(),
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
            options: HashMap::from([("thinking".to_string(), json!("disabled"))]),
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
            options: HashMap::new(),
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
            max_steps: Some(30),
            skills: Vec::new(),
            options: HashMap::new(),
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
            options: HashMap::new(),
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
            max_steps: Some(15),
            skills: Vec::new(),
            options: HashMap::new(),
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
            options: HashMap::new(),
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
            options: HashMap::new(),
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
            options: HashMap::new(),
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

fn default_permissions() -> PermissionRuleset {
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
pub fn build_system_prompt(
    agent: &AgentInfo,
    working_dir: &Path,
    file_tree: &str,
    skills: Option<&crate::skill::SkillRegistry>,
) -> String {
    let mut prompt = String::new();

    // Agent identity and role
    if let Some(ref agent_prompt) = agent.prompt {
        prompt.push_str(agent_prompt);
        prompt.push_str("\n\n");
    }

    // Single-step agents (e.g. "ask") are tool-free; skip project context
    // so the model focuses on answering the user's question directly.
    let has_tools = agent.max_steps.map_or(true, |s| s > 1);
    if !has_tools {
        return prompt;
    }

    // Working directory context
    prompt.push_str(&format!(
        "## Working Directory\n\
         You are operating in: {}\n\n",
        working_dir.display()
    ));

    // File tree context
    if !file_tree.is_empty() {
        prompt.push_str("## Project Structure\n");
        prompt.push_str("```\n");
        prompt.push_str(file_tree);
        prompt.push_str("\n```\n\n");
    }

    // Load AGENTS.md project guidelines if present
    let agents_md = working_dir.join("AGENTS.md");
    if agents_md.is_file() {
        if let Ok(contents) = std::fs::read_to_string(&agents_md) {
            prompt.push_str("## Project Guidelines (AGENTS.md)\n");
            prompt.push_str(&contents);
            prompt.push_str("\n\n");
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
                let desc = skill
                    .description
                    .as_deref()
                    .unwrap_or("(no description)");
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
    // Agent list is generated dynamically from builtins so it stays in sync.
    if agent.mode == AgentMode::Primary {
        let builtins = create_builtin_agents();
        let spawnable: Vec<&AgentInfo> = builtins
            .iter()
            .filter(|a| a.mode == AgentMode::Subagent && !a.hidden)
            .collect();

        let mut section = String::from(
            "## Sub-Agent Spawning\n\n\
             Use the `new_task` tool to delegate work to a specialised sub-agent whenever a task is \
             clearly separable, time-consuming, or benefits from isolation.\n\n\
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

            let can_write = sa.permission.iter().any(|r| {
                r.permission == Permission::Edit && r.action == PermissionAction::Allow
            });
            let can_bash = sa.permission.iter().any(|r| {
                r.permission == Permission::Bash && r.action == PermissionAction::Allow
            });

            let mut traits = Vec::new();
            if !can_write { traits.push("read-only"); }
            if can_bash   { traits.push("can run shell commands"); }
            traits.push(model_tier);

            section.push_str(&format!(
                "- `{}` — {} [{}]\n",
                sa.name,
                sa.description,
                traits.join(", "),
            ));
        }

        section.push_str(
            "\n**Choosing an agent:**\n\
             - `explore` — fastest and cheapest; use for any codebase search, reading, or understanding.\n\
               Cannot edit files. Ideal for parallel discovery tasks.\n\
             - `build`   — use when you need to compile, run tests, apply fixes, or execute shell commands.\n\
             - `plan`    — use to produce a structured implementation plan without making any changes.\n\
             - `general` — full-capability fallback; use when the task doesn't fit a specialist agent.\n\n\
             **Modes:**\n\
             - `background: false` (default) — blocks until complete; result returned inline.\n\
             - `background: true` — returns immediately with a `task_id`; agent runs concurrently.\n\
               A SubagentComplete notification will arrive when it finishes.\n\n\
             **When to spawn:**\n\
             - Parallelize independent explorations (`explore`, background: true)\n\
             - Offload a slow build/test cycle while you continue reasoning\n\
             - Isolate risky or speculative work in a focused agent\n\n\
             **Example calls:**\n\
             ```json\n\
             {\"agent\": \"explore\", \"task\": \"Find all usages of EventBus in src/\", \"background\": false}\n\
             {\"agent\": \"build\",   \"task\": \"Run cargo test and fix failing tests\",  \"background\": true}\n\
             ```\n\n\
             Use `list_tasks` to check background task status. Use `cancel_task` to stop a task early.\n\n",
        );

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
