//! Extended document, web, memory, and code index tools for ragent.
//!
//! This crate owns the Milestone 5 extracted tool set while keeping a small
//! compatibility surface for the extracted runtime crates.

// The `yfinance_rs` dependency generates deeply nested async futures that trip
// the `recursion_depth_exceeding_limit` future-incompat lint (rust-lang/rust
// #159228). Raising the recursion limit keeps the crate compiling on newer
// toolchains until the dependency is fixed upstream.
#![recursion_limit = "256"]

pub mod browser;
pub mod channels;
pub mod codeindex_communities;
pub mod codeindex_dependencies;
pub mod codeindex_explain;
pub mod codeindex_godnodes;
pub mod codeindex_path;
pub mod codeindex_references;
pub mod codeindex_reindex;
pub mod codeindex_search;
pub mod codeindex_status;
pub mod codeindex_symbols;
pub(crate) mod codeindex_utils;
pub mod document_extract;
pub mod finance;
pub mod gmail;
pub mod http_request;
pub mod libreoffice_common;
pub mod libreoffice_info;
pub mod libreoffice_read;
pub mod libreoffice_write;
pub mod masterfetch;
pub mod office_common;
pub mod office_info;
pub mod office_read;
pub mod office_write;
pub mod pdf_read;
pub mod pdf_write;
pub mod task;
pub mod webfetch;
pub mod websearch;

pub mod memory {
    //! Memory helpers reused by the extracted memory tools.

    pub mod embedding;
}

use anyhow::Result;
use ragent_types::event::EventBus;
use ragent_types::llm::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Compatibility re-export for moved tools that still reference `crate::event`.
pub mod event {
    pub use ragent_types::event::{Event, EventBus};
}

