//! Extended document, web, memory, code index, and LSP tools for ragent.
//!
//! This crate owns the Milestone 5 extracted tool set while keeping a small
//! compatibility surface for the extracted runtime crates.

pub mod aiwiki_export;
pub mod aiwiki_import;
pub mod aiwiki_ingest;
pub mod aiwiki_search;
pub mod aiwiki_status;
pub mod codeindex_dependencies;
pub mod codeindex_references;
pub mod codeindex_reindex;
pub mod codeindex_search;
pub mod codeindex_status;
pub mod codeindex_symbols;
pub mod http_request;
pub mod journal;
pub mod libreoffice_common;
pub mod libreoffice_info;
pub mod libreoffice_read;
pub mod libreoffice_write;
pub mod lsp_definition;
pub mod lsp_diagnostics;
pub mod lsp_hover;
pub mod lsp_references;
pub mod lsp_symbols;
pub mod memory_migrate;
pub mod memory_replace;
pub mod memory_search;
pub mod memory_write;
pub mod office_common;
pub mod office_info;
pub mod office_read;
pub mod office_write;
pub mod pdf_read;
pub mod pdf_write;
pub mod todo;
pub mod webfetch;
pub mod websearch;

pub mod memory {
    //! Memory helpers reused by the extracted memory and journal tools.

    pub mod block;
    pub mod cross_project;
    pub mod embedding;
    pub mod journal;
    pub mod migrate;
    pub mod storage;
}

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

/// Compatibility re-export for moved helpers that still reference `crate::config`.
pub mod config {
    pub use ragent_config::CrossProjectConfig;
}

