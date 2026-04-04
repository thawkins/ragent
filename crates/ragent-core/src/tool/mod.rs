//! Tool system for agent capabilities.
//!
//! This module defines the [`Tool`] trait for implementing agent-callable tools,
//! the [`ToolRegistry`] for managing available tools by name, and supporting types
//! [`ToolContext`] and [`ToolOutput`] used during tool execution.
//!
//! Built-in tools (file I/O, shell execution, search, and user interaction) are
//! provided via [`create_default_registry`].

/// MCP server tool wrapper.
pub mod mcp_tool;
pub use mcp_tool::McpToolWrapper;

/// Shell command execution tool.
pub mod bash;
/// Persistent shell state reset tool.
pub mod bash_reset;
/// Task cancellation tool.
pub mod cancel_task;
/// File creation tool.
pub mod create;
/// File editing tool.
pub mod edit;
/// Per-file locking for concurrent edit operations.
mod file_lock;
/// Concurrent file operations tool (batch read/write).
pub mod file_ops_tool;
pub mod github_issues;
pub mod github_prs;
/// File globbing tool.
pub mod glob;
pub mod grep;
pub mod libreoffice_common;
pub mod libreoffice_info;
pub mod libreoffice_read;
pub mod libreoffice_write;
pub mod list;
pub mod list_tasks;
pub mod lsp_definition;
pub mod lsp_diagnostics;
pub mod lsp_hover;
pub mod lsp_references;
pub mod lsp_symbols;
pub mod memory_write;
pub mod multiedit;
pub mod new_task;
pub mod office_common;
pub mod office_info;
pub mod office_read;
pub mod office_write;
pub mod patch;
pub mod pdf_read;
pub mod pdf_write;
pub mod plan;
pub mod question;
pub mod read;
pub mod rm;
pub mod task_complete;
/// Team coordination tools (create, spawn, message, tasks, etc.).
pub mod team_approve_plan;
pub mod team_assign_task;
pub mod team_broadcast;
pub mod team_cleanup;
pub mod team_create;
pub mod team_idle;
pub mod team_memory_read;
pub mod team_memory_write;
pub mod team_message;
pub mod team_read_messages;
pub mod team_shutdown_ack;
pub mod team_shutdown_teammate;
pub mod team_spawn;
pub mod team_status;
pub mod team_submit_plan;
pub mod team_task_claim;
pub mod team_task_complete;
pub mod team_task_create;
pub mod team_task_list;
pub mod team_wait;
pub mod think;
pub mod todo;
pub mod wait_tasks;
pub mod webfetch;
pub mod websearch;
pub mod write;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Verify that `path` resolves to somewhere within `root` after canonicalization.
/// Prevents directory traversal attacks (e.g., `../../etc/passwd`).
pub fn check_path_within_root(path: &Path, root: &Path) -> anyhow::Result<()> {
    let canonical = if path.exists() {
        path.canonicalize()?
    } else {
        let parent = path.parent().unwrap_or(path);
        let canonical_parent = if parent.exists() {
            parent.canonicalize()?
        } else {
            let mut p = parent;
            let mut parts = vec![];
            loop {
                if p.exists() {
                    let mut base = p.canonicalize()?;
                    for part in parts.iter().rev() {
                        base = base.join(part);
                    }
                    break base;
                }
                if let Some(name) = p.file_name() {
                    parts.push(name.to_os_string());
                }
                p = match p.parent() {
                    Some(pp) => pp,
                    None => break root.to_path_buf(),
                };
            }
        };
        if let Some(filename) = path.file_name() {
            canonical_parent.join(filename)
        } else {
            canonical_parent
        }
    };

    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    if !canonical.starts_with(&canonical_root) {
        anyhow::bail!(
            "Path escape rejected: '{}' resolves outside project root '{}'",
            path.display(),
            canonical_root.display()
        );
    }
    Ok(())
}

use crate::event::EventBus;
use crate::llm::ToolDefinition;

