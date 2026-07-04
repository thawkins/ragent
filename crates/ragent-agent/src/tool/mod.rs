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

/// Alias tools that map commonly hallucinated tool names to canonical implementations.
pub mod aliases;
/// Task cancellation tool.
pub mod cancel_task;
pub mod github_issues;
pub mod github_prs;
/// GitLab issue tools (list, get, create, comment, close).
pub mod gitlab_issues;
/// GitLab merge request tools (list, get, create, merge, approve).
pub mod gitlab_mrs;
/// GitLab CI/CD pipeline and job tools.
pub mod gitlab_pipelines;
pub mod list_tasks;
pub mod new_task;
pub mod plan;
/// Structured memory store, recall, and forget tools.
pub mod structured_memory;
/// Team coordination tools (create, spawn, message, tasks, etc.).
///
/// These tool implementations live in `crates/ragent-team/src/tools/` and are
/// compiled into `ragent-agent` via `#[path]` includes, exactly like the
/// team *runtime* modules in `crate::team`. This keeps a single source of
/// truth for the team tools so fixes no longer have to be applied twice.
/// See `docs/team-unification-decision.md` and
/// `scripts/check-team-duplication.sh`.
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
pub mod wait_tasks;

/// Spec management tools.
pub mod spec_coverage;
pub mod spec_list;
pub mod spec_read;
pub mod spec_search;
pub mod spec_task_update;

/// Metadata builder for consistent tool output metadata.
pub mod metadata;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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

/// Async interface for spawning and coordinating teammate sessions.
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

    /// Request shutdown of a teammate by agent ID.
    ///
    /// This is the **single unified shutdown path** used by both the
    /// `team_shutdown_teammate` tool and the `TeamManager` itself (M3-T5/T6).
    ///
    /// When `graceful` is `true`, the teammate is marked `ShuttingDown`, a
    /// `ShutdownRequest` mailbox message is pushed, and the member is left
    /// running so it can call `team_shutdown_ack` and terminate cleanly.
    ///
    /// When `graceful` is `false` (immediate), the agent-loop cancel flags are
    /// set, the mailbox notifier is deregistered, a `ShutdownRequest` is still
    /// pushed (so a lingering loop sees it), and the member is marked
    /// `Stopped` on disk.
    ///
    /// Returns `Ok(())` on success. Errors from mailbox or store I/O are
    /// propagated. Returns `Ok(())` even if the agent ID is not currently
    /// registered as a live handle (the on-disk status is still updated).
    async fn shutdown_teammate(&self, agent_id: &str, graceful: bool) -> anyhow::Result<()>;

    /// Returns the lead session id for this team, if known.
    ///
    /// PERF-017 / PERF-018: tools that previously loaded `TeamStore` from
    /// disk just to read `config.lead_session_id` can call this instead and
    /// avoid the file read when a `TeamManager` is wired into the
    /// `ToolContext`. Returns `None` when no manager is available (e.g. in
    /// tests that construct a `ToolContext` without a team manager).
    fn lead_session_id(&self) -> Option<&str>;
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
/// use ragent_agent::tool::ToolContext;
/// use ragent_agent::event::EventBus;
/// use std::sync::Arc;
/// use std::path::PathBuf;
///
/// let ctx = ToolContext {
///     session_id: "session-1".to_string(),
///     working_dir: PathBuf::from("/tmp"),
///     event_bus: Arc::new(EventBus::new(128)),
///     storage: None,
///     task_manager: None,
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
    /// Optional spec manager for reading and updating specifications.
    /// `None` when no specs/ directory is configured.
    pub spec_manager: Option<Arc<ragent_specs::SpecManager>>,
    /// Currently active spec ID for automatic task status updates.
    pub active_spec_id: Option<String>,
    /// Optional ragent configuration loaded from config files.
    /// Provides tools access to settings like API keys, permissions, etc.
    pub config: Option<Arc<ragent_config::Config>>,
    /// Read timestamps (mtime in milliseconds since UNIX epoch) for files that
    /// have been read by this session. Used by edit tools to detect stale-file
    /// edits (editrenewal FR-003).
    pub read_timestamps: Arc<std::sync::RwLock<std::collections::HashMap<PathBuf, u64>>>,
    /// PERF-019: cache for the most recently resolved team directory.
    ///
    /// Team tools call [`find_team_dir`] on every `execute()`, and that
    /// function walks up the directory tree calling `stat()` on every
    /// parent. Within a single `process_user_message` turn the team
    /// directory never moves, so the first resolution is cached here and
    /// reused by [`find_team_dir_cached`]. The cache is keyed by team name
    /// so switching teams (rare) simply overwrites the entry.
    pub cached_team_dir: Arc<std::sync::Mutex<Option<(String, PathBuf)>>>,
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

