//! GitHub and GitLab tools for ragent.
//!
//! This crate owns the Milestone 6 extracted VCS layer while keeping a small
//! compatibility surface for the extracted runtime crates.

pub mod github;
pub mod gitlab;
pub mod registry;

use anyhow::Result;
use ragent_types::llm::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Compatibility re-export for moved helpers that still reference `crate::config`.
pub mod config {
    pub use ragent_config::Config;
}

/// Storage adapter types for GitLab-backed VCS helpers.
pub mod storage {
    use anyhow::Result;

    /// Storage backend abstraction used by GitLab auth/client helpers.
    pub trait StorageBackend: Send + Sync {
        fn get_provider_auth(&self, provider_id: &str) -> Result<Option<String>>;
        fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> Result<()>;
        fn delete_provider_auth(&self, provider_id: &str) -> Result<()>;
        fn get_setting(&self, key: &str) -> Result<Option<String>>;
        fn set_setting(&self, key: &str, value: &str) -> Result<()>;
        fn delete_setting(&self, key: &str) -> Result<()>;
    }

    /// Compatibility alias for migrated code that still references `crate::storage::Storage`.
    pub type Storage = dyn StorageBackend;
}

/// The result of a tool execution, including optional structured metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
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
    pub session_id: String,
    pub working_dir: PathBuf,
    pub storage: Option<Arc<dyn storage::StorageBackend>>,
}

/// A tool that an agent can invoke to perform actions.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn permission_category(&self) -> &str;
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}

/// Tool registry for managing available tools by name.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    hidden: RwLock<HashSet<String>>,
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            hidden: RwLock::new(HashSet::new()),
        }
    }

    pub fn register(&self, tool: Arc<dyn Tool>) {
        self.tools
            .write()
            .expect("tool registry lock poisoned")
            .insert(tool.name().to_string(), tool);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .read()
            .expect("tool registry lock poisoned")
            .get(name)
            .cloned()
    }

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

    pub fn set_hidden(&self, names: &[String]) {
        let mut hidden = self.hidden.write().expect("tool hidden lock poisoned");
        *hidden = names.iter().cloned().collect();
    }

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
