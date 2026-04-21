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

/// AIWiki export tool for agents.
pub mod aiwiki_export;
/// AIWiki import tool for agents.
pub mod aiwiki_import;
pub mod aiwiki_ingest;
/// AIWiki search tool for agents.
pub mod aiwiki_search;
/// AIWiki status tool for agents.
pub mod aiwiki_status;
/// Alias tools that map commonly hallucinated tool names to canonical implementations.
pub mod aliases;
/// File append tool.
pub mod append_file;
/// Shell command execution tool.
pub mod bash;
/// Persistent shell state reset tool.
pub mod bash_reset;
/// Math expression calculator tool.
pub mod calculator;
/// Task cancellation tool.
pub mod cancel_task;
/// Codebase index dependency graph tool.
pub mod codeindex_dependencies;
/// Codebase index reference lookup tool.
pub mod codeindex_references;
/// Codebase index re-index trigger tool.
pub mod codeindex_reindex;
/// Codebase index full-text search tool.
pub mod codeindex_search;
/// Codebase index status tool.
pub mod codeindex_status;
/// Codebase index symbol query tool.
pub mod codeindex_symbols;
/// File copy tool.
pub mod copy_file;
/// File creation tool.
pub mod create;
/// File diff tool.
pub mod diff;
/// File editing tool.
pub mod edit;
/// Python code execution tool.
pub mod execute_python;
/// File metadata / info tool.
pub mod file_info;
/// Per-file locking for concurrent edit operations.
mod file_lock;
/// Concurrent file operations tool (batch read/write).
pub mod file_ops_tool;
/// Environment variable read tool.
pub mod get_env;
pub mod github_issues;
pub mod github_prs;
/// GitLab issue tools (list, get, create, comment, close).
pub mod gitlab_issues;
/// GitLab merge request tools (list, get, create, merge, approve).
pub mod gitlab_mrs;
/// GitLab CI/CD pipeline and job tools.
pub mod gitlab_pipelines;
/// File globbing tool.
pub mod glob;
pub mod grep;
/// Full HTTP client tool.
pub mod http_request;
/// Journal write, search, and read tools.
pub mod journal;
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
/// Memory block migration tool.
pub mod memory_migrate;
/// Memory block replace tool.
pub mod memory_replace;
/// Semantic memory search tool (embeddings + FTS5).
pub mod memory_search;
pub mod memory_write;
/// Directory creation tool.
pub mod mkdir;
/// File move / rename tool.
pub mod move_file;
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
pub mod search;
/// Claude-compatible multi-command file editor.
pub mod str_replace_editor;
/// Structured memory store, recall, and forget tools.
pub mod structured_memory;
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

/// Content formatting utilities for standardized tool output.
pub mod format;
/// Metadata builder for consistent tool output metadata.
pub mod metadata;
/// Content truncation utilities for managing large tool outputs.
pub mod truncate;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tokio::task::JoinHandle;

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

use crate::event::{Event, EventBus};
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
///     code_index: None,
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
    /// Optional code index for codebase search and symbol lookup.
    /// `None` when code indexing is disabled or not yet initialised.
    pub code_index: Option<Arc<ragent_codeindex::CodeIndex>>,
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

struct ExtractedCoreToolAdapter {
    inner: Arc<dyn ragent_tools_core::Tool>,
}

impl ExtractedCoreToolAdapter {
    fn new(inner: Arc<dyn ragent_tools_core::Tool>) -> Self {
        Self { inner }
    }
}

fn convert_extracted_event(event: ragent_tools_core::event::Event) -> Option<Event> {
    match event {
        ragent_tools_core::event::Event::ReasoningDelta { session_id, text } => {
            Some(Event::ReasoningDelta { session_id, text })
        }
        ragent_tools_core::event::Event::TaskCompleted {
            session_id,
            summary,
        } => Some(Event::TaskCompleted {
            session_id,
            summary,
        }),
        ragent_tools_core::event::Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
        } => Some(Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
        }),
        ragent_tools_core::event::Event::ShellCwdChanged { session_id, cwd } => {
            Some(Event::ShellCwdChanged { session_id, cwd })
        }
        _ => None,
    }
}

