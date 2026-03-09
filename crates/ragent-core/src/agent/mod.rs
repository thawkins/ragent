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

use crate::permission::{PermissionAction, PermissionRule, PermissionRuleset};

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
    /// Arbitrary key-value options forwarded to the provider.
    // TODO: Replace `Value` with typed agent option structs.
    pub options: HashMap<String, Value>,
}

impl AgentInfo {
    /// Creates a new agent with the given name and description, using default values.
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
            max_steps: Some(50),
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
            options: HashMap::new(),
        },
    ]
}

fn default_permissions() -> PermissionRuleset {
    vec![
        PermissionRule {
            permission: "file:read".into(),
            pattern: "**".to_string(),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: "file:write".into(),
            pattern: "**".to_string(),
            action: PermissionAction::Ask,
        },
        PermissionRule {
            permission: "bash:execute".into(),
            pattern: "*".to_string(),
            action: PermissionAction::Ask,
        },
    ]
}

fn read_only_permissions() -> PermissionRuleset {
    vec![
        PermissionRule {
            permission: "file:read".into(),
            pattern: "**".to_string(),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: "file:write".into(),
            pattern: "**".to_string(),
            action: PermissionAction::Deny,
        },
        PermissionRule {
            permission: "bash:execute".into(),
            pattern: "*".to_string(),
            action: PermissionAction::Deny,
        },
    ]
}

/// Resolve an agent by name, merging built-in definition with config overrides.
///
/// # Errors
///
/// Returns an error if config overlay parsing fails (e.g. invalid model string format).
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
        for (k, v) in &agent_config.options {
            agent.options.insert(k.clone(), v.clone());
        }
    }

    Ok(agent)
}

/// Build the system prompt for an agent invocation.
pub fn build_system_prompt(agent: &AgentInfo, working_dir: &Path, file_tree: &str) -> String {
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

    // Tool usage guidelines
    prompt.push_str(
        "## Guidelines\n\
         - Use tools to verify information rather than guessing\n\
         - Read files before editing them to understand context\n\
         - Make precise, targeted changes\n\
         - Test changes when possible using the bash tool\n\
         - Explain what you're doing and why\n",
    );

    prompt
}