/// Storage adapter types for the extracted tools.
pub mod storage {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};

    /// Row representation of a task item.
    ///
    /// (todo2tasks T-001: extended with `active_form`, `owner`, `metadata`,
    /// and `blocked_by` fields. `#[serde(default)]` on each new field
    /// ensures legacy JSON rows deserialize without error — FR-002.)
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TaskRow {
        /// The unique task identifier.
        pub id: String,
        /// The session that owns the task.
        pub session_id: String,
        /// The task's display title.
        pub title: String,
        /// The task's current status.
        pub status: String,
        /// Free-text description carrying acceptance criteria.
        pub description: String,
        /// When the task was created.
        pub created_at: String,
        /// When the task was last updated.
        pub updated_at: String,
        /// Present-continuous phrase shown in progress indicators (FR-007).
        #[serde(default)]
        pub active_form: Option<String>,
        /// Free-form owner label naming the agent/worker responsible (FR-006).
        #[serde(default)]
        pub owner: Option<String>,
        /// Arbitrary key-value metadata (FR-008). Defaults to `{}`.
        #[serde(default = "default_metadata_object")]
        pub metadata: serde_json::Value,
        /// Task IDs that must reach `completed` before this task (FR-001).
        #[serde(default)]
        pub blocked_by: Vec<String>,
    }

    /// Default value for `TaskRow::metadata` — an empty JSON object.
    fn default_metadata_object() -> serde_json::Value {
        serde_json::Value::Object(serde_json::Map::new())
    }

    /// Row representation of a structured memory.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MemoryRow {
        /// The unique memory identifier.
        pub id: i64,
        /// The memory's stored content.
        pub content: String,
        /// The memory's category.
        pub category: String,
        /// Where the memory came from.
        pub source: String,
        /// The confidence score of the memory.
        pub confidence: f64,
        /// The project the memory belongs to.
        pub project: String,
        /// The session that stored the memory.
        pub session_id: String,
        /// When the memory was created.
        pub created_at: String,
        /// When the memory was last updated.
        pub updated_at: String,
        /// How many times the memory has been accessed.
        pub access_count: i64,
        /// When the memory was last accessed.
        pub last_accessed: Option<String>,
    }

    /// Result row for embedding-based memory search.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EmbeddingMatch {
        /// The matching row identifier.
        pub row_id: i64,
        /// The similarity score of the match.
        pub score: f32,
    }

    /// Storage backend abstraction used by session-scoped tools.
    pub trait StorageBackend: Send + Sync {
        /// List tasks for a session, optionally filtered by status.
        fn list_tasks(&self, session_id: &str, status: Option<&str>) -> Result<Vec<TaskRow>>;

        /// Creates a task row with a simplified, legacy-compatible set
        /// of fields.
        ///
        /// Implementors that support the full Task model should implement
        /// [`create_task`] directly and leave this method as the provided
        /// default.
        fn create_task_simple(
            &self,
            id: &str,
            session_id: &str,
            title: &str,
            status: &str,
            description: &str,
        ) -> Result<()> {
            self.create_task(
                id,
                session_id,
                title,
                description,
                status,
                None,
                None,
                &serde_json::Value::Object(serde_json::Map::new()),
                &[],
            )
        }

        /// Creates a new Task row with all Task-model fields populated
        /// (todo2tasks T-007, FR-009, FR-011, FR-012).
        ///
        /// This is the canonical method.  Implementors that do not
        /// support the full Task model can implement `create_task_simple`
        /// and leave this default, which delegates to the simple variant.
        ///
        /// # Errors
        ///
        /// Returns an error if the storage backend fails to persist the
        /// row.
        #[allow(clippy::too_many_arguments)]
        fn create_task(
            &self,
            id: &str,
            session_id: &str,
            subject: &str,
            description: &str,
            status: &str,
            active_form: Option<&str>,
            owner: Option<&str>,
            metadata: &serde_json::Value,
            blocked_by: &[String],
        ) -> Result<()> {
            let _ = (active_form, owner, metadata, blocked_by);
            self.create_task_simple(id, session_id, subject, status, description)
        }

        /// Updates a task row with a simplified, legacy-compatible set
        /// of fields.
        ///
        /// Implementors that support the full Task model should implement
        /// [`update_task`] directly and leave this method as the provided
        /// default.
        fn update_task_simple(
            &self,
            id: &str,
            session_id: &str,
            title: Option<&str>,
            status: Option<&str>,
            description: Option<&str>,
        ) -> Result<bool> {
            self.update_task(
                id,
                session_id,
                title,
                status,
                description,
                None,
                None,
                None,
                None,
            )
        }

        /// Updates a Task row with all Task-model fields (todo2tasks T-008,
        /// T-017).
        ///
        /// Each `Option<T>` parameter is `None` → unchanged.  For
        /// `active_form` and `owner`, `Some(None)` clears the field to
        /// empty; `Some(Some(v))` sets it.  `blocked_by` is a full
        /// replacement (`Some(slice)` replaces, `None` leaves unchanged).
        ///
        /// This is the canonical method.  Implementors that do not
        /// support the full Task model can implement `update_task_simple`
        /// and leave this default, which delegates to the simple variant.
        ///
        /// # Errors
        ///
        /// Returns an error if the storage backend fails.  Returns
        /// `Ok(false)` if no row matched `id` + `session_id`.
        #[allow(clippy::type_complexity, clippy::too_many_arguments)]
        fn update_task(
            &self,
            id: &str,
            session_id: &str,
            subject: Option<&str>,
            status: Option<&str>,
            description: Option<&str>,
            active_form: Option<Option<&str>>,
            owner: Option<Option<&str>>,
            metadata: Option<&serde_json::Value>,
            blocked_by: Option<&[String]>,
        ) -> Result<bool> {
            let _ = (active_form, owner, metadata, blocked_by);
            self.update_task_simple(id, session_id, subject, status, description)
        }
        /// Delete a task row, returning whether it existed.
        fn delete_task(&self, id: &str, session_id: &str) -> Result<bool>;
        /// Remove all tasks for a session, returning how many were removed.
        fn clear_tasks(&self, session_id: &str) -> Result<usize>;

        /// Fetch a single memory row by identifier.
        fn get_memory(&self, id: i64) -> Result<Option<MemoryRow>>;
        /// Fetch the tags associated with a memory row.
        fn get_memory_tags(&self, id: i64) -> Result<Vec<String>>;
        /// Search memory rows by keyword across optional filters.
        fn search_memories(
            &self,
            query: &str,
            category: Option<&str>,
            source: Option<&str>,
            limit: usize,
            min_confidence: f64,
        ) -> Result<Vec<MemoryRow>>;
        /// List the most recent memories for a project.
        fn list_memories(&self, project: &str, limit: usize) -> Result<Vec<MemoryRow>>;
        /// Store an embedding blob for a memory row.
        fn store_memory_embedding(&self, id: i64, embedding_blob: &[u8]) -> Result<bool>;
        /// List all memory embeddings as (id, blob) pairs.
        fn list_memory_embeddings(&self) -> Result<Vec<(i64, Vec<u8>)>>;
        /// Search memories by embedding similarity.
        fn search_memories_by_embedding(
            &self,
            query_embedding: &[f32],
            dimensions: usize,
            limit: usize,
            min_similarity: f32,
        ) -> Result<Vec<EmbeddingMatch>>;
    }

    /// Compatibility alias for migrated code that still references `crate::storage::Storage`.
    pub type Storage = dyn StorageBackend;
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
    /// Shared event bus for emitting tool events.
    pub event_bus: Arc<EventBus>,
    /// Optional storage backend for persistence.
    pub storage: Option<Arc<dyn storage::StorageBackend>>,
    /// Optional codebase index for code-intelligence tools.
    pub code_index: Option<Arc<ragent_codeindex::CodeIndex>>,
    /// Optional ragent configuration loaded from config files.
    /// Provides tools access to settings like API keys, permissions, etc.
    pub config: Option<Arc<ragent_config::Config>>,
    /// Read timestamps (mtime in milliseconds since UNIX epoch) for files that
    /// have been read by this session. Used by edit tools to detect stale-file
    /// edits (editrenewal FR-003).
    pub read_timestamps: Arc<std::sync::RwLock<std::collections::HashMap<PathBuf, u64>>>,
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