/// The result of a tool execution, containing textual output and optional structured metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Human-readable output text returned to the agent.
    pub content: String,
    /// Optional structured metadata (e.g., exit codes, byte counts) as JSON.
    // TODO: Replace `Value` with a typed `ToolMetadata` struct once the set of metadata
    // fields stabilises across tools.
    pub metadata: Option<Value>,
}

/// Identity and working context for a team session (lead or teammate).
///
/// Injected into [`ToolContext`] when a session is participating in a team.
/// Team tools use this to determine the caller's role and agent ID.
#[derive(Debug, Clone)]
pub struct TeamContext {
    /// Name of the team this session belongs to.
    pub team_name: String,
    /// Agent ID for the current session: `"lead"` or `"tm-NNN"`.
    pub agent_id: String,
    /// `true` if this session is the team lead.
    pub is_lead: bool,
}

/// Async interface for spawning teammate sessions.
///
/// Implemented by `TeamManager` (M3). During M2, the tool registry holds
/// `Option<Arc<dyn TeamManagerInterface>>` which is `None` until M3 is wired in.
#[async_trait::async_trait]
pub trait TeamManagerInterface: Send + Sync {
    /// Spawn a new teammate session and return its agent ID.
    ///
    /// `teammate_model` is an optional per-teammate model override. When `None`,
    /// the teammate inherits `lead_model` (the lead session's active model).
    async fn spawn_teammate(
        &self,
        team_name: &str,
        teammate_name: &str,
        agent_type: &str,
        prompt: &str,
        teammate_model: Option<&crate::agent::ModelRef>,
        lead_model: Option<&crate::agent::ModelRef>,
        working_dir: &std::path::Path,
    ) -> anyhow::Result<String>;
}

/// Execution context passed to each tool invocation.
impl Default for ToolOutput {
    fn default() -> Self {
        Self {
            content: String::new(),
            metadata: None,
        }
    }
}

/// Execution context passed to each tool invocation.
///
/// Carries the session identity, working directory, and event bus that
/// tools use to resolve paths and publish events.
///
/// # Examples
///
/// ```
/// use ragent_core::tool::ToolContext;
/// use ragent_core::event::EventBus;
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// let ctx = ToolContext {
///     session_id: "session-1".to_string(),
///     working_dir: PathBuf::from("/tmp"),
///     event_bus: Arc::new(EventBus::new(128)),
///     storage: None,
///     task_manager: None,
///     lsp_manager: None,
///     active_model: None,
///     team_context: None,
///     team_manager: None,
/// };
/// assert_eq!(ctx.session_id, "session-1");
/// ```
#[derive(Clone)]
pub struct ToolContext {
    /// Unique identifier for the current agent session.
    pub session_id: String,
    /// The working directory for file and command operations.
    pub working_dir: PathBuf,
    /// Event bus for publishing tool events (e.g., permission requests).
    pub event_bus: Arc<EventBus>,
    /// Optional storage handle for tools that need database access.
    pub storage: Option<Arc<crate::storage::Storage>>,
    /// Optional task manager for spawning sub-agent tasks.
    pub task_manager: Option<Arc<crate::task::TaskManager>>,
    /// Optional LSP manager for code-intelligence queries.
    pub lsp_manager: Option<crate::lsp::SharedLspManager>,
    /// The active model (provider + model ID) used by the parent session.
    /// Sub-agent tools use this to inherit the parent's provider when no
    /// explicit model override is specified in the tool call.
    pub active_model: Option<crate::agent::ModelRef>,
    /// Team identity for sessions participating in a team (lead or teammate).
    /// `None` when the session is not part of a team.
    pub team_context: Option<Arc<TeamContext>>,
    /// Optional team manager for spawning teammate sessions (M3+).
    /// `None` until `TeamManager` is wired into the session processor.
    pub team_manager: Option<Arc<dyn TeamManagerInterface>>,
}

