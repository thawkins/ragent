//! Essential file, shell, and search tools for ragent.
//!
//! This crate provides the extracted Milestone 4 tool set together with the
//! minimal shared tool abstractions those moved implementations require.

// File operation tools
pub mod append_file;
pub mod copy_file;
pub mod create;
pub mod diff;
pub mod edit;
pub mod file_info;
pub mod mkdir;
pub mod move_file;
pub mod multiedit;
pub mod patch;
pub mod read;
pub mod rm;
pub mod truncate;
pub mod write;

// Search tools
pub mod glob;
pub mod grep;
pub mod list;
pub mod search;

// Shell tools
pub mod bash;
pub mod bash_reset;

// Interaction tools
pub mod question;
pub mod task_complete;
pub mod think;

// Utility tools
pub mod calculator;
pub mod get_env;

mod file_lock;

use anyhow::Result;
use ragent_types::event::EventBus;
use ragent_types::llm::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Compatibility re-export for moved tools that still reference `crate::event`.
pub mod event {
    pub use ragent_types::event::{Event, EventBus};
}

/// Compatibility re-export for moved tools that still reference `crate::sanitize`.
pub mod sanitize {
    pub use ragent_types::sanitize::*;
}

/// Minimal process resource gate used by shell-based tools.
pub mod resource {
    use std::sync::LazyLock;

    use tokio::sync::{OwnedSemaphorePermit, Semaphore};

    /// Maximum number of concurrent child processes the core tools may spawn.
    pub const MAX_CONCURRENT_PROCESSES: usize = 16;

    static PROCESS_SEMAPHORE: LazyLock<std::sync::Arc<Semaphore>> =
        LazyLock::new(|| std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSES)));

    /// Acquire a permit to spawn a child process.
    ///
    /// # Errors
    ///
    /// Returns an error only if the semaphore has been closed.
    pub async fn acquire_process_permit() -> anyhow::Result<OwnedSemaphorePermit> {
        PROCESS_SEMAPHORE
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| anyhow::anyhow!("process semaphore closed"))
    }
}

/// The result of a tool execution, including optional structured metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Human-readable output text returned to the caller.
    pub content: String,
    /// Optional structured metadata for machine-readable follow-up handling.
    pub metadata: Option<Value>,
}

impl Default for ToolOutput {
    fn default() -> Self {
        Self {
            content: String::new(),
            metadata: None,
        }
    }
}

/// Execution context passed to each tool invocation.
#[derive(Clone)]
pub struct ToolContext {
    /// Unique identifier for the current agent session.
    pub session_id: String,
    /// Working directory for file and command operations.
    pub working_dir: PathBuf,
    /// Event bus used to publish tool-side UI/runtime events.
    pub event_bus: Arc<EventBus>,
}

/// A tool that an agent can invoke to perform actions.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Returns the unique name used to invoke this tool.
    fn name(&self) -> &str;
    /// Returns a human-readable description of the tool.
    fn description(&self) -> &str;
    /// Returns the JSON Schema for this tool's parameters.
    fn parameters_schema(&self) -> Value;
    /// Returns the permission category required to use this tool.
    fn permission_category(&self) -> &str;
    /// Executes the tool.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is invalid or the operation fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}

/// Verify that `path` resolves within `root` after canonicalization.
///
/// # Errors
///
/// Returns an error if the path escapes the given root.
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

/// Tool registry for managing available tools by name.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    hidden: RwLock<HashSet<String>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            hidden: RwLock::new(HashSet::new()),
        }
    }

    /// Register a tool by name.
    pub fn register(&self, tool: Arc<dyn Tool>) {
        self.tools
            .write()
            .expect("tool registry lock poisoned")
            .insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .read()
            .expect("tool registry lock poisoned")
            .get(name)
            .cloned()
    }

    /// List all registered tool names.
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .tools
            .read()
            .expect("tool registry lock poisoned")
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// Check if a tool is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tools
            .read()
            .expect("tool registry lock poisoned")
            .contains_key(name)
    }

    /// Remove a tool by name.
    pub fn remove(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .write()
            .expect("tool registry lock poisoned")
            .remove(name)
    }

    /// Clear all tools from the registry.
    pub fn clear(&self) {
        self.tools
            .write()
            .expect("tool registry lock poisoned")
            .clear();
    }

    /// Hide tools from advertised tool definitions while keeping them executable.
    pub fn set_hidden(&self, names: &[String]) {
        let mut hidden = self.hidden.write().expect("tool hidden lock poisoned");
        *hidden = names.iter().cloned().collect();
    }

    /// Export visible tools as LLM tool definitions.
    #[must_use]
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().expect("tool registry lock poisoned");
        let hidden = self.hidden.read().expect("tool hidden lock poisoned");
        let mut defs: Vec<ToolDefinition> = tools
            .values()
            .filter(|tool| !hidden.contains(tool.name()))
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema(),
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

/// Create a tool registry with all core tools registered.
///
/// # Example
///
/// ```
/// use ragent_tools_core::create_core_registry;
///
/// let registry = create_core_registry();
/// assert!(registry.contains("read"));
/// assert!(registry.contains("bash"));
/// ```
#[must_use]
pub fn create_core_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();

    // File operations
    registry.register(Arc::new(read::ReadTool));
    registry.register(Arc::new(write::WriteTool));
    registry.register(Arc::new(create::CreateTool));
    registry.register(Arc::new(edit::EditTool));
    registry.register(Arc::new(multiedit::MultiEditTool));
    registry.register(Arc::new(patch::PatchTool));
          registry.register(Arc::new(copy_file::CopyFileTool));    registry.register(Arc::new(move_file::MoveFileTool));
    registry.register(Arc::new(rm::RmTool));
    registry.register(Arc::new(mkdir::MakeDirTool));
    registry.register(Arc::new(append_file::AppendFileTool));
    registry.register(Arc::new(file_info::FileInfoTool));
    registry.register(Arc::new(diff::DiffFilesTool));
    // Search tools
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(list::ListTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(search::SearchTool));

    // Shell tools
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(bash_reset::BashResetTool));

    // Interaction tools
    registry.register(Arc::new(question::QuestionTool));
    registry.register(Arc::new(task_complete::TaskCompleteTool));
    registry.register(Arc::new(think::ThinkTool));

    // Utility tools
    registry.register(Arc::new(get_env::GetEnvTool));
    registry.register(Arc::new(calculator::CalculatorTool));

    registry
}