/// Storage adapter types for the extracted tools.
pub mod storage {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};

    /// Row representation of a TODO item.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TodoRow {
        pub id: String,
        pub session_id: String,
        pub title: String,
        pub status: String,
        pub description: String,
        pub created_at: String,
        pub updated_at: String,
    }

    /// Row representation of a journal entry.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct JournalEntryRow {
        pub id: String,
        pub title: String,
        pub content: String,
        pub project: String,
        pub session_id: String,
        pub timestamp: String,
        pub created_at: String,
    }

    /// Row representation of a structured memory.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MemoryRow {
        pub id: i64,
        pub content: String,
        pub category: String,
        pub source: String,
        pub confidence: f64,
        pub project: String,
        pub session_id: String,
        pub created_at: String,
        pub updated_at: String,
        pub access_count: i64,
        pub last_accessed: Option<String>,
    }

    /// Result row for embedding-based memory search.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct EmbeddingMatch {
        pub row_id: i64,
        pub score: f32,
    }

    /// Storage backend abstraction used by session-scoped tools.
    pub trait StorageBackend: Send + Sync {
        fn get_todos(&self, session_id: &str, status: Option<&str>) -> Result<Vec<TodoRow>>;
        fn create_todo(
            &self,
            id: &str,
            session_id: &str,
            title: &str,
            status: &str,
            description: &str,
        ) -> Result<()>;
        fn update_todo(
            &self,
            id: &str,
            session_id: &str,
            title: Option<&str>,
            status: Option<&str>,
            description: Option<&str>,
        ) -> Result<bool>;
        fn delete_todo(&self, id: &str, session_id: &str) -> Result<bool>;
        fn clear_todos(&self, session_id: &str) -> Result<usize>;

        fn create_journal_entry(
            &self,
            id: &str,
            title: &str,
            content: &str,
            project: &str,
            session_id: &str,
            tags: &[String],
        ) -> Result<()>;
        fn get_journal_entry(&self, id: &str) -> Result<Option<JournalEntryRow>>;
        fn search_journal_entries(
            &self,
            query: &str,
            tags: Option<&[String]>,
            limit: usize,
        ) -> Result<Vec<JournalEntryRow>>;
        fn get_journal_tags(&self, id: &str) -> Result<Vec<String>>;

        fn get_memory(&self, id: i64) -> Result<Option<MemoryRow>>;
        fn get_memory_tags(&self, id: i64) -> Result<Vec<String>>;
        fn search_memories(
            &self,
            query: &str,
            category: Option<&str>,
            source: Option<&str>,
            limit: usize,
            min_confidence: f64,
        ) -> Result<Vec<MemoryRow>>;
        fn list_memories(&self, project: &str, limit: usize) -> Result<Vec<MemoryRow>>;
        fn store_memory_embedding(&self, id: i64, embedding_blob: &[u8]) -> Result<bool>;
        fn list_memory_embeddings(&self) -> Result<Vec<(i64, Vec<u8>)>>;
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

/// LSP adapter used by the extracted tools so they stay independent of the runtime crate.
pub mod lsp {
    use anyhow::Result;
    use lsp_types::{Diagnostic, DocumentSymbolResponse, Hover, Location};
    use std::path::Path;

    /// Backend interface for LSP-backed tools.
    #[async_trait::async_trait]
    pub trait LspBackend: Send + Sync {
        async fn hover(&self, path: &Path, line: u32, column: u32) -> Result<Option<Hover>>;
        async fn definition(&self, path: &Path, line: u32, column: u32) -> Result<Vec<Location>>;
        async fn references(
            &self,
            path: &Path,
            line: u32,
            column: u32,
            include_declaration: bool,
        ) -> Result<Vec<Location>>;
        async fn document_symbols(&self, path: &Path) -> Result<Option<DocumentSymbolResponse>>;
        async fn diagnostics(&self, path: Option<&Path>) -> Result<Vec<(String, Vec<Diagnostic>)>>;
    }
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
    pub event_bus: Arc<EventBus>,
    pub storage: Option<Arc<dyn storage::StorageBackend>>,
    pub code_index: Option<Arc<ragent_codeindex::CodeIndex>>,
    pub lsp_backend: Option<Arc<dyn lsp::LspBackend>>,
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
    pub fn contains(&self, name: &str) -> bool {
        self.tools
            .read()
            .expect("tool registry lock poisoned")
            .contains_key(name)
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
    registry.register(Arc::new(memory_write::MemoryWriteTool));
    registry.register(Arc::new(memory_write::MemoryReadTool));
    registry.register(Arc::new(memory_replace::MemoryReplaceTool));
    registry.register(Arc::new(memory_search::MemorySearchTool));
    registry.register(Arc::new(memory_migrate::MemoryMigrateTool));
    registry.register(Arc::new(journal::JournalWriteTool));
    registry.register(Arc::new(journal::JournalSearchTool));
    registry.register(Arc::new(journal::JournalReadTool));
    registry.register(Arc::new(todo::TodoReadTool));
    registry.register(Arc::new(todo::TodoWriteTool));
    registry.register(Arc::new(aiwiki_search::AiwikiSearchTool));
    registry.register(Arc::new(aiwiki_status::AiwikiStatusTool));
    registry.register(Arc::new(aiwiki_ingest::AiwikiIngestTool));
    registry.register(Arc::new(aiwiki_export::AiwikiExportTool));
    registry.register(Arc::new(aiwiki_import::AiwikiImportTool));
    registry.register(Arc::new(codeindex_search::CodeIndexSearchTool));
    registry.register(Arc::new(codeindex_status::CodeIndexStatusTool));
    registry.register(Arc::new(codeindex_symbols::CodeIndexSymbolsTool));
    registry.register(Arc::new(codeindex_references::CodeIndexReferencesTool));
    registry.register(Arc::new(codeindex_dependencies::CodeIndexDependenciesTool));
    registry.register(Arc::new(codeindex_reindex::CodeIndexReindexTool));
    registry.register(Arc::new(lsp_hover::LspHoverTool));
    registry.register(Arc::new(lsp_definition::LspDefinitionTool));
    registry.register(Arc::new(lsp_references::LspReferencesTool));
    registry.register(Arc::new(lsp_symbols::LspSymbolsTool));
    registry.register(Arc::new(lsp_diagnostics::LspDiagnosticsTool));

    registry
}
