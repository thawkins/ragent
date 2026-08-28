//! GitHub and GitLab tools for ragent.
//!
//! This crate owns the Milestone 6 extracted VCS layer while keeping a small
//! compatibility surface for the extracted runtime crates.

pub mod git;
pub mod github;
pub mod gitlab;
pub mod registry;
pub mod vcs_provider;

use anyhow::Result;
use ragent_types::llm::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Storage adapter types for GitLab-backed VCS helpers.
pub mod storage {
    use anyhow::Result;

    /// Storage backend abstraction used by GitLab auth/client helpers.
    pub trait StorageBackend: Send + Sync {
        /// Fetch a stored provider credential by id.
        fn get_provider_auth(&self, provider_id: &str) -> Result<Option<String>>;
        /// Persist a provider credential by id.
        fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> Result<()>;
        /// Remove a stored provider credential by id.
        fn delete_provider_auth(&self, provider_id: &str) -> Result<()>;
        /// Fetch a stored setting by key.
        fn get_setting(&self, key: &str) -> Result<Option<String>>;
        /// Persist a setting value by key.
        fn set_setting(&self, key: &str, value: &str) -> Result<()>;
        /// Remove a stored setting by key.
        fn delete_setting(&self, key: &str) -> Result<()>;
    }

    /// Compatibility alias for migrated code that still references `crate::storage::Storage`.
    pub type Storage = dyn StorageBackend;

    /// Blanket adapter: `ragent_storage::Storage` already provides all the
    /// methods `StorageBackend` requires, so this impl forwards directly.
    /// Defining it here (in the crate that owns `StorageBackend`) avoids the
    /// orphan-rule violation that previously forced `ragent-agent` to define
    /// the impl on a foreign type (see `REMPLAN.md` M2 / T2.2).
    impl StorageBackend for ragent_storage::Storage {
        fn get_provider_auth(&self, provider_id: &str) -> Result<Option<String>> {
            self.get_provider_auth(provider_id)
        }

        fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> Result<()> {
            self.set_provider_auth(provider_id, api_key)
        }

        fn delete_provider_auth(&self, provider_id: &str) -> Result<()> {
            self.delete_provider_auth(provider_id)
        }

        fn get_setting(&self, key: &str) -> Result<Option<String>> {
            self.get_setting(key)
        }

        fn set_setting(&self, key: &str, value: &str) -> Result<()> {
            self.set_setting(key, value)
        }

        fn delete_setting(&self, key: &str) -> Result<()> {
            self.delete_setting(key)
        }
    }
}

/// The result of a tool execution, including optional structured metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolOutput {
    /// The textual result of the tool execution.
    pub content: String,
    /// Optional structured metadata associated with the result.
    pub metadata: Option<Value>,
}

/// Execution context passed to each tool invocation.
#[derive(Clone)]
pub struct ToolContext {
    /// Identifier of the session running the tool.
    pub session_id: String,
    /// Base directory tools should operate within.
    pub working_dir: PathBuf,
    /// Optional storage backend for VCS credential access.
    pub storage: Option<Arc<dyn storage::StorageBackend>>,
    /// Optional ragent configuration loaded from config files.
    pub config: Option<Arc<ragent_config::Config>>,
}

/// A tool that an agent can invoke to perform actions.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// The unique name used to invoke the tool.
    fn name(&self) -> &str;
    /// A human-readable description of what the tool does.
    fn description(&self) -> &str;
    /// The JSON schema describing the tool's input parameters.
    fn parameters_schema(&self) -> Value;
    /// The permission category the tool belongs to.
    fn permission_category(&self) -> &str;
    /// Execute the tool with the given input and context.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}

/// Tool registry for managing available tools by name.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    hidden: RwLock<HashSet<String>>,
}

impl ToolRegistry {
    /// Create an empty tool registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            hidden: RwLock::new(HashSet::new()),
        }
    }

    /// Register a tool, replacing any existing tool with the same name.
    pub fn register(&self, tool: Arc<dyn Tool>) {
        self.tools
            .write()
            .expect("tool registry lock poisoned")
            .insert(tool.name().to_string(), tool);
    }

    /// Fetch a registered tool by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .read()
            .expect("tool registry lock poisoned")
            .get(name)
            .cloned()
    }

    /// List the names of all registered tools, sorted.
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

    /// Replace the set of hidden (invisible) tool names.
    pub fn set_hidden(&self, names: &[String]) {
        let mut hidden = self.hidden.write().expect("tool hidden lock poisoned");
        *hidden = names.iter().cloned().collect();
    }

    /// Return the definitions of all registered, non-hidden tools.
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
