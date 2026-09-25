//! Session, team, and miscellaneous operations for the TUI.
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LockResult, MutexGuard};

use ratatui::layout::Rect;

use ragent_agent::{
    event::Event,
    mcp::discovery::DiscoveredMcpServer,
    message::{Message, MessagePart, Role},
    session::processor::estimate_tool_definition_bytes,
};
use ragent_llm::provider::tool_cache::{ToolFormat, cached_tools};
use ragent_plugins::{
    FetchLimits, StoreDirs, StoreError, StoreIndex, StoreIndexFetcher, StoreKind, add,
    add_error_report, add_report, installed_ids, probe_stores, render_stores_report_with_probes,
    store_and_config,
};
use ragent_team::team::TeamStore;
use ragent_tools_core::{Tool, ToolContext};

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{
    App, ContextAction, ContextPartitionSnapshot, FileMenuEntry, FileMenuState, LlmRequestStat,
    LlmStatsSummary, LogEntry, LogLevel, OutputViewState, OutputViewTarget, PluginStoreBrowser,
    PluginStoreFetchResult, PluginStoreInstallResult, ProviderSetupStep, QueuedInput, ScreenMode,
    ScrollbarDragPane, SelectionPane, TextSelection, atomic_config_update, is_image_path,
    percent_decode_path, save_clipboard_image_to_temp,
};

// Helpers
use crate::app::helpers::{MentionSpan, short_session_id};

// Re-export status types from theme
use crate::theme::{StatusCategory, StatusMessage};

/// Recover from a poisoned mutex, logging the incident and returning the
/// guarded value so the caller can keep running.
// reason: only consumed inside this crate (session_ops) - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn recover_poisoned<'a, T>(
    result: LockResult<MutexGuard<'a, T>>,
    name: &str,
) -> MutexGuard<'a, T> {
    match result {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::error!("{name} mutex poisoned, recovering");
            poisoned.into_inner()
        }
    }
}

/// Run one plugin-store install through the existing `add` entry point and map
/// its result to a TUI report (spec `pluginstores` T-009; FR-006, FR-024,
/// FR-025).
///
/// The store-supplied `source` is forwarded verbatim: `add(force = false)`
/// applies its own HTTPS, traversal, and size guards, so a store entry receives
/// no elevated trust (FR-024). The success report names the installed id and
/// dialect (FR-006); every [`AddError`] becomes the `[err]` report naming the
/// cause (FR-025). Never panics.
fn run_plugin_store_install(
    dirs: &StoreDirs,
    workdir: &std::path::Path,
    kind: StoreKind,
    id: &str,
    source: &str,
) -> PluginStoreInstallResult {
    match add(dirs, workdir, source, false) {
        Ok(outcome) => PluginStoreInstallResult {
            kind,
            id: outcome.parsed.descriptor.id.clone(),
            notice: format!("installed {}", outcome.parsed.descriptor.id),
            report: add_report(&outcome),
            succeeded: true,
        },
        Err(err) => PluginStoreInstallResult {
            kind,
            id: id.to_string(),
            notice: format!("install failed: {err}"),
            report: add_error_report(&err),
            succeeded: false,
        },
    }
}

/// Disk/SQLite-backed context partitions computed off the UI thread
/// (T-013/FR-015).
///
/// Performs exactly the I/O-bound work behind the Context panel's system
/// prompt, skills, memory and AGENTS.md partitions: skill-registry loading,
/// AGENTS.md discovery, structured-memory reads and the system-prompt
/// assembly. Runs on `tokio::task::spawn_blocking` from
/// [`App::schedule_context_snapshot_refresh`]; the synchronous
/// [`App::context_partition_snapshot`] fallback uses the same helper so the
/// two paths can never disagree.
fn compute_disk_context_partitions(
    working_dir: &std::path::Path,
    agent: &ragent_agent::agent::AgentInfo,
    storage: &ragent_agent::storage::Storage,
    config: &ragent_agent::Config,
    session_processor: &ragent_agent::session::processor::SessionProcessor,
) -> DiskContextPartitions {
    // C-001: the skill registry is mtime-cached on the processor, so this
    // only touches the skills directories when they change.
    let skills = session_processor.skill_registry(working_dir, &config.skill_dirs);
    let memory_config = config.memory.clone();
    let prompt = ragent_agent::agent::build_system_prompt_with_storage_and_memory_and_config(
        agent,
        working_dir,
        "",
        skills.as_ref(),
        None,
        None,
        None,
        Some(storage),
        Some(&memory_config),
        None,
        Some(config),
    );
    let (agents_md, _) = ragent_agent::agent::collect_agents_md_content_with_discovery(working_dir);
    let memory = ragent_agent::agent::build_memory_prompt_section(
        working_dir,
        Some(storage),
        Some(&config.memory),
    );
    let skills_section = skills
        .as_ref()
        .map(|registry| ragent_agent::agent::skills_prompt_section(registry, agent))
        .unwrap_or_default();
    DiskContextPartitions {
        system_prompt: bytes_to_tokens(prompt.len()),
        skills: bytes_to_tokens(skills_section.len()),
        memory: bytes_to_tokens(memory.len()),
        agents_md: bytes_to_tokens(agents_md.len()),
    }
}

/// The disk/SQLite-backed subset of context partitions returned by
/// [`compute_disk_context_partitions`].
struct DiskContextPartitions {
    system_prompt: u64,
    skills: u64,
    memory: u64,
    agents_md: u64,
}

impl DiskContextPartitions {
    fn into_snapshot(
        self,
        tool_catalog_tokens: u64,
        tool_metadata_tokens: u64,
        history_tokens: u64,
        history_message_count: usize,
        context_window_tokens: Option<usize>,
        last_input_tokens: u64,
    ) -> ContextPartitionSnapshot {
        ContextPartitionSnapshot {
            system_prompt_tokens: self.system_prompt,
            tool_catalog_tokens,
            tool_metadata_tokens,
            history_tokens,
            history_message_count,
            skills_tokens: self.skills,
            memory_tokens: self.memory,
            agents_md_tokens: self.agents_md,
            context_window_tokens,
            last_input_tokens,
        }
    }
}

/// Approximate token count from a UTF-8 byte length (contextpanel FR-005..FR-012).
///
/// The context panel's partition estimates are byte-based (serialised JSON
/// lengths of the system prompt, tool catalog, wire envelope and history).
/// They are converted to token estimates with the standard ~4-bytes-per-token
/// heuristic for mixed English/code/JSON content so the panel percentages are
/// directly comparable with the provider-reported prompt tokens the status
/// bar's `ctx:` indicator shows from the last `TokenUsage` event.
const BYTES_PER_TOKEN: u64 = 4;

#[inline]
fn bytes_to_tokens(bytes: usize) -> u64 {
    bytes as u64 / BYTES_PER_TOKEN
}

impl App {
    /// Clone the current agent info, apply the selected model/thinking settings,
    /// and inject the role-mode system prompt addition when active.
    pub(crate) fn prepare_agent_for_dispatch(&self) -> ragent_agent::agent::AgentInfo {
        let mut agent = self.agent_info.clone();
        self.apply_selected_model_and_thinking(&mut agent);
        if let Some(ref mode) = self.role_mode {
            let addition = mode.system_prompt_addition();
            if !addition.is_empty() {
                let existing = agent.prompt.clone().unwrap_or_default();
                agent.prompt = Some(Arc::from(format!("{existing}\n\n{addition}")));
            }
        }
        agent
    }