/// PERF-030: extracted-tool adapter that owns a per-adapter event bus so
/// the bus and its forwarder tasks are created **once** (lazily on the
/// first call) and reused across all subsequent calls, rather than being
/// allocated and spawned per `execute()` invocation.
///
/// Previously every `execute()` call did `Arc::new(EventBus::new(256))`,
/// spawned two forwarder tasks, and aborted them at the end — paying an
/// event-bus allocation + two task spawns + two channel subscriptions per
/// tool call. With this wrapper the bus lives for the lifetime of the
/// adapter and the forwarders are long-lived.
///
/// The forwarders connect the adapter's bus to a **session** event bus
/// that is supplied per call via the `ToolContext`. Because the session
/// changes per turn, we re-bind the forwarders' destination bus on each
/// call: the long-lived part is the adapter-local bus + the forwarder
/// tasks, while the routing target is swapped by passing the current
/// `ctx.event_bus` into `execute`.
/// Adapter that wraps a `ragent_tools_core::Tool` and exposes it as an
/// agent-local [`Tool`]. The runtime registry uses this to register the core
/// tool implementations under the agent's `Tool` trait; it is also reused by
/// `aliases.rs` to delegate alias calls (`update_file`→`write`, `run_code`→
/// `bash`) to the single source-of-truth implementations in
/// `ragent-tools-core` (see DCREMOVALPLAN M3).
pub(crate) struct ExtractedCoreToolAdapter {
    inner: Arc<dyn ragent_tools_core::Tool>,
    /// PERF-030: lazily-created per-adapter event bus + forwarder handles.
    /// Stored as `OnceLock` so the first `execute()` call bootstraps them
    /// and all later calls reuse the same bus.
    bus: std::sync::OnceLock<ExtractedCoreBus>,
}

/// PERF-030: the long-lived adapter event bus plus the forwarder task
/// handles that connect it to a session bus. The forwarders are spawned
/// once and live for the lifetime of the adapter; on each `execute()` the
/// caller passes the current session `EventBus`, which we route events
/// to/from.
struct ExtractedCoreBus {
    bus: Arc<ragent_tools_core::event::EventBus>,
}

impl ExtractedCoreToolAdapter {
    pub(crate) fn new(inner: Arc<dyn ragent_tools_core::Tool>) -> Self {
        Self {
            inner,
            bus: std::sync::OnceLock::new(),
        }
    }