fn convert_core_event(event: Event) -> Option<ragent_tools_core::event::Event> {
    match event {
        Event::UserInput {
            session_id,
            request_id,
            response,
        } => Some(ragent_tools_core::event::Event::UserInput {
            session_id,
            request_id,
            response,
        }),
        _ => None,
    }
}

fn spawn_extracted_to_core_forwarder(
    tool_bus: Arc<ragent_tools_core::event::EventBus>,
    core_bus: Arc<EventBus>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = tool_bus.subscribe();
        while let Ok(event) = rx.recv().await {
            if let Some(core_event) = convert_extracted_event(event) {
                core_bus.publish(core_event);
            }
        }
    })
}

fn spawn_core_to_extracted_forwarder(
    core_bus: Arc<EventBus>,
    tool_bus: Arc<ragent_tools_core::event::EventBus>,
    session_id: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = core_bus.subscribe();
        while let Ok(event) = rx.recv().await {
            if event.session_id() != Some(session_id.as_str()) {
                continue;
            }
            if let Some(tool_event) = convert_core_event(event) {
                tool_bus.publish(tool_event);
            }
        }
    })
}

#[async_trait::async_trait]
impl Tool for ExtractedCoreToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn permission_category(&self) -> &str {
        self.inner.permission_category()
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let tool_bus = Arc::new(ragent_tools_core::event::EventBus::new(256));
        let forward_out =
            spawn_extracted_to_core_forwarder(tool_bus.clone(), ctx.event_bus.clone());
        let forward_in = spawn_core_to_extracted_forwarder(
            ctx.event_bus.clone(),
            tool_bus.clone(),
            ctx.session_id.clone(),
        );

        let tool_ctx = ragent_tools_core::ToolContext {
            session_id: ctx.session_id.clone(),
            working_dir: ctx.working_dir.clone(),
            event_bus: tool_bus,
        };

        let result = self
            .inner
            .execute(input, &tool_ctx)
            .await
            .map(|output| ToolOutput {
                content: output.content,
                metadata: output.metadata,
            });

        forward_out.abort();
        forward_in.abort();

        result
    }
}

fn register_extracted_core_tools(registry: &ToolRegistry) {
    let extracted = ragent_tools_core::create_core_registry();
    for name in extracted.list() {
        if let Some(tool) = extracted.get(&name) {
            registry.register(Arc::new(ExtractedCoreToolAdapter::new(tool)));
        }
    }
}

struct CoreStorageAdapter {
    inner: Arc<crate::storage::Storage>,
}

impl CoreStorageAdapter {
    fn new(inner: Arc<crate::storage::Storage>) -> Self {
        Self { inner }
    }
}

