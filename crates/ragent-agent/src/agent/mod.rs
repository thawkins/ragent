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
/// `compaction`, `rust-coder`, `python-coder`, `typescript-coder`, `fastapi-agent`,
/// `security-auditor`, `test-writer`, `documenter`, `devops-agent`, `database-agent`,
/// and `frontend-agent` agents.
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
/// assert!(names.contains(&"rust-coder"));
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
            model: None,
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
            model: None,
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
            model: None,
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
            model: None,
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
            model: None,
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
            model: None,
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
            model: None,
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
                      model: None,
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
                  // ── Domain-specific agents ───────────────────────────────────────
                  AgentInfo {
                      name: "rust-coder".to_string(),
                      description: "Rust coding specialist — idiomatic code, error handling, async".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: None,
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a Rust coding specialist. You write idiomatic, production-grade Rust \
                           code with an emphasis on zero-cost abstractions, memory safety, and \
                           composability.\n\n\
                           Expertise:\n\
                           - Ownership, borrowing, and lifetimes\n\
                           - Error handling with Result<T, E> and anyhow/thiserror\n\
                           - Async Rust with tokio and futures\n\
                           - Traits and trait objects (dyn Trait vs impl Trait)\n\
                           - Unsafe code when necessary (with safety comments)\n\
                           - Cargo workspace management and dependency hygiene\n\
                           - Testing with cargo test, mockall, and insta\n\
                           - Performance: zero-copy, SIMD, rayon parallelism\n\n\
                           When reviewing or writing Rust:\n\
                           - Prefer `?` over `.unwrap()` / `.expect()` in library code\n\
                           - Use `tracing` (not println!) for structured logging\n\
                           - Follow the Rust API Guidelines and naming conventions\n\
                           - Minimize allocations; prefer iterators over loops\n\
                           - Document public APIs with `///` doc comments"
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
                      name: "python-coder".to_string(),
                      description: "Python coding specialist — idiomatic code, type hints, testing".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: None,
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a Python coding specialist. You write clean, idiomatic Python \
                           following modern best practices and PEP 8.\n\n\
                           Expertise:\n\
                           - Type hints (PEP 484), generics, and mypy/pyright compliance\n\
                           - Async Python with asyncio, aiohttp, and FastAPI\n\
                           - Data modelling with dataclasses, Pydantic, and attrs\n\
                           - Testing with pytest, unittest, and coverage\n\
                           - Packaging with pyproject.toml, poetry, and uv\n\
                           - Virtual environments and dependency management\n\
                           - Performance: profiling, caching (functools.lru_cache), vectorisation\n\
                           - Python 3.11+ features (task groups, exception groups, tomllib)\n\n\
                           When reviewing or writing Python:\n\
                           - Use type hints everywhere; avoid bare `Any`\n\
                           - Prefer f-strings and pathlib over os.path\n\
                           - Use context managers (`with`) for resource cleanup\n\
                           - Prefer composition over inheritance\n\
                           - Use `isinstance()` checks, not `type()` comparisons\n\
                           - Keep functions small and testable (single responsibility)"
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
                      name: "typescript-coder".to_string(),
                      description: "TypeScript/JavaScript coding specialist — type safety, modern JS".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: None,
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a TypeScript and JavaScript coding specialist. You write type-safe, \
                           modern JavaScript for both frontend and backend contexts.\n\n\
                           Expertise:\n\
                           - Strict TypeScript with explicit types; minimal use of `any`\n\
                           - Union types, discriminated unions, and type narrowing\n\
                           - Generic constraints and mapped types\n\
                           - Async/await, Promises, and error handling patterns\n\
                           - React hooks, Next.js, and component architecture\n\
                           - Node.js, Express, and Fastify server patterns\n\
                           - Testing with Vitest, Jest, and Playwright\n\
                           - Build tools: Vite, Rollup, Webpack, esbuild, tsup\n\
                           - Package managers: npm, pnpm, yarn (Berry)\n\n\
                           When reviewing or writing TS/JS:\n\
                           - Use `const` and `let`; avoid `var`\n\
                           - Prefer arrow functions for callbacks; named functions for hoisting\n\
                           - Use optional chaining (`?.`) and nullish coalescing (`??`)\n\
                           - Keep components small and focused; extract hooks early\n\
                           - Use ESLint + Prettier for consistency"
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
                      name: "fastapi-agent".to_string(),
                      description: "FastAPI project specialist — API design, Pydantic, async".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: None,
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a FastAPI and Python web-backend specialist. You design and build \
                           high-performance REST and WebSocket APIs.\n\n\
                           Expertise:\n\
                           - FastAPI routing, dependency injection, and lifespan events\n\
                           - Pydantic v2 models, validators, and serialization\n\
                           - SQLAlchemy 2.0 ORM, Alembic migrations, and async engines\n\
                           - Authentication: OAuth2, JWT, API keys, and session management\n\
                           - Background tasks, Celery, and message queues\n\
                           - Docker multi-stage builds and docker-compose orchestration\n\
                           - Testing: pytest-asyncio, httpx.AsyncClient, TestClient\n\
                           - Deployment: Gunicorn + Uvicorn, ASGI servers, reverse proxies\n\n\
                           When designing APIs:\n\
                           - Use HTTP status codes correctly (201 Created, 204 No Content)\n\
                           - Version URLs (`/api/v1/...`) and use HATEOAS sparingly\n\
                           - Document all endpoints with OpenAPI (auto-generated by FastAPI)\n\
                           - Implement rate limiting and input validation at the edge\n\
                           - Use structured logging (JSON) for observability"
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
                      name: "security-auditor".to_string(),
                      description: "Security code reviewer — OWASP Top 10, CWE, mitigations".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: Some(0.2),
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a security-focused code reviewer specialising in the OWASP Top 10.\n\n\
                           For every review:\n\
                           1. Identify injection flaws (SQL, command, LDAP, XPath, template)\n\
                           2. Check authentication and session management weaknesses\n\
                           3. Look for sensitive data exposure (keys, tokens, PII in logs)\n\
                           4. Flag insecure direct object references and broken access control\n\
                           5. Detect security misconfiguration and outdated dependencies\n\
                           6. Highlight XXE and deserialization risks\n\
                           7. Note XSS vectors, CSP bypasses, and CSRF weaknesses\n\
                           8. Flag use of components with known vulnerabilities (CVE checks)\n\
                           9. Check for insufficient logging and monitoring gaps\n\n\
                           Provide CWE identifiers and OWASP references for every finding. \
                           Suggest concrete mitigations with code examples."
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
                      name: "test-writer".to_string(),
                      description: "Test generation specialist — unit, integration, e2e coverage".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: Some(0.3),
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a test-writing specialist. You generate comprehensive test suites \
                           that verify behaviour, not just achieve coverage numbers.\n\n\
                           Expertise:\n\
                           - Unit tests: arrange-act-assert, table-driven tests, property-based testing\n\
                           - Integration tests: database fixtures, HTTP client tests, API contracts\n\
                           - E2E tests: Playwright, Cypress, user-journey scenarios\n\
                           - Mocking and stubbing (mockall, Mockito, jest.mock, sinon)\n\
                           - Coverage analysis: branch coverage, mutation testing\n\
                           - CI-friendly tests: idempotent, parallel-safe, deterministic\n\n\
                           When writing tests:\n\
                           - Test one thing per function; use descriptive names\n\
                           - Test edge cases, error paths, and boundary conditions\n\
                           - Use fixtures and factories for test data, not hard-coded values\n\
                           - Mock at the boundary; test real collaborators where possible\n\
                           - Keep tests fast (< 100ms per test ideally)\n\
                           - Add `#[should_panic]` / `pytest.raises` for expected failures"
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
                      name: "documenter".to_string(),
                      description: "Documentation specialist — docstrings, READMEs, API docs".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: Some(0.5),
                      top_p: None,
                      model: None,
                                              prompt: Some(
                                                  "You are a technical documentation specialist. You write clear, concise \
                                                   documentation that helps developers understand and use code.\n\n\
                                                   Expertise:\n\
                                                   - API documentation: docstrings, OpenAPI specs, type signatures\n\
                                                   - README files: quick-start, installation, configuration, examples\n\
                                                   - Architecture Decision Records (ADRs) and design docs\n\
                                                   - User guides and tutorials with runnable examples\n\
                                                   - Changelog management (Keep a Changelog format)\n\
                                                   - Inline comments for complex algorithms and business logic\n\n\
                                                   When documenting:\n\
                                                   - Lead with the \"why\", then the \"what\", then the \"how\"\n\
                                                   - Include practical code examples that compile/run\n\
                                                   - Use tables for parameter references and configuration options\n\
                                                   - Keep headings hierarchical and scannable\n\
                                                   - Cross-reference related documents with relative links\n\
                                                   - Update tables of contents when adding new sections"
                                                      .to_string(),
                                              ),                      permission: default_permissions(),
                      max_steps: Some(500),
                      skills: Vec::new(),
                      memory: crate::team::config::MemoryScope::None,
                      thinking: None,
                      options: HashMap::new(),
                      model_pinned: false,
                  },
                  AgentInfo {
                      name: "devops-agent".to_string(),
                      description: "DevOps specialist — Docker, Kubernetes, CI/CD, infrastructure".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: None,
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a DevOps and infrastructure specialist. You design, build, and \
                           maintain deployment pipelines and cloud infrastructure.\n\n\
                           Expertise:\n\
                           - Containerisation: Docker, BuildKit, multi-stage builds, distroless images\n\
                           - Orchestration: Kubernetes manifests, Helm charts, Kustomize\n\
                           - CI/CD: GitHub Actions, GitLab CI, Azure DevOps, ArgoCD\n\
                           - Infrastructure as Code: Terraform, Pulumi, AWS CDK, CloudFormation\n\
                           - Monitoring: Prometheus, Grafana, OpenTelemetry, structured logging\n\
                           - Secrets management: Vault, Sealed Secrets, AWS Secrets Manager\n\
                           - Networking: Ingress, service mesh (Istio, Linkerd), TLS termination\n\
                           - Cloud platforms: AWS, GCP, Azure (serverless, VMs, managed services)\n\n\
                           When working on infrastructure:\n\
                           - Use declarative configuration (YAML, HCL) over imperative scripts\n\
                           - Implement health checks, readiness probes, and graceful shutdowns\n\
                           - Follow the principle of least privilege for IAM and RBAC\n\
                           - Version-pin all base images and dependencies\n\
                           - Document runbooks and rollback procedures"
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
                      name: "database-agent".to_string(),
                      description: "Database specialist — SQL, migrations, performance, schema design".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: None,
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a database specialist. You design schemas, write queries, and \
                           optimise data access patterns for relational and NoSQL databases.\n\n\
                           Expertise:\n\
                           - Relational: PostgreSQL, MySQL, SQLite — schema design, indexing, query plans\n\
                           - NoSQL: MongoDB, Redis, DynamoDB — document modelling, key patterns\n\
                           - Migrations: Alembic, Flyway, dbmate — forward-only, rollback-safe\n\
                           - ORMs: SQLAlchemy, Diesel, Prisma, TypeORM — type-safe query builders\n\
                           - Performance: EXPLAIN ANALYZE, query rewriting, materialised views\n\
                           - Transactions: ACID guarantees, isolation levels, deadlock avoidance\n\
                           - Data integrity: constraints, triggers, foreign keys, normalisation\n\
                           - Backup and replication: pg_dump, logical replication, read replicas\n\n\
                           When working with databases:\n\
                           - Normalise to 3NF initially; denormalise selectively for read performance\n\
                           - Add indexes after profiling; avoid over-indexing on write-heavy tables\n\
                           - Use connection pooling (PgBouncer, r2d2, sqlx::Pool)\n\
                           - Parameterise queries; never concatenate user input into SQL\n\
                           - Add database-level constraints as a safety net, not just application validation"
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
                      name: "frontend-agent".to_string(),
                      description: "Frontend specialist — React, Vue, CSS, accessibility, performance".to_string(),
                      mode: AgentMode::Primary,
                      hidden: false,
                      temperature: None,
                      top_p: None,
                      model: None,
                      prompt: Some(
                          "You are a frontend web development specialist. You build responsive, \
                           accessible, and performant user interfaces.\n\n\
                           Expertise:\n\
                           - React: hooks, context, suspense, server components, Next.js App Router\n\
                           - Vue: Composition API, Pinia, Nuxt.js, VueUse\n\
                           - Styling: Tailwind CSS, CSS-in-JS (styled-components, emotion), PostCSS\n\
                           - State management: Zustand, Redux Toolkit, Pinia, signals (Solid, Preact)\n\
                           - Accessibility: ARIA roles, keyboard navigation, focus management, axe\n\
                           - Performance: Core Web Vitals, code splitting, image optimisation, caching\n\
                           - Testing: React Testing Library, Vitest, Playwright, Storybook\n\
                           - Build tools: Vite, Webpack, esbuild, SWC, Turbopack\n\n\
                           When building frontend:\n\
                           - Mobile-first responsive design with Tailwind breakpoints\n\
                           - Ensure WCAG 2.1 AA compliance (contrast ratios, focus indicators)\n\
                           - Use semantic HTML (`<header>`, `<nav>`, `<main>`, `<article>`)\n\
                           - Lazy-load images and heavy components below the fold\n\
                           - Keep bundle sizes small; tree-shake unused dependencies\n\
                           - Use `key` props correctly in lists; avoid index-as-key"
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

fn config_permission_rule_to_runtime(
    rule: &ragent_config::permission::PermissionRule,
) -> PermissionRule {
    PermissionRule {
        permission: Permission::from(rule.permission.to_string().as_str()),
        pattern: rule.pattern.clone(),
        action: match rule.action {
            ragent_config::permission::PermissionAction::Allow => PermissionAction::Allow,
            ragent_config::permission::PermissionAction::Deny => PermissionAction::Deny,
            ragent_config::permission::PermissionAction::Ask => PermissionAction::Ask,
        },
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
    config: &crate::Config,
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
    config: &crate::Config,
    provider_registry: &crate::provider::ProviderRegistry,
) {
    if agent.thinking.is_none()
        && let Some(model_ref) = agent.model.as_ref()
    {
        agent.thinking = fallback_thinking_for_model_ref(config, provider_registry, model_ref);
    }
}

/// Resolve a default model for the given agent when no model is configured.
///
/// Scans the `provider_registry` for the first provider that has at least one
/// default model and returns the first model from that provider.  The
/// provider list order matches the registry registration order.
///
/// Returns `None` if the registry is empty or no provider advertises default
/// models.
#[must_use]
pub fn resolve_default_model(
    agent: &AgentInfo,
    provider_registry: &crate::provider::ProviderRegistry,
) -> Option<ModelRef> {
    // If the agent already has a model, keep it.
    if agent.model.is_some() {
        return agent.model.clone();
    }

    for provider_info in provider_registry.list() {
        if let Some(first_model) = provider_info.models.first() {
            return Some(ModelRef {
                provider_id: provider_info.id.clone(),
                model_id: first_model.id.clone(),
            });
        }
    }

    None
}

/// Like [`resolve_agent`] but also ensures the returned agent has a model.
///
/// When the built-in definition has no `model` set and no config override
/// supplies one, this function queries the `provider_registry` for the first
/// available provider/model pair and assigns it.
pub fn resolve_agent_with_model(
    name: &str,
    config: &crate::Config,
    provider_registry: &crate::provider::ProviderRegistry,
) -> anyhow::Result<AgentInfo> {
    let mut agent = resolve_agent(name, config)?;
    if agent.model.is_none() {
        if let Some(model_ref) = resolve_default_model(&agent, provider_registry) {
            tracing::info!(
                agent = %agent.name,
                provider = %model_ref.provider_id,
                model = %model_ref.model_id,
                "Auto-assigned default model to agent"
            );
            agent.model = Some(model_ref);
        }
    }
    Ok(agent)
}

/// Like [`resolve_agent_with_customs`] but also ensures the returned agent
/// has a model by falling back to the first available provider/model pair.
pub fn resolve_agent_with_customs_and_model(
    name: &str,
    config: &crate::Config,
    working_dir: &std::path::Path,
    provider_registry: &crate::provider::ProviderRegistry,
) -> anyhow::Result<AgentInfo> {
    let mut agent = resolve_agent_with_customs(name, config, working_dir)?;
    if agent.model.is_none() {
        if let Some(model_ref) = resolve_default_model(&agent, provider_registry) {
            tracing::info!(
                agent = %agent.name,
                provider = %model_ref.provider_id,
                model = %model_ref.model_id,
                "Auto-assigned default model to agent"
            );
            agent.model = Some(model_ref);
        }
    }
    Ok(agent)
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
pub fn resolve_agent(name: &str, config: &crate::Config) -> anyhow::Result<AgentInfo> {
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
            agent.permission = agent_config
                .permission
                .iter()
                .map(config_permission_rule_to_runtime)
                .collect();
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
    config: &crate::Config,
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

/// Information about discovered instruction files.
#[derive(Debug, Clone)]
pub struct InstructionFileDiscovery {
    /// The file names being searched for
    pub searched_names: Vec<String>,
    /// The working directory searched
    pub working_dir: std::path::PathBuf,
    /// The global directory searched (if applicable)
    pub global_dir: Option<std::path::PathBuf>,
    /// Whether global fallback was used (no local files found)
    pub used_global_fallback: bool,
    /// All instruction files discovered (for display)
    pub all_discovered_files: Vec<std::path::PathBuf>,
    /// The single file actually loaded for instructions
    pub loaded_file: Option<std::path::PathBuf>,
}

impl InstructionFileDiscovery {
    /// Format a human-readable summary of the discovery, showing which files
    /// were actually loaded for instructions versus merely found.
    pub fn format_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push("📋 Instruction File Discovery".to_string());
        lines.push(format!(
            "  Searched for: {}",
            self.searched_names.join(", ")
        ));
        lines.push(format!(
            "  Working directory: {}",
            self.working_dir.display()
        ));

        if self.all_discovered_files.is_empty() {
            lines.push(String::new());
            lines.push("⚠️  No instruction files found".to_string());
            lines.push("  No instructions were loaded for this session.".to_string());
        } else {
            if let Some(ref gdir) = self.global_dir {
                lines.push(format!("  Global directory: {}", gdir.display()));
            }
            lines.push(format!(
                "📁 Discovered {} file(s):",
                self.all_discovered_files.len()
            ));
            for file in &self.all_discovered_files {
                let rel = file
                    .strip_prefix(&self.working_dir)
                    .unwrap_or(file)
                    .display()
                    .to_string();
                let is_global = self
                    .global_dir
                    .as_ref()
                    .map_or(false, |gd| file.starts_with(gd));
                let source = if is_global { "global" } else { "local" };
                let is_loaded = self.loaded_file.as_ref() == Some(file);
                let marker = if is_loaded { " ✅ LOADED" } else { "" };
                lines.push(format!("   • {} ({}){}", rel, source, marker));
            }
            lines.push(String::new());
            if let Some(ref loaded) = self.loaded_file {
                let rel = loaded
                    .strip_prefix(&self.working_dir)
                    .unwrap_or(loaded)
                    .display()
                    .to_string();
                let priority_note = if self.used_global_fallback {
                    "global fallback — no local root instruction file found"
                } else {
                    "project root takes priority"
                };
                lines.push(format!(
                    "✅ Instructions loaded from: {} ({})",
                    rel, priority_note
                ));
            }
            if self.all_discovered_files.len() > 1 {
                lines.push(
                                        "ℹ️  Additional instruction files were discovered but ignored: only one file is loaded per session.".to_string(),
                                    );
                lines.push("\n".to_string());
            }
        }
        lines.join("\n")
    }
}

/// Discover all AGENTS.md-style instruction files from the project tree
/// or the global directory, but load instructions from ONLY ONE file.
///
/// Searches recursively for `AGENTS.md`, `CLAUDE.md`, `.ragent.md`, and
/// `INSTRUCTIONS.md` in the working directory. ALL discovered files are
/// reported in the discovery info, but only ONE file's content is loaded:
///
/// Priority order:
/// 1. `AGENTS.md` in the project root (working directory itself)
/// 2. Any other instruction file in the project root
/// 3. The global directory at `~/.local/share/ragent/` (prioritised over
///    subdirectory files when no root file exists)
/// 4. The shallowest instruction file found in project subdirectories
///
/// Returns a string containing the loaded file's content, along with
/// discovery information showing all found files and which one was loaded.
pub fn collect_agents_md_content_with_discovery(
    working_dir: &Path,
) -> (String, InstructionFileDiscovery) {
    const AGENT_FILE_NAMES: &[&str] = &["AGENTS.md", "CLAUDE.md", ".ragent.md", "INSTRUCTIONS.md"];

    use ignore::WalkBuilder;

    // Build discovery info
    let global_dir = dirs::data_dir()
        .map(|d| d.join("ragent"))
        .filter(|d| d.is_dir());

    // First, collect ALL local project files (for discovery display)
    let mut local_files: Vec<(usize, std::path::PathBuf)> = Vec::new();

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
            // Depth is 0 for files directly in working_dir, 1 for
            // immediate subdirectories, etc.
            let depth = path
                .strip_prefix(working_dir)
                .map(|rel| rel.components().count().saturating_sub(1))
                .unwrap_or(usize::MAX);
            local_files.push((depth, path));
        }
    }

    // Collect global files for discovery (even if local files exist)
    let mut global_files: Vec<(usize, std::path::PathBuf)> = Vec::new();
    if let Some(ref global_path) = global_dir {
        for name in AGENT_FILE_NAMES {
            let file_path = global_path.join(name);
            if file_path.is_file() {
                global_files.push((0, file_path));
            }
        }
    }

    // Split local files into root (depth 0) and subdirectory (depth > 0)
    let root_files: Vec<_> = local_files
        .iter()
        .filter(|(depth, _)| *depth == 0)
        .cloned()
        .collect();
    let sub_files: Vec<_> = local_files
        .iter()
        .filter(|(depth, _)| *depth > 0)
        .cloned()
        .collect();

    // Sort each priority tier independently by name priority (AGENTS.md
    // first, etc.) so we never mix root/global/sub ordering.
    let sort_by_name = |v: &mut Vec<(usize, std::path::PathBuf)>| {
        v.sort_by(|a, b| {
            let a_name = a.1.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let b_name = b.1.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let a_idx = AGENT_FILE_NAMES
                .iter()
                .position(|n| *n == a_name)
                .unwrap_or(usize::MAX);
            let b_idx = AGENT_FILE_NAMES
                .iter()
                .position(|n| *n == b_name)
                .unwrap_or(usize::MAX);
            a_idx.cmp(&b_idx)
        });
    };
    let mut root_files = root_files;
    let mut sub_files = sub_files;
    sort_by_name(&mut root_files);
    sort_by_name(&mut global_files);
    sort_by_name(&mut sub_files);

    // Priority: project root → global → subdirectories
    let mut candidates: Vec<(usize, std::path::PathBuf)> = Vec::new();
    candidates.extend(root_files);
    candidates.extend(global_files);
    candidates.extend(sub_files);
    let loaded_file = candidates.first().map(|(_, p)| p.clone());
    let used_global = loaded_file.as_ref().map_or(false, |f| {
        global_dir.as_ref().map_or(false, |gd| f.starts_with(gd))
    });

    // Build the full discovery list (all found files for display)
    let mut all_discovered: Vec<std::path::PathBuf> = Vec::new();
    for (_, path) in &candidates {
        all_discovered.push(path.clone());
    }
    let discovery = InstructionFileDiscovery {
        searched_names: AGENT_FILE_NAMES.iter().map(|s| s.to_string()).collect(),
        working_dir: working_dir.to_path_buf(),
        global_dir: global_dir.clone(),
        all_discovered_files: all_discovered,
        loaded_file,
        used_global_fallback: used_global,
    };

    // If no files found at all, return empty
    if discovery.loaded_file.is_none() {
        return (String::new(), discovery);
    }

    // Build result content from the SINGLE loaded file only
    let mut result = String::new();

    if let Some(ref path) = discovery.loaded_file {
        let rel = path
            .strip_prefix(working_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        result.push_str("### Discovered Instruction Files\n");
        // Show all discovered files, marking the loaded one
        for file in &discovery.all_discovered_files {
            let file_rel = file
                .strip_prefix(working_dir)
                .unwrap_or(file)
                .display()
                .to_string();
            let marker = if Some(file) == discovery.loaded_file.as_ref() {
                " ✅ LOADED"
            } else {
                ""
            };
            result.push_str(&format!("- {file_rel}{marker}\n"));
        }
        result.push('\n');

        // Only load content from the single selected file
        if let Ok(content) = std::fs::read_to_string(path) {
            let content = content.trim();
            if !content.is_empty() {
                result.push_str(&format!("### From: {rel}\n\n"));
                result.push_str(content);
                result.push_str("\n\n");
            }
        }
    }

    (result, discovery)
}

/// Discover and load all AGENTS.md-style instruction files from the project tree
/// or the global directory.
///
/// Searches recursively for `AGENTS.md`, `CLAUDE.md`, `.ragent.md`, and
/// `INSTRUCTIONS.md` in the working directory. Priority order:
/// project root → global directory → project subdirectories.
///
/// Returns a combined string listing the discovered file paths and their
/// concatenated content.
fn collect_agents_md_content(working_dir: &Path) -> String {
    collect_agents_md_content_with_discovery(working_dir).0
}
/// Build a system prompt for the given agent using cached context.#[must_use]
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
    memory_config: Option<&crate::MemoryConfig>,
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
        let max_background_agents = crate::Config::load()
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
                                     - **CRITICAL — end_line is an absolute line number, NOT a length or count**:\n\
                                       - ✅ CORRECT: `start_line=200, end_line=300` reads lines 200 through 300 (101 lines total)\n\
                                       - ❌ WRONG: `start_line=200, end_line=100` — this fails because 100 < 200\n\
                                       - ✅ CORRECT: `start_line=1, end_line=100` reads lines 1–100\n\
                                       - ❌ WRONG: `start_line=1, end_line=100` when the file has only 50 lines — end_line must not exceed total_lines\n\
                                     - `start_line` and `end_line` are **absolute 1-based line numbers** (not offsets from start_line).\n\
                                     - **ALTERNATIVE — use `num_lines` instead of `end_line`**: When `start_line` is provided,\n\
                                       you may pass `num_lines` to specify the COUNT of lines to read from that start.\n\
                                       Example: `start_line=201, num_lines=100` reads lines 201–300 (inclusive).\n\
                                         This is useful if you naturally think in start + count rather than start + end.
\
                                       If both `end_line` and `num_lines` are provided, `end_line` takes precedence.\n\
                                     - Strategy:\n\
                                       1. Read the file without start_line/end_line first — for large files this returns\n\
                                          the first 100 lines plus a section map with the total line count\n\
                                       2. Use the total_lines from the response to plan your subsequent reads\n\
                                       3. Then read specific sections using valid line ranges\n\
                                       4. Never read an entire file >100 lines in a single call\n",
                                ); // Guidance on using edit / multiedit tools
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

    #[test]
    fn test_domain_agents_exist() {
        let agents = create_builtin_agents();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();

        let expected = [
            "ask",
            "general",
            "build",
            "plan",
            "explore",
            "title",
            "summary",
            "compaction",
            "rust-coder",
            "python-coder",
            "typescript-coder",
            "fastapi-agent",
            "security-auditor",
            "test-writer",
            "documenter",
            "devops-agent",
            "database-agent",
            "frontend-agent",
        ];

        for name in &expected {
            assert!(
                names.contains(name),
                "built-in agent '{}' should exist",
                name
            );
        }

        assert_eq!(
            agents.len(),
            expected.len(),
            "built-in agent count mismatch"
        );
    }

    #[test]
    fn test_domain_agents_are_primary() {
        let agents = create_builtin_agents();
        let primary_names = [
            "ask",
            "general",
            "rust-coder",
            "python-coder",
            "typescript-coder",
            "fastapi-agent",
            "security-auditor",
            "test-writer",
            "documenter",
            "devops-agent",
            "database-agent",
            "frontend-agent",
        ];

        for name in &primary_names {
            let agent = agents.iter().find(|a| a.name == *name).expect(name);
            assert_eq!(
                agent.mode,
                AgentMode::Primary,
                "agent '{}' should be Primary",
                name
            );
        }
    }

    #[test]
    fn test_security_auditor_is_read_only() {
        let agents = create_builtin_agents();
        let auditor = agents
            .iter()
            .find(|a| a.name == "security-auditor")
            .expect("security-auditor agent");

        // Should have read-only permissions (no edit, no bash)
        assert!(
            auditor.permission.iter().any(|r| {
                matches!(r.action, PermissionAction::Deny)
                    && matches!(r.permission, Permission::Edit | Permission::Bash)
            }),
            "security-auditor should deny edit and bash permissions"
        );
    }
}