/// A tool that an agent can invoke to perform actions.
///
/// Implementations provide a JSON schema for parameters, a permission category,
/// and an async [`Tool::execute`] method that carries out the operation.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Returns the unique name used to invoke this tool.
    fn name(&self) -> &str;
    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &str;
    /// Returns the JSON Schema describing the tool's accepted parameters.
    fn parameters_schema(&self) -> Value;
    /// Returns the permission category required to use this tool (e.g., `"file:read"`).
    fn permission_category(&self) -> &str;
    /// Executes the tool with the given JSON `input` and [`ToolContext`].
    ///
    /// # Errors
    ///
    /// Returns an error if required parameters are missing or the operation fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}

/// A registry that maps tool names to their implementations.
///
/// Tools are registered by name and can be looked up, listed, or exported
/// as [`ToolDefinition`] descriptors for LLM function-calling.
///
/// The internal map uses a [`RwLock`] so tools can be registered dynamically
/// (e.g., MCP tools) after the registry is wrapped in an `Arc`.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    /// Creates an empty tool registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::tool::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// assert!(registry.list().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a tool, keyed by its [`Tool::name`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::tool::{ToolRegistry, read::ReadTool};
    /// use std::sync::Arc;
    ///
    /// let registry = ToolRegistry::new();
    /// registry.register(Arc::new(ReadTool));
    /// assert_eq!(registry.list().len(), 1);
    /// ```
    pub fn register(&self, tool: Arc<dyn Tool>) {
        let mut tools = self.tools.write().expect("tool registry lock poisoned");
        tools.insert(tool.name().to_string(), tool);
    }

    /// Looks up a tool by name, returning a shared reference if found.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::tool::create_default_registry;
    ///
    /// let registry = create_default_registry();
    /// assert!(registry.get("read").is_some());
    /// assert!(registry.get("nonexistent").is_none());
    /// ```
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().expect("tool registry lock poisoned");
        tools.get(name).cloned()
    }

    /// Returns an alphabetically sorted list of all registered tool names.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::tool::create_default_registry;
    ///
    /// let registry = create_default_registry();
    /// let names = registry.list();
    /// assert!(names.contains(&"read".to_string()));
    /// assert!(names.contains(&"bash".to_string()));
    /// ```
    pub fn list(&self) -> Vec<String> {
        let tools = self.tools.read().expect("tool registry lock poisoned");
        let mut names: Vec<String> = tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Returns [`ToolDefinition`] descriptors for all registered tools, sorted by name.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::tool::create_default_registry;
    ///
    /// let registry = create_default_registry();
    /// let defs = registry.definitions();
    /// assert!(!defs.is_empty());
    /// assert!(defs.windows(2).all(|w| w[0].name <= w[1].name));
    /// ```
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().expect("tool registry lock poisoned");
        let mut defs: Vec<ToolDefinition> = tools
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters_schema(),
            })
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a [`ToolRegistry`] pre-populated with all built-in tools.
///
/// Included tools: `read`, `write`, `edit`, `bash`, `grep`, `glob`, `list`,
/// `question`, `office_read`, `office_write`, `office_info`, `pdf_read`,
/// `pdf_write`, `new_task`, `cancel_task`, `list_tasks`.
///
/// # Examples
///
/// ```
/// use ragent_core::tool::create_default_registry;
///
/// let registry = create_default_registry();
/// assert!(registry.list().contains(&"think".to_string()));
/// ```
pub fn create_default_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(read::ReadTool));
    registry.register(Arc::new(write::WriteTool));
    registry.register(Arc::new(create::CreateTool));
    registry.register(Arc::new(edit::EditTool));
    registry.register(Arc::new(multiedit::MultiEditTool));
    registry.register(Arc::new(patch::PatchTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(bash_reset::BashResetTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(list::ListTool));
    registry.register(Arc::new(question::QuestionTool));
    registry.register(Arc::new(webfetch::WebFetchTool));
    registry.register(Arc::new(websearch::WebSearchTool));
    registry.register(Arc::new(plan::PlanEnterTool));
    registry.register(Arc::new(plan::PlanExitTool));
    registry.register(Arc::new(think::ThinkTool));
    registry.register(Arc::new(todo::TodoReadTool));
    registry.register(Arc::new(todo::TodoWriteTool));
    registry.register(Arc::new(office_read::OfficeReadTool));
    registry.register(Arc::new(office_write::OfficeWriteTool));
    registry.register(Arc::new(office_info::OfficeInfoTool));
    // LibreOffice / OpenDocument tools
    registry.register(Arc::new(libreoffice_read::LibreReadTool));
    registry.register(Arc::new(libreoffice_write::LibreWriteTool));
    registry.register(Arc::new(libreoffice_info::LibreInfoTool));
    registry.register(Arc::new(pdf_read::PdfReadTool));
    registry.register(Arc::new(pdf_write::PdfWriteTool));
    registry.register(Arc::new(rm::RmTool));
    registry.register(Arc::new(new_task::NewTaskTool));
    registry.register(Arc::new(cancel_task::CancelTaskTool));
    registry.register(Arc::new(list_tasks::ListTasksTool));
    registry.register(Arc::new(wait_tasks::WaitTasksTool));
    registry.register(Arc::new(lsp_symbols::LspSymbolsTool));
    registry.register(Arc::new(lsp_hover::LspHoverTool));
    registry.register(Arc::new(lsp_definition::LspDefinitionTool));
    registry.register(Arc::new(lsp_references::LspReferencesTool));
    registry.register(Arc::new(lsp_diagnostics::LspDiagnosticsTool));
    registry.register(Arc::new(memory_write::MemoryWriteTool));
    registry.register(Arc::new(memory_write::MemoryReadTool));
    // GitHub tools
    registry.register(Arc::new(github_issues::GithubListIssuesTool));
    registry.register(Arc::new(github_issues::GithubGetIssueTool));
    registry.register(Arc::new(github_issues::GithubCreateIssueTool));
    registry.register(Arc::new(github_issues::GithubCommentIssueTool));
    registry.register(Arc::new(github_issues::GithubCloseIssueTool));
    // Team coordination tools
    registry.register(Arc::new(team_approve_plan::TeamApprovePlanTool));
    registry.register(Arc::new(team_assign_task::TeamAssignTaskTool));
    registry.register(Arc::new(team_broadcast::TeamBroadcastTool));
    registry.register(Arc::new(team_cleanup::TeamCleanupTool));
    registry.register(Arc::new(team_create::TeamCreateTool));
    registry.register(Arc::new(team_idle::TeamIdleTool));
    registry.register(Arc::new(team_message::TeamMessageTool));
    registry.register(Arc::new(team_memory_read::TeamMemoryReadTool));
    registry.register(Arc::new(team_memory_write::TeamMemoryWriteTool));
    registry.register(Arc::new(team_read_messages::TeamReadMessagesTool));
    registry.register(Arc::new(team_shutdown_ack::TeamShutdownAckTool));
    registry.register(Arc::new(team_shutdown_teammate::TeamShutdownTeammateTool));
    registry.register(Arc::new(team_spawn::TeamSpawnTool));
    registry.register(Arc::new(team_status::TeamStatusTool));
    registry.register(Arc::new(team_submit_plan::TeamSubmitPlanTool));
    registry.register(Arc::new(team_task_claim::TeamTaskClaimTool));
    registry.register(Arc::new(team_task_complete::TeamTaskCompleteTool));
    registry.register(Arc::new(task_complete::TaskCompleteTool));
    registry.register(Arc::new(team_task_create::TeamTaskCreateTool));
    registry.register(Arc::new(team_task_list::TeamTaskListTool));
    registry.register(Arc::new(team_wait::TeamWaitTool));
    // GitHub PR tools
    registry.register(Arc::new(github_prs::GithubListPrsTool));
    registry.register(Arc::new(github_prs::GithubGetPrTool));
    registry.register(Arc::new(github_prs::GithubCreatePrTool));
    registry.register(Arc::new(github_prs::GithubMergePrTool));
    registry.register(Arc::new(github_prs::GithubReviewPrTool));
    registry
}
