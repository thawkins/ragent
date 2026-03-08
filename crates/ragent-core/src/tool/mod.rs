//! Tool system for agent capabilities.
//!
//! This module defines the [`Tool`] trait for implementing agent-callable tools,
//! the [`ToolRegistry`] for managing available tools by name, and supporting types
//! [`ToolContext`] and [`ToolOutput`] used during tool execution.
//!
//! Built-in tools (file I/O, shell execution, search, and user interaction) are
//! provided via [`create_default_registry`].

pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod list;
pub mod question;
pub mod read;
pub mod write;

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

#[derive(Clone)]
pub struct ToolContext {
    /// Unique identifier for the current agent session.
    pub session_id: String,
    /// The working directory for file and command operations.
    pub working_dir: PathBuf,
    /// Event bus for publishing tool events (e.g., permission requests).
    pub event_bus: Arc<EventBus>,
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
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Registers a tool, keyed by its [`Tool::name`].
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Looks up a tool by name, returning a shared reference if found.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Returns an alphabetically sorted list of all registered tool names.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Returns [`ToolDefinition`] descriptors for all registered tools, sorted by name.
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
/// Included tools: `read`, `write`, `edit`, `bash`, `grep`, `glob`, `list`, `question`.
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(read::ReadTool));
    registry.register(Arc::new(write::WriteTool));
    registry.register(Arc::new(edit::EditTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(list::ListTool));
    registry.register(Arc::new(question::QuestionTool));
    registry
}