    pub(crate) fn ollama_cloud_api_key(&self) -> Option<String> {
        self.storage
            .get_provider_auth("ollama_cloud")
            .ok()
            .flatten()
            .filter(|k| !k.is_empty())
            .or_else(|| {
                std::env::var("OLLAMA_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
    }

    /// Compute the token size of the visible toolset catalog.
    ///
    /// FR-006: returns the token estimate for every tool currently exposed
    /// to the model (name + description + parameter schema), using the same
    /// shared estimator as the agent request-size path so the panel stays in
    /// sync with what the LLM actually receives. The estimator yields a byte
    /// count which is converted to tokens via [`BYTES_PER_TOKEN`] because
    /// exact tokenisation depends on the active provider.
    pub fn tool_catalog_token_count(&self) -> u64 {
        let defs = self.session_processor.tool_registry.definitions();
        bytes_to_tokens(estimate_tool_definition_bytes(&defs) as usize)
    }

    /// Return the provider id of the model that will serve the next request.
    ///
    /// Mirrors the resolution order used by
    /// [`App::apply_selected_model_and_thinking`]: the user's selected model
    /// wins, then the agent's pinned/unpinned model, then the first provider
    /// in the registry that advertises a default model.
    fn active_provider_id(&self) -> Option<String> {
        if let Some(model_str) = self.selected_model.as_deref()
            && let Some((provider, _)) = model_str.split_once('/')
        {
            return Some(provider.to_string());
        }
        if let Some(model) = &self.agent_info.model {
            return Some(model.provider_id.clone());
        }
        ragent_agent::agent::resolve_default_model(&self.agent_info, &self.provider_registry)
            .map(|m| m.provider_id)
    }

    /// Resolve the LLM tool wire format for a provider id.
    ///
    /// Providers that share an OpenAI-compatible envelope (including the
    /// router, which forwards to a concrete backend) map to
    /// [`ToolFormat::OpenAi`]; Anthropic, Gemini, Bedrock and HuggingFace use
    /// their own shapes.
    fn tool_format_for_provider(provider_id: &str) -> ToolFormat {
        match provider_id {
            "anthropic" => ToolFormat::Anthropic,
            "gemini" => ToolFormat::Gemini,
            "bedrock" => ToolFormat::Bedrock,
            "huggingface" => ToolFormat::HuggingFace,
            _ => ToolFormat::OpenAi,
        }
    }

    /// Compute the token size of the toolset metadata/wrapper overhead.
    ///
    /// FR-007: measures the extra per-tool bytes the provider wire envelope
    /// adds beyond the raw tool definitions — JSON envelope keys (e.g.
    /// `"type":"function","input_schema":`), wrapper objects, and list
    /// separators. The estimate is derived from the same shared provider tool
    /// serialisation cache the LLM clients use (`ragent_llm`'s `tool_cache`),
    /// so it always reflects what is actually sent on the wire for the active
    /// provider's format. Computed as the serialised wire byte length minus
    /// the raw definition bytes from [`App::tool_catalog_token_count`],
    /// saturating at zero; converted to tokens via [`BYTES_PER_TOKEN`] like
    /// the rest of the context panel.
    pub fn tool_metadata_token_count(&self) -> u64 {
        let defs = self.session_processor.tool_registry.definitions();
        let catalog_bytes = estimate_tool_definition_bytes(&defs);
        let format = Self::tool_format_for_provider(
            self.active_provider_id()
                .unwrap_or_else(|| "openai".into())
                .as_str(),
        );
        let wire_bytes = cached_tools(format, &defs).byte_len as u64;
        bytes_to_tokens(wire_bytes.saturating_sub(catalog_bytes) as usize)
    }

    /// Compute the token size of the conversation history held in the active
    /// session.
    ///
    /// FR-008: sums the same per-message byte accounting used by the agent
    /// request-size estimator (`estimate_request_bytes_with_tool_bytes`):
    /// role label length + a fixed ~40-byte per-message JSON overhead plus
    /// each content part — text, reasoning, tool-call identifier/input and
    /// tool-result output. The byte total is converted with the
    /// [`BYTES_PER_TOKEN`] heuristic so the estimate is comparable with
    /// provider-reported prompt tokens.
    pub fn conversation_history_token_count(&self) -> u64 {
        bytes_to_tokens(
            self.messages
                .iter()
                .map(|msg| {
                    let content_len: usize = msg
                        .parts
                        .iter()
                        .map(|part| match part {
                            MessagePart::Text { text } => text.len(),
                            MessagePart::Reasoning { text } => text.len(),
                            MessagePart::ToolCall { call_id, state, .. } => {
                                call_id.len()
                                    + state.input.to_string().len()
                                    + state.output.as_ref().map_or(0, |v| v.to_string().len())
                                    + state.error.as_ref().map_or(0, String::len)
                            }
                            MessagePart::Image(_) => 0,
                        })
                        .sum();
                    msg.role.to_string().len() + content_len + 40
                })
                .sum::<usize>(),
        )
    }

    /// Return the number of messages currently held in the active session's
    /// conversation history (FR-008).
    pub fn conversation_message_count(&self) -> usize {
        self.messages.len()
    }

    /// Compute the token size of the assembled system prompt.
    ///
    /// FR-005: returns an estimated token size of the system prompt the agent
    /// loop would assemble for the next turn — base agent prompt, project
    /// context (working directory, AGENTS.md, git status, README), memory
    /// injections and the skills catalog. The file tree is intentionally
    /// omitted (empty) because it is rendered from the TUI's own cached
    /// snapshot and the builder only appends a non-empty tree; the estimate
    /// therefore tracks the stable prompt core. The assembled prompt's byte
    /// length is converted to tokens via [`BYTES_PER_TOKEN`].
    pub fn system_prompt_token_count(&self) -> u64 {
        let working_dir = crate::app::helpers::current_working_dir();
        let config = self.current_config();
        let memory_config = config.memory.clone();
        let skills = self
            .session_processor
            .skill_registry(&working_dir, &config.skill_dirs);
        let agent = self.prepare_agent_for_dispatch();
        let prompt = ragent_agent::agent::build_system_prompt_with_storage_and_memory_and_config(
            &agent,
            &working_dir,
            "",
            skills.as_ref(),
            None,
            None,
            None,
            Some(&self.storage),
            Some(&memory_config),
            None,
            Some(&config),
        );
        bytes_to_tokens(prompt.len())
    }

    /// Compute the token size of the project-guideline (`AGENTS.md`) block
    /// the system prompt carries.
    ///
    /// FR-009: this is a sub-partition of the assembled system prompt (see
    /// [`App::system_prompt_token_count`]), surfaced separately so the panel
    /// can show where the prompt's weight comes from. Uses the same
    /// discovery-and-load precedence as the prompt builder.
    pub fn agents_md_token_count(&self) -> u64 {
        let working_dir = crate::app::helpers::current_working_dir();
        let (content, _) =
            ragent_agent::agent::collect_agents_md_content_with_discovery(&working_dir);
        bytes_to_tokens(content.len())
    }

    /// Compute the token size of the memory injections the system prompt
    /// carries (structured memories, MEMORY.md blocks, project analysis).
    ///
    /// FR-009: sub-partition of the assembled system prompt; the same
    /// section builder the agent loop passes through `spawn_blocking` is
    /// measured here synchronously.
    pub fn memory_injection_token_count(&self) -> u64 {
        let working_dir = crate::app::helpers::current_working_dir();
        let config = self.current_config();
        bytes_to_tokens(
            ragent_agent::agent::build_memory_prompt_section(
                &working_dir,
                Some(&self.storage),
                Some(&config.memory),
            )
            .len(),
        )
    }

    /// Compute the token size of the skills context injected into the system
    /// prompt.
    ///
    /// FR-009: sub-partition of the assembled system prompt, measured via the
    /// shared [`ragent_agent::agent::skills_prompt_section`] formatter so it
    /// can never drift from what the model actually receives. Zero when no
    /// agent-invocable skills resolve for the active agent.
    pub fn skills_token_count(&self) -> u64 {
        let working_dir = crate::app::helpers::current_working_dir();
        let config = self.current_config();
        let Some(registry) = self
            .session_processor
            .skill_registry(&working_dir, &config.skill_dirs)
        else {
            return 0;
        };
        let agent = self.agent_info.clone();
        bytes_to_tokens(ragent_agent::agent::skills_prompt_section(&registry, &agent).len())
    }

    /// Return the context-window capacity (tokens) of the model that will
    /// serve the next request.
    ///
    /// FR-010/FR-011: resolves the active model the same way as
    /// [`App::active_provider_id`] (selected model, then agent model, then
    /// the first registry default) and queries the provider registry's
    /// static catalog for its context window. Falls back to the cached
    /// context window recorded during model selection for dynamically
    /// discovered models that the registry does not list. Returns `None`
    /// when the provider does not advertise a limit — the panel shows
    /// "unknown" percentages rather than guessing (FR-011).
    pub fn active_context_window_tokens(&self) -> Option<usize> {
        if let Some(model_str) = self.selected_model.as_deref()
            && let Some((provider_id, model_id)) = model_str.split_once('/')
            && let Some(window) = self
                .provider_registry
                .resolve_model(provider_id, model_id)
                .map(|m| m.context_window)
                .filter(|w| *w > 0)
        {
            return Some(window);
        }
        if let Some(model) = &self.agent_info.model
            && let Some(window) = self
                .provider_registry
                .resolve_model(&model.provider_id, &model.model_id)
                .map(|m| m.context_window)
                .filter(|w| *w > 0)
        {
            return Some(window);
        }
        self.selected_model_ctx_window.filter(|w| *w > 0)
    }

    /// Collect a snapshot of every context partition for the Context panel.
    ///
    /// FR-012: gathers all top-level partitions (system prompt, tool catalog,
    /// tool metadata wrapper, conversation history), the sub-partition
    /// breakdown of the system prompt (skills, memory, AGENTS.md) and the
    /// model's context-window capacity (FR-010/FR-011) in one call so the
    /// panel renders from a consistent instant.
    ///
    /// T-013/FR-015: disk- and SQLite-backed partitions (system prompt build,
    /// AGENTS.md discovery, memory section, skill registry) make this
    /// unsuitable for per-frame renders on the UI thread. The panel render
    /// path uses the cached snapshot from [`App::context_snapshot_cache`]
    /// instead; this method is the synchronous fallback (first frame) and the
    /// reference implementation the blocking refresh task mirrors via
    /// [`compute_disk_context_partitions`].
    pub fn context_partition_snapshot(&self) -> ContextPartitionSnapshot {
        let working_dir = crate::app::helpers::current_working_dir();
        let config = self.current_config();
        // Pre-compute the UI-thread-safe partitions (registry caches only)
        // before handing the disk/SQLite-bound work to the shared helper.
        let history_tokens = self.conversation_history_token_count();
        let history_message_count = self.conversation_message_count();
        let tool_catalog_tokens = self.tool_catalog_token_count();
        let tool_metadata_tokens = self.tool_metadata_token_count();
        let context_window_tokens = self.active_context_window_tokens();
        let last_input_tokens = self.last_input_tokens;

        let disk = compute_disk_context_partitions(
            &working_dir,
            &self.prepare_agent_for_dispatch(),
            &self.storage,
            &config,
            &self.session_processor,
        );
        disk.into_snapshot(
            tool_catalog_tokens,
            tool_metadata_tokens,
            history_tokens,
            history_message_count,
            context_window_tokens,
            last_input_tokens,
        )
    }

    /// Schedule a background refresh of the Context panel snapshot.
    ///
    /// T-013/FR-015: the disk- and SQLite-bound partition computations never
    /// run on the UI thread. This spawns [`compute_disk_context_partitions`]
    /// on `tokio::task::spawn_blocking`, combining its result with the
    /// UI-thread-safe partitions captured here. Calls are coalesced: while a
    /// refresh is in flight, further scheduling is a no-op. The finished
    /// snapshot is deposited into [`App::context_snapshot_result`] and
    /// adopted by [`App::poll_context_snapshot_refresh`] on a later frame.
    pub fn schedule_context_snapshot_refresh(&mut self) {
        // Only schedule when the panel is actually open: the snapshot exists
        // purely for the panel render, and skipping while closed also keeps
        // non-TUI code paths that poll without a tokio reactor (unit tests,
        // headless flows) from reaching `spawn_blocking` needlessly.
        if self.context_refresh_inflight || !self.show_context_panel {
            return;
        }
        self.context_refresh_inflight = true;

        let working_dir = crate::app::helpers::current_working_dir();
        let agent = self.prepare_agent_for_dispatch();
        let storage = Arc::clone(&self.storage);
        let session_processor = Arc::clone(&self.session_processor);
        // Cheap, cache-backed partitions captured on the UI thread.
        let history_tokens = self.conversation_history_token_count();
        let history_message_count = self.conversation_message_count();
        let tool_catalog_tokens = self.tool_catalog_token_count();
        let tool_metadata_tokens = self.tool_metadata_token_count();
        let context_window_tokens = self.active_context_window_tokens();
        let last_input_tokens = self.last_input_tokens;
        let result_slot = Arc::clone(&self.context_snapshot_result);

        tokio::task::spawn_blocking(move || {
            let config = match ragent_agent::Config::load() {
                Ok(cfg) => cfg,
                Err(_) => ragent_agent::Config::default(),
            };
            let disk = compute_disk_context_partitions(
                &working_dir,
                &agent,
                &storage,
                &config,
                &session_processor,
            );
            let snapshot = disk.into_snapshot(
                tool_catalog_tokens,
                tool_metadata_tokens,
                history_tokens,
                history_message_count,
                context_window_tokens,
                last_input_tokens,
            );
            if let Ok(mut guard) = result_slot.lock() {
                *guard = Some(snapshot);
            } else {
                tracing::error!("context_snapshot_result mutex poisoned, snapshot dropped");
            }
        });
    }

    /// Adopt a completed background context snapshot, if one has landed.
    ///
    /// T-013/FR-015: called from the TUI main loop every frame. Drains
    /// [`App::context_snapshot_result`], stores the snapshot in the render
    /// cache, clears the in-flight latch and flags a redraw.
    ///
    /// Stale-snapshot guard: when the conversation history size has changed
    /// since the snapshot was scheduled (e.g. `/compact` replaced `messages`
    /// while the blocking task was running, or `load_session` resumed a
    /// different session), the snapshot reflects the pre-mutation state and
    /// would corrupt the Context panel. We drop it and immediately schedule a
    /// fresh refresh so the panel tracks the current history without waiting
    /// for the next event.
    pub fn poll_context_snapshot_refresh(&mut self) {
        let deposited = {
            let mut guard = recover_poisoned(
                self.context_snapshot_result.lock(),
                "context_snapshot_result",
            );
            guard.take()
        };
        let Some(snapshot) = deposited else {
            return;
        };
        if snapshot.history_message_count != self.conversation_message_count() {
            tracing::debug!(
                scheduled = snapshot.history_message_count,
                current = self.conversation_message_count(),
                "dropping stale context snapshot — history changed mid-refresh"
            );
            self.context_refresh_inflight = false;
            self.schedule_context_snapshot_refresh();
            return;
        }
        self.context_snapshot_cache = Some(snapshot);
        self.context_refresh_inflight = false;
        self.needs_redraw = true;
    }

    /// Snapshot for the Context panel render: the cached background result
    /// when available, else the synchronous computation as a first-frame
    /// fallback (the async refresh then replaces it with identical values).
    pub fn context_effective_snapshot(&self) -> ContextPartitionSnapshot {
        if let Some(snapshot) = &self.context_snapshot_cache {
            return *snapshot;
        }
        self.context_partition_snapshot()
    }

    pub(crate) fn should_auto_compact_before_send(&self) -> bool {
        if self.auto_compact_in_progress
            || self.auto_compact_failed
            || self.pending_send_after_compact.is_some()
        {
            return false;
        }
        if self.session_id.is_none() || self.messages.is_empty() || self.last_input_tokens == 0 {
            return false;
        }
        let Some(context_window) = self.selected_model_context_window() else {
            return false;
        };

        // Use the shared estimator logic so the TUI pre-send check matches the
        // agent/server path. The threshold is a fraction of the model's context
        // window (default 70%), which makes the trigger independent of the
        // model's absolute context size.
        let threshold = ragent_agent::compaction::compaction_threshold(
            context_window,
            0,
            self.current_config().compaction.buffer,
            self.current_config().compaction.threshold,
        ) as u64;
        self.last_input_tokens > threshold
    }

    /// Start manual compaction (the `/compact` / `/compress` slash commands)
    /// and warn when it cannot start (no active session or no messages).
    /// Shared so both command arms log identically.
    pub(crate) fn start_compaction_or_warn(&mut self) {
        if !self.start_compaction(false) {
            tracing::warn!("compaction did not start (no active session or no messages)");
        }
    }

    /// Begin compaction of the active session's history. `auto_triggered`
    /// marks the threshold-driven path, which blocks the next send until the
    /// compacted history is applied.
    pub(crate) fn start_compaction(&mut self, auto_triggered: bool) -> bool {
        let Some(sid) = self.session_id.clone() else {
            self.status = "[warn] No active session to compact".to_string();
            return false;
        };
        if self.messages.is_empty() {
            self.status = "[warn] No messages to compact".to_string();
            return false;
        }
        self.start_provider_compaction_for_session(&sid, auto_triggered)
    }

    /// Run a shell command (input started with `!`) and render its output in
    /// the chat panel, then dispatch the output to the model for review.
    ///
    /// The command is executed through [`ragent_tools_core::bash::BashTool`]
    /// so it passes through the same 7-layer security model (safe-command
    /// whitelist, banned/denied patterns, directory-escape prevention, syntax
    /// validation, obfuscation detection, and user allow/deny lists) and the
    /// default 120-second timeout used by the regular `bash` tool. This keeps
    /// bang commands (`! ...`) from bypassing ragent's shell protections.
    pub(crate) fn dispatch_bang_command(&mut self, raw: String) {
        // Strip the leading `!` and trim surrounding whitespace.
        let command = raw.strip_prefix('!').unwrap_or(&raw).trim().to_string();
        if command.is_empty() {
            self.status = "[warn] Empty shell command".to_string();
            return;
        }

        self.auto_compact_failed = false;
        let Some(sid) = self.session_id.clone() else {
            self.status = "[warn] No active session".to_string();
            return;
        };

        // Show the command in the chat as a user message.
        let display_text = format!("$ {command}");
        let msg = Message::user_text(&sid, &display_text);
        self.messages.push(msg);
        self.last_prompt = raw.clone();
        // T-010/FR-013: user message added; refresh the Context panel so the
        // history message count stays current while the panel is open.
        self.schedule_context_snapshot_refresh();
        self.add_to_history(raw.clone());
        self.input.clear();
        self.input_cursor = 0;
        self.file_menu = None;
        self.set_status_working("running command");
        self.stream_in_bytes = 0;
        self.stream_out_bytes = 0;
        // R-10: trim messages to bound memory.
        self.trim_messages_if_needed();

        self.push_log_no_agent(LogLevel::Info, format!("bang command: {command}"));

        let agent = self.prepare_agent_for_dispatch();
        let command_for_prompt = command.clone();
        let processor = self.session_processor.clone();
        let event_bus = self.event_bus.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());

        // Build a minimal tool context so the validated `bash` tool can run
        // the command with the same security and timeout as a normal tool call.
        let working_dir = crate::app::helpers::current_working_dir();
        let tool_ctx = ToolContext {
            session_id: sid.clone(),
            working_dir: working_dir.clone(),
            event_bus: event_bus.clone(),
            read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
            allowed_roots: vec![working_dir], // Default to working_dir for bang commands
        };

        tokio::spawn(async move {
            let input = serde_json::json!({"command": command, "timeout": 120});
            let combined = match ragent_tools_core::bash::BashTool
                .execute(input, &tool_ctx)
                .await
            {
                Ok(out) => out.content,
                Err(e) => format!("failed to execute command: {e}"),
            };

            // Render the shell command output in the chat panel so the user
            // can see what the command produced before the model reviews it.
            event_bus.publish(Event::AgentNotice {
                session_id: sid.clone(),
                message: format!("```\n{combined}\n```"),
            });

            let prompt = ragent_agent::bang_command::build_bang_command_prompt(
                &command_for_prompt,
                &combined,
            );

            if let Err(e) = processor.process_message(&sid, &prompt, &agent, flag).await {
                tracing::debug!(error = %e, "Failed to process bang command output");
            }
        });
    }

    pub(crate) fn dispatch_user_message(
        &mut self,
        text: String,
        image_paths: Vec<std::path::PathBuf>,
    ) {
        self.auto_compact_failed = false;
        let Some(sid) = self.session_id.clone() else {
            self.status = "[warn] No active session".to_string();
            return;
        };

        let display_text = if image_paths.is_empty() {
            text.clone()
        } else {
            let names: Vec<String> = image_paths
                .iter()
                .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
                .collect();
            format!("[attach {}] {}", names.join(", "), text)
        };
        let msg = Message::user_text(&sid, &display_text);
        self.messages.push(msg);
        self.last_prompt = text.clone();
        // T-010/FR-013: user message added; refresh the Context panel so the
        // history message count stays current while the panel is open.
        self.schedule_context_snapshot_refresh();
        self.add_to_history(text.clone());
        self.input.clear();
        self.input_cursor = 0;
        self.file_menu = None;
        self.set_status_working("processing");
        self.stream_in_bytes = 0;
        self.stream_out_bytes = 0;
        // R-10: trim messages to bound memory.
        self.trim_messages_if_needed();
        let refs = ragent_agent::reference::parse::parse_refs(&text);
        let has_refs = !refs.is_empty();
        if has_refs {
            let ref_names: Vec<String> = refs.iter().map(|r| r.raw.clone()).collect();
            self.push_log_no_agent(
                LogLevel::Info,
                format!("resolving refs: {}", ref_names.join(", ")),
            );
        }

        let truncated = crate::app::helpers::truncate_to_char_boundary(&text, 120);
        let model_tag = if let Some(ref model_str) = self.selected_model {
            format!(" [{}]", model_str)
        } else {
            String::new()
        };
        self.push_log_no_agent(
            LogLevel::Info,
            format!("prompt sent{}: {}", model_tag, truncated),
        );

        let agent = self.prepare_agent_for_dispatch();

        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        tokio::spawn(async move {
            let final_text = if has_refs {
                let wd = crate::app::helpers::current_working_dir();
                match ragent_agent::reference::resolve::resolve_all_refs(&text, &wd).await {
                    Ok((resolved, _)) => resolved,
                    Err(e) => {
                        tracing::warn!(error = %e, "ref resolution failed, using original text");
                        text.clone()
                    }
                }
            } else {
                text.clone()
            };

            if image_paths.is_empty() {
                if let Err(e) = processor
                    .process_message(&sid, &final_text, &agent, flag)
                    .await
                {
                    tracing::debug!(error = %e, "Failed to process message");
                }
            } else {
                let mut parts: Vec<ragent_agent::message::MessagePart> = image_paths
                    .into_iter()
                    .filter(|p| p.exists())
                    .map(|p| {
                        let mime = if p
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("png"))
                            .unwrap_or(false)
                        {
                            "image/png"
                        } else if p
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("gif"))
                            .unwrap_or(false)
                        {
                            "image/gif"
                        } else {
                            "image/jpeg"
                        };
                        ragent_agent::message::MessagePart::Image(Box::new(
                            ragent_agent::message::ImageData {
                                mime_type: mime.to_string(),
                                path: p,
                            },
                        ))
                    })
                    .collect();
                parts.push(ragent_agent::message::MessagePart::Text { text: final_text });
                let user_msg = ragent_agent::message::Message::new(
                    &sid,
                    ragent_agent::message::Role::User,
                    parts,
                );
                if let Err(e) = processor
                    .process_user_message(&sid, user_msg, &agent, flag)
                    .await
                {
                    tracing::debug!(error = %e, "Failed to process message with images");
                }
            }
        });
    }