pub use ragent_tools_core::check_path_within_root;

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

    /// Return whether a tool with the given name is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tools
            .read()
            .expect("tool registry lock poisoned")
            .contains_key(name)
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

/// Create a registry with all extracted Milestone 5 tools registered.
#[must_use]
pub fn create_extended_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();

    registry.register(Arc::new(pdf_read::PdfReadTool));
    registry.register(Arc::new(pdf_write::PdfWriteTool));
    registry.register(Arc::new(office_read::OfficeReadTool));
    registry.register(Arc::new(office_write::OfficeWriteTool));
    registry.register(Arc::new(office_info::OfficeInfoTool));
    registry.register(Arc::new(libreoffice_read::LibreReadTool));
    registry.register(Arc::new(libreoffice_write::LibreWriteTool));
    registry.register(Arc::new(libreoffice_info::LibreInfoTool));
    registry.register(Arc::new(webfetch::WebFetchTool));
    registry.register(Arc::new(websearch::WebSearchTool));
    registry.register(Arc::new(http_request::HttpRequestTool));
    // todo2tasks T-011: register all four task tools (FR-011, FR-017).
    // TaskGetTool (T-009, FR-011, FR-014) and TaskListTool (T-010,
    // FR-011, FR-015) are read-only; TaskCreateTool (T-007) and
    // TaskUpdateTool (T-008/T-017) are write tools. All four use the
    // "task" permission category and are hardwired auto-approve.
    registry.register(Arc::new(task::TaskCreateTool));
    registry.register(Arc::new(task::TaskUpdateTool));
    registry.register(Arc::new(task::TaskGetTool));
    registry.register(Arc::new(task::TaskListTool));
    registry.register(Arc::new(codeindex_search::CodeIndexSearchTool));
    registry.register(Arc::new(codeindex_status::CodeIndexStatusTool));
    registry.register(Arc::new(codeindex_symbols::CodeIndexSymbolsTool));
    registry.register(Arc::new(codeindex_references::CodeIndexReferencesTool));
    registry.register(Arc::new(codeindex_dependencies::CodeIndexDependenciesTool));
    registry.register(Arc::new(codeindex_reindex::CodeIndexReindexTool));
    // graphCI T-015: codeindex_godnodes tool (FR-014, FR-017).
    registry.register(Arc::new(codeindex_godnodes::CodeIndexGodnodesTool));
    // graphCI T-013: codeindex_path tool (FR-012, FR-017).
    registry.register(Arc::new(codeindex_path::CodeIndexPathTool));
    // graphCI T-012: codeindex_explain tool (FR-011, FR-017).
    registry.register(Arc::new(codeindex_explain::CodeIndexExplainTool));
    // graphCI T-014: codeindex_communities tool (FR-013, FR-017).
    registry.register(Arc::new(codeindex_communities::CodeIndexCommunitiesTool));
    registry.register(Arc::new(browser::BrowserTool));
    // JCODEPLAN M7 — external integrations.
    registry.register(Arc::new(gmail::GmailTool::new()));
    registry.register(Arc::new(channels::SendChannelMessageTool));

    // MasterFetch tools (FR-020)
    registry.register(Arc::new(masterfetch::tools::fetch::MfFetchTool));
    registry.register(Arc::new(masterfetch::tools::crawl_tool::MfCrawlTool));
    registry.register(Arc::new(masterfetch::tools::search_tool::MfSearchTool));
    registry.register(Arc::new(masterfetch::tools::screenshot::MfScreenshotTool));
    registry.register(Arc::new(masterfetch::tools::cache_clear::MfCacheClearTool));
    registry.register(Arc::new(masterfetch::tools::version::MfVersionTool));

    // yfinance tools (T-015)
    registry.register(Arc::new(finance::tools::quote::StockQuoteTool::new()));
    registry.register(Arc::new(finance::tools::history::StockHistoryTool));
    registry.register(Arc::new(
        finance::tools::fundamentals::StockFundamentalsTool,
    ));
    registry.register(Arc::new(finance::tools::currency_rate::CurrencyRateTool));
    registry.register(Arc::new(
        finance::tools::currency_history::CurrencyHistoryTool,
    ));
    registry.register(Arc::new(finance::tools::search::StockSearchTool));
    registry.register(Arc::new(finance::tools::options::StockOptionsTool));
    registry.register(Arc::new(
        finance::tools::recommendations::StockRecommendationsTool,
    ));

    registry
}