    /// PERF-030: return the per-adapter bus, creating it on first use.
    fn bus(&self) -> &ExtractedCoreBus {
        self.bus.get_or_init(|| ExtractedCoreBus {
            bus: Arc::new(ragent_tools_core::event::EventBus::new(256)),
        })
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
            options,
        } => Some(Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
            options,
        }),
        ragent_tools_core::event::Event::ShellCwdChanged { session_id, cwd } => {
            Some(Event::ShellCwdChanged { session_id, cwd })
        }
        ragent_tools_core::event::Event::QuestionRequested {
            session_id,
            request_id,
            question,
            options,
        } => Some(Event::QuestionRequested {
            session_id,
            request_id,
            question,
            options,
        }),
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
        Event::QuestionAnswered {
            session_id,
            request_id,
            response,
        } => Some(ragent_tools_core::event::Event::QuestionAnswered {
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
        // PERF-030: reuse the per-adapter event bus instead of allocating a
        // fresh one (and spawning two forwarder tasks) on every call. The
        // forwarders are spawned per-call because the destination
        // (`ctx.event_bus`) is the *current session* bus, which changes
        // across turns; the expensive part (the bus itself + its channel)
        // is reused.
        let tool_bus = self.bus().bus.clone();
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
            read_timestamps: ctx.read_timestamps.clone(),
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

/// Legacy alias that exposes the core `multi_edit` tool under the old
/// `multiedit` name (editrenewal FR-012).
///
/// Wraps an [`ExtractedCoreToolAdapter`] and overrides [`Tool::name`] to
/// return `"multiedit"`. On [`Tool::execute`] it normalises legacy
/// parameter names (`path`/`old_str`/`new_str` → `file_path`/`old_string`/
/// `new_string`) inside each edit object before delegating to the inner
/// adapter, so existing agent prompts continue to work during the
/// deprecation window.
struct LegacyMultiEditAlias {
    inner: ExtractedCoreToolAdapter,
}

impl LegacyMultiEditAlias {
    /// Wrap the core `multi_edit` tool (already wrapped in an
    /// [`ExtractedCoreToolAdapter`]) so it can be registered under the
    /// legacy `multiedit` name.
    fn new(core_tool: Arc<dyn ragent_tools_core::Tool>) -> Self {
        Self {
            inner: ExtractedCoreToolAdapter::new(core_tool),
        }
    }

    /// Normalise legacy parameter names inside each edit object of an
    /// `edits` array. Canonical names (`file_path`/`old_string`/
    /// `new_string`) are left untouched; legacy names (`path`/`old_str`/
    /// `new_str`) are copied into their canonical slots when the canonical
    /// slot is absent.
    fn normalise_legacy_params(input: Value) -> Value {
        let mut input = input;
        let Some(edits) = input.get_mut("edits").and_then(|e| e.as_array_mut()) else {
            return input;
        };
        for edit in edits {
            if edit.get("file_path").is_none() {
                if let Some(path) = edit.get("path").cloned() {
                    edit["file_path"] = path;
                }
            }
            if edit.get("old_string").is_none() {
                if let Some(old) = edit.get("old_str").cloned() {
                    edit["old_string"] = old;
                }
            }
            if edit.get("new_string").is_none() {
                if let Some(new) = edit.get("new_str").cloned() {
                    edit["new_string"] = new;
                }
            }
        }
        input
    }
}

#[async_trait::async_trait]
impl Tool for LegacyMultiEditAlias {
    fn name(&self) -> &'static str {
        "multiedit"
    }

    fn description(&self) -> &'static str {
        "Deprecated alias for 'multi_edit'. Apply multiple edits to one or more \
           files atomically. Prefer 'multi_edit' with file_path/old_string/new_string; \
           this alias also accepts the legacy path/old_str/new_str parameter names."
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn permission_category(&self) -> &str {
        self.inner.permission_category()
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let normalised = Self::normalise_legacy_params(input);
        let mut output = self.inner.execute(normalised, ctx).await?;
        // editrenewal FR-012: emit a deprecation warning whenever the legacy
        // `multiedit` tool name is used, directing callers to `multi_edit`.
        let metadata = output.metadata.get_or_insert_with(|| json!({}));
        if let Some(obj) = metadata.as_object_mut() {
            obj.insert(
                "deprecation_warning".to_string(),
                json!(
                    "The 'multiedit' tool name is deprecated. Use 'multi_edit' \
                           with file_path/old_string/new_string parameters instead."
                ),
            );
        }
        Ok(output)
    }
}

fn register_extracted_core_tools(registry: &ToolRegistry) {
    let extracted = ragent_tools_core::create_core_registry();
    for name in extracted.list() {
        if let Some(tool) = extracted.get(&name) {
            registry.register(Arc::new(ExtractedCoreToolAdapter::new(tool)));
        }
    }
    // editrenewal FR-012 — legacy `multiedit` alias for the renamed
    // `multi_edit` tool. The alias forwards calls to the same core
    // `MultiEditTool` (now registered as `multi_edit` above) and normalises
    // legacy parameter names (path/old_str/new_str → file_path/old_string/
    // new_string) so existing agent prompts keep working during the
    // deprecation window.
    if let Some(multi_edit) = extracted.get("multi_edit") {
        registry.register(Arc::new(LegacyMultiEditAlias::new(multi_edit)));
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
            .search_memories(query, categories.as_deref(), None, limit, min_confidence)
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
        // `Storage::search_memories_by_embedding` now lives in
        // `ragent-storage` and takes a caller-supplied cosine-similarity
        // closure so it does not need to depend on the embedding helpers
        // (which live in `ragent-tools-extended`).
        use ragent_tools_extended::memory::embedding::cosine_similarity as storage_cosine;
        self.inner
            .search_memories_by_embedding(
                query_embedding,
                dimensions,
                limit,
                min_similarity,
                storage_cosine,
            )
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

struct ExtractedExtendedToolAdapter {
    inner: Arc<dyn ragent_tools_extended::Tool>,
    /// PERF-030: lazily-created per-adapter event bus so the bus + its
    /// channel are allocated once and reused across all `execute()` calls
    /// rather than being re-allocated per call.
    bus: std::sync::OnceLock<Arc<ragent_tools_extended::event::EventBus>>,
}

impl ExtractedExtendedToolAdapter {
    fn new(inner: Arc<dyn ragent_tools_extended::Tool>) -> Self {
        Self {
            inner,
            bus: std::sync::OnceLock::new(),
        }
    }

    /// PERF-030: return the per-adapter bus, creating it on first use.
    fn bus(&self) -> Arc<ragent_tools_extended::event::EventBus> {
        self.bus
            .get_or_init(|| Arc::new(ragent_tools_extended::event::EventBus::new(256)))
            .clone()
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
            options,
        } => Some(Event::PermissionRequested {
            session_id,
            request_id,
            permission,
            description,
            options,
        }),
        ragent_tools_extended::event::Event::ShellCwdChanged { session_id, cwd } => {
            Some(Event::ShellCwdChanged { session_id, cwd })
        }
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
        // PERF-030: reuse the per-adapter event bus (allocated once on the
        // first call) instead of allocating a fresh one + spawning two
        // forwarder tasks on every call. The forwarders are still spawned
        // per-call because their destination is the current session bus.
        let tool_bus = self.bus();
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

        let tool_ctx = ragent_tools_extended::ToolContext {
            session_id: ctx.session_id.clone(),
            working_dir: ctx.working_dir.clone(),
            event_bus: tool_bus,
            storage: storage_adapter,
            code_index: ctx.code_index.clone(),
            config: ctx.config.clone(),
            read_timestamps: ctx.read_timestamps.clone(),
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
            config: ctx.config.clone(),
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
    /// PERF-012: monotonic version counter bumped on every `register()` /
    /// `set_hidden()` call.  Caches (the system-prompt component cache,
    /// `cached_tool_definitions`, `estimate_request_bytes`) compare this
    /// version against the version they were last built at to decide
    /// whether to invalidate in O(1), instead of re-hashing all ~111 tool
    /// definitions on every step.
    version: std::sync::atomic::AtomicU64,
    /// PERF-012: cached sorted [`ToolDefinition`] list. `None` means the cache
    /// is stale and must be rebuilt by [`definitions`](Self::definitions);
    /// `Some(vec)` is reused until the next `register()` / `set_hidden()`
    /// invalidation. This converts the per-call O(n log n) sort into a
    /// one-time cost amortised across every step of the agent loop.
    definitions_cache: RwLock<Option<Vec<ToolDefinition>>>,
}

impl ToolRegistry {
    /// Creates an empty tool registry.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_agent::tool::ToolRegistry;
    ///
    /// let registry = ToolRegistry::new();
    /// assert!(registry.list().is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            hidden: RwLock::new(HashSet::new()),
            version: std::sync::atomic::AtomicU64::new(0),
            definitions_cache: RwLock::new(None),
        }
    }

    /// PERF-012: invalidate the `definitions` cache. Called by `register()`
    /// and `set_hidden()` so the next `definitions()` call rebuilds the
    /// sorted `Vec<ToolDefinition>` from scratch.
    fn invalidate_definitions_cache(&self) {
        let mut guard = self
            .definitions_cache
            .write()
            .expect("definitions cache lock poisoned");
        *guard = None;
        self.version
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// PERF-012: return the current registry version. Caches compare this
    /// against the version they stored at build time to decide whether the
    /// tool set has changed.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version.load(std::sync::atomic::Ordering::Relaxed)
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
        // PERF-012: the hidden set participates in `definitions()` filtering,
        // so a change here must invalidate the sorted-definition cache.
        drop(hidden);
        self.invalidate_definitions_cache();
    }

    /// Registers a tool, keyed by its [`Tool::name`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_agent::tool::{ToolRegistry, read::ReadTool};
    /// use std::sync::Arc;
    ///
    /// let registry = ToolRegistry::new();
    /// registry.register(Arc::new(ReadTool));
    /// assert_eq!(registry.list().len(), 1);
    /// ```
    pub fn register(&self, tool: Arc<dyn Tool>) {
        let mut tools = self.tools.write().expect("tool registry lock poisoned");
        tools.insert(tool.name().to_string(), tool);
        // PERF-012: invalidate the sorted-definition cache so the next
        // `definitions()` call rebuilds it with the newly registered tool.
        drop(tools);
        self.invalidate_definitions_cache();
    }

    /// Looks up a tool by name, returning a shared reference if found.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_agent::tool::create_default_registry;
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
    /// use ragent_agent::tool::create_default_registry;
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
    /// use ragent_agent::tool::create_default_registry;
    ///
    /// let registry = create_default_registry();
    /// let defs = registry.definitions();
    /// assert!(!defs.is_empty());
    /// assert!(defs.windows(2).all(|w| w[0].name <= w[1].name));
    /// ```
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        // PERF-012: serve the sorted-definition cache when it is valid.
        // The cache is invalidated (set to `None`) by `register()` and
        // `set_hidden()`, so a steady-state agent loop reuses the same
        // sorted `Vec` across every step instead of paying an O(n log n)
        // sort on every uncached call.
        {
            let guard = self
                .definitions_cache
                .read()
                .expect("definitions cache lock poisoned");
            if let Some(ref cached) = *guard {
                return cached.clone();
            }
        }
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
        // Populate the cache so subsequent calls skip the sort.
        {
            let mut guard = self
                .definitions_cache
                .write()
                .expect("definitions cache lock poisoned");
            *guard = Some(defs.clone());
        }
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
/// use ragent_agent::tool::create_default_registry;
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
    // Spec management tools
    registry.register(Arc::new(spec_read::SpecReadTool));
    registry.register(Arc::new(spec_list::SpecListTool));
    registry.register(Arc::new(spec_search::SpecSearchTool));
    registry.register(Arc::new(spec_task_update::SpecTaskUpdateTool));
    registry.register(Arc::new(spec_coverage::SpecCoverageTool));
    // Phase 1 — alias layer (commonly hallucinated tool names)
    registry.register(Arc::new(aliases::UpdateFileTool));
    registry.register(Arc::new(aliases::AskUserTool));
    registry
}