    /// Set the status line to an informational message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_info(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Info.format(&msg);
        self.status_history.push(StatusMessage::info(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to a success message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_success(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Success.format(&msg);
        self.status_history.push(StatusMessage::success(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to a warning message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_warning(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Warning.format(&msg);
        self.status_history.push(StatusMessage::warning(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to an error message and record it in history.
    #[allow(dead_code)]
    pub(crate) fn set_status_error(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Error.format(&msg);
        self.status_history.push(StatusMessage::error(msg));
        self.needs_redraw = true;
    }

    /// Set the status line to a working/in-progress message and record it in history.
    pub(crate) fn set_status_working(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.status = StatusCategory::Working.format(&msg);
        self.status_history.push(StatusMessage::working(msg));
        self.needs_redraw = true;
    }

    /// Return whether user input is currently blocked (agent is processing,
    /// compression is running, or a post-compact send is pending).
    ///
    /// This is the "busy" gate: while a turn is executing a plain message is
    /// still accepted (FR-002) but slash commands, bang commands, and
    /// teammate-targeted messages are refused (FR-017).
    pub(crate) fn is_input_blocked(&self) -> bool {
        self.is_processing
            || self.compact_in_progress
            || self.auto_compact_in_progress
            || self.pending_send_after_compact.is_some()
    }

    /// Return whether the input field is genuinely locked, as opposed to merely
    /// busy because the primary agent is executing.
    ///
    /// While the primary agent executes the input field stays editable and
    /// renders its unlocked border (FR-002, FR-011, FR-012). Input is only
    /// locked when an overlay would swallow keystrokes (a modal dialog, picker,
    /// or full-screen view) or while a compaction run owns the turn.
    pub(crate) fn is_input_locked(&self) -> bool {
        !self.permission_queue.is_empty()
            || !self.question_queue.is_empty()
            || self.provider_setup.is_some()
            || self.mcp_discover.is_some()
            || self.loop_setup.is_some()
            || self.pending_forcecleanup.is_some()
            || self.pending_rollback.is_some()
            || self.plan_approval_pending.is_some()
            || self.pending_memory_delete.is_some()
            || self.pending_router_save.is_some()
            || self.pending_stop_confirm
            || self.output_view.is_some()
            || self.memory_view.is_some()
            || self.research_view.is_some()
            || self.config_save_picker.is_some()
            || self.history_picker.is_some()
            || self.context_menu.is_some()
            || self.show_shortcuts
            || self.compact_in_progress
            || self.auto_compact_in_progress
            || self.pending_send_after_compact.is_some()
            // The queue-control menu (ALT-Q) is a modal overlay that swallows
            // every keystroke, so it must never let a character reach the input
            // buffer (spec `inputqueue` FR-031). The per-key interception lives
            // in `input::handle_key`; this entry keeps the guard layer and the
            // locked-border render consistent with the other modals.
            || self.queue_menu_open
            // The `Clear the input queue?` confirmation dialog is likewise a modal
            // overlay that swallows every keystroke (spec `inputqueue` FR-035).
            || self.queue_clear_confirm_open
            // The queue-entry panel (ALT-Q `Show` row) is a modal overlay too:
            // every key is routed to it, so no character may reach the input
            // buffer while it is open.
            || self.queue_show_open
            // The plugin-store browse panel is a modal overlay that swallows
            // every keystroke and places the keyboard focus in its own search
            // field, so the message input field and the input queue stay locked
            // while it is open (spec `pluginstores` FR-015).
            || self.plugin_store.is_some()
    }

    /// Return the number of Unicode code points currently in the input buffer.
    pub fn input_len_chars(&self) -> usize {
        self.input.chars().count()
    }

    /// Return the number of messages currently waiting in the input queue
    /// (spec `inputqueue` FR-001).
    pub fn input_queue_len(&self) -> usize {
        self.input_queue.len()
    }

    /// Labels for the four queue-control menu rows, in fixed order: `Next`,
    /// `Stop`/`Resume`, `Clear`, `Show` (spec `inputqueue` FR-021, FR-023,
    /// FR-026).
    ///
    /// The halt row reads `Stop` while a turn is in flight and `Resume` once it
    /// has stopped (FR-026). The render path in `crate::layout` and the action
    /// dispatch both read this one source, so the painted label can never
    /// disagree with what selecting the row does.
    pub fn queue_menu_labels(&self) -> [&'static str; 4] {
        ["Next", self.queue_menu_halt_label(), "Clear", "Show"]
    }

    /// Whether the given queue-control menu row can be selected.
    ///
    /// Only the `Next` row is conditional: it is non-selectable while the input
    /// queue is empty (FR-023). `Stop`/`Resume`, `Clear`, and `Show` are always
    /// selectable, so the keyboard navigation skips only the dead `Next` row.
    pub fn queue_menu_row_selectable(&self, row: usize) -> bool {
        if row == crate::app::QUEUE_MENU_ROW_NEXT {
            return self.input_queue_len() > 0;
        }
        row < crate::app::QUEUE_MENU_ROWS
    }

    /// Move the queue-control menu highlight up one row, skipping non-selectable
    /// rows (spec `inputqueue` FR-023). A no-op at the top row.
    pub fn queue_menu_move_up(&mut self) {
        let mut row = self.queue_menu_selected;
        while row > 0 {
            row -= 1;
            if self.queue_menu_row_selectable(row) {
                self.queue_menu_selected = row;
                self.needs_redraw = true;
                return;
            }
        }
    }

    /// Move the queue-control menu highlight down one row, skipping
    /// non-selectable rows (spec `inputqueue` FR-023). A no-op at the bottom row.
    pub fn queue_menu_move_down(&mut self) {
        let mut row = self.queue_menu_selected;
        while row + 1 < crate::app::QUEUE_MENU_ROWS {
            row += 1;
            if self.queue_menu_row_selectable(row) {
                self.queue_menu_selected = row;
                self.needs_redraw = true;
                return;
            }
        }
    }

    /// Close the queue-control menu and reset its highlight.
    ///
    /// The single place the menu's open flag and selection are cleared, so every
    /// dismiss path (Esc, activating a row, or opening the entry panel) leaves
    /// the same state (FR-022, FR-032).
    pub(crate) fn close_queue_menu(&mut self) {
        self.queue_menu_open = false;
        self.queue_menu_selected = 0;
    }

    /// Activate the highlighted queue-control menu row (spec `inputqueue`
    /// FR-024, FR-025, FR-027, FR-028, and the `Show` row).
    ///
    /// `Enter` dispatches to the row-specific action: `Next` runs the oldest
    /// entry, the halt row stops or resumes the agent, `Clear` opens the
    /// confirmation dialog, and `Show` opens the queue-entry panel. A
    /// non-selectable row (the empty-queue `Next`) is a no-op, so a stray
    /// `Enter` can never act on a dead row (FR-023).
    pub async fn queue_menu_activate_selected(&mut self) {
        if !self.queue_menu_row_selectable(self.queue_menu_selected) {
            return;
        }
        match self.queue_menu_selected {
            crate::app::QUEUE_MENU_ROW_NEXT => self.queue_menu_select_next().await,
            crate::app::QUEUE_MENU_ROW_HALT => self.queue_menu_select_halt(),
            crate::app::QUEUE_MENU_ROW_CLEAR => self.queue_menu_select_clear(),
            crate::app::QUEUE_MENU_ROW_SHOW => self.queue_show_open_panel(),
            _ => {}
        }
    }

    /// Open the scrollable queue-entry panel (spec `inputqueue` `Show` row).
    ///
    /// Closes the queue-control menu and shows every queued entry oldest-first
    /// so the user can reorder or delete entries without leaving the chat
    /// screen. The queue itself, the input buffer, the staged attachments, and
    /// the running turn are all left untouched; only `Esc` dismisses the panel.
    pub fn queue_show_open_panel(&mut self) {
        self.close_queue_menu();
        self.queue_show_open = true;
        self.queue_show_selected = 0;
        self.needs_redraw = true;
    }

    /// Dismiss the queue-entry panel (spec `inputqueue` `Show` row).
    ///
    /// `Esc` is the only key that closes the panel; the highlighted index is
    /// reset so the next open starts at the oldest entry.
    pub fn queue_show_close(&mut self) {
        self.queue_show_open = false;
        self.queue_show_selected = 0;
        self.needs_redraw = true;
    }

    /// Move the queue-entry panel highlight up one entry. A no-op at the first
    /// entry. The panel is scrolled to keep the highlight visible by the render
    /// pass, which keeps the selected entry on screen.
    pub fn queue_show_move_up(&mut self) {
        if self.queue_show_selected > 0 {
            self.queue_show_selected -= 1;
            self.needs_redraw = true;
        }
    }

    /// Move the queue-entry panel highlight down one entry. A no-op at the last
    /// entry. The changed selection is painted on the next frame.
    pub fn queue_show_move_down(&mut self) {
        if self.queue_show_selected + 1 < self.input_queue.len() {
            self.queue_show_selected += 1;
            self.needs_redraw = true;
        }
    }

    /// Move the highlighted queue entry one step toward the front of the queue
    /// (spec `inputqueue` `Show` row, `Enter`).
    ///
    /// The entry at `index` is swapped with the one at `index - 1` so it runs
    /// sooner; a no-op at the front entry. The highlight follows the moved entry
    /// so a second `Enter` advances it again. FIFO order is preserved for every
    /// other entry.
    pub fn queue_show_promote_selected(&mut self) {
        let index = self.queue_show_selected;
        if index == 0 || index >= self.input_queue.len() {
            return;
        }
        self.input_queue.swap(index, index - 1);
        self.queue_show_selected = index - 1;
        self.needs_redraw = true;
    }

    /// Remove the highlighted queue entry (spec `inputqueue` `Show` row, `Del`).
    ///
    /// The entry is dropped from the queue; the highlight stays within range and
    /// the queue counter repaints on the next frame. A no-op on an empty queue.
    pub fn queue_show_delete_selected(&mut self) {
        let index = self.queue_show_selected;
        if index >= self.input_queue.len() {
            return;
        }
        if let Some(entry) = self.input_queue.remove(index) {
            let text = crate::app::helpers::truncate_to_char_boundary(&entry.text, 120);
            self.push_log_no_agent(
                LogLevel::Info,
                format!("queue: deleted queued entry: {text}"),
            );
        }
        self.queue_show_selected = self
            .queue_show_selected
            .min(self.input_queue.len().saturating_sub(1));
        if self.input_queue.is_empty() {
            // An empty queue has nothing left to show, so the panel closes
            // itself (the counter also disappears).
            self.queue_show_open = false;
            self.queue_show_selected = 0;
        }
        self.needs_redraw = true;
    }

    /// The label for the queue-control menu's second row: `Stop` while a turn is
    /// executing, `Resume` once the agent has stopped (spec `inputqueue` FR-026).
    pub fn queue_menu_halt_label(&self) -> &'static str {
        if self.is_input_blocked() {
            "Stop"
        } else {
            "Resume"
        }
    }

    /// Empty the input queue.
    ///
    /// Called wherever the TUI session is reset (session switch/resume, new
    /// session creation, and process teardown) so a queued message never
    /// crosses a session boundary (NFR-005). The queue is never persisted, so
    /// no save is needed here.
    pub fn clear_input_queue(&mut self) {
        // A pending queue-control `Next` is meaningless once the queue is
        // emptied, so drop it with the entries (FR-024).
        self.queue_next_pending = false;
        // The queue-entry panel has nothing left to list once the queue is
        // emptied, so it closes with the entries.
        self.queue_show_open = false;
        self.queue_show_selected = 0;
        if self.input_queue.is_empty() {
            return;
        }
        self.input_queue.clear();
        // The prompt's queue counter disappears once the queue is empty, so the
        // change must be painted on the next frame (NFR-003).
        self.needs_redraw = true;
    }

    /// Open the plugin-store browse panel for `kind`, pre-filling the search
    /// field from `prefill` and recording a `--refresh` request (spec
    /// `pluginstores` FR-007, FR-012, FR-020, FR-021).
    ///
    /// Both `/plugins codex` and `/plugins claude` drive the same browser,
    /// parameterised only by store (A2). The panel opens in the `Loading` state
    /// and its index fetch is started off-loop ([`Self::spawn_plugin_store_fetch`])
    /// so the event loop and any in-progress agent turn keep animating (FR-007,
    /// FR-016, FR-026). Opening the panel installs nothing and touches no input
    /// state (FR-022); while it is open the input field and the queue are locked
    /// (FR-015).
    ///
    /// The installed-plugin-id set is derived once from the store scan on open
    /// (T-007, FR-005, A5) so the renderer can colour already-present rows.
    pub fn open_plugin_store(&mut self, kind: StoreKind, prefill: &str, refresh: bool) {
        let installed = self.derive_installed_set();
        let mut browser = PluginStoreBrowser::new(kind, prefill, refresh);
        browser.set_installed(installed);
        self.plugin_store = Some(browser);
        self.needs_redraw = true;
        self.spawn_plugin_store_fetch(kind);
    }

    /// Start the store-index fetch for `kind` off the event loop (spec
    /// `pluginstores` T-008; FR-007, FR-016, FR-026).
    ///
    /// Resolves the effective endpoint (configured override, else the compiled
    /// default; T-015, FR-027..FR-029) and the fetch budgets from config on the
    /// UI thread, then hands the blocking `reqwest` download to the TUI's
    /// background path so the event loop never stalls. The parsed index (or the
    /// contained [`StoreError`]) is delivered back through the
    /// [`App::plugin_store_result`] slot and applied by
    /// [`App::poll_plugin_store_result`] on a later frame, so the panel renders
    /// its loading row until the result lands (FR-016).
    ///
    /// A refused endpoint (non-`https` or malformed, FR-024) is surfaced
    /// immediately as the panel's inline error state instead of spawning, and
    /// the fetch is skipped entirely when no async reactor is available. No path
    /// panics (FR-025).
    pub(crate) fn spawn_plugin_store_fetch(&mut self, kind: StoreKind) {
        let (_dirs, plugins) = store_and_config(&self.cwd_path);
        let stores = plugins.stores_or_default();
        self.spawn_plugin_store_fetch_with_stores(kind, &stores);
    }

    /// Install an injectable store-index fetch seam (spec `pluginstores` T-019;
    /// FR-037).
    ///
    /// Replaces the production HTTPS fetcher with an alternative implementation
    /// (typically an offline [`ragent_plugins::FixtureStoreFetcher`]) so the
    /// store-browser launch and fetch paths can be driven against fixture index
    /// bytes with no network access (FR-037, NFR-003). The seam is
    /// production-transparent: unless a test calls this, the browser keeps the
    /// default HTTPS fetcher set at construction and the live launch path is
    /// unchanged.
    pub fn set_plugin_store_fetcher(&mut self, fetcher: Arc<dyn StoreIndexFetcher>) {
        self.plugin_store_fetcher = fetcher;
    }

    /// [`Self::spawn_plugin_store_fetch`] with the store budget/endpoint block
    /// supplied by the caller, so the resolution and refusal paths are testable
    /// without reading a real config file (FR-024, FR-027..FR-029).
    pub fn spawn_plugin_store_fetch_with_stores(
        &mut self,
        kind: StoreKind,
        stores: &ragent_config::PluginStoresConfig,
    ) {
        // A refused endpoint (non-https or malformed, FR-024) is reported as the
        // panel's inline error with no fetch attempted.
        let endpoint = match kind.effective_endpoint(stores) {
            Ok(endpoint) => endpoint,
            Err(err) => {
                self.plugin_store_failed(kind, err.to_string());
                return;
            }
        };
        let limits = FetchLimits::from(stores);
        let slot = Arc::clone(&self.plugin_store_result);
        // The seam the browser drives: the production HTTPS fetcher unless a test
        // has replaced it with an offline fixture fetcher (FR-037).
        let fetcher = Arc::clone(&self.plugin_store_fetcher);
        // The blocking download runs inside the spawned task: `reqwest::blocking`
        // must not occupy a runtime worker thread, so it goes to the blocking
        // pool and the UI thread returns immediately, keeping the event loop and
        // any agent turn animating while the fetch is in flight (FR-016, FR-026).
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn_blocking(move || {
                let outcome = fetcher.fetch_index(kind, &endpoint, &limits);
                Self::deposit_plugin_store_result(&slot, kind, outcome);
            });
        } else {
            // Without an async reactor (headless/unit-test contexts) the fetch
            // cannot run off-loop, so it is not attempted at all: running it
            // inline would block the caller and violate FR-016/FR-026. The panel
            // stays in its loading state.
            tracing::debug!(
                store = kind.token(),
                "plugin-store fetch skipped: no async runtime available"
            );
        }
    }

    /// Store one fetch outcome in the shared delivery slot, recovering a
    /// poisoned lock (FR-025).
    fn deposit_plugin_store_result(
        slot: &std::sync::Mutex<Option<PluginStoreFetchResult>>,
        kind: StoreKind,
        outcome: Result<StoreIndex, StoreError>,
    ) {
        *recover_poisoned(slot.lock(), "plugin_store_result") =
            Some(PluginStoreFetchResult { kind, outcome });
    }

    /// Drain a completed off-loop store-index fetch and apply it to the open
    /// panel (spec `pluginstores` T-008; FR-013, FR-016, FR-017).
    ///
    /// Called each housekeeping frame. A result whose store no longer matches the
    /// open browser (the panel was closed or re-opened for the other store) is
    /// discarded, so a late arrival can never fill the wrong panel. On success
    /// the entries are installed (deriving the `Ready`/`Empty` status, FR-017);
    /// on failure the contained [`StoreError`] is rendered as the inline error
    /// detail (FR-013). No path panics (FR-025).
    pub fn poll_plugin_store_result(&mut self) {
        let result = {
            let mut guard =
                recover_poisoned(self.plugin_store_result.lock(), "plugin_store_result");
            guard.take()
        };
        let Some(result) = result else {
            return;
        };
        let open_kind = self.plugin_store.as_ref().map(|b| b.kind);
        if open_kind != Some(result.kind) {
            // Stale: the panel is closed or showing the other store (FR-007).
            return;
        }
        match result.outcome {
            Ok(index) => {
                if let Some(browser) = self.plugin_store.as_mut() {
                    browser.set_index(index);
                }
            }
            Err(err) => self.plugin_store_failed(result.kind, err.to_string()),
        }
        self.needs_redraw = true;
    }

    /// Apply an inline failure detail to the open browser for `kind` (FR-013).
    ///
    /// A no-op when the panel is closed or showing the other store, so a
    /// late/foreign failure never corrupts an unrelated panel.
    fn plugin_store_failed(&mut self, kind: StoreKind, detail: String) {
        if let Some(browser) = self.plugin_store.as_mut()
            && browser.kind == kind
        {
            browser.set_failed(detail);
            self.needs_redraw = true;
        }
    }

    /// Re-derive the panel's installed-plugin-id set from the store scan
    /// (T-007, FR-005).
    ///
    /// Called on open and (by a later task) after an install commits, so a newly
    /// installed row re-colours without keeping a second install registry (A5).
    /// A no-op when no panel is open.
    pub fn refresh_plugin_store_installed(&mut self) {
        if self.plugin_store.is_none() {
            return;
        }
        let installed = self.derive_installed_set();
        if let Some(browser) = self.plugin_store.as_mut() {
            browser.set_installed(installed);
            self.needs_redraw = true;
        }
    }

    /// Derive the installed-plugin-id set for the current working directory from
    /// the plugin store scan (FR-005, A5). Never fails: an absent or unreadable
    /// store scans as empty.
    fn derive_installed_set(&self) -> BTreeSet<String> {
        let (dirs, _config) = store_and_config(&self.cwd_path);
        installed_ids(dirs)
    }

    /// Handle `ENTER` on the highlighted plugin-store result (spec `pluginstores`
    /// FR-006, FR-011, FR-014, FR-022).
    ///
    /// A highlighted result whose id is already in the store scan is refused
    /// with an already-installed notice and nothing is written (FR-014). A
    /// not-installed result starts the off-loop install of its `source` through
    /// the existing `ragent_plugins::add(force = false)` entry point (FR-006,
    /// FR-024), which the poll later drains to report the installed id/dialect
    /// and re-derive the installed set so the row re-colours (FR-011). An empty
    /// result set records a neutral notice and installs nothing (FR-022). No path
    /// panics (FR-025).
    ///
    /// Installing is only ever reached from this explicit `ENTER`: opening,
    /// typing, and moving never call it, so nothing is installed without a
    /// deliberate key press (FR-022).
    pub fn plugin_store_install_selected(&mut self) {
        let Some(browser) = self.plugin_store.as_mut() else {
            return;
        };
        let Some(entry) = browser.selected() else {
            browser.last_install = Some("no result highlighted".to_string());
            self.needs_redraw = true;
            return;
        };
        let id = entry.id.clone();
        if browser.is_installed(&id) {
            // FR-014: a re-install is refused; nothing is written.
            browser.last_install = Some(format!("plugin {id} is already installed"));
            self.needs_redraw = true;
            return;
        }
        let source = entry.source.clone();
        let kind = browser.kind;
        browser.last_install = Some(format!("installing {id}..."));
        self.needs_redraw = true;
        self.spawn_plugin_store_install(kind, id, source);
    }

    /// Start the install of `source` off the event loop (spec `pluginstores`
    /// T-009; FR-006, FR-011, FR-024, FR-025, FR-026).
    ///
    /// Resolves the plugin store directories from the same config the store scan
    /// uses, then hands the blocking `ragent_plugins::add(force = false)` call to
    /// a worker thread so the TUI event loop and any agent turn keep animating
    /// while the download/extract/commit runs (FR-026). The store-supplied
    /// `source` is passed through unchanged, so it receives no elevated trust:
    /// the existing HTTPS, traversal, and size guards still apply (FR-024).
    ///
    /// The worker turns the [`AddOutcome`]/[`AddError`] into a
    /// [`PluginStoreInstallResult`] (success report naming the installed id and
    /// dialect, or the `[err]` report naming the cause) and deposits it in
    /// [`App::plugin_store_install_result`] for
    /// [`App::poll_plugin_store_install_result`]. When no worker thread can be
    /// spawned the install runs inline so the action still completes; the
    /// installed set is then re-derived immediately. No path panics (FR-025).
    fn spawn_plugin_store_install(&mut self, kind: StoreKind, id: String, source: String) {
        let (dirs, _config) = store_and_config(&self.cwd_path);
        let workdir = self.cwd_path.clone();
        let slot = Arc::clone(&self.plugin_store_install_result);

        let worker_id = id.clone();
        let worker_dirs = dirs.clone();
        let worker_workdir = workdir.clone();
        let worker_source = source.clone();
        let spawn = std::thread::Builder::new()
            .name("plugin-store-install".to_owned())
            .spawn(move || {
                let result = run_plugin_store_install(
                    &worker_dirs,
                    &worker_workdir,
                    kind,
                    &worker_id,
                    &worker_source,
                );
                Self::deposit_plugin_store_install_result(&slot, result);
            });

        match spawn {
            Ok(handle) => {
                // The worker owns the install now and deposits its result for
                // the next poll to drain.
                drop(handle);
            }
            Err(err) => {
                tracing::warn!(error = %err, "plugin-store install worker spawn failed; running inline");
                let result = run_plugin_store_install(&dirs, &workdir, kind, &id, &source);
                if result.succeeded {
                    self.refresh_plugin_store_installed();
                }
                self.apply_plugin_store_install_result(result);
            }
        }
    }

    /// Store one install outcome in the shared delivery slot, recovering a
    /// poisoned lock (FR-025).
    fn deposit_plugin_store_install_result(
        slot: &std::sync::Mutex<Option<PluginStoreInstallResult>>,
        result: PluginStoreInstallResult,
    ) {
        *recover_poisoned(slot.lock(), "plugin_store_install_result") = Some(result);
    }

    /// Drain a completed off-loop install and apply it (spec `pluginstores`
    /// T-009; FR-006, FR-011, FR-014, FR-025).
    ///
    /// Called each housekeeping frame. A result whose store no longer matches the
    /// open browser (the panel was closed or re-opened for the other store) is
    /// still reported to the message window — the install already happened — but
    /// never touches a different store's panel. On success the installed set is
    /// re-derived so the newly installed row re-colours (FR-011). No path panics.
    pub fn poll_plugin_store_install_result(&mut self) {
        let result = {
            let mut guard = recover_poisoned(
                self.plugin_store_install_result.lock(),
                "plugin_store_install_result",
            );
            guard.take()
        };
        let Some(result) = result else {
            return;
        };
        if result.succeeded {
            self.refresh_plugin_store_installed();
        }
        self.apply_plugin_store_install_result(result);
    }

    /// Apply one install result: record the panel-footer notice when the panel
    /// still belongs to the same store, and append the report to the message
    /// window (FR-006, FR-011, FR-025).
    fn apply_plugin_store_install_result(&mut self, result: PluginStoreInstallResult) {
        if let Some(browser) = self.plugin_store.as_mut()
            && browser.kind == result.kind
        {
            browser.last_install = Some(result.notice);
        }
        self.append_assistant_text(&result.report);
        self.needs_redraw = true;
    }

    /// Start the off-loop `/plugins stores --check` availability probe (spec
    /// `pluginstores` FR-031 `--check`).
    ///
    /// Resolves the store block and spawns a blocking task that contacts each
    /// store endpoint through the same injectable [`StoreIndexFetcher`] seam the
    /// browser uses, then deposits the fully rendered report into
    /// [`App::plugin_store_probe_result`] for
    /// [`App::poll_plugin_store_probe_result`] to append. The event loop never
    /// stalls on the network probe. Without an async reactor (headless/unit-test
    /// contexts) the probe is skipped and the plain report is deposited, exactly
    /// as the browser fetch is skipped, so no path panics.
    pub fn begin_plugin_store_probe(&mut self) {
        let (_dirs, plugins) = store_and_config(&self.cwd_path);
        let stores = plugins.stores_or_default();
        let slot = Arc::clone(&self.plugin_store_probe_result);
        let fetcher = Arc::clone(&self.plugin_store_fetcher);
        if let Ok(handle) = tokio::runtime::Handle::try_current()
            && handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
        {
            // Only a multi-thread reactor can run the blocking probe alongside
            // the caller; on a current-thread reactor `spawn_blocking` would
            // queue the probe without ever letting it make progress (the test
            // reactor is dropped before the task runs), so the plain report is
            // deposited instead — matching the no-reactor branch.
            handle.spawn_blocking(move || {
                let probes = probe_stores(&stores, fetcher.as_ref());
                let report = render_stores_report_with_probes(&stores, Some(&probes));
                *recover_poisoned(slot.lock(), "plugin_store_probe_result") = Some(report);
            });
        } else {
            // No usable reactor: report the plain config view rather than block.
            let report = render_stores_report_with_probes(&stores, None);
            *recover_poisoned(slot.lock(), "plugin_store_probe_result") = Some(report);
        }
    }

    /// Drain a completed off-loop `/plugins stores --check` probe and append its
    /// report to the message window (spec `pluginstores` FR-031 `--check`).
    ///
    /// Called each housekeeping frame, mirroring
    /// [`App::poll_websearch_test_result`]. No path panics (FR-025).
    pub fn poll_plugin_store_probe_result(&mut self) {
        let report = {
            let mut guard = recover_poisoned(
                self.plugin_store_probe_result.lock(),
                "plugin_store_probe_result",
            );
            guard.take()
        };
        let Some(report) = report else {
            return;
        };
        self.append_assistant_text(&report);
        self.status = "plugins: store check complete".to_string();
        self.needs_redraw = true;
    }

    /// Close the plugin-store browse panel and drop its state (FR-012).
    ///
    /// This is the only place the browser is cleared, so every close path
    /// leaves the input field, the staged attachments, the queue, and the
    /// running turn exactly as they were (FR-012).
    pub fn close_plugin_store(&mut self) {
        if self.plugin_store.take().is_some() {
            self.needs_redraw = true;
        }
    }

    /// Handle the shared `Backspace` / `Esc` query-editing key while the panel
    /// is open (spec `pluginstores` FR-009).
    ///
    /// Removes the last query character and returns `true` when the query is
    /// non-empty; when it is already empty, dismisses the panel (FR-012) and
    /// returns `false`.
    pub fn plugin_store_edit_or_close(&mut self) -> bool {
        let Some(browser) = self.plugin_store.as_mut() else {
            return false;
        };
        if browser.backspace() {
            self.needs_redraw = true;
            true
        } else {
            self.close_plugin_store();
            false
        }
    }

    /// Append a typed character to the panel's search query (FR-008).
    pub fn plugin_store_push_char(&mut self, c: char) {
        if let Some(browser) = self.plugin_store.as_mut() {
            browser.push_char(c);
            self.needs_redraw = true;
        }
    }

    /// Move the panel's block cursor up one result (FR-010).
    pub fn plugin_store_move_up(&mut self) {
        if let Some(browser) = self.plugin_store.as_mut() {
            browser.move_up();
            self.needs_redraw = true;
        }
    }

    /// Move the panel's block cursor down one result (FR-010).
    pub fn plugin_store_move_down(&mut self) {
        if let Some(browser) = self.plugin_store.as_mut() {
            browser.move_down();
            self.needs_redraw = true;
        }
    }

    /// Append a message to the input queue (spec `inputqueue` FR-005).
    ///
    /// Called when the user presses Enter with non-empty input while the
    /// primary agent is executing. The entry is appended to the tail so
    /// submission order is preserved (FR-001, FR-019); the input field only
    /// loses its text on success. Entries are added to the input history at
    /// submission time (FR-003) rather than when they are dispatched.
    ///
    /// Returns the untouched [`QueuedInput`] as `Err` when the queue is already
    /// at capacity (FR-004): the submission is rejected, a status message
    /// explains why, and the caller can restore the typed text and staged
    /// attachments so the user does not lose the message. The capacity comes
    /// from [`App::input_queue_capacity`] (FR-015).
    ///
    /// A successful enqueue also echoes a `queued (N in queue)` notice into the
    /// log panel (FR-014), where `N` is the depth *after* the append.
    pub(crate) fn enqueue_input(
        &mut self,
        text: String,
        image_paths: Vec<std::path::PathBuf>,
    ) -> Result<(), QueuedInput> {
        if self.input_queue.len() >= self.input_queue_capacity {
            self.status = format!(
                "queue full (max {}) - wait for the current turn to finish",
                self.input_queue_capacity
            );
            return Err(QueuedInput { text, image_paths });
        }
        // History is updated at entry time, before execution (FR-003).
        self.add_to_history(text.clone());
        self.input.clear();
        self.input_cursor = 0;
        self.history_index = None;
        self.file_menu = None;
        // A queued slash command (FR-017 amendment) must also dismiss the
        // completion menu, otherwise it would float over a field it no longer
        // belongs to once the entry runs.
        self.slash_menu = None;
        self.input_queue
            .push_back(QueuedInput { text, image_paths });
        // FR-014: echo the enqueue into the log panel with the post-enqueue depth.
        self.push_log_no_agent(
            LogLevel::Info,
            format!("queued ({} in queue)", self.input_queue.len()),
        );
        // The counter renders the new length on the next frame (NFR-003).
        self.needs_redraw = true;
        Ok(())
    }

    /// Pop the oldest queued entry and dispatch it as the next user turn
    /// (spec `inputqueue` FR-006, FR-007, FR-019).
    ///
    /// Called at the turn boundaries (`MessageEnd` with a non-cancelled finish
    /// reason, and `AgentError`) once the finishing handler has cleared
    /// `is_processing`. Entries are popped from the head so FIFO order is never
    /// violated (FR-019) and the drain reuses [`App::dispatch_user_message`],
    /// which spawns the turn asynchronously so the UI thread is not blocked
    /// (NFR-004). The counter reflects the decremented length on the next frame
    /// (FR-008).
    ///
    /// FR-016 (T-008): dispatch is guarded against processing/compaction
    /// overlap. The queue is left untouched — deferring to the next safe turn
    /// boundary — while the primary agent is still executing (`is_processing`)
    /// or a compaction run owns the turn (`compact_in_progress`,
    /// `auto_compact_in_progress`, or `pending_send_after_compact`). The check
    /// lives here rather than in the callers so every drain path
    /// (`MessageEnd`, `AgentError`, and the queue-control `Next` selection)
    /// shares one boundary guard and no path can dispatch over a live turn.
    ///
    /// A dispatch runs when either the queue is non-empty (the ordinary
    /// turn-boundary drain) or a queue-control `Next` selection is pending
    /// (FR-024). The pending flag is consumed first, so `Next` fires exactly once
    /// instead of re-arming at every later boundary. A `Next`-triggered dispatch
    /// preserves whatever draft the user is editing, so selecting the row never
    /// mutates the editable input buffer (FR-031).
    pub async fn advance_input_queue(&mut self) {
        // FR-017 amendment: a queued *synchronous* slash command never sets
        // `is_processing`, so no `MessageEnd`/`AgentError` boundary follows it and
        // the rest of the queue would otherwise stall behind it. Drain in a loop
        // so consecutive command entries run back-to-back; the boundary guard at
        // the top of each pass stops the moment an entry leaves the turn busy.
        loop {
            let next_pending = self.queue_next_pending;
            self.queue_next_pending = false;
            if self.session_id.is_none()
                || self.is_processing
                || self.compact_in_progress
                || self.auto_compact_in_progress
                || self.pending_send_after_compact.is_some()
            {
                // FR-030: a `Next` that cannot dispatch yet stays pending and is
                // retried at the next safe turn boundary instead of being lost.
                self.queue_next_pending = next_pending;
                return;
            }
            if !next_pending && self.input_queue.is_empty() {
                return;
            }
            let Some(entry) = self.input_queue.pop_front() else {
                return;
            };
            // FR-008: the counter decrements, so the field must repaint next frame.
            self.needs_redraw = true;
            if next_pending {
                // FR-031: a menu `Next` dispatch must not mutate the editable buffer,
                // so snapshot the live draft and restore it once the queued entry has
                // been handed to the (asynchronous) dispatch path. `Next` runs exactly
                // one entry, so the drain ends here.
                let saved_input = std::mem::take(&mut self.input);
                let saved_cursor = self.input_cursor;
                let saved_anchor = self.kb_select_anchor;
                self.dispatch_queued_input(entry.text, entry.image_paths)
                    .await;
                self.input = saved_input;
                self.input_cursor = saved_cursor;
                self.kb_select_anchor = saved_anchor;
                return;
            }
            // A plain message starts an asynchronous turn that sets `is_processing`,
            // so it ends the drain; a synchronous slash command leaves the boundary
            // free and the loop continues with the next queued entry.
            if self
                .dispatch_queued_input(entry.text, entry.image_paths)
                .await
            {
                return;
            }
        }
    }

    /// Dispatch one queued entry, routing a slash command back through the
    /// slash-command executor instead of the chat path (spec `inputqueue`
    /// FR-017 amendment).
    ///
    /// A queued plain message runs as a user turn via
    /// [`App::dispatch_user_message`]; a queued entry whose text begins with `/`
    /// is a slash command and is handed to [`App::execute_slash_command`] exactly
    /// as a freshly typed one, so a queued `/status`, `/agent`, or `/spec …`
    /// behaves identically to typing it at a free boundary. This is the single
    /// place the two entry kinds diverge.
    ///
    /// Returns `true` when a chat turn was started (a plain message, which leaves
    /// the boundary busy and ends the drain) and `false` when a synchronous slash
    /// command ran and left the boundary free for the next queued entry.
    async fn dispatch_queued_input(
        &mut self,
        text: String,
        image_paths: Vec<std::path::PathBuf>,
    ) -> bool {
        if text.starts_with('/') {
            self.execute_slash_command(&text).await;
            false
        } else {
            self.dispatch_user_message(text, image_paths);
            true
        }
    }

    /// Select the queue-control menu's `Next` row (spec `inputqueue` FR-024,
    /// FR-029, FR-030, NFR-006).
    ///
    /// Stops the running turn exactly as `InputAction::CancelAgent` does (FR-024)
    /// and dispatches the oldest queued entry. A live turn cannot be dispatched
    /// over (FR-016), so while the agent is executing the dispatch is *deferred*:
    /// the selection is recorded in [`App::queue_next_pending`] and runs at the
    /// turn boundary the cancel opens, never overlapping the running turn (FR-029,
    /// FR-030). The deferral reuses the shared asynchronous dispatch path, so the
    /// UI thread is never blocked (NFR-006). The menu closes and the change paints
    /// on the next frame.
    ///
    /// When no turn is executing there is nothing to cancel, so the oldest entry
    /// dispatches immediately. Selecting `Next` never mutates the input buffer or
    /// the staged attachments (FR-031).
    ///
    /// Public so the T-019 integration tests can drive the row directly, mirroring
    /// [`App::queue_menu_select_halt`]. The Up/Down/Enter menu key handling that
    /// invokes it is added by the menu key-handling task; it is not part of this
    /// action.
    pub async fn queue_menu_select_next(&mut self) {
        if self.input_queue.is_empty() {
            // FR-023: the `Next` row is non-selectable with an empty queue, so
            // this is only reachable defensively; report and close.
            self.status = "queue: nothing to run — the queue is empty".to_string();
            self.close_queue_menu();
            self.needs_redraw = true;
            return;
        }
        // FR-030: never dispatch over a compaction run or a post-compact send.
        // There is no live turn to cancel in that window, so refuse with a status
        // message rather than deferring an entry that no boundary would pick up.
        if self.compact_in_progress
            || self.auto_compact_in_progress
            || self.pending_send_after_compact.is_some()
        {
            self.status = "queue: next deferred — compaction in progress".to_string();
            self.push_log_no_agent(
                LogLevel::Warn,
                "queue: Next deferred — compaction owns the turn (FR-030)".to_string(),
            );
            self.close_queue_menu();
            self.needs_redraw = true;
            return;
        }
        if self.is_processing {
            // FR-024/FR-029: stop the running turn exactly like `CancelAgent`.
            // The queue is untouched — halt never advances it — so the oldest
            // entry survives into the resumed turn.
            self.halt_turn_like_cancel_agent();
            // FR-030: the live turn cannot be dispatched over, so defer the run
            // of the oldest entry to the turn boundary the cancel opens.
            self.queue_next_pending = true;
            self.status = "queue: stopping turn — running the next queued entry".to_string();
            self.push_log_no_agent(
                LogLevel::Info,
                "queue: Next — halting turn, dispatching oldest queued entry at the boundary"
                    .to_string(),
            );
        } else {
            // Nothing to stop, so the oldest entry runs straight away on the
            // shared asynchronous path (FR-024, NFR-006). The same pending flag
            // drives it so the editable draft is preserved (FR-031).
            let text = self
                .input_queue
                .front()
                .map(|e| crate::app::helpers::truncate_to_char_boundary(&e.text, 120))
                .unwrap_or_default();
            self.push_log_no_agent(
                LogLevel::Info,
                format!("queue: Next — dispatching oldest queued entry: {text}"),
            );
            self.queue_next_pending = true;
            self.advance_input_queue().await;
        }
        self.close_queue_menu();
        self.needs_redraw = true;
    }

    /// Select the queue-control menu's halt row (spec `inputqueue` FR-025,
    /// FR-026, FR-027, FR-029, NFR-008).
    ///
    /// While a turn is executing (`is_input_blocked`) this performs exactly the
    /// `InputAction::CancelAgent` behaviour and leaves the queue untouched, so
    /// halting never advances it (FR-025, FR-029). Once the agent has stopped the
    /// same row is labelled `Resume` (FR-026) and selecting it resumes the
    /// interrupted work instead (FR-027). Either way the menu closes and the
    /// change paints on the next frame (NFR-008).
    ///
    /// Public so the T-019 integration tests can drive the row directly, mirroring
    /// [`App::advance_input_queue`]. The Up/Down/Enter menu key handling that
    /// invokes it is added by the menu key-handling task; it is not part of this
    /// action.
    pub fn queue_menu_select_halt(&mut self) {
        if self.is_input_blocked() {
            // The halt row performs exactly the `CancelAgent` behaviour.
            self.halt_turn_like_cancel_agent();
        } else {
            // FR-027: the agent is already stopped, so the row resumes the
            // interrupted work instead of halting.
            if !self.agent_halted {
                self.status = "Nothing to resume — agent was not halted".to_string();
            } else if !self.dispatch_resume_continuation() {
                self.status = "No active session".to_string();
            }
        }
        self.close_queue_menu();
        self.needs_redraw = true;
    }

    /// Halt the running turn exactly as `InputAction::CancelAgent` does: ask a
    /// live govcreate run to cancel at its next stage boundary, then set the
    /// turn cancel flag. Shared by the queue-control `Next` and halt rows so the
    /// two stay in step with the cancel path.
    fn halt_turn_like_cancel_agent(&mut self) {
        if self.govcreate_run_active() {
            self.poll_govcreate_cancel();
        }
        self.halt_running_agent();
    }

    /// Resume a halted agent: clear the halted flag, append the continuation
    /// user turn, and dispatch it asynchronously on the shared processor (the
    /// behaviour `/resume` and the queue-control `Resume` row both present).
    ///
    /// Returns `false` without side effects when there is no active session, so
    /// the caller can report `No active session`.
    pub(crate) fn dispatch_resume_continuation(&mut self) -> bool {
        let Some(sid) = self.session_id.clone() else {
            return false;
        };
        self.agent_halted = false;
        let resume_text = "You were previously interrupted by the user. Continue the task from where you left off.";
        self.messages.push(Message::user_text(&sid, resume_text));
        self.set_status_working("processing");
        self.push_log_no_agent(LogLevel::Info, "Resuming halted agent".to_string());

        let mut agent = self.agent_info.clone();
        self.apply_selected_model_and_thinking(&mut agent);

        let processor = self.session_processor.clone();
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flag = Some(flag.clone());
        tokio::spawn(async move {
            if let Err(e) = processor
                .process_message(&sid, resume_text, &agent, flag)
                .await
            {
                tracing::debug!(error = %e, "Failed to resume agent");
            }
        });
        true
    }

    /// Select the queue-control menu's `Clear` row (spec `inputqueue` FR-028,
    /// FR-033, NFR-008).
    ///
    /// Selecting `Clear` never empties the queue directly: it closes the menu and
    /// opens the `Clear the input queue?` confirmation dialog instead (FR-033).
    /// The queue is emptied only after the user confirms with `Yes` (FR-028,
    /// FR-037), so this action deliberately leaves the queue, the input buffer,
    /// the staged attachments, and the running turn untouched. The dialog paints
    /// on the next frame because the redraw flag is set (NFR-008).
    ///
    /// The dialog's default selection is reset to `No` every time it opens so a
    /// stray `Enter` cannot empty the queue (FR-034).
    ///
    /// Public so the T-019 integration tests can drive the row directly, mirroring
    /// [`App::queue_menu_select_halt`]. The Up/Down/Enter menu key handling that
    /// invokes it is added by the menu key-handling task; it is not part of this
    /// action.
    pub fn queue_menu_select_clear(&mut self) {
        self.close_queue_menu();
        self.queue_clear_confirm_open = true;
        self.queue_clear_confirm_selected = crate::app::QUEUE_CLEAR_CONFIRM_NO;
        self.needs_redraw = true;
    }

    /// Flip the `Clear the input queue?` dialog selection between `Yes` and
    /// `No` (spec `inputqueue` FR-034, FR-035, NFR-011).
    ///
    /// The dialog has exactly two options, so any horizontal move (`Left` /
    /// `Right` / `Tab` / `BackTab`) simply toggles the current selection. The
    /// redraw flag is set so the change paints on the next frame.
    pub fn queue_clear_confirm_toggle(&mut self) {
        self.queue_clear_confirm_selected = if self.queue_clear_confirm_is_yes() {
            crate::app::QUEUE_CLEAR_CONFIRM_NO
        } else {
            crate::app::QUEUE_CLEAR_CONFIRM_YES
        };
        self.needs_redraw = true;
    }

    /// Whether the `Clear the input queue?` dialog currently has `Yes`
    /// selected (spec `inputqueue` FR-037).
    #[must_use]
    pub fn queue_clear_confirm_is_yes(&self) -> bool {
        self.queue_clear_confirm_selected == crate::app::QUEUE_CLEAR_CONFIRM_YES
    }

    pub(crate) fn assert_input_cursor_invariant(&self) {
        debug_assert!(self.input_cursor <= self.input_len_chars());
    }

    pub(crate) fn pane_area(&self, pane: SelectionPane) -> Rect {
        match pane {
            SelectionPane::Messages => self.message_area,
            SelectionPane::Log => self.log_area,
            SelectionPane::Profile => self.profile_area,
            SelectionPane::Tasks => self.tasks_area,
            SelectionPane::Memory => self.memory_area,
            SelectionPane::Telemetry => self.telemetry_area,
            SelectionPane::Input => self.input_area,
            SelectionPane::ContextPanel => self.context_panel_area,
        }
    }

    /// Clear any text selection or context menu anchored on a pane whose
    /// layout area has collapsed to zero (e.g. a side panel that was just
    /// dismissed by a toggle such as Alt+T). The render pass zeroes the
    /// `*_area` of hidden panels, but the `text_selection`/`context_menu`
    /// state may still reference that pane — left stale, the next
    /// [`Self::assert_ui_invariants`] call would panic. This self-heals
    /// that state whenever input arrives.
    pub(crate) fn prune_stale_selection(&mut self) {
        if let Some(sel) = &self.text_selection {
            if self.pane_area(sel.pane).area() == 0 {
                self.text_selection = None;
            }
        }
        if let Some(menu) = &self.context_menu {
            if self.pane_area(menu.pane).area() == 0 {
                self.context_menu = None;
            }
        }
    }

    pub(crate) fn assert_ui_invariants(&self) {
        self.assert_input_cursor_invariant();
        if let Some(sel) = &self.text_selection {
            debug_assert!(
                self.pane_area(sel.pane).area() > 0,
                "selection pane {:?} has no active area",
                sel.pane
            );
        }
        if let Some(menu) = &self.context_menu {
            debug_assert!(
                self.pane_area(menu.pane).area() > 0,
                "context menu pane {:?} has no active area",
                menu.pane
            );
        }
    }

    #[allow(unused_variables)]
    pub(crate) fn debug_log_input_transition(
        &self,
        source: &str,
        before_input: &str,
        before_cursor: usize,
    ) {
        #[cfg(debug_assertions)]
        {
            if before_input != self.input || before_cursor != self.input_cursor {
                tracing::debug!(
                    source = source,
                    before_chars = before_input.chars().count(),
                    before_cursor = before_cursor,
                    after_chars = self.input_len_chars(),
                    after_cursor = self.input_cursor,
                    screen = ?self.current_screen,
                    slash_menu = self.slash_menu.is_some(),
                    file_menu = self.file_menu.is_some(),
                    "input transition"
                );
            }
        }
    }

    pub(crate) fn set_cursor_char_index_clamped(&mut self, index: usize) {
        self.input_cursor = index.min(self.input_len_chars());
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn refresh_input_menus(&mut self) {
        if self.input.starts_with('/') {
            self.update_slash_menu();
        } else {
            self.slash_menu = None;
        }
        if self.input.contains('@') {
            self.update_file_menu();
        } else {
            self.file_menu = None;
        }
    }

    pub(crate) fn get_parameter_hint(&self, trigger: &str) -> Option<String> {
        match trigger {
            "team" => Some("<subcommand>".to_string()),
            "memory" => Some("<subcommand> [<arg>]".to_string()),
            "agent" => Some("[<name>]".to_string()),
            "codeindex" => Some("[on|off|show|sync|reindex|help]".to_string()),
            "gcf" => Some("[on|off|show|help]".to_string()),
            "tools" => Some(
                "[show|help|office|github|gitlab|teams|agents|plan|codeindex] [on|off]".to_string(),
            ),
            "model" => Some("[show]".to_string()),
            "spec" => Some("[create|add|delete|list|search|validate|status|task|govcreate <specid> <content-ref> <target-folder> [--language <lang>] [--type <type>] [--stack <name>] [--github|--gitlab] [--force]|activate|deactivate|coverage|impl|jtbd|help]".to_string()),
            "router" => {
                Some("[on|off|status|tiers|weights|boundaries|test|stats|reload|help]".to_string())
            }
            "config" => Some("[show|save|list|help]".to_string()),
            "triggers" => Some("[list|enable|disable|remove|status|help]".to_string()),
            "websearch" => Some("[show|help]".to_string()),
            "init" => Some("[config|help]".to_string()),
            "thinking" => Some("[auto|off|low|medium|high]".to_string()),
            "mouse" => Some("[on|off|help]".to_string()),
            "status" => Some("[clear]".to_string()),
            "queue" => Some("[list|clear|next|help]".to_string()),
            "alog" => Some("[help|on|off|config|list|status|delete <run-id> --yes|export <run-id> --yes]".to_string()),
            "toolchain" => Some("[help|list [lang] [--json]]".to_string()),
            "prompt" => Some("[help|primary [agent]|subagent [agent]|list|<agent>]".to_string()),
            "log" => Some("[clear subagents|panics|research|editlog|help]".to_string()),
            "loop" => Some("[help | <agent> <goal text...>]".to_string()),
            "blueprints" => Some("[help|list|<name>]".to_string()),
            "research" => Some(
                "[create [--mode tiered|supervisor|competitive] [--summarization-model <model>] [--evaluate] [--clarify|--no-clarify] [--format report|executive-summary|comparison-table|source-bibliography|imrad] [--tier light|full|dissertation] [--depth shallow|standard|deep] [--iterations N] [--fetch-concurrently N] [--use-local] [--use-specs] [--use-low-relevance] [--use-pdf] [--no-papers] [--oa-enable|--no-oa] [--web-time N] [--max-concepts N] [--max-findings N] [--url-cloak] <name> <topic...>] | [list|open|search|show|delete|archive|cluster]".to_string(),
            ),
            "help" => Some("[<command>]".to_string()),
            "quit" | "exit" => None,
            "clear" => None,
            "undo" => None,
            "redo" => None,
            "compact" | "compress" => None,
            "halt" => None,
            "resume" => None,
            _ => Some("<arg>".to_string()),
        }
    }

    pub(crate) fn refresh_project_files_cache(&mut self) {
        let wd = crate::app::helpers::current_working_dir();
        let files = ragent_agent::reference::fuzzy::collect_project_files(&wd, 10_000);
        self.project_files_cache_count = files.len();
        self.project_files_cache = Some(files);
        self.project_files_cache_cwd = Some(wd);
        self.project_files_cache_refreshed_at = Some(std::time::SystemTime::now());
    }

    /// Insert a single character at the cursor and refresh autocomplete menus.
    pub fn insert_char_at_cursor(&mut self, c: char) {
        let insert_pos = self.cursor_byte_pos();
        self.input.insert(insert_pos, c);
        self.cursor_move_right();
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Insert a string at the cursor and refresh autocomplete menus.
    pub fn insert_text_at_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let insert_pos = self.cursor_byte_pos();
        let added = text.chars().count();
        self.input.insert_str(insert_pos, text);
        self.set_cursor_char_index_clamped(self.input_cursor + added);
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_prev_char(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let delete_pos = self.cursor_byte_pos_at_char_index(self.input_cursor - 1);
        self.input.remove(delete_pos);
        self.cursor_move_left();
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Delete the character at the cursor (delete).
    pub fn delete_next_char(&mut self) {
        if self.input_cursor >= self.input_len_chars() {
            return;
        }
        let delete_pos = self.cursor_byte_pos();
        self.input.remove(delete_pos);
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    /// Remove the inclusive character range `[start, end)` from the input
    /// buffer, clamping to the current length. No-op when `start >= end`.
    pub fn remove_input_char_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let clamped_start = start.min(self.input_len_chars());
        let clamped_end = end.min(self.input_len_chars());
        if clamped_start >= clamped_end {
            return;
        }
        let byte_start = self.cursor_byte_pos_at_char_index(clamped_start);
        let byte_end = self.cursor_byte_pos_at_char_index(clamped_end);
        self.input.replace_range(byte_start..byte_end, "");
        self.set_cursor_char_index_clamped(clamped_start);
        self.refresh_input_menus();
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn input_selection_char_range(&self, sel: &TextSelection) -> Option<(usize, usize)> {
        if !matches!(sel.pane, SelectionPane::Input) {
            return None;
        }
        let area = self.input_area;
        if area.width < 2 || area.height < 2 {
            return None;
        }
        let inner_x = area.x + 1;
        let inner_y = area.y + 1;
        let inner_w = area.width.saturating_sub(2).max(1) as usize;
        let ((start_col, start_row), (end_col, end_row)) = sel.normalized();
        let start_disp = start_row.saturating_sub(inner_y) as usize * inner_w
            + start_col.saturating_sub(inner_x) as usize;
        let end_disp_exclusive = end_row.saturating_sub(inner_y) as usize * inner_w
            + end_col.saturating_sub(inner_x) as usize
            + 1;
        // The prompt prefix widens by two columns while the queue counter is
        // shown, so selection columns only line up with the input text when the
        // counter width is subtracted too (spec `inputqueue` NFR-002, FR-020).
        let prefix_len = crate::layout::input_prompt_prefix_len(self.input_queue_len());
        let display_len = self.input_len_chars() + prefix_len;
        let start_disp = start_disp.min(display_len);
        let end_disp_exclusive = end_disp_exclusive.min(display_len);
        if end_disp_exclusive <= start_disp {
            return None;
        }
        let start_input = start_disp
            .saturating_sub(prefix_len)
            .min(self.input_len_chars());
        let end_input = end_disp_exclusive
            .saturating_sub(prefix_len)
            .min(self.input_len_chars());
        if end_input <= start_input {
            None
        } else {
            Some((start_input, end_input))
        }
    }

    pub(crate) fn active_input_widget_area(&self) -> Rect {
        self.input_area
    }

    /// Return the byte offset in `self.input` corresponding to the current
    /// cursor character index.
    pub(crate) fn cursor_byte_pos(&self) -> usize {
        self.cursor_byte_pos_at_char_index(self.input_cursor)
    }

    /// Return the byte offset in `self.input` for a given character index.
    /// `char_index == 0` short-circuits to `0`.
    pub fn cursor_byte_pos_at_char_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        // Single pass: nth() returns None when char_index is past the end.
        self.input
            .char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or_else(|| self.input.len())
    }

    pub(crate) fn cursor_move_left(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
        }
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn cursor_move_right(&mut self) {
        if self.input_cursor < self.input_len_chars() {
            self.input_cursor += 1;
        }
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn cursor_move_word_left(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        // Walk backwards from the cursor byte position without materialising
        // the whole input into a char vector on every keypress.
        let cursor_byte = self.cursor_byte_pos();
        let mut cursor_chars = self.input_cursor;
        let mut byte = cursor_byte;
        let prev_is_ws = |b: usize| {
            self.input[..b]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
        };
        while cursor_chars > 0 && prev_is_ws(byte) {
            // Step back one char.
            let prev = self.input[..byte]
                .char_indices()
                .next_back()
                .map(|(b, _)| b)
                .unwrap_or(0);
            byte = prev;
            cursor_chars -= 1;
        }
        while cursor_chars > 0 && !prev_is_ws(byte) {
            let prev = self.input[..byte]
                .char_indices()
                .next_back()
                .map(|(b, _)| b)
                .unwrap_or(0);
            byte = prev;
            cursor_chars -= 1;
        }
        self.set_cursor_char_index_clamped(cursor_chars);
    }

    pub(crate) fn cursor_move_word_right(&mut self) {
        let mut cursor_chars = self.input_cursor;
        let mut byte = self.cursor_byte_pos();
        let total_chars = self.input_len_chars();
        let at_char = |b: usize| {
            self.input[b..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        };
        while cursor_chars < total_chars && !at_char(byte) {
            let next = byte + self.input[byte..].chars().next().map_or(1, char::len_utf8);
            byte = next;
            cursor_chars += 1;
        }
        while cursor_chars < total_chars && at_char(byte) {
            let next = byte + self.input[byte..].chars().next().map_or(1, char::len_utf8);
            byte = next;
            cursor_chars += 1;
        }
        self.set_cursor_char_index_clamped(cursor_chars);
    }

    pub(crate) fn cursor_move_home(&mut self) {
        self.input_cursor = 0;
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn cursor_move_end(&mut self) {
        self.input_cursor = self.input_len_chars();
        self.assert_input_cursor_invariant();
    }

    pub(crate) fn cursor_on_first_logical_line(&self) -> bool {
        let byte = self.cursor_byte_pos();
        !self.input[..byte].contains('\n')
    }

    pub(crate) fn cursor_on_last_logical_line(&self) -> bool {
        let byte = self.cursor_byte_pos();
        !self.input[byte..].contains('\n')
    }

    pub(crate) fn cursor_move_up_logical_line(&mut self) {
        let byte = self.cursor_byte_pos();
        let before = &self.input[..byte];
        let Some(nl_pos) = before.rfind('\n') else {
            return;
        };

        // Column (char count) within current line
        let line_start_byte = nl_pos + 1;
        let col = before[line_start_byte..].chars().count();

        // Previous line spans from after its preceding '\n' (or 0) to nl_pos
        let prev_line_start = before[..nl_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let prev_line_len = before[prev_line_start..nl_pos].chars().count();

        let target_col = col.min(prev_line_len);
        let new_char = self.input[..prev_line_start].chars().count() + target_col;
        self.set_cursor_char_index_clamped(new_char);
    }

    pub(crate) fn cursor_move_down_logical_line(&mut self) {
        let byte = self.cursor_byte_pos();
        let after = &self.input[byte..];
        let Some(nl_offset) = after.find('\n') else {
            return;
        };

        // Column within current line
        let before = &self.input[..byte];
        let line_start_byte = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let col = before[line_start_byte..].chars().count();

        // Next line
        let next_start = byte + nl_offset + 1;
        let next_line = &self.input[next_start..];
        let next_line_end = next_line.find('\n').unwrap_or(next_line.len());
        let next_line_len = next_line[..next_line_end].chars().count();

        let target_col = col.min(next_line_len);
        let new_char = self.input[..next_start].chars().count() + target_col;
        self.set_cursor_char_index_clamped(new_char);
    }

    /// Return the `(start, end)` character range of the current keyboard
    /// selection, or `None` when the anchor and cursor coincide.
    pub(crate) fn kb_selection_char_range(&self) -> Option<(usize, usize)> {
        let anchor = self.kb_select_anchor?;
        let cursor = self.input_cursor;
        if anchor == cursor {
            None
        } else if anchor < cursor {
            Some((anchor, cursor))
        } else {
            Some((cursor, anchor))
        }
    }

    pub(crate) fn copy_kb_selection(&mut self) {
        if let Some((start, end)) = self.kb_selection_char_range() {
            let selected: String = self.input.chars().skip(start).take(end - start).collect();
            Self::set_clipboard(&selected);
        }
    }

    pub(crate) fn cut_kb_selection(&mut self) {
        if let Some((start, end)) = self.kb_selection_char_range() {
            let selected: String = self.input.chars().skip(start).take(end - start).collect();
            Self::set_clipboard(&selected);
            self.remove_input_char_range(start, end);
            self.kb_select_anchor = None;
        }
    }

    pub(crate) fn paste_text_from_clipboard(&mut self) {
        if let Some(text) = Self::get_clipboard() {
            self.handle_paste_text(&text);
        }
    }

    /// Insert pasted text at the cursor, stripping `\r` and replacing any
    /// active keyboard or mouse selection.
    pub fn handle_paste_text(&mut self, text: &str) {
        let clean: String = text.chars().filter(|&c| c != '\r').collect();
        if clean.is_empty() {
            return;
        }

        // Replace a mouse-driven selection first.
        if let Some(sel) = self.text_selection.clone() {
            if let Some((start, end)) = self.input_selection_char_range(&sel) {
                self.remove_input_char_range(start, end);
            }
            self.text_selection = None;
        }

        // Replace the keyboard-driven selection, if any.
        if let Some((start, end)) = self.kb_selection_char_range() {
            self.remove_input_char_range(start, end);
            self.kb_select_anchor = None;
        }

        self.insert_text_at_cursor(&clean);
    }

    pub(crate) fn clear_kb_selection(&mut self) {
        self.kb_select_anchor = None;
    }

    pub(crate) fn delete_prev_word(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let end = self.input_cursor;
        self.cursor_move_word_left();
        let start = self.input_cursor;
        self.remove_input_char_range(start, end);
    }

    pub(crate) fn delete_to_end_of_line(&mut self) {
        let end = self.input_len_chars();
        self.remove_input_char_range(self.input_cursor, end);
    }

    /// Set the active code index handle and trigger stats refresh.
    pub(crate) fn set_code_index(&mut self, code_index: Option<Arc<ragent_codeindex::CodeIndex>>) {
        self.code_index = code_index;
    }

    /// Refresh cached code-index stats (file/symbol counts and busy flag)
    /// on a throttled interval (1s while busy, 5s otherwise).
    pub(crate) fn refresh_code_index_stats(&mut self) {
        let interval = if self.code_index_busy {
            std::time::Duration::from_secs(1)
        } else {
            std::time::Duration::from_secs(5)
        };
        if self.code_index_stats_last_refresh.elapsed() < interval {
            return;
        }
        if let Some(ref idx) = self.code_index {
            // Check progress atomics (lock-free) to detect active reindex
            // even when locks are momentarily free between chunks.
            // The reindex is active only while work remains (done < total);
            // a completed reindex keeps total > 0 forever, so comparing
            // against `done` here is what lets the busy latch clear once the
            // initial reindex finishes (idle-CPU fix: a permanently-true
            // `reindex_active` pinned compute_next_deadline at 250 ms).
            let (done, total) = idx.reindex_progress();
            let reindex_active = total > 0 && done < total;

            // Graph-build latch: the lock-free graph_busy atomic is set by
            // build_graph / build_graph_for_language / the graph phase of
            // full_reindex, so it stays observable even while the store lock
            // is held for the whole build. Read it BEFORE the try_status
            // probe: the graph build holds the store mutex for its entire
            // duration, so try_status() returning None while graph_busy is
            // set means the graph build holds the lock — not indexing.
            let graph_busy = idx.graph_busy();

            if let Some(stats) = idx.try_status() {
                self.code_index_stats_cache = Some(stats);
                self.code_index_stats_last_refresh = std::time::Instant::now();
                if self.code_index_busy && !reindex_active {
                    self.code_index_busy = false;
                    self.needs_redraw = true;
                }
            } else if graph_busy {
                // Store lock held by the graph build. Do not latch the idx
                // indicator from the lock-held heuristic; clear it if a
                // previous poll set it, so each indicator tracks its own
                // phase and clears at its own time instead of both
                // vanishing the moment the graph build finishes.
                if self.code_index_busy && !reindex_active {
                    self.code_index_busy = false;
                    self.needs_redraw = true;
                }
            } else {
                // Locks busy — indexing in progress
                if !self.code_index_busy {
                    self.code_index_busy = true;
                    self.needs_redraw = true;
                }
            }
            // If reindex counters indicate active work, keep busy flag set.
            if reindex_active && !self.code_index_busy {
                self.code_index_busy = true;
                self.needs_redraw = true;
            }
            if graph_busy != self.code_index_graph_busy {
                self.code_index_graph_busy = graph_busy;
                self.needs_redraw = true;
            }
        } else {
            self.code_index_stats_cache = None;
            self.code_index_stats_last_refresh = std::time::Instant::now();
            self.code_index_busy = false;
            if self.code_index_graph_busy {
                self.code_index_graph_busy = false;
                self.needs_redraw = true;
            }
        }
    }

    /// Spawn `/codeindex graph build` (or `graph lang <l>`) on the dedicated
    /// codeindex graph-build thread. The command handler sets a `[wait]`
    /// status; the spawned thread delivers the rendered result through
    /// `code_index_bg_result`, drained by `poll_codeindex_bg_result`.
    ///
    /// Returns `Ok(())` when the build was spawned, `Err(reason)` when it was
    /// refused (no index attached, or a build is already running).
    pub(crate) fn spawn_codeindex_graph_build(
        &mut self,
        language: Option<String>,
    ) -> Result<(), String> {
        let Some(idx) = self.code_index.clone() else {
            return Err("Code index is not active. Enable it first with `/codeindex on`.".into());
        };
        if self.code_index_graph_spawned {
            return Err("a graph build is already running".into());
        }
        self.code_index_graph_spawned = true;
        self.needs_redraw = true;
        let results = self.code_index_bg_result.clone();
        let spawn_result = std::thread::Builder::new()
            .name("codeindex-graph-build-request".into())
            .spawn(move || {
                let outcome = if let Some(lang) = language {
                    idx.build_graph_for_language(&lang).map(|result| {
                        if result.edges_total == 0 {
                            (
                                format!(
                                    "[warn] No edges found for language `{lang}`. Ensure \
                                     files of that language are indexed (run \
                                     `/codeindex reindex`)."
                                ),
                                format!("codeindex: graph lang {lang} (no edges)"),
                            )
                        } else {
                            (
                                format!(
                                    "\u{2705} Graph built for `{lang}`: {} edges \
                                     ({} EXTRACTED, {} INFERRED) in {}ms.",
                                    result.edges_total,
                                    result.edges_extracted,
                                    result.edges_inferred,
                                    result.elapsed_ms
                                ),
                                format!(
                                    "codeindex: graph lang {lang} ({} edges)",
                                    result.edges_total
                                ),
                            )
                        }
                    })
                } else {
                    idx.build_graph().map(|result| {
                        (
                            format!(
                                "\u{2705} Graph built: {} edges ({} EXTRACTED, {} INFERRED) \
                                 in {}ms.",
                                result.edges_total,
                                result.edges_extracted,
                                result.edges_inferred,
                                result.elapsed_ms
                            ),
                            format!("codeindex: graph built ({} edges)", result.edges_total),
                        )
                    })
                };
                let payload = outcome.map_err(|e| format!("graph build failed: {e}"));
                if let Ok(mut guard) = results.lock() {
                    *guard = Some(payload);
                }
            });
        if let Err(e) = spawn_result {
            // Roll the latch back so a later command can retry.
            self.code_index_graph_spawned = false;
            return Err(format!("failed to spawn graph build thread: {e}"));
        }
        Ok(())
    }

    /// Spawn a full reindex on a dedicated thread so the TUI event loop stays
    /// responsive. Progress is observable via the existing reindex atomics
    /// (and the graph phase via the graph-busy indicator).
    pub(crate) fn spawn_codeindex_reindex(&mut self) -> Result<(), String> {
        let Some(idx) = self.code_index.clone() else {
            return Err("Code index is not active. Enable it first with `/codeindex on`.".into());
        };
        if self.code_index_reindex_spawned {
            return Err("a reindex is already running".into());
        }
        self.code_index_reindex_spawned = true;
        self.needs_redraw = true;
        let results = self.code_index_bg_result.clone();
        let spawn_result = std::thread::Builder::new()
            .name("codeindex-manual-reindex".into())
            .spawn(move || {
                let payload = match idx.full_reindex() {
                    Ok(result) => Ok((
                        format!(
                            "[ok] Re-index complete: +{} ~{} -{} files, {} symbols in {}ms.",
                            result.files_added,
                            result.files_updated,
                            result.files_removed,
                            result.symbols_extracted,
                            result.elapsed_ms
                        ),
                        format!(
                            "codeindex: reindexed {} files",
                            result.files_added + result.files_updated
                        ),
                    )),
                    Err(e) => Err(format!("Re-index failed: {e}")),
                };
                if let Ok(mut guard) = results.lock() {
                    *guard = Some(payload);
                }
            });
        if let Err(e) = spawn_result {
            self.code_index_reindex_spawned = false;
            return Err(format!("failed to spawn reindex thread: {e}"));
        }
        Ok(())
    }

    /// Drain the completion result from an off-thread codeindex graph build
    /// or full reindex and surface it in the message window. Mirrors
    /// `poll_compaction_result`; the success payload carries the message and the
    /// status-bar text as a tuple.
    pub fn poll_codeindex_bg_result(&mut self) {
        let outcome = {
            let mut guard = match self.code_index_bg_result.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take()
        };
        let Some(outcome) = outcome else {
            return;
        };
        self.code_index_graph_spawned = false;
        self.code_index_reindex_spawned = false;
        match outcome {
            Ok((message, status)) => {
                self.append_assistant_text(&message);
                self.status = status;
                self.arm_status_expiry();
                self.push_log_no_agent(
                    LogLevel::Info,
                    "Finished /codeindex background task".to_string(),
                );
            }
            Err(msg) => {
                self.append_assistant_text(&format!("[err] {msg}"));
                self.status = "[warn] codeindex: background task failed".to_string();
                self.push_log_no_agent(LogLevel::Error, msg);
            }
        }
        self.needs_redraw = true;
    }

    /// Drain the loop-rollback result (FR-020/T-013) deposited by the async
    /// rollback task and surface it in the status line and message window.
    /// Without this poll the status stays at "rolling back to pre-loop
    /// snapshot…" forever and a failed restore is invisible to the user.
    pub fn poll_rollback_result(&mut self) {
        let outcome = {
            let mut guard = match self.rollback_result.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take()
        };
        let Some(outcome) = outcome else {
            return;
        };
        match outcome {
            Ok(true) => {
                self.status = "rollback complete".to_string();
                self.append_assistant_text(
                    "[ok] Rolled back to the pre-loop snapshot; loop changes reverted.",
                );
                self.push_log_no_agent(
                    LogLevel::Info,
                    "Loop rollback completed (FR-020)".to_string(),
                );
            }
            Ok(false) => {
                self.status = "rollback: nothing to restore".to_string();
                self.append_assistant_text("No pending pre-loop capture was found to restore.");
            }
            Err(e) => {
                self.status = format!("[err] rollback failed: {e}");
                self.append_assistant_text(&format!(
                    "[err] Rollback failed: {e}. The pre-loop capture is kept — retry \
                     the rollback to try again."
                ));
                self.push_log_no_agent(LogLevel::Error, format!("rollback error: {e}"));
            }
        }
        self.needs_redraw = true;
    }

    /// Drain the `/websearch test` engine-diagnostic result deposited by the
    /// spawned test task and surface it in the message window. Without this
    /// poll the command would have to block the UI thread for the whole
    /// multi-engine network probe, deferring the "Starting Websearch
    /// test…" acknowledgement until after the test had already finished.
    pub fn poll_websearch_test_result(&mut self) {
        let outcome = {
            let mut guard = match self.websearch_test_result.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.take()
        };
        let Some(output) = outcome else {
            return;
        };
        self.append_assistant_text(&output);
        self.status = "websearch: engine test complete".to_string();
        self.needs_redraw = true;
    }

    /// Test hook: exposes the crate-internal stats refresh to integration
    /// tests (busy-latch regression coverage). Not part of the public API.
    #[doc(hidden)]
    pub fn refresh_code_index_stats_for_test(&mut self) {
        self.refresh_code_index_stats();
    }

    /// Refresh cached structured-memory stats on a throttled 5s interval.
    pub(crate) fn refresh_memory_stats(&mut self) {
        if self.memory_stats_last_refresh.elapsed() < std::time::Duration::from_secs(5) {
            return;
        }
        self.memory_stats_last_refresh = std::time::Instant::now();
        let storage = self.storage.clone();
        let project_dir = crate::app::helpers::current_working_dir();
        let pending = self.memory_entry_count_pending.clone();
        tokio::task::spawn_blocking(move || {
            let count = storage
                .count_memories_for_project(&project_dir)
                .unwrap_or(0);
            pending.store(count, std::sync::atomic::Ordering::Relaxed);
        });
    }
    /// Map the primary session's short id to the current agent name for log display.
    pub(crate) fn register_primary_session_mapping(&mut self) {
        if let Some(ref sid) = self.session_id {
            let short_sid = short_session_id(sid);
            self.sid_to_display_name
                .insert(short_sid, self.agent_name.clone());
        }
    }

    /// Persist a discovered MCP server into `ragent.json`. Returns an error
    /// message when the server is already configured.
    pub(crate) fn enable_discovered_mcp_server(
        &self,
        server: &DiscoveredMcpServer,
    ) -> Result<String, String> {
        use ragent_agent::Config;

        // Load the current config, surfacing a parse error so a malformed
        // ragent.json is not silently replaced by an empty default (which
        // could clobber the user's real settings on save).
        let config = Config::load().map_err(|e| format!("failed to load config: {e}"))?;

        if config.mcp.contains_key(&server.id) {
            return Err(format!(
                "'{}' is already in ragent.json. Edit it manually to change settings.",
                server.id
            ));
        }

        // Persist back to ragent.json in the working directory.
        let config_path = std::env::current_dir()
            .unwrap_or_default()
            .join(".ragent")
            .join("ragent.json");

        let server_id = server.id.clone();
        let mcp_entry = serde_json::json!({
            "type": "stdio",
            "command": server.executable.to_string_lossy(),
            "args": server.args,
            "env": server.env,
            "disabled": false,
        });

        atomic_config_update(&config_path, |json| {
            json["mcp"][&server_id] = mcp_entry;
            Ok(())
        })?;

        Ok(format!(
            "✓ '{}' added to ragent.json. Restart ragent to activate the MCP server.",
            server.id
        ))
    }

    pub(crate) fn ensure_session(&mut self) -> bool {
        if self.session_id.is_some() {
            return true;
        }
        let dir = crate::app::helpers::current_working_dir();
        match self.session_processor.session_manager.create_session(dir) {
            Ok(session) => {
                self.session_id = Some(session.id.clone());
                // A fresh session starts with an empty queue (NFR-005).
                self.clear_input_queue();
                // Map the primary session's short_sid to the current agent name
                let short_sid = short_session_id(&session.id);
                self.sid_to_display_name
                    .insert(short_sid, self.agent_name.clone());
                true
            }
            Err(e) => {
                // Surface a visible assistant message so slash commands don't fail silently.
                self.status = format!("error: {}", e);
                let msg = format!("From: /system\nFailed to create session: {}", e);
                self.append_assistant_text(&msg);
                false
            }
        }
    }

    /// Sync the in-memory `team_members` list with the on-disk team store,
    /// copying session ids, status, and current task ids so the UI reflects the
    /// authoritative persisted state. Also registers session_id → teammate
    /// name mappings for log display.
    ///
    /// **Note:** This method is no longer called from the render path
    /// (FR-009).  Team member state is kept in sync by event handlers
    /// (`TeammateSpawned`, `TeammateIdle`, `TeamTaskClaimed`, etc.).  The
    /// method is retained for explicit one-shot refreshes when a team is
    /// first opened.
    #[allow(dead_code)]
    pub(crate) fn refresh_team_member_session_ids(&mut self) {
        let Some(team_name) = self.active_team.as_ref().map(|t| t.name.clone()) else {
            return;
        };
        let working_dir = crate::app::helpers::current_working_dir();
        let Ok(store) = TeamStore::load_by_name(&team_name, &working_dir) else {
            return;
        };

        for member in &mut self.team_members {
            // If a stored entry exists for this agent, copy session_id, status,
            // and current_task_id so the UI reflects the authoritative on-disk state.
            if let Some(stored_member) = store
                .config
                .members
                .iter()
                .find(|m| m.agent_id == member.agent_id)
            {
                if member.session_id.is_none() {
                    if let Some(sid) = &stored_member.session_id {
                        member.session_id = Some(sid.clone());
                    }
                }
                // Always sync status and current task from the store so races
                // between disk hydration and spawn events don't leave the UI
                // showing an outdated "spawning" state.
                member.status = stored_member.status.clone();
                member.current_task_id = stored_member.current_task_id.clone();
            }
        }
        // Register session_id → teammate name mappings for log display.
        for member in &self.team_members {
            if let Some(ref sid) = member.session_id {
                let short_sid = short_session_id(sid);
                self.sid_to_display_name
                    .entry(short_sid)
                    .or_insert_with(|| member.name.clone());
            }
        }
    }

    /// Load a session by id, replacing the current messages and updating the
    /// session_id mapping. Returns an error if the session cannot be found.
    pub fn load_session(&mut self, session_id: &str) -> anyhow::Result<()> {
        let session = self
            .storage
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        let messages = self.storage.get_messages(session_id)?;
        let msg_count = messages.len();

        self.session_id = Some(session_id.to_string());
        // Switching or resuming a session drops any queued messages from the
        // previous session (NFR-005: the queue is not persisted and does not
        // cross a session boundary).
        self.clear_input_queue();
        // Cache the resumed session's creation timestamp for the teams panel
        // elapsed-time display (FR-009: avoids per-frame storage reads).
        self.lead_session_created_at = chrono::DateTime::parse_from_rfc3339(&session.created_at)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc));
        // Map the primary session's short_sid to the current agent name
        let short_sid = short_session_id(session_id);
        self.sid_to_display_name
            .insert(short_sid, self.agent_name.clone());
        self.messages = messages;
        // Structural change: restored messages replace the current timeline,
        // so the per-message render cache must be rebuilt from scratch.
        self.reset_message_cache();
        self.current_screen = ScreenMode::Chat;
        self.status = format!("resumed ({} messages)", msg_count);

        // Rebuild tool_step_map from restored tool calls and populate log
        // (step count comes from event_bus, not local counter)
        self.tool_step_map.clear();
        self.last_step_per_session.clear();
        self.substep_counter_per_session.clear();
        self.sid_to_display_name.clear();
        // Map the primary session's short_sid to the current agent name
        let short_sid = short_session_id(session_id);
        self.sid_to_display_name
            .insert(short_sid, self.agent_name.clone());
        let mut restored_logs: Vec<(u32, u32, String, String)> = Vec::new();
        let mut step_counter = 0u32;
        for msg in &self.messages {
            for part in &msg.parts {
                if let MessagePart::ToolCall {
                    call_id,
                    tool,
                    state,
                } = part
                {
                    // For restoration, treat each tool call as a unique step.1
                    step_counter += 1;
                    let substep = 1u32;
                    let short_sid = self
                        .session_id
                        .as_deref()
                        .map(short_session_id)
                        .unwrap_or_default();
                    self.tool_step_map
                        .insert(call_id.clone(), (short_sid, step_counter, substep));
                    let icon = match state.status {
                        ragent_agent::message::ToolCallStatus::Completed => "✓",
                        ragent_agent::message::ToolCallStatus::Error => "✗",
                        _ => "…",
                    };
                    restored_logs.push((step_counter, substep, tool.clone(), icon.to_string()));
                }
            }
        }
        for (step, substep, tool, icon) in restored_logs {
            let _short_sid = self
                .session_id
                .as_deref()
                .map(short_session_id)
                .unwrap_or_default();
            self.push_log_no_agent(
                LogLevel::Tool,
                format!("[{step}.{substep}] {tool} {icon} (restored)"),
            );
        }

        // Update cwd to match the session's working directory. Keep the
        // display string (`~`-collapsed) and the real path in sync so path
        // consumers (`cwd_path`, e.g. the research manager root) follow the
        // resumed session's directory.
        if !session.directory.is_empty() {
            self.cwd_path = Self::expand_home_path(std::path::Path::new(&session.directory));
            self.cwd = Self::collapse_home_path(&self.cwd_path);
        }

        // T-010/FR-013: the conversation history was just replaced; refresh
        // the Context panel snapshot so the History partition and message
        // count reflect the resumed session rather than the previous one.
        self.schedule_context_snapshot_refresh();

        self.push_log_no_agent(
            LogLevel::Info,
            format!(
                "Resumed session {} ({} messages)",
                &session_id[..8.min(session_id.len())],
                msg_count
            ),
        );

        Ok(())
    }

    pub(crate) fn detect_git_branch() -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if branch.is_empty() {
                None
            } else {
                Some(branch)
            }
        } else {
            None
        }
    }

    pub(crate) fn current_config(&self) -> ragent_agent::Config {
        ragent_agent::Config::load().unwrap_or_default()
    }

    /// Compute an [`LlmStatsSummary`] (averages of elapsed time, prompt and
    /// output tokens/sec) for samples belonging to the currently active model.
    /// Returns `None` when no samples are recorded for that model.
    pub(crate) fn llm_stats_summary(&self) -> Option<LlmStatsSummary> {
        let model_ref = self.active_model_ref_string()?;
        let samples: Vec<&LlmRequestStat> = self
            .llm_request_stats
            .iter()
            .filter(|sample| sample.model_ref == model_ref)
            .collect();
        if samples.is_empty() {
            return None;
        }

        let count = samples.len() as f64;
        let avg_elapsed_ms = samples.iter().map(|s| s.elapsed_ms as f64).sum::<f64>() / count;
        let avg_prompt_tps = samples
            .iter()
            .map(|s| Self::tokens_per_second(s.input_tokens, s.elapsed_ms))
            .sum::<f64>()
            / count;
        let avg_output_tps = samples
            .iter()
            .map(|s| Self::tokens_per_second(s.output_tokens, s.elapsed_ms))
            .sum::<f64>()
            / count;

        Some(LlmStatsSummary {
            samples: samples.len(),
            avg_elapsed_ms,
            avg_prompt_tps,
            avg_output_tps,
        })
    }

    pub(crate) fn tokens_per_second(tokens: u64, elapsed_ms: u64) -> f64 {
        if elapsed_ms == 0 {
            return 0.0;
        }
        tokens as f64 / (elapsed_ms as f64 / 1000.0)
    }

    /// Build the usage display string (provider quota % or token-rate info)
    /// and a flag indicating whether the value is a rate limit.
    pub fn usage_display(&self) -> (String, bool) {
        let provider_id = self
            .configured_provider
            .as_ref()
            .map(|p| p.id.as_str())
            .unwrap_or("");

        // Provider rate-limit quota % takes priority when available.
        if let Some(quota) = self.quota_percent {
            let label = if provider_id == "copilot" {
                let plan = ragent_agent::provider::copilot::cached_copilot_plan()
                    .unwrap_or_else(|| "Copilot".to_string());
                format!("{} quota: {:.1}%", plan, quota)
            } else {
                format!("quota: {:.1}%", quota)
            };
            return (label, false);
        }

        let ctx_detail = self.context_window_display();
        let ctx_label = |prefix: &str| -> String {
            match ctx_detail.as_deref() {
                Some(detail) if prefix.is_empty() => format!("ctx: {detail}"),
                Some(detail) => format!("{prefix} ctx: {detail}"),
                None => prefix.to_string(),
            }
        };

        if provider_id == "copilot" {
            let plan = ragent_agent::provider::copilot::cached_copilot_plan()
                .unwrap_or_else(|| "Copilot".to_string());
            (ctx_label(&plan), false)
        } else if provider_id == "ollama" || provider_id == "ollama_cloud" {
            let label = ctx_label("");
            if label.is_empty() {
                (
                    if provider_id == "ollama" {
                        "local"
                    } else {
                        "ollama"
                    }
                    .to_string(),
                    false,
                )
            } else {
                (label, false)
            }
        } else {
            let label = ctx_label("");
            if label.is_empty() {
                ("unknown".to_string(), true)
            } else {
                (label, false)
            }
        }
    }

    /// Build the `"pct usedK/contextK"` context-window usage display, or
    /// `None` when no model context window is configured.
    pub(crate) fn context_window_display(&self) -> Option<String> {
        let ctx_window = self.selected_model_context_window()?;
        let pct = (self.last_input_tokens as f32 / ctx_window as f32 * 100.0).min(100.0);
        Some(format!(
            "{pct:.0}% {}K/{}K",
            self.last_input_tokens / 1000,
            ctx_window / 1000
        ))
    }

    /// Refresh the file-mention autocomplete menu based on the active mention
    /// span. Populates directory listings when navigating into a directory,
    /// otherwise fuzzy-matches against the cached project file list.
    pub fn update_file_menu(&mut self) {
        let Some(active) = self.active_mention_span() else {
            self.file_menu = None;
            return;
        };
        let query = active.query(&self.input).to_string();

        if let Some(dir) = self.file_menu.as_ref().and_then(|m| m.current_dir.clone()) {
            self.populate_directory_menu(&dir, Some(&query));
            return;
        }

        // Lazily populate or refresh the project file cache when cwd changes.
        let wd = crate::app::helpers::current_working_dir();
        let cache_stale = self
            .project_files_cache_cwd
            .as_ref()
            .is_none_or(|cached| *cached != wd);
        if self.project_files_cache.is_none() || cache_stale {
            self.refresh_project_files_cache();
        }

        if let Some(ref candidates) = self.project_files_cache {
            let matches = ragent_agent::reference::fuzzy::fuzzy_match(&query, candidates);

            let entries: Vec<FileMenuEntry> = matches
                .into_iter()
                .take(15)
                .map(|m| {
                    let is_dir = m.path.to_string_lossy().ends_with('/');
                    FileMenuEntry {
                        display: m.path.to_string_lossy().to_string(),
                        path: m.path,
                        is_dir,
                    }
                })
                .collect();

            let prev_selected = self.file_menu.as_ref().map(|m| m.selected).unwrap_or(0);
            self.file_menu = Some(FileMenuState {
                selected: prev_selected.min(entries.len().saturating_sub(1)),
                matches: entries,
                scroll_offset: 0,
                query,
                current_dir: None,
            });
        } else {
            self.file_menu = None;
        }
    }

    /// Accept the currently-selected file-menu entry: navigates into a
    /// directory or inserts the chosen file path into the input buffer.
    /// Returns `false` when the menu is empty or no selection can be made.
    pub fn accept_file_menu_selection(&mut self) -> bool {
        if self
            .file_menu
            .as_ref()
            .is_some_and(|m| m.matches.is_empty())
        {
            return false;
        }
        // Clone the selected entry out of the menu to avoid holding an
        // immutable borrow of self while we call mutating methods below.
        let selected_entry: Option<FileMenuEntry> = self
            .file_menu
            .as_ref()
            .and_then(|m| m.matches.get(m.selected).cloned());

        if let Some(entry) = selected_entry {
            if entry.is_dir {
                if entry.display == "<back to fuzzy>" {
                    self.update_file_menu();
                    return false;
                }
                // Navigate into the directory instead of inserting it.
                self.populate_directory_menu(&entry.path, None);
                return false;
            } else {
                // Insert file path into the input and close the menu.
                let path = entry.display.clone();
                if let Some(active) = self.active_mention_span() {
                    let replacement = format!("@{path}");
                    self.input
                        .replace_range(active.at_start..active.token_end, &replacement);
                    let cursor_chars =
                        self.input[..active.at_start].chars().count() + replacement.chars().count();
                    self.set_cursor_char_index_clamped(cursor_chars);
                } else {
                    self.file_menu = None;
                    return false;
                }
                self.file_menu = None;
                return true;
            }
        }

        self.file_menu = None;
        false
    }

    pub(crate) fn mention_spans(&self) -> Vec<MentionSpan> {
        let bytes = self.input.as_bytes();
        let mut spans = Vec::new();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'@' {
                if i > 0 {
                    let prev = bytes[i - 1];
                    if prev.is_ascii_alphanumeric() || prev == b'.' {
                        i += 1;
                        continue;
                    }
                }
                let at_start = i;
                i += 1;
                let token_start = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i > token_start {
                    spans.push(MentionSpan {
                        at_start,
                        token_start,
                        token_end: i,
                    });
                }
                continue;
            }
            i += 1;
        }
        spans
    }

    pub(crate) fn active_mention_span(&self) -> Option<MentionSpan> {
        let cursor = self.cursor_byte_pos();
        let spans = self.mention_spans();
        spans
            .iter()
            .find(|span| cursor >= span.at_start && cursor <= span.token_end)
            .copied()
    }

    /// Map a screen coordinate to the side-panel pane it lands in.
    ///
    /// Returns the [`SelectionPane`] (Messages / Profile / Log / Todo /
    /// Memory / Input) whose cached area contains `(col, row)`, or `None`
    /// when the coordinate is outside every active pane. Side-panel panes
    /// (Profile / Log / Todo / Memory) are only reported when their
    /// corresponding `show_*` flag is true, so hidden panels never win
    /// hit-testing even if their cached rect is stale. This is the single
    /// mouse hit-testing entry point used by `handle_mouse_event` for
    /// left-click selection start, right-click context-menu open, and
    /// scrollbar-gutter detection (FR-013).
    pub fn pane_at(&self, col: u16, row: u16) -> Option<SelectionPane> {
        let pos = (col, row).into();
        if self.message_area.area() > 0 && self.message_area.contains(pos) {
            Some(SelectionPane::Messages)
        } else if self.show_profile
            && self.profile_area.area() > 0
            && self.profile_area.contains(pos)
        {
            Some(SelectionPane::Profile)
        } else if self.show_log && self.log_area.area() > 0 && self.log_area.contains(pos) {
            Some(SelectionPane::Log)
        } else if self.show_tasks_panel
            && self.tasks_area.area() > 0
            && self.tasks_area.contains(pos)
        {
            Some(SelectionPane::Tasks)
        } else if self.show_memory && self.memory_area.area() > 0 && self.memory_area.contains(pos)
        {
            Some(SelectionPane::Memory)
        } else if self.show_telemetry
            && self.telemetry_area.area() > 0
            && self.telemetry_area.contains(pos)
        {
            Some(SelectionPane::Telemetry)
        } else if self.show_context_panel
            && self.context_panel_area.area() > 0
            && self.context_panel_area.contains(pos)
        {
            Some(SelectionPane::ContextPanel)
        } else if self.input_area.area() > 0 && self.input_area.contains(pos) {
            Some(SelectionPane::Input)
        } else {
            None
        }
    }

    /// Extract a text selection spanning `[start_col, start_row]` to
    /// `[end_col, end_row]` from `lines`, which are positioned at the inner
    /// origin `(inner_x, inner_y)`. Joins multi-line selections with `\n`.
    pub fn extract_text_from_lines(
        lines: &[String],
        inner_x: u16,
        inner_y: u16,
        start_col: u16,
        start_row: u16,
        end_col: u16,
        end_row: u16,
    ) -> String {
        let mut result = String::new();
        for screen_row in start_row..=end_row {
            let line_idx = screen_row.saturating_sub(inner_y) as usize;
            let line = lines.get(line_idx).map(|s| s.as_str()).unwrap_or("");
            let line_start = if screen_row == start_row {
                start_col.saturating_sub(inner_x) as usize
            } else {
                0
            };
            let line_end = if screen_row == end_row {
                end_col.saturating_sub(inner_x) as usize + 1
            } else {
                line.chars().count()
            };
            let line_char_count = line.chars().count();
            let start = line_start.min(line_char_count);
            let end = line_end.min(line_char_count);
            if start < end {
                result.extend(line.chars().skip(start).take(end - start));
            }
            if screen_row < end_row {
                result.push('\n');
            }
        }
        result
    }

    pub(crate) fn set_clipboard(text: &str) {
        crate::clipboard::set_clipboard_text(text);
    }

    pub(crate) fn get_clipboard() -> Option<String> {
        crate::clipboard::get_clipboard_text()
    }

    /// Paste an image (or image file path) from the clipboard into the pending
    /// attachments. Checks the text clipboard first for a `file://` URI or
    /// path, then falls back to raw pixel data which is saved to
    /// `target/temp/` with restrictive permissions.
    pub(crate) fn paste_image_from_clipboard(&mut self) {
        // --- Phase 1: look for a file reference in the text clipboard ---
        if let Some(text) = Self::get_clipboard() {
            let trimmed = text.trim();

            // Resolve file:// URI
            let candidate = if let Some(rest) = trimmed.strip_prefix("file://") {
                Some(percent_decode_path(rest))
            } else if trimmed.starts_with('/') || trimmed.starts_with('.') {
                // Plain absolute or relative path
                Some(std::path::PathBuf::from(trimmed))
            } else {
                None
            };

            if let Some(path) = candidate {
                if path.exists() && is_image_path(&path) {
                    self.warn_if_path_outside_safe_scope(&path);
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!(
                            "[attach] Image attached from clipboard path: {}",
                            path.display()
                        ),
                    );
                    self.pending_attachments.push(path);
                    return;
                }
            }
        }

        // --- Phase 2: try raw pixel data ---
        if let Some(img_data) = crate::clipboard::get_clipboard_image() {
            match save_clipboard_image_to_temp(&img_data) {
                Ok(path) => {
                    self.push_log_no_agent(
                        LogLevel::Info,
                        format!("[attach] Image saved from clipboard: {}", path.display()),
                    );
                    self.pending_attachments.push(path);
                }
                Err(e) => {
                    self.push_log_no_agent(
                        LogLevel::Warn,
                        format!("Failed to save clipboard image: {e}"),
                    );
                }
            }
        } else {
            self.push_log_no_agent(
                LogLevel::Info,
                "No image data found in clipboard".to_string(),
            );
        }
    }

    /// Log a warning when a clipboard-resolved image path lies outside the
    /// current working directory or the user's home directory. The file is
    /// still attached (the user may intentionally want a screenshot or asset
    /// from elsewhere), but the warning provides a visible security nudge.
    fn warn_if_path_outside_safe_scope(&mut self, path: &std::path::Path) {
        let cwd = crate::app::helpers::current_working_dir();
        let home = dirs::home_dir().unwrap_or_default();
        let inside_cwd = path.strip_prefix(&cwd).is_ok();
        let inside_home = path.strip_prefix(&home).is_ok();
        if !inside_cwd && !inside_home {
            tracing::warn!(
                path = %path.display(),
                cwd = %cwd.display(),
                "clipboard image path is outside the working directory and home directory"
            );
            self.push_log_no_agent(
                LogLevel::Warn,
                format!(
                    "[warn] Clipboard image path is outside the working directory and home: {}. \
                       Attaching anyway.",
                    path.display()
                ),
            );
        }
    }

    /// Execute a context-menu action (copy/cut/paste/etc.) against the current
    /// text selection, dismissing the menu afterwards.
    pub fn execute_context_action(&mut self, action: ContextAction) {
        let pane = self.context_menu.as_ref().map(|m| m.pane);
        let selection = self.text_selection.clone();
        self.context_menu = None;

        match action {
            ContextAction::Copy => {
                self.copy_selection(false);
            }
            ContextAction::Cut => {
                // Copy selected text then remove only the selected span in input pane.
                self.copy_selection(true);
                if matches!(pane, Some(SelectionPane::Input)) {
                    if let Some(sel) = selection.as_ref()
                        && let Some((start, end)) = self.input_selection_char_range(sel)
                    {
                        self.remove_input_char_range(start, end);
                    }
                }
            }
            ContextAction::Paste => {
                if matches!(
                    self.provider_setup,
                    Some(ProviderSetupStep::EnterKey { .. })
                        | Some(ProviderSetupStep::GitLabSetup { .. })
                        | Some(ProviderSetupStep::TelemetrySetup { .. })
                ) {
                    self.paste_provider_setup_from_clipboard();
                } else if matches!(pane, Some(SelectionPane::Input)) {
                    if let Some(text) = Self::get_clipboard() {
                        self.handle_paste_text(&text);
                    }
                }
            }
        }
    }

    pub(crate) fn apply_scrollbar_drag(&mut self, mouse_y: u16, pane: ScrollbarDragPane) {
        let (area, max_scroll) = match pane {
            ScrollbarDragPane::Messages => (self.message_area, self.message_max_scroll),
            ScrollbarDragPane::Log => (self.log_area, self.log_max_scroll),
            ScrollbarDragPane::Profile => (self.profile_area, self.profile_max_scroll),
            ScrollbarDragPane::Tasks => (self.tasks_area, self.tasks_max_scroll),
            ScrollbarDragPane::Memory => (self.memory_area, self.memory_max_scroll),
            ScrollbarDragPane::Telemetry => (self.telemetry_area, self.telemetry_max_scroll),
            ScrollbarDragPane::ContextPanel => {
                (self.context_panel_area, self.context_panel_max_scroll)
            }
        };
        if area.height <= 1 || max_scroll == 0 {
            return;
        }

        // Clamp mouse_y to the pane area
        let y = mouse_y.clamp(area.y, area.bottom().saturating_sub(1));
        let relative = y.saturating_sub(area.y) as f32;
        let track_height = (area.height.saturating_sub(1)) as f32;
        let fraction = (relative / track_height).clamp(0.0, 1.0);

        // fraction 0.0 = top of scrollbar track, 1.0 = bottom.
        // Messages, Log, Profile, and Tasks use "lines from bottom"
        // semantics (scroll_offset = 0 -> bottom of content, max_scroll
        // -> top; render_tasks_panel maps the stored offset via
        // max_scroll - scroll), so dragging to the top of the track must
        // produce offset = max_scroll: offset = (1.0 - fraction) *
        // max_scroll.  Memory and Telemetry use "lines from top"
        // semantics (scroll_offset = 0 -> top of content, max_scroll ->
        // bottom), so dragging to the top must produce offset = 0 and
        // dragging to the bottom must produce offset = max_scroll:
        // offset = fraction * max_scroll.  Using a single formula for all
        // panels inverts the drag direction for one of the two families,
        // which is the erratic-scrollbar bug.
        let offset = match pane {
            ScrollbarDragPane::Messages
            | ScrollbarDragPane::Log
            | ScrollbarDragPane::Profile
            | ScrollbarDragPane::Tasks => ((1.0 - fraction) * max_scroll as f32).round() as u16,
            ScrollbarDragPane::Memory
            | ScrollbarDragPane::Telemetry
            | ScrollbarDragPane::ContextPanel => (fraction * max_scroll as f32).round() as u16,
        };

        match pane {
            ScrollbarDragPane::Messages => self.scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Log => self.log_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Profile => self.profile_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Tasks => self.tasks_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Memory => self.memory_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::Telemetry => self.telemetry_scroll_offset = offset.min(max_scroll),
            ScrollbarDragPane::ContextPanel => self.context_scroll_offset = offset.min(max_scroll),
        }
    }

    pub(crate) fn execute_plan_restore(&mut self, session_id: &str, summary: &str) {
        if let Some(prev_agent) = self.agent_stack.pop() {
            let from_name = self.agent_name.clone();
            let to_name = prev_agent.name.clone();

            self.agent_info = prev_agent;
            self.agent_name = to_name.clone();
            self.status = format!("agent: {}", to_name);
            self.push_log_no_agent(LogLevel::Info, format!("plan restore: plan → {}", to_name));

            self.event_bus.publish(Event::AgentSwitched {
                session_id: session_id.to_string(),
                from: from_name,
                to: to_name,
            });

            // Inject the plan summary into the chat so the restored agent
            // can see it in context.
            let plan_text = format!("📋 **Plan summary:**\n{}", summary);
            self.append_assistant_text(&plan_text);

            // Offer /swarm as an execution option after plan completion
            self.append_assistant_text(
                "\n💡 **Tip:** You can execute this plan in parallel with `/swarm <goal>`, \
                 or implement it step-by-step.\n",
            );
            self.force_new_message = true;
        } else {
            self.push_log_no_agent(
                LogLevel::Error,
                "plan_exit called but agent stack is empty".to_string(),
            );
        }
    }

    /// Push a log entry at the given level, tagging it with an optional agent
    /// id and an optional explicit session id. When `session_id` is `None`, the
    /// entry is stamped with the primary TUI session id so it appears in the
    /// main log panel and the primary output-view overlay. Tracked sub-agent
    /// and teammate events should pass the child/teammate session id so the
    /// per-agent output-view overlay can include their activity.
    pub(crate) fn push_log_for(
        &mut self,
        level: LogLevel,
        message: String,
        agent_id: Option<String>,
        session_id: Option<String>,
    ) {
        // C-008: stamp each entry with its own monotonic sequence number so a
        // new log line only invalidates the *new* group in `log_line_cache`,
        // not the whole cache (the cache compares against `entry.seq`).
        self.log_seq = self.log_seq.wrapping_add(1);
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            level,
            message: message.clone(),
            session_id: session_id.or_else(|| self.session_id.clone()),
            agent_id,
            seq: self.log_seq,
        };
        self.log_entries.push(entry);
        // Keep the per-entry cache in lockstep: push a fresh (stale) group
        // now so the render path never has to reconcile a length mismatch.
        self.log_line_cache.push(crate::app::LogLineGroup {
            lines: Vec::new(),
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            version: 0, // stale → rendered on next frame
        });
        // R-11: Cap log entries with FIFO eviction so the Vec (and its
        // mirror `log_line_cache`) do not grow without bound over a long
        // session.
        self.trim_log_entries_if_needed();
        if self.show_log {
            if let Some(ref path) = self.log_window_path {
                self.append_log_entry_to_spool(path, level, &message);
            }
        }
    }

    /// Append a single formatted log line to the log-window spool file.
    fn append_log_entry_to_spool(&self, path: &std::path::Path, level: LogLevel, message: &str) {
        use std::io::Write;
        let level_str = match level {
            LogLevel::Info => "INF",
            LogLevel::Tool => "TUL",
            LogLevel::Warn => "WRN",
            LogLevel::Error => "ERR",
        };
        let ts = chrono::Utc::now().to_rfc3339();
        let line = format!("{ts} {level_str} {message}\n");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            if let Err(e) = file.write_all(line.as_bytes()) {
                tracing::warn!(error = %e, "failed to append log entry to spool");
            }
        }
    }

    /// Convenience wrapper for [`push_log_for`](Self::push_log_for) that stamps
    /// the entry with the primary TUI session id.
    pub(crate) fn push_log(&mut self, level: LogLevel, message: String, agent_id: Option<String>) {
        self.push_log_for(level, message, agent_id, None);
    }

    /// R-10: Trim `messages` and `message_line_cache` to `MAX_TUI_MESSAGES`
    /// using FIFO eviction so long sessions do not accumulate every message
    /// (with all `MessagePart`s up to 12 KB each) without bound.
    pub(crate) fn trim_messages_if_needed(&mut self) {
        const MAX_TUI_MESSAGES: usize = 500;
        if self.messages.len() > MAX_TUI_MESSAGES {
            let drop_count = self.messages.len() - MAX_TUI_MESSAGES;
            self.messages.drain(0..drop_count);
            // Trim the cache by the same count so cache slots stay aligned
            // with their messages (both vectors drop from the front).
            let drop_cache = drop_count.min(self.message_line_cache.len());
            self.message_line_cache.drain(0..drop_cache);
            // PERF-043: the cache shifted left by `drop_cache`; the watermark
            // addresses the new (post-drain) indices, so subtract as well.
            self.message_cache_dirty_from =
                self.message_cache_dirty_from.saturating_sub(drop_cache);
        }
    }

    /// PERF-043: record that the message at `index` was mutated in place.
    ///
    /// Lowers [`App::message_cache_dirty_from`] to `index` so the next
    /// `render_messages` re-renders only the cache groups from there on,
    /// instead of scanning the whole transcript for staleness every frame.
    /// Call this immediately after `Message::touch()` on a message that was
    /// mutated through `self.messages`.
    pub fn mark_message_dirty(&mut self, index: usize) {
        if index < self.message_cache_dirty_from {
            self.message_cache_dirty_from = index;
        }
    }

    /// PERF-041: prepare the plain-text rows of the message window for a copy.
    ///
    /// Copy paths (`/clip`, `/copy`, keyboard/right-click copy of the Messages
    /// pane) call this instead of reading `message_content_lines` directly.
    /// Any cache group that is still behind its message (a streaming group
    /// waiting out its throttle window) is rendered first, then the flat
    /// buffer is rebuilt from the per-message cache — so a copy always reads
    /// fresh rows even mid-stream.
    pub fn ensure_copy_content_lines(&mut self) {
        crate::layout::ensure_message_copy_lines(self);
    }

    /// PERF-043/PERF-041: reset the per-message cache and its staleness
    /// watermark.
    ///
    /// Called by every site that structurally replaces the message list
    /// (session resume, compaction, `/clear`) so the next render rebuilds the
    /// cache from index 0 instead of trusting a stale watermark that may point
    /// past the new, shorter cache.
    pub fn reset_message_cache(&mut self) {
        self.message_line_cache.clear();
        self.message_cache_dirty_from = 0;
        self.message_content_lines.clear();
    }

    /// R-11: Trim `log_entries` and `log_line_cache` to `MAX_LOG_ENTRIES`
    /// using FIFO eviction.
    pub(crate) fn trim_log_entries_if_needed(&mut self) {
        const MAX_LOG_ENTRIES: usize = 1000;
        if self.log_entries.len() > MAX_LOG_ENTRIES {
            let drop_count = self.log_entries.len() - MAX_LOG_ENTRIES;
            self.log_entries.drain(0..drop_count);
            if self.log_line_cache.len() > MAX_LOG_ENTRIES {
                let drop_cache = self.log_line_cache.len() - MAX_LOG_ENTRIES;
                self.log_line_cache.drain(0..drop_cache);
            }
        }
    }

    /// Flush every current log entry to the log-window spool file. Called when
    /// the log panel is toggled on so the file contains the full history that
    /// is currently visible in the panel. The file is opened once and all
    /// entries are written in a single buffered pass instead of one open/write
    /// syscall per entry.
    pub(crate) fn spool_log_window_history(&mut self) {
        use std::io::Write;
        if !self.show_log {
            return;
        }
        let Some(ref path) = self.log_window_path else {
            return;
        };
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        else {
            return;
        };
        for entry in &self.log_entries {
            let level_str = match entry.level {
                LogLevel::Info => "INF",
                LogLevel::Tool => "TUL",
                LogLevel::Warn => "WRN",
                LogLevel::Error => "ERR",
            };
            let ts = chrono::Utc::now().to_rfc3339();
            let line = format!("{ts} {level_str} {}\n", entry.message);
            if let Err(e) = file.write_all(line.as_bytes()) {
                tracing::warn!(error = %e, "failed to spool log entry");
                break;
            }
        }
    }

    /// Convenience wrapper for [`push_log`](Self::push_log) with no agent id.
    pub(crate) fn push_log_no_agent(&mut self, level: LogLevel, message: String) {
        self.push_log(level, message, None);
    }

    pub(crate) fn open_output_view_session(&mut self, session_id: String, label: String) {
        self.selected_agent_session_id = Some(session_id.clone());
        self.output_view = Some(OutputViewState {
            target: OutputViewTarget::Session { session_id, label },
            scroll_offset: 0,
            max_scroll: 0,
            line_cache: crate::app::OutputViewLineCache {
                wrapped_lines: Vec::new(),
                content_lines: Vec::new(),
                wrapped_count: 0,
                cache_width: 0,
                source_generation: 0,
            },
        });
    }

    pub(crate) fn assistant_output_lines(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .map(|m| m.text_content().lines().count())
            .sum()
    }
}
