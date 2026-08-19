//! Essential file, shell, and search tools for ragent.
//!
//! This crate provides the extracted Milestone 4 tool set together with the
//! minimal shared tool abstractions those moved implementations require.

// Shared exact-byte replacement matcher (used by edit, multi_edit,
// apply_patch) plus the match-failure diagnostics formatter.
// Extracted in WSPLAN Milestone 2.
pub mod replace;

// Shared path-resolution helper (DUPPLAN.md Milestone B).
pub mod path_util;

// Shared stale-file / timestamp helpers used by edit and multi_edit.
pub(crate) mod edit_common;

// File operation tools
pub mod append_file;
pub mod apply_patch;
pub mod copy_file;
pub mod create;
pub mod cron_log;
pub mod diff;
pub mod edit;
pub mod edit_log;
pub mod file_info;
pub mod mkdir;
pub mod move_file;
pub mod multiedit;
pub mod patch;
pub mod read;
pub mod rm;
pub mod truncate;
pub mod write;
pub mod xlsx;

// Search tools
pub mod glob;
pub mod grep;
pub mod list;

// Shell tools
mod askpass;
pub mod bash;
pub mod bash_reset;
pub mod bg;
pub mod open;

// Interaction tools
pub mod agent_complete;
pub mod think;

// Utility tools
pub mod calculator;
pub mod get_env;

pub mod file_lock;

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
///
/// Re-exported from `ragent_types::resource` (DUPPLAN.md Milestone E).
/// Previously duplicated as an inline module; now a single source of truth
/// lives in `ragent_types::resource`.
pub use ragent_types::resource;

/// The result of a tool execution, including optional structured metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolOutput {
    /// Human-readable output text returned to the caller.
    pub content: String,
    /// Optional structured metadata for machine-readable follow-up handling.
    pub metadata: Option<Value>,
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
    /// Read timestamps (mtime in milliseconds since UNIX epoch) for files that
    /// have been read by this session. Used by edit tools to detect stale-file
    /// edits (editrenewal FR-003).
    pub read_timestamps: Arc<RwLock<HashMap<PathBuf, u64>>>,
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
/// Canonicalises both `path` and `root` and checks that the canonical path is
/// either the root itself or a true child of it using path-component equality,
/// which avoids the string-prefix confusion where `/foo` appears to contain
/// `/foobar`.
///
/// For paths that do not yet exist, the function canonicalises the longest
/// existing prefix and then appends the remaining, normalised components.  This
/// catches traversal attempts in non-existent paths such as `../etc/passwd`.
///
/// # Errors
///
/// Returns an error if the path escapes the given root.
pub fn check_path_within_root(path: &Path, root: &Path) -> anyhow::Result<()> {
    let canonical_root = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf().clean_path());

    // Canonicalise the existing portion of `path`.  We cannot call
    // `canonicalize` directly on a non-existent path, so we walk up until we
    // find something that exists, record the missing tail components, then
    // reconstruct the path from the canonical base.
    let canonical = if let Ok(c) = path.canonicalize() {
        c
    } else {
        let mut existing: &Path = path;
        let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
        loop {
            if existing.exists() {
                let mut base = existing.canonicalize()?;
                for part in tail.iter().rev() {
                    base = base.join(part);
                }
                break base;
            }
            if let Some(name) = existing.file_name() {
                tail.push(name);
            }
            match existing.parent() {
                Some(parent) => existing = parent,
                None => break canonical_root.clone(),
            }
        }
    };

    if !is_path_within(&canonical, &canonical_root) {
        anyhow::bail!(
            "Path escape rejected: '{}' resolves outside project root '{}'",
            path.display(),
            canonical_root.display()
        );
    }
    Ok(())
}

/// Returns true when `child` is `root` or a descendant of `root`, using path
/// components rather than string prefixing.
fn is_path_within(child: &Path, root: &Path) -> bool {
    if child == root {
        return true;
    }
    let root_components: Vec<_> = root.components().collect();
    let child_components: Vec<_> = child.components().collect();
    child_components.len() >= root_components.len()
        && child_components[..root_components.len()] == root_components[..]
}

/// Lightweight in-place path cleaning that removes `.` and `..` components
/// without touching the filesystem.  Used only as a fallback when canonicalising
/// the root itself fails.
trait CleanPath {
    fn clean_path(&self) -> PathBuf;
}

impl CleanPath for Path {
    fn clean_path(&self) -> PathBuf {
        let mut out = PathBuf::new();
        for comp in self.components() {
            match comp {
                std::path::Component::ParentDir => {
                    out.pop();
                }
                std::path::Component::CurDir => {}
                _ => out.push(comp),
            }
        }
        out
    }
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
    registry.register(Arc::new(apply_patch::ApplyPatchTool));
    registry.register(Arc::new(read::ReadTool));
    registry.register(Arc::new(write::WriteTool));
    registry.register(Arc::new(create::CreateTool));
    registry.register(Arc::new(edit::EditTool));
    registry.register(Arc::new(multiedit::MultiEditTool));
    registry.register(Arc::new(patch::PatchTool));
    registry.register(Arc::new(copy_file::CopyFileTool));
    registry.register(Arc::new(move_file::MoveFileTool));
    registry.register(Arc::new(rm::RmTool));
    registry.register(Arc::new(mkdir::MakeDirTool));
    registry.register(Arc::new(append_file::AppendFileTool));
    registry.register(Arc::new(file_info::FileInfoTool));
    registry.register(Arc::new(diff::DiffFilesTool));
    // Search tools
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(list::ListTool));
    registry.register(Arc::new(grep::GrepTool));

    // Shell tools
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(bash_reset::BashResetTool));
    registry.register(Arc::new(open::OpenTool));
    // Interaction tools
    registry.register(Arc::new(agent_complete::AgentCompleteTool));
    registry.register(Arc::new(think::ThinkTool));

    // Utility tools
    registry.register(Arc::new(get_env::GetEnvTool));
    registry.register(Arc::new(calculator::CalculatorTool));

    registry
}
