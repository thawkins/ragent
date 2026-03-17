//! Tool system for agent capabilities.
//!
//! This module defines the [`Tool`] trait for implementing agent-callable tools,
//! the [`ToolRegistry`] for managing available tools by name, and supporting types
//! [`ToolContext`] and [`ToolOutput`] used during tool execution.
//!
//! Built-in tools (file I/O, shell execution, search, and user interaction) are
//! provided via [`create_default_registry`].

pub mod bash;
pub mod cancel_task;
pub mod create;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod list;
pub mod list_tasks;
pub mod lsp_definition;
pub mod lsp_diagnostics;
pub mod lsp_hover;
pub mod lsp_references;
pub mod lsp_symbols;
pub mod multiedit;
pub mod new_task;
pub mod wait_tasks;
pub mod office_common;
pub mod office_info;
pub mod office_read;
pub mod office_write;
pub mod libreoffice_common;
pub mod libreoffice_info;
pub mod libreoffice_read;
pub mod libreoffice_write;
pub mod patch;
pub mod pdf_read;
pub mod pdf_write;
pub mod plan;
pub mod question;
pub mod read;
pub mod rm;
pub mod todo;
pub mod webfetch;
pub mod websearch;
pub mod write;
pub mod file_ops_tool;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
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
            tools: HashMap::new(),
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
    /// let mut registry = ToolRegistry::new();
    /// registry.register(Arc::new(ReadTool));
    /// assert_eq!(registry.list().len(), 1);
    /// ```
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
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
        self.tools.get(name).cloned()
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
    /// assert!(names.contains(&"read"));
    /// assert!(names.contains(&"bash"));
    /// ```
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
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
        let mut defs: Vec<ToolDefinition> = self
            .tools
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
/// assert_eq!(registry.list().len(), 31);
/// ```
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(read::ReadTool));
    registry.register(Arc::new(write::WriteTool));
    registry.register(Arc::new(create::CreateTool));
    registry.register(Arc::new(edit::EditTool));
    registry.register(Arc::new(multiedit::MultiEditTool));
    registry.register(Arc::new(patch::PatchTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(list::ListTool));
    registry.register(Arc::new(question::QuestionTool));
    registry.register(Arc::new(webfetch::WebFetchTool));
    registry.register(Arc::new(websearch::WebSearchTool));
    registry.register(Arc::new(plan::PlanEnterTool));
    registry.register(Arc::new(plan::PlanExitTool));
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
    registry
}