impl ragent_tools_extended::storage::StorageBackend for CoreStorageAdapter {
    fn get_todos(
        &self,
        session_id: &str,
        status: Option<&str>,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::TodoRow>> {
        self.inner.get_todos(session_id, status).map(|rows| {
            rows.into_iter()
                .map(|row| ragent_tools_extended::storage::TodoRow {
                    id: row.id,
                    session_id: row.session_id,
                    title: row.title,
                    status: row.status,
                    description: row.description,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                })
                .collect()
        })
    }

    fn create_todo(
        &self,
        id: &str,
        session_id: &str,
        title: &str,
        status: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        self.inner
            .create_todo(id, session_id, title, status, description)
    }

    fn update_todo(
        &self,
        id: &str,
        session_id: &str,
        title: Option<&str>,
        status: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<bool> {
        self.inner
            .update_todo(id, session_id, title, status, description)
    }

    fn delete_todo(&self, id: &str, session_id: &str) -> anyhow::Result<bool> {
        self.inner.delete_todo(id, session_id)
    }

    fn clear_todos(&self, session_id: &str) -> anyhow::Result<usize> {
        self.inner.clear_todos(session_id)
    }

    fn create_journal_entry(
        &self,
        id: &str,
        title: &str,
        content: &str,
        project: &str,
        session_id: &str,
        tags: &[String],
    ) -> anyhow::Result<()> {
        self.inner
            .create_journal_entry(id, title, content, project, session_id, tags)
    }

    fn get_journal_entry(
        &self,
        id: &str,
    ) -> anyhow::Result<Option<ragent_tools_extended::storage::JournalEntryRow>> {
        self.inner.get_journal_entry(id).map(|row| {
            row.map(|row| ragent_tools_extended::storage::JournalEntryRow {
                id: row.id,
                title: row.title,
                content: row.content,
                project: row.project,
                session_id: row.session_id,
                timestamp: row.timestamp,
                created_at: row.created_at,
            })
        })
    }

    fn search_journal_entries(
        &self,
        query: &str,
        tags: Option<&[String]>,
        limit: usize,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::JournalEntryRow>> {
        self.inner
            .search_journal_entries(query, tags, limit)
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ragent_tools_extended::storage::JournalEntryRow {
                        id: row.id,
                        title: row.title,
                        content: row.content,
                        project: row.project,
                        session_id: row.session_id,
                        timestamp: row.timestamp,
                        created_at: row.created_at,
                    })
                    .collect()
            })
    }

    fn get_journal_tags(&self, id: &str) -> anyhow::Result<Vec<String>> {
        self.inner.get_journal_tags(id)
    }

    fn get_memory(
        &self,
        id: i64,
    ) -> anyhow::Result<Option<ragent_tools_extended::storage::MemoryRow>> {
        self.inner.get_memory(id).map(|row| {
            row.map(|row| ragent_tools_extended::storage::MemoryRow {
                id: row.id,
                content: row.content,
                category: row.category,
                source: row.source,
                confidence: row.confidence,
                project: row.project,
                session_id: row.session_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
                access_count: row.access_count,
                last_accessed: row.last_accessed,
            })
        })
    }

    fn get_memory_tags(&self, id: i64) -> anyhow::Result<Vec<String>> {
        self.inner.get_memory_tags(id)
    }

    fn search_memories(
        &self,
        query: &str,
        category: Option<&str>,
        _source: Option<&str>,
        limit: usize,
        min_confidence: f64,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::MemoryRow>> {
        let categories = category.map(|value| vec![value.to_string()]);
        self.inner
            .search_memories(
                query,
                categories.as_ref().map(|values| values.as_slice()),
                None,
                limit,
                min_confidence,
            )
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ragent_tools_extended::storage::MemoryRow {
                        id: row.id,
                        content: row.content,
                        category: row.category,
                        source: row.source,
                        confidence: row.confidence,
                        project: row.project,
                        session_id: row.session_id,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                        access_count: row.access_count,
                        last_accessed: row.last_accessed,
                    })
                    .collect()
            })
    }

    fn list_memories(
        &self,
        project: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::MemoryRow>> {
        self.inner.list_memories(project, limit).map(|rows| {
            rows.into_iter()
                .map(|row| ragent_tools_extended::storage::MemoryRow {
                    id: row.id,
                    content: row.content,
                    category: row.category,
                    source: row.source,
                    confidence: row.confidence,
                    project: row.project,
                    session_id: row.session_id,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    access_count: row.access_count,
                    last_accessed: row.last_accessed,
                })
                .collect()
        })
    }

    fn store_memory_embedding(&self, id: i64, embedding_blob: &[u8]) -> anyhow::Result<bool> {
        self.inner.store_memory_embedding(id, embedding_blob)
    }

    fn list_memory_embeddings(&self) -> anyhow::Result<Vec<(i64, Vec<u8>)>> {
        self.inner.list_memory_embeddings()
    }

    fn search_memories_by_embedding(
        &self,
        query_embedding: &[f32],
        dimensions: usize,
        limit: usize,
        min_similarity: f32,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::EmbeddingMatch>> {
        self.inner
            .search_memories_by_embedding(query_embedding, dimensions, limit, min_similarity)
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ragent_tools_extended::storage::EmbeddingMatch {
                        row_id: row.row_id,
                        score: row.score,
                    })
                    .collect()
            })
    }
}

