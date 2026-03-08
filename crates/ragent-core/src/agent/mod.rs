use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use crate::permission::{PermissionAction, PermissionRule, PermissionRuleset};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Primary,
    Subagent,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRef {
    pub provider_id: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
    pub mode: AgentMode,
    pub hidden: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub model: Option<ModelRef>,
    pub prompt: Option<String>,
    pub permission: PermissionRuleset,
    pub max_steps: Option<u32>,
    pub options: HashMap<String, Value>,
}

impl AgentInfo {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
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

pub fn create_builtin_agents() -> Vec<AgentInfo> {
    vec![
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
            permission: "file:read".to_string(),
            pattern: "**".to_string(),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: "file:write".to_string(),
            pattern: "**".to_string(),
            action: PermissionAction::Ask,
        },
        PermissionRule {
            permission: "bash:execute".to_string(),
            pattern: "*".to_string(),
            action: PermissionAction::Ask,
        },
    ]
}

fn read_only_permissions() -> PermissionRuleset {
    vec![
        PermissionRule {
            permission: "file:read".to_string(),
            pattern: "**".to_string(),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: "file:write".to_string(),
            pattern: "**".to_string(),
            action: PermissionAction::Deny,
        },
        PermissionRule {
            permission: "bash:execute".to_string(),
            pattern: "*".to_string(),
            action: PermissionAction::Deny,
        },
    ]
}

/// Resolve an agent by name, merging built-in definition with config overrides.
pub fn resolve_agent(
    name: &str,
    config: &crate::config::Config,
) -> anyhow::Result<AgentInfo> {
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
pub fn build_system_prompt(
    agent: &AgentInfo,
    working_dir: &Path,
    file_tree: &str,
) -> String {
    let mut prompt = String::new();

    // Agent identity and role
    if let Some(ref agent_prompt) = agent.prompt {
        prompt.push_str(agent_prompt);
        prompt.push_str("\n\n");
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