struct CoreLspAdapter {
    inner: crate::lsp::SharedLspManager,
}

impl CoreLspAdapter {
    fn new(inner: crate::lsp::SharedLspManager) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl ragent_tools_extended::lsp::LspBackend for CoreLspAdapter {
    async fn hover(
        &self,
        path: &std::path::Path,
        line: u32,
        column: u32,
    ) -> anyhow::Result<Option<lsp_types::Hover>> {
        let client = {
            let guard = self.inner.read().await;
            guard.client_for_path(path).with_context(|| {
                format!(
                    "No LSP server for '{}' files",
                    path.extension().and_then(|e| e.to_str()).unwrap_or("?")
                )
            })?
        };
        client
            .open_document(path)
            .await
            .with_context(|| format!("LSP: failed to open {}", path.display()))?;
        let uri = client.text_document_id(path)?;
        let params = lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: uri,
                position: lsp_types::Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
        };
        client
            .request("textDocument/hover", params)
            .await
            .context("LSP hover request failed")
    }

    async fn definition(
        &self,
        path: &std::path::Path,
        line: u32,
        column: u32,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        let client = {
            let guard = self.inner.read().await;
            guard.client_for_path(path).with_context(|| {
                format!(
                    "No LSP server for '{}' files",
                    path.extension().and_then(|e| e.to_str()).unwrap_or("?")
                )
            })?
        };
        client
            .open_document(path)
            .await
            .with_context(|| format!("LSP: failed to open {}", path.display()))?;
        let uri = client.text_document_id(path)?;
        let params = lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: uri,
                position: lsp_types::Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };
        let result: Option<lsp_types::GotoDefinitionResponse> = client
            .request("textDocument/definition", params)
            .await
            .context("LSP definition request failed")?;
        Ok(match result {
            None => vec![],
            Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => vec![loc],
            Some(lsp_types::GotoDefinitionResponse::Array(locs)) => locs,
            Some(lsp_types::GotoDefinitionResponse::Link(links)) => links
                .into_iter()
                .map(|link| lsp_types::Location {
                    uri: link.target_uri,
                    range: link.target_range,
                })
                .collect(),
        })
    }

    async fn references(
        &self,
        path: &std::path::Path,
        line: u32,
        column: u32,
        include_declaration: bool,
    ) -> anyhow::Result<Vec<lsp_types::Location>> {
        let client = {
            let guard = self.inner.read().await;
            guard.client_for_path(path).with_context(|| {
                format!(
                    "No LSP server for '{}' files",
                    path.extension().and_then(|e| e.to_str()).unwrap_or("?")
                )
            })?
        };
        client
            .open_document(path)
            .await
            .with_context(|| format!("LSP: failed to open {}", path.display()))?;
        let uri = client.text_document_id(path)?;
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: uri,
                position: lsp_types::Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
            context: lsp_types::ReferenceContext {
                include_declaration,
            },
        };
        let result: Option<Vec<lsp_types::Location>> = client
            .request("textDocument/references", params)
            .await
            .context("LSP references request failed")?;
        Ok(result.unwrap_or_default())
    }

    async fn document_symbols(
        &self,
        path: &std::path::Path,
    ) -> anyhow::Result<Option<lsp_types::DocumentSymbolResponse>> {
        let client = {
            let guard = self.inner.read().await;
            guard.client_for_path(path).with_context(|| {
                format!(
                    "No LSP server for '{}' files — check your ragent.json 'lsp' configuration",
                    path.extension().and_then(|e| e.to_str()).unwrap_or("?")
                )
            })?
        };
        client
            .open_document(path)
            .await
            .with_context(|| format!("LSP: failed to open {}", path.display()))?;
        let uri = client.text_document_id(path)?;
        let params = lsp_types::DocumentSymbolParams {
            text_document: uri,
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
        };
        client
            .request("textDocument/documentSymbol", params)
            .await
            .context("LSP documentSymbol request failed")
    }

    async fn diagnostics(
        &self,
        path: Option<&std::path::Path>,
    ) -> anyhow::Result<Vec<(String, Vec<lsp_types::Diagnostic>)>> {
        let guard = self.inner.read().await;
        Ok(guard.diagnostics_for(path).await)
    }
}

struct ExtractedExtendedToolAdapter {
    inner: Arc<dyn ragent_tools_extended::Tool>,
}

impl ExtractedExtendedToolAdapter {
    fn new(inner: Arc<dyn ragent_tools_extended::Tool>) -> Self {
        Self { inner }
    }
}

fn convert_extracted_extended_event(event: ragent_tools_extended::event::Event) -> Option<Event> {
    match event {
        ragent_tools_extended::event::Event::ReasoningDelta { session_id, text } => {
            Some(Event::ReasoningDelta { session_id, text })
        }
        ragent_tools_extended::event::Event::TaskCompleted {
            session_id,
            summary,
        } => Some(Event::TaskCompleted {
            session_id,
            summary,
        }),
        ragent_tools_extended::event::Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
        } => Some(Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
        }),
        ragent_tools_extended::event::Event::ShellCwdChanged { session_id, cwd } => {
            Some(Event::ShellCwdChanged { session_id, cwd })
        }
        ragent_tools_extended::event::Event::JournalEntryCreated {
            session_id,
            id,
            title,
        } => Some(Event::JournalEntryCreated {
            session_id,
            id,
            title,
        }),
        ragent_tools_extended::event::Event::JournalSearched {
            session_id,
            query,
            result_count,
        } => Some(Event::JournalSearched {
            session_id,
            query,
            result_count,
        }),
        ragent_tools_extended::event::Event::MemorySearched {
            session_id,
            query,
            result_count,
            mode,
        } => Some(Event::MemorySearched {
            session_id,
            query,
            result_count,
            mode,
        }),
        _ => None,
    }
}

fn convert_core_event_to_extended(event: Event) -> Option<ragent_tools_extended::event::Event> {
    match event {
        Event::UserInput {
            session_id,
            request_id,
            response,
        } => Some(ragent_tools_extended::event::Event::UserInput {
            session_id,
            request_id,
            response,
        }),
        _ => None,
    }
}

fn spawn_extracted_extended_to_core_forwarder(
    tool_bus: Arc<ragent_tools_extended::event::EventBus>,
    core_bus: Arc<EventBus>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = tool_bus.subscribe();
        while let Ok(event) = rx.recv().await {
            if let Some(core_event) = convert_extracted_extended_event(event) {
                core_bus.publish(core_event);
            }
        }
    })
}

fn spawn_core_to_extracted_extended_forwarder(
    core_bus: Arc<EventBus>,
    tool_bus: Arc<ragent_tools_extended::event::EventBus>,
    session_id: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = core_bus.subscribe();
        while let Ok(event) = rx.recv().await {
            if event.session_id() != Some(session_id.as_str()) {
                continue;
            }
            if let Some(tool_event) = convert_core_event_to_extended(event) {
                tool_bus.publish(tool_event);
            }
        }
    })
}

#[async_trait::async_trait]
impl Tool for ExtractedExtendedToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn permission_category(&self) -> &str {
        self.inner.permission_category()
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let tool_bus = Arc::new(ragent_tools_extended::event::EventBus::new(256));
        let forward_out =
            spawn_extracted_extended_to_core_forwarder(tool_bus.clone(), ctx.event_bus.clone());
        let forward_in = spawn_core_to_extracted_extended_forwarder(
            ctx.event_bus.clone(),
            tool_bus.clone(),
            ctx.session_id.clone(),
        );

        let storage_adapter = ctx.storage.as_ref().map(
            |storage| -> Arc<dyn ragent_tools_extended::storage::StorageBackend> {
                Arc::new(CoreStorageAdapter::new(storage.clone()))
            },
        );
        let lsp_adapter = ctx.lsp_manager.as_ref().map(
            |manager| -> Arc<dyn ragent_tools_extended::lsp::LspBackend> {
                Arc::new(CoreLspAdapter::new(manager.clone()))
            },
        );

        let tool_ctx = ragent_tools_extended::ToolContext {
            session_id: ctx.session_id.clone(),
            working_dir: ctx.working_dir.clone(),
            event_bus: tool_bus,
            storage: storage_adapter,
            code_index: ctx.code_index.clone(),
            lsp_backend: lsp_adapter,
        };

        let result = self
            .inner
            .execute(input, &tool_ctx)
            .await
            .map(|output| ToolOutput {
                content: output.content,
                metadata: output.metadata,
            });

        forward_out.abort();
        forward_in.abort();

        result
    }
}

fn register_extracted_extended_tools(registry: &ToolRegistry) {
    let extracted = ragent_tools_extended::create_extended_registry();
    for name in extracted.definitions().into_iter().map(|tool| tool.name) {
        if let Some(tool) = extracted.get(&name) {
            registry.register(Arc::new(ExtractedExtendedToolAdapter::new(tool)));
        }
    }
}

struct CoreVcsStorageAdapter {
    inner: Arc<crate::storage::Storage>,
}

impl CoreVcsStorageAdapter {
    fn new(inner: Arc<crate::storage::Storage>) -> Self {
        Self { inner }
    }
}

impl ragent_tools_vcs::storage::StorageBackend for CoreVcsStorageAdapter {
    fn get_provider_auth(&self, provider_id: &str) -> anyhow::Result<Option<String>> {
        self.inner.get_provider_auth(provider_id)
    }

    fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> anyhow::Result<()> {
        self.inner.set_provider_auth(provider_id, api_key)
    }

    fn delete_provider_auth(&self, provider_id: &str) -> anyhow::Result<()> {
        self.inner.delete_provider_auth(provider_id)
    }

    fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        self.inner.get_setting(key)
    }

    fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.inner.set_setting(key, value)
    }

    fn delete_setting(&self, key: &str) -> anyhow::Result<()> {
        self.inner.delete_setting(key)
    }
}

impl ragent_tools_vcs::storage::StorageBackend for crate::storage::Storage {
    fn get_provider_auth(&self, provider_id: &str) -> anyhow::Result<Option<String>> {
        self.get_provider_auth(provider_id)
    }

    fn set_provider_auth(&self, provider_id: &str, api_key: &str) -> anyhow::Result<()> {
        self.set_provider_auth(provider_id, api_key)
    }

    fn delete_provider_auth(&self, provider_id: &str) -> anyhow::Result<()> {
        self.delete_provider_auth(provider_id)
    }

    fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        self.get_setting(key)
    }

    fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.set_setting(key, value)
    }

    fn delete_setting(&self, key: &str) -> anyhow::Result<()> {
        self.delete_setting(key)
    }
}

struct ExtractedVcsToolAdapter {
    inner: Arc<dyn ragent_tools_vcs::Tool>,
}

impl ExtractedVcsToolAdapter {
    fn new(inner: Arc<dyn ragent_tools_vcs::Tool>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl Tool for ExtractedVcsToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn permission_category(&self) -> &str {
        self.inner.permission_category()
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage_adapter = ctx.storage.as_ref().map(
            |storage| -> Arc<dyn ragent_tools_vcs::storage::StorageBackend> {
                Arc::new(CoreVcsStorageAdapter::new(storage.clone()))
            },
        );

        let tool_ctx = ragent_tools_vcs::ToolContext {
            session_id: ctx.session_id.clone(),
            working_dir: ctx.working_dir.clone(),
            storage: storage_adapter,
        };

        self.inner
            .execute(input, &tool_ctx)
            .await
            .map(|output| ToolOutput {
                content: output.content,
                metadata: output.metadata,
            })
    }
}

fn register_extracted_vcs_tools(registry: &ToolRegistry) {
    let extracted = ragent_tools_vcs::registry::create_vcs_registry();
    for name in extracted.list() {
        if let Some(tool) = extracted.get(&name) {
            registry.register(Arc::new(ExtractedVcsToolAdapter::new(tool)));
        }
    }
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
    /// Tool names that are hidden from LLM tool definitions and system-prompt listings.
    hidden: RwLock<HashSet<String>>,
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            hidden: RwLock::new(HashSet::new()),
        }
    }

    /// Marks the given tool names as hidden so they are excluded from
    /// [`definitions`](Self::definitions) and the system-prompt tool listing.
    /// Hidden tools remain registered and can still be executed if the LLM
    /// happens to call them by name; they are simply not advertised.
    ///
    /// Call this once after constructing the registry, before the first session.
    pub fn set_hidden(&self, names: &[String]) {
        let mut hidden = self.hidden.write().expect("tool hidden lock poisoned");
        *hidden = names.iter().cloned().collect();
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
        let hidden = self.hidden.read().expect("tool hidden lock poisoned");
        let mut defs: Vec<ToolDefinition> = tools
            .values()
            .filter(|t| !hidden.contains(t.name()))
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
#[must_use]
pub fn create_default_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();
    register_extracted_core_tools(&registry);
    register_extracted_extended_tools(&registry);
    register_extracted_vcs_tools(&registry);
    registry.register(Arc::new(plan::PlanEnterTool));
    registry.register(Arc::new(plan::PlanExitTool));
    registry.register(Arc::new(new_task::NewTaskTool));
    registry.register(Arc::new(cancel_task::CancelTaskTool));
    registry.register(Arc::new(list_tasks::ListTasksTool));
    registry.register(Arc::new(wait_tasks::WaitTasksTool));
    // Structured memory tools
    registry.register(Arc::new(structured_memory::MemoryStoreTool));
    registry.register(Arc::new(structured_memory::MemoryRecallTool));
    registry.register(Arc::new(structured_memory::MemoryForgetTool));
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
    registry.register(Arc::new(team_task_create::TeamTaskCreateTool));
    registry.register(Arc::new(team_task_list::TeamTaskListTool));
    registry.register(Arc::new(team_wait::TeamWaitTool));
    // Phase 3 — code execution & Claude-compatible editor
    registry.register(Arc::new(execute_python::ExecutePythonTool));
    // Phase 1 — alias layer (commonly hallucinated tool names)
    registry.register(Arc::new(aliases::ViewFileTool));
    registry.register(Arc::new(aliases::ReadFileTool));
    registry.register(Arc::new(aliases::GetFileContentsTool));
    registry.register(Arc::new(aliases::ListFilesTool));
    registry.register(Arc::new(aliases::ListDirectoryTool));
    registry.register(Arc::new(aliases::FindFilesTool));
    registry.register(Arc::new(aliases::SearchInRepoTool));
    registry.register(Arc::new(aliases::FileSearchTool));
    registry.register(Arc::new(aliases::ReplaceInFileTool));
    registry.register(Arc::new(aliases::UpdateFileTool));
    registry.register(Arc::new(aliases::RunShellCommandTool));
    registry.register(Arc::new(aliases::RunTerminalCmdTool));
    registry.register(Arc::new(aliases::ExecuteBashTool));
    registry.register(Arc::new(aliases::ExecuteCodeTool));
    registry.register(Arc::new(aliases::RunCodeTool));
    registry.register(Arc::new(aliases::AskUserTool));
    registry.register(Arc::new(aliases::OpenFileTool));
    registry
}
