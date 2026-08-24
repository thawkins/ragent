//! Message processing pipeline for agent sessions.
//!
//! [`SessionProcessor`] orchestrates the agentic loop: it accepts a user message,
//! streams an LLM response, executes any requested tool calls, and iterates
//! until the model signals completion or the step limit is reached.
//!
//! The free-standing helpers that support the loop live in sibling modules:
//! - [`crate::session::stream_buffer`] — stream buffering and stall detection,
//! - [`crate::session::prompt_builders`] — system-prompt / tool-reference builders,
//! - [`crate::session::permissions`] — bash splitting and permission prompting,
//! - [`crate::session::history`] — history↔chat conversion, token-overflow and
//!   stream-error classification.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use futures::StreamExt;
use ragent_types::ThinkingConfig;
use serde_json::{Value, json};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::agent::AgentInfo;
use crate::cost::{UsageRecord, compute_run_cost, merged_prices};
use crate::event::{Event, EventBus, FinishReason};
use crate::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent, ToolDefinition};
use crate::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use crate::permission::PermissionChecker;
use crate::provider::ProviderRegistry;
use crate::session::SessionManager;
use crate::session::cache::SystemPromptCache;
use crate::session::history::PendingToolCall;
use crate::session::permissions::{
    extract_command_name, extract_resource_from_input, split_bash_command,
};
use crate::telemetry::{LlmRecorder, SessionRecorder, ToolRecorder};
use crate::tool::{McpToolWrapper, ToolContext, ToolRegistry};

// Re-export the public helpers that external callers (tests, benches) import
// from `session::processor::...` so the extraction does not break those paths.
pub use crate::session::history::{
    chat_request_payload_bytes, estimate_request_bytes, estimate_tool_definition_bytes,
    history_to_chat_messages, is_permanent_llm_api_error, is_token_overflow_error_message,
    should_retry_stream_error, stream_has_meaningful_partial_output, tool_result_content_for_llm,
};
pub use crate::session::permissions::check_permission_with_prompt;
pub use crate::session::prompt_builders::build_detailed_tool_reference_section;

/// Maximum wall-clock time a single tool call may run before the watchdog
/// aborts it and terminates the agent run (1000 seconds).
const TOOL_WATCHDOG_TIMEOUT: Duration = Duration::from_secs(1000); // ≈ 16m 40s

/// Nudge injected when a sub-agent's loop terminates with a short text-only
/// response after prior tool-use steps. The model often produces narration
/// ("Now let me check …") as a text-only message without a tool call, causing
/// the loop to treat it as the final answer — even though no findings report
/// was ever produced. This nudge asks the model to emit its complete findings
/// immediately so the deliverable is not lost.
const SUBAGENT_SUMMARY_NUDGE: &str = "System note: you stopped calling tools \
     and produced a short narrative message instead of your findings report. \
     Do NOT call any more tools. Produce your complete written findings report \
     NOW — all issues, ranked by impact, with file, line numbers, and \
     concrete fixes. This is the deliverable.";

/// Sub-agent responses shorter than this many bytes after tool-use steps
/// are treated as narration, not findings, and trigger a summary nudge.
const SUBAGENT_NARRATION_BYTE_LIMIT: usize = 2000;

/// Build the watchdog-timeout error message for a tool that stalled past
/// [`TOOL_WATCHDOG_TIMEOUT`].
fn watchdog_timeout_msg(tool_desc: &str) -> String {
    format!(
        "Tool call {tool_desc} stalled for over {}s \
         (watchdog timeout); aborting the run.",
        TOOL_WATCHDOG_TIMEOUT.as_secs()
    )
}

/// Error type for a spawned tool-execution task in the agent loop.
///
/// Distinguishes a tokio join failure (panic or external abort) from a
/// watchdog-forced abort after [`TOOL_WATCHDOG_TIMEOUT`]. The loop must tell
/// these apart because a watchdog abort terminates the whole run while a
/// join failure is logged and skipped.
#[derive(Debug, thiserror::Error)]
enum ToolTaskError {
    /// The spawned tool task failed to join (panic or external abort).
    #[error("tool task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    /// The tool task exceeded the watchdog timeout and was aborted.
    #[error("tool execution aborted: exceeded the watchdog timeout")]
    WatchdogAbort,
}

/// Drives the agentic conversation loop for a single session.///
/// Holds shared references to the session manager, LLM provider registry,
/// tool registry, permission checker, and event bus.
pub struct SessionProcessor {
    /// Manages session persistence and lifecycle.
    pub session_manager: Arc<SessionManager>,
    /// Registry of available LLM providers.
    pub provider_registry: Arc<ProviderRegistry>,
    /// Registry of available tools the agent may invoke.
    pub tool_registry: Arc<ToolRegistry>,
    /// Checks whether a tool invocation is permitted.
    pub permission_checker: Arc<parking_lot::RwLock<PermissionChecker>>,
    /// Bus for broadcasting session and processing events.
    pub event_bus: Arc<EventBus>,
    /// Optional agent manager for sub-agent spawning (F13/F14).
    /// Uses `OnceLock` to break the circular dependency with `AgentManager`.
    pub agent_manager: std::sync::OnceLock<Arc<crate::task::AgentManager>>,
    /// Optional team manager for spawning and coordinating teammate sessions.
    /// Uses `OnceLock` to break the circular dependency with `TeamManager`.
    pub team_manager: std::sync::OnceLock<Arc<crate::team::TeamManager>>,
    /// M8-T1: cache for `resolve_team_context_for_session`. Maps session id
    /// → `(TeamContext, Instant)` with a 5-second TTL. Invalidated on team
    /// create/join/leave. Avoids scanning every team directory on every
    /// message (O(teams) → O(1) amortised).
    pub team_context_cache: std::sync::Arc<
        parking_lot::RwLock<
            std::collections::HashMap<String, (crate::tool::TeamContext, std::time::Instant)>,
        >,
    >,
    /// Optional MCP client for dynamic MCP tool registration.
    /// Set once after startup via [`SessionProcessor::set_mcp_client`].
    pub mcp_client: std::sync::OnceLock<Arc<tokio::sync::RwLock<crate::mcp::McpClient>>>,
    /// Optional code index for codebase search and symbol lookup.
    /// Uses `OnceLock` so it can be set after the processor is constructed.
    pub code_index: std::sync::OnceLock<Arc<ragent_codeindex::CodeIndex>>,
    /// Optional background task service for the `bg` tool (M3).
    /// Uses `OnceLock` so it can be set after storage is created.
    pub bg_service: std::sync::OnceLock<Arc<crate::background::BackgroundTaskService>>,
    /// Active spec ID for context injection into agent prompts.
    /// Set via `/spec activate` in the TUI or via programmatic API.
    pub active_spec: tokio::sync::RwLock<Option<String>>,
    /// Optional spec manager for reading and updating specifications.
    /// Uses `OnceLock` so it can be set after the processor is constructed.
    pub spec_manager: std::sync::OnceLock<Arc<ragent_specs::SpecManager>>,
    /// Cached tool definitions for the agent loop. Populated after MCP client
    /// registration via [`set_mcp_client`] and invalidated when tools change.
    pub cached_tool_definitions: parking_lot::RwLock<Option<Arc<Vec<ToolDefinition>>>>,
    /// Cached tool *names* for the `ToolsSent` event (PERF-003).
    ///
    /// Mirrors [`cached_tool_definitions`]: the comma-joined name list is built
    /// once when the definitions cache is populated and reused on every loop
    /// step, avoiding the 111-String-per-step allocation on the hot path.
    /// Invalidated together with [`cached_tool_definitions`] by
    /// [`invalidate_tool_cache`].
    pub cached_tool_names: parking_lot::RwLock<Option<Arc<[String]>>>,
    /// Cached total serialised byte size of the cached tool definitions
    /// (P-7 / PERF-014). Populated alongside [`cached_tool_definitions`] so
    /// the per-step request-size estimate can reuse the sum instead of
    /// re-serialising ~111 `ToolDefinition::parameters` JSON schemas on every
    /// step. Invalidated together with [`cached_tool_definitions`] by
    /// [`invalidate_tool_cache`].
    pub cached_tool_definition_bytes: parking_lot::RwLock<Option<u64>>,
    /// H2: cache of warm LLM clients keyed by `provider/model`. Providers
    /// rebuild their `reqwest::Client` (and thus their connection pool / TLS /
    /// keep-alive state) inside `create_client`, which `prepare_client` used
    /// to call on every turn. Caching the resulting `Arc<dyn LlmClient>` here
    /// lets a session reuse one warm client across all its turns and loop
    /// steps instead of re-establishing connections each time.
    pub llm_client_cache:
        parking_lot::RwLock<std::collections::HashMap<String, Arc<dyn crate::llm::LlmClient>>>,
    /// LLM stream configuration (timeouts, retries, backoff).
    pub stream_config: crate::StreamConfig,
    /// Memory extraction engine for automatic memory candidate generation.
    pub extraction_engine: std::sync::OnceLock<Arc<crate::memory::ExtractionEngine>>,
    /// Auto-approve all permissions without prompting (set by --yes / --no-prompt CLI flag).
    pub auto_approve: bool,
    /// Cached per-component system-prompt builders (FR-008, FR-009).
    ///
    /// Sharing a single cache across all turns of a session lets the
    /// `tool_reference`, `codeindex_guidance`, and `team_guidance` strings
    /// be computed at most once per process-user-message, instead of once
    /// per step.  Treated as a best-effort cache: a miss is always
    /// acceptable, the cache exists only to skip the work when inputs are
    /// unchanged.
    pub system_prompt_cache: parking_lot::RwLock<Option<Arc<SystemPromptCache>>>,
    /// Read timestamps (mtime in milliseconds since UNIX epoch) for files
    /// that have been read by this session. Shared with edit tools via
    /// [`crate::tool::ToolContext`] so they can detect stale-file edits
    /// (editrenewal FR-003).
    pub read_timestamps: Arc<RwLock<HashMap<PathBuf, u64>>>,
    /// P-2: cache of the resolved [`ragent_config::Config`] keyed by the
    /// modification times of every config file that contributed to it. The
    /// config is immutable for the lifetime of a session, so re-reading
    /// `ragent.json` from disk on every `process_user_message` call is pure
    /// I/O. This cache reloads only when one of the contributing files
    /// changes on disk (or when an environment-variable override
    /// (`RAGENT_CONFIG` / `RAGENT_CONFIG_CONTENT`) is present, which
    /// disables the cache because env vars have no mtime to track).
    pub cached_config: parking_lot::Mutex<Option<CachedConfig>>,
    /// Telemetry subsystem for recording LLM, tool, session, and permission
    /// metrics. Wired into the binary unconditionally.
    pub telemetry: Arc<crate::telemetry::TelemetrySubsystem>,
    /// Per-session cache of invoked skill bodies (FR-008).
    ///
    /// Maps skill name → processed body text. Populated on demand when a skill
    /// is invoked, so repeated invocations of the same skill within a session
    /// avoid re-reading the `SKILL.md` body from disk. The cache is a
    /// best-effort optimisation: a miss simply triggers a fresh load.
    pub skill_body_cache: Arc<RwLock<HashMap<String, String>>>,
}

/// P-2: the cached resolved config plus the inputs used to build it.
///
/// `file_mtines` records `(path, mtime)` for every entry in
/// [`Config::config_paths`]. The cache is valid while none of those mtimes
/// change. `env_overrides_present` records whether either
/// `RAGENT_CONFIG` or `RAGENT_CONFIG_CONTENT` was set at load time — when
/// set, the cache is bypassed on the next load because env vars have no
/// mtime to track.
#[derive(Clone)]
pub struct CachedConfig {
    /// The resolved configuration wrapped in `Arc` for cheap cloning.
    pub config: Arc<ragent_config::Config>,
    /// `(path, mtime)` pairs for each contributing config file.
    pub file_mtimes: Vec<(PathBuf, std::time::SystemTime)>,
    /// Whether env-var overrides were present when this entry was built.
    pub env_overrides_present: bool,
}

impl SessionProcessor {
    /// Set the MCP client and register all tools from connected servers into the tool registry.
    ///
    /// This should be called once after the MCP client has connected to all configured servers.
    /// Tools are registered with names in the format `mcp_{server_id}_{tool_name}`.
    pub async fn set_mcp_client(&self, client: Arc<tokio::sync::RwLock<crate::mcp::McpClient>>) {
        // Register all currently connected MCP tools into the shared registry.
        let tool_defs = {
            let c = client.read().await;
            // Collect (server_id, tool_def) pairs for all connected servers.
            let mut pairs = Vec::new();
            for server in c.servers() {
                if server.status == crate::mcp::McpStatus::Connected {
                    for tool in &server.tools {
                        pairs.push((server.id.clone(), tool.clone()));
                    }
                }
            }
            pairs
        };

        let registered = tool_defs.len();
        for (server_id, tool_def) in tool_defs {
            let wrapper = McpToolWrapper::new(
                &server_id,
                &tool_def.name,
                &tool_def.description,
                tool_def.parameters,
                client.clone(),
            );
            tracing::debug!(
                server_id = %server_id,
                tool = %tool_def.name,
                ragent_name = %wrapper.ragent_name,
                "Registering MCP tool"
            );
            self.tool_registry.register(Arc::new(wrapper));
        }

        if registered > 0 {
            tracing::info!(
                count = registered,
                "Registered MCP tools into tool registry"
            );
        } else {
            tracing::debug!("No connected MCP tools to register");
        }

        let _ = self.mcp_client.set(client);
        self.invalidate_tool_cache();
    }

    /// Invalidate the cached tool definitions so they are rebuilt on the next loop step.
    pub fn invalidate_tool_cache(&self) {
        {
            let mut guard = self.cached_tool_definitions.write();
            *guard = None;
        }
        {
            let mut names = self.cached_tool_names.write();
            *names = None;
        }
        {
            let mut bytes = self.cached_tool_definition_bytes.write();
            *bytes = None;
        }
        // H2: the tool-JSON byte cache in ragent-llm is keyed by tool content
        // fingerprint + provider format; invalidate it whenever the session's
        // tool definitions change so stale serialised tool lists are never
        // sent after a registry update.
        ragent_llm::provider::tool_cache::invalidate_tool_cache();
    }

    /// Return cached tool definitions, populating the cache if necessary.
    ///
    /// P-7 / PERF-014: also populates [`cached_tool_definition_bytes`] with
    /// the total serialised byte size of the definitions (computed once via
    /// [`estimate_tool_definition_bytes`]) so the per-step request-size
    /// estimate can reuse the sum instead of re-serialising ~111 tool
    /// schemas on every step.
    fn get_cached_tool_definitions(&self) -> Arc<Vec<ToolDefinition>> {
        {
            let guard = self.cached_tool_definitions.read();
            if let Some(ref defs) = *guard {
                return defs.clone();
            }
        }
        let defs = self.tool_registry.definitions();
        // PERF-003: also cache the tool-name list used by the `ToolsSent`
        // event so we don't allocate ~111 Strings on every loop step.
        let names: Arc<[String]> = defs.iter().map(|t| t.name.clone()).collect();
        // P-7: pre-compute the total serialised byte size of the tool
        // definitions so `estimate_request_bytes`-style estimators can
        // reuse the sum without re-serialising every schema per step.
        let tool_bytes = estimate_tool_definition_bytes(&defs);
        {
            let mut guard = self.cached_tool_definitions.write();
            *guard = Some(defs.clone());
        }
        {
            let mut names_guard = self.cached_tool_names.write();
            *names_guard = Some(names);
        }
        {
            let mut bytes_guard = self.cached_tool_definition_bytes.write();
            *bytes_guard = Some(tool_bytes);
        }
        defs
    }

    /// Return the cached total serialised byte size of the tool definitions
    /// (P-7). Populated alongside [`get_cached_tool_definitions`]; returns
    /// `None` when the cache is empty or invalidated.
    fn get_cached_tool_definition_bytes(&self) -> Option<u64> {
        self.cached_tool_definition_bytes.read().as_ref().copied()
    }

    /// Return the cached tool names for the `ToolsSent` event (PERF-003).
    ///
    /// Returns `None` when no tools are registered. The cache is populated
    /// alongside [`get_cached_tool_definitions`] and invalidated together
    /// with it by [`invalidate_tool_cache`].
    fn get_cached_tool_names(&self) -> Option<Arc<[String]>> {
        let guard = self.cached_tool_names.read();
        guard.as_ref().cloned()
    }

    /// Return the per-session system-prompt component cache, creating it
    /// on first use (FR-008, FR-009).
    ///
    /// Sharing a single `SystemPromptCache` across all turns of a session
    /// lets the `tool_reference`, `codeindex_guidance`, and `team_guidance`
    /// strings be computed at most once per process-user-message instead
    /// of once per step.  The cache is advisory: a miss is always
    /// acceptable, it exists only to skip the work when inputs are
    /// unchanged.
    pub fn system_prompt_cache(&self) -> Arc<SystemPromptCache> {
        {
            let guard = self.system_prompt_cache.read();
            if let Some(ref cache) = *guard {
                return cache.clone();
            }
        }
        let cache = Arc::new(SystemPromptCache::new());
        let mut guard = self.system_prompt_cache.write();
        *guard = Some(cache.clone());
        cache
    }

    /// Invalidate the system-prompt component cache.
    ///
    /// Call this when the tool registry, code-index state, or team
    /// membership changes.  Cheap: just bumps a global version counter
    /// and clears the per-component entries.
    pub fn invalidate_system_prompt_cache(&self) {
        self.system_prompt_cache().invalidate_all();
    }

    /// Invalidate the P-2 config cache (force the next `prepare_client`
    /// call to re-read `ragent.json` from disk). Call this when the config
    /// is known to have changed externally (e.g. after a `/config save`
    /// slash command) so the next turn picks up the new values.
    pub fn invalidate_config_cache(&self) {
        let mut guard = self.cached_config.lock();
        *guard = None;
    }

    /// Resolve the per-turn [`ragent_config::Config`], using the P-2 mtime
    /// cache to skip the disk read when none of the contributing config
    /// files have changed since the last load.
    ///
    /// Returns the config wrapped in `Arc` so it can be cloned cheaply into
    /// the per-turn [`TurnClient`] and per-tool-call [`ToolContext`]s.
    pub(crate) fn load_config_cached(&self) -> Arc<ragent_config::Config> {
        use std::time::SystemTime;
        let env_overrides_present = std::env::var_os("RAGENT_CONFIG").is_some()
            || std::env::var_os("RAGENT_CONFIG_CONTENT").is_some();
        // Try the cache: valid only when no env-var overrides are present and
        // every recorded file's mtime is unchanged.
        {
            let guard = self.cached_config.lock();
            if let Some(cached) = guard.as_ref()
                && !env_overrides_present
                && cached.file_mtimes.iter().all(|(path, mtime)| {
                    std::fs::metadata(path)
                        .and_then(|m| m.modified())
                        .ok()
                        .map_or(false, |current| current == *mtime)
                })
            {
                return cached.config.clone();
            }
        }
        // Cache miss (or invalid): reload from disk.
        let cfg = ragent_config::Config::load().unwrap_or_default();
        let file_mtimes: Vec<(PathBuf, SystemTime)> = cfg
            .config_paths
            .iter()
            .filter_map(|p| {
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
                    .map(|mt| (p.clone(), mt))
            })
            .collect();
        let arc = Arc::new(cfg);
        let mut guard = self.cached_config.lock();
        *guard = Some(CachedConfig {
            config: Arc::clone(&arc),
            file_mtimes,
            env_overrides_present,
        });
        arc
    }
    /// Run a blocking storage operation on a dedicated thread to avoid
    /// stalling the Tokio runtime.
    ///
    /// (AgentPerf T-012 / FR-010 / FR-011.)  This is the canonical
    /// way for the agent action loop to talk to the underlying SQLite
    /// store.  All callers MUST go through this helper rather than
    /// calling `Storage` methods directly from the async path, so the
    /// executor is never blocked on synchronous I/O.
    pub async fn storage_op<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&crate::storage::Storage) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let storage = self.session_manager.storage().clone();
        tokio::task::spawn_blocking(move || f(&storage))
            .await
            .map_err(|e| anyhow::anyhow!("storage task panicked: {e}"))?
    }
    /// Processes a user message within an agent session.
    ///
    /// Persists the user message, then enters an agentic loop that streams
    /// LLM responses, executes tool calls, and feeds results back to the model
    /// until completion or the agent's max-step limit is reached.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured model or provider is missing, if the
    /// API key cannot be resolved, or if an LLM call fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example() -> anyhow::Result<()> {
    /// use std::sync::Arc;
    /// use std::sync::atomic::AtomicBool;
    /// use ragent_agent::session::processor::SessionProcessor;
    /// use ragent_agent::agent::AgentInfo;
    ///
    /// // Assumes `processor` is a fully configured SessionProcessor.
    /// # let processor: SessionProcessor = todo!();
    /// let agent = AgentInfo::new("coder", "A coding assistant");
    /// let cancel = Arc::new(AtomicBool::new(false));
    /// let reply = processor.process_message("session-1", "Hello!", &agent, cancel).await?;
    /// println!("Assistant replied: {}", reply.text_content());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_message(
        &self,
        session_id: &str,
        user_text: &str,
        agent: &AgentInfo,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Message> {
        let user_msg = Message::user_text(session_id, user_text);
        self.process_user_message(session_id, user_msg, agent, cancel_flag)
            .await
    }

    /// Process a pre-built user [`Message`] (e.g. one containing image attachments).
    ///
    /// Unlike [`process_message`] which always creates a plain-text user message,
    /// this method accepts any `Message` so the TUI can pass multipart messages
    /// that include [`MessagePart::Image`] parts alongside the text.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The user message cannot be stored in the database
    /// - The configured model or provider is missing
    /// - The API key for the provider cannot be resolved
    /// - An LLM API call fails
    /// - Tool execution fails and no tool-result recovery is possible
    /// - The processing is cancelled via the cancel flag
    pub async fn process_user_message(
        &self,
        session_id: &str,
        user_msg: Message,
        agent: &AgentInfo,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<Message> {
        let profiler = crate::session::profiler::agent_loop_profiler();
        let session_recorder = SessionRecorder::from_subsystem(&self.telemetry);
        session_recorder.record_session_start();

        let session_id_arc: std::sync::Arc<str> = std::sync::Arc::from(session_id);
        #[allow(clippy::needless_borrow)]
        let session_id: &str = &session_id_arc;

        // 1. Store user message
        {
            let _scope = profiler.scope("storage.user_message.create");
            let msg = user_msg.clone();
            self.storage_op(move |s| s.create_message(&msg)).await?;
        }
        self.event_bus.publish(Event::MessageStart {
            session_id: session_id.to_string(),
            message_id: user_msg.id.clone(),
        });

        // 2. Prepare LLM client, config, working dir, team context
        let turn = self
            .prepare_client(session_id, &user_msg.id, agent, &profiler)
            .await?;

        // T-012: per-run cost tracking. Set up a listener for `Event::TokenUsage`
        // so we can accumulate usage across the init exchange and all loop
        // iterations, then publish a single `Event::RunCostSummary` when the
        // run ends. The listener is aborted on return so it never leaks.
        let usage_accum = Arc::new(Mutex::new((0u64, 0u64)));
        let usage_listener = {
            let bus = self.event_bus.clone();
            let sid = session_id.to_string();
            let accum = usage_accum.clone();
            tokio::spawn(async move {
                let mut rx = bus.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(Event::TokenUsage {
                            session_id,
                            input_tokens,
                            output_tokens,
                        }) if session_id == sid => {
                            let mut locked = accum
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            locked.0 += input_tokens;
                            locked.1 += output_tokens;
                        }
                        Ok(Event::MessageEnd { session_id, .. }) if session_id == sid => break,
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
            })
        };
        struct AbortOnDrop(tokio::task::JoinHandle<()>);
        impl Drop for AbortOnDrop {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _usage_guard = AbortOnDrop(usage_listener);

        let prices = merged_prices(&turn.session_config.prices);
        let publish_run_cost_summary = {
            let bus = self.event_bus.clone();
            let storage = self.session_manager.storage().clone();
            let sid = session_id.to_string();
            let model_id = turn.model_ref.model_id.clone();
            let accum = usage_accum.clone();
            move |duration_ms: u64| {
                let (input_tokens, output_tokens) = accum
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                let summary = compute_run_cost(
                    vec![UsageRecord {
                        model_id: model_id.clone(),
                        input_tokens,
                        output_tokens,
                    }],
                    &prices,
                );
                ragent_telemetry::counters::set_cost_session_last(summary.total_cost_usd);
                // Persist the summary so it can be attached to an explicit
                // `--include-cost` session export (FR-018). Stored separately
                // from the transcript so the default export never leaks cost.
                let row = crate::storage::RunCostSummaryRow {
                    id: Uuid::new_v4().to_string(),
                    session_id: sid.clone(),
                    model_id: model_id.clone(),
                    input_tokens: summary.total_input_tokens,
                    output_tokens: summary.total_output_tokens,
                    total_cost_usd: summary.total_cost_usd,
                    duration_ms,
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                let storage_for_persist = storage.clone();
                tokio::task::spawn_blocking(move || {
                    let _ = storage_for_persist.create_run_cost_summary(&row);
                });
                bus.publish(Event::RunCostSummary {
                    session_id: sid.clone(),
                    model_id: model_id.clone(),
                    input_tokens: summary.total_input_tokens,
                    output_tokens: summary.total_output_tokens,
                    total_cost_usd: summary.total_cost_usd,
                    duration_ms,
                });
            }
        };

        // 3. Build system prompt
        let system_prompt = self
            .build_turn_system_prompt(
                agent,
                &turn.session_config,
                &turn.working_dir,
                turn.team_context.as_ref(),
                &profiler,
            )
            .await?;

        // 4. Build chat messages from history
        // P-3: `build_turn_chat_messages` also returns the resolved
        // `context_window` so the orchestrator does not re-resolve it below.
        let (
            chat_messages_vec,
            mut compressed_this_turn,
            mut last_reported_input_tokens,
            context_window,
        ) = self
            .build_turn_chat_messages(
                session_id,
                agent,
                &turn.model_ref,
                &turn.session_config,
                &profiler,
            )
            .await?;
        // P-6: hold the per-turn chat history as `Arc<Vec<ChatMessage>>`
        // so the per-retry `ChatRequest` can share it by refcount bump
        // instead of cloning the entire `Vec` on every attempt.
        let mut chat_messages: std::sync::Arc<Vec<ChatMessage>> =
            std::sync::Arc::new(chat_messages_vec);

        // 5. Run AGENTS.md init exchange (display-only, skipped for subagents)
        {
            // P-1: route `get_messages` through `storage_op` so the SQLite
            // read runs on a dedicated blocking thread instead of stalling
            // the async runtime. Every other storage call in the loop already
            // goes through `storage_op`; this one site was missed.
            let sid = session_id.to_string();
            let history = self.storage_op(move |s| s.get_messages(&sid)).await?;
            self.run_inline_init_acknowledgement(
                session_id,
                agent,
                &history,
                &system_prompt,
                &turn.client,
                &turn.model_ref,
                &turn.working_dir,
            )
            .await?;
        }

        // 6. Agent loop setup
        let max_steps = agent.max_steps.unwrap_or(1024) as usize;
        self.event_bus.set_step(session_id, 0);
        let tool_definitions: std::sync::Arc<Vec<ToolDefinition>> = if max_steps <= 1 {
            std::sync::Arc::new(Vec::new())
        } else {
            self.get_cached_tool_definitions()
        };
        // P-7: prime the tool-definition byte cache alongside the definitions
        // cache so the per-step request-size estimator can reuse the sum.
        let _ = self.get_cached_tool_definition_bytes();
        let cached_tool_names: Option<std::sync::Arc<[String]>> = if max_steps > 1 {
            self.get_cached_tool_names()
        } else {
            None
        };
        let mut assistant_parts: std::sync::Arc<Vec<MessagePart>> = std::sync::Arc::new(Vec::new());
        let mut agent_switch_requested = false;
        let mut agent_complete_requested = false;
        // Set when a tool call stalls past `TOOL_WATCHDOG_TIMEOUT`; the run is
        // terminated after the tool phase.
        let mut watchdog_timed_out = false;
        // Set to true after injecting the sub-agent summary nudge so we only
        // nudge once per run. See [`SUBAGENT_SUMMARY_NUDGE`].
        let mut subagent_summary_nudged = false;
        let mut last_interim_hash: Option<u64> = None;
        let total_start = Instant::now();
        let mut cumulative_model_wait_ms: u64 = 0;
        let mut compaction_attempted_this_turn = false;
        // Finish reason observed by the inner loop for the most recent
        // assistant message this turn. Used at finalise time to publish
        // a visible notice for interactive sessions when the provider
        // silently truncated the reply.
        let mut last_finish_reason: Option<FinishReason> = None;
        // P-17: reuse the per-step ContentPart buffers across loop iterations
        // to avoid reallocating two `Vec<ContentPart>`s on every step. They
        // are emptied via `std::mem::take` when pushed into `chat_messages`,
        // so the next iteration starts with an empty (but allocated) Vec.
        let mut assistant_content_parts: Vec<ContentPart> = Vec::new();
        let mut tool_result_parts: Vec<ContentPart> = Vec::new();
        let mut bg_parts: Vec<ContentPart> = Vec::new();

        let assistant_msg_id = {
            let _scope = profiler.scope("storage.assistant_placeholder.create");
            let placeholder = Message::new(session_id, Role::Assistant, vec![]);
            let id = placeholder.id.clone();
            self.storage_op(move |s| s.create_message(&placeholder))
                .await?;
            id
        };

        // P-3: reuse the `context_window` returned by
        // `build_turn_chat_messages` instead of re-resolving it from the
        // provider registry a second time.

        // 7. Agent loop
        loop {
            let _step_scope = profiler.scope("loop.step.total");
            let step = {
                let _scope = profiler.scope("loop.step.setup");
                // P-13: compute the new step once, set it, and reuse the
                // value. The previous form called `current_step` twice
                // (once to read, once to re-read after `set_step`); the
                // second read is redundant because we already know the
                // value we just stored.
                let new_step = self.event_bus.current_step(session_id) + 1;
                self.event_bus.set_step(session_id, new_step);
                new_step as usize
            };
            if step > max_steps {
                warn!("Reached max steps ({}), stopping agent loop", max_steps);
                self.event_bus.publish(Event::AgentError {
                    session_id: session_id.to_string(),
                    error: format!("Reached maximum steps ({max_steps})"),
                });
                break;
            }

            if cancel_flag.load(Ordering::Relaxed) {
                warn!("Agent loop cancelled by user at step {}", step);
                let total_elapsed_ms = total_start.elapsed().as_millis() as u64;
                let other_ms = total_elapsed_ms.saturating_sub(cumulative_model_wait_ms);
                tracing::info!(
                    session_id = %session_id,
                    total_ms = total_elapsed_ms,
                    model_wait_ms = cumulative_model_wait_ms,
                    other_ms = other_ms,
                    "Agent loop cancelled - timing breakdown: total={}ms, model_wait={}ms, other={}ms",
                    total_elapsed_ms,
                    cumulative_model_wait_ms,
                    other_ms
                );
                let parts_owned = std::sync::Arc::try_unwrap(assistant_parts)
                    .unwrap_or_else(|arc| (*arc).clone());
                let mut assistant_msg = Message::new(session_id, Role::Assistant, parts_owned);
                assistant_msg.id = assistant_msg_id;
                let cancelled_id = assistant_msg.id.clone();
                self.storage_op(move |s| s.update_message(&assistant_msg))
                    .await?;
                self.event_bus.publish(Event::MessageEnd {
                    session_id: session_id.to_string(),
                    message_id: cancelled_id,
                    reason: FinishReason::Cancelled,
                });
                publish_run_cost_summary(total_elapsed_ms);
                return Ok(Message::new(session_id, Role::Assistant, vec![]));
            }
            debug!("Agent loop step {}/{}", step, max_steps);

            // P-4: only publish `ToolsSent` on the first step of the turn —
            // the TUI only renders the tool list once and the previous form
            // cloned ~111 `String`s (or cloned the cached `Arc<[String]>`
            // into a `Vec<String>`) on every step. When `cached_tool_names`
            // is populated we hand the `Arc<[String]>` straight to the event
            // (P-4/D-1), so the publish is a single refcount bump.
            if step == 1 && !tool_definitions.is_empty() {
                let _scope = profiler.scope("loop.step.publish_tools");
                let tool_names: Vec<String> = match &cached_tool_names {
                    Some(names) => names.iter().cloned().collect(),
                    None => tool_definitions.iter().map(|t| t.name.clone()).collect(),
                };
                self.event_bus.publish(Event::ToolsSent {
                    session_id: session_id_arc.to_string(),
                    tools: tool_names,
                });
            }

            // Maybe compact (per-iteration pre-send check) — T-008, FR-003,
            // FR-006, FR-008. When `compaction.auto` is enabled and compaction
            // has not already run this turn, estimate the request token load
            // and — if it exceeds `context_window - max(output, buffer)` —
            // invoke the OpenCode-derived summarisation runner before sending
            // the user prompt to the LLM.
            let llm_request_start = std::time::Instant::now();

            if turn.session_config.compaction.auto && !compaction_attempted_this_turn {
                // Skip the local estimate when the provider already reported
                // input tokens for the previous turn — `evaluate_trigger`
                // prefers the provider value, so the local estimate is pure
                // wasted work (it serialises every message + tool definition).
                let estimate = if last_reported_input_tokens > 0 {
                    0
                } else {
                    crate::compaction::estimate_request_tokens(
                        Some(system_prompt.as_ref()),
                        &chat_messages,
                        &tool_definitions[..],
                    )
                };
                let decision = crate::compaction::evaluate_trigger(
                    &turn.session_config.compaction,
                    estimate,
                    last_reported_input_tokens,
                    context_window,
                    0,
                );
                tracing::debug!(
                    session_id,
                    effective_tokens = decision.effective_tokens,
                    threshold = decision.threshold,
                    context_window,
                    estimated_tokens = decision.estimated_tokens,
                    last_reported_input_tokens,
                    should_compact = decision.should_compact,
                    "pre-send compaction trigger evaluation"
                );
                if decision.should_compact {
                    // Convert the provider-facing chat messages into the
                    // internal `Message` form the compaction runner expects,
                    // run summarisation, persist the synthetic compaction
                    // message, and replace the in-memory history with the
                    // compaction message plus the verbatim recent tail.
                    let messages =
                        crate::compaction::convert::chat_messages_to_messages(&chat_messages);
                    let previous_summary = messages
                        .iter()
                        .rev()
                        .find(|m| m.role == Role::Compaction)
                        .map(|m| m.text_content());
                    compaction_attempted_this_turn = true;
                    let compact_result = crate::compaction::compact(
                        session_id,
                        messages,
                        &turn.model_ref.model_id,
                        context_window,
                        0,
                        &turn.session_config.compaction,
                        previous_summary.as_deref(),
                        &turn.client,
                        &self.event_bus,
                        "auto",
                        &self.stream_config,
                    )
                    .await;
                    match compact_result {
                        Ok(outcome) => {
                            // Persist the synthetic compaction message so
                            // future turns load history from the compaction
                            // point forward (FR-005 / FR-007).
                            let compaction_msg = outcome.compaction_message.clone();
                            let persist_err = self
                                .storage_op(move |s| s.create_message(&compaction_msg))
                                .await
                                .err();
                            if let Some(e) = persist_err {
                                warn!(error = %e, "failed to persist compaction message");
                            }
                            let new_chat = crate::compaction::convert::messages_to_chat_messages(
                                &outcome.new_messages,
                            );
                            chat_messages = Arc::new(new_chat);
                            compressed_this_turn = true;
                            last_reported_input_tokens = outcome.compressed_tokens as u64;
                            {
                                let session_state_lock = self
                                    .session_manager
                                    .as_ref()
                                    .session_state_cache(session_id);
                                if let Ok(mut guard) = session_state_lock.lock() {
                                    guard.set_last_reported_input_tokens(
                                        outcome.compressed_tokens as u64,
                                    );
                                }
                            }
                            tracing::info!(
                                original_tokens = outcome.original_tokens,
                                compressed_tokens = outcome.compressed_tokens,
                                kept_messages = outcome.kept_message_count,
                                "pre-send compaction applied"
                            );
                        }
                        Err(e) => {
                            warn!(
                                error = %e,
                                "pre-send compaction failed; continuing with uncompressed history"
                            );
                        }
                    }
                }
            }

            // Call LLM with retry + handle stream events
            let mut loop_state = crate::session::loop_steps::LoopState {
                chat_messages: std::mem::take(&mut chat_messages),
                assistant_parts: std::sync::Arc::clone(&assistant_parts),
                agent_switch_requested,
                agent_complete_requested,
                last_interim_hash,
                cumulative_model_wait_ms,
                compressed_this_turn,
                compaction_attempted_this_turn,
                last_reported_input_tokens,
                last_finish_reason: last_finish_reason.clone(),
            };
            let mut llm_result = self
                .call_llm_step(
                    session_id,
                    agent,
                    &turn,
                    &mut loop_state,
                    &tool_definitions,
                    &system_prompt,
                    context_window,
                    llm_request_start,
                    &profiler,
                )
                .await?;
            chat_messages = loop_state.chat_messages;
            compressed_this_turn = loop_state.compressed_this_turn;
            compaction_attempted_this_turn = loop_state.compaction_attempted_this_turn;
            last_reported_input_tokens = loop_state.last_reported_input_tokens;
            if let Some(reason) = loop_state.last_finish_reason.clone() {
                last_finish_reason = Some(reason);
            }

            if llm_result.last_input_tokens > 0 {
                last_reported_input_tokens = llm_result.last_input_tokens;
                // Persist the provider-reported input tokens into the session state cache
                // so the next turn starts with the same usage value shown in the TUI.
                {
                    let session_state_lock = self
                        .session_manager
                        .as_ref()
                        .session_state_cache(session_id);
                    if let Ok(mut guard) = session_state_lock.lock() {
                        guard.set_last_reported_input_tokens(llm_result.last_input_tokens);
                    }
                }
            }

            // Collect parts from this turn
            {
                let _scope = profiler.scope("loop.response.process");
                if !llm_result.reasoning_buffer.is_empty() {
                    std::sync::Arc::make_mut(&mut assistant_parts).push(MessagePart::Reasoning {
                        text: llm_result.reasoning_buffer.clone(),
                    });
                }
                if !llm_result.text_buffer.is_empty() {
                    let response_preview =
                        ragent_types::truncate_bytes(&llm_result.text_buffer, 200);
                    let model_elapsed_ms = llm_request_start.elapsed().as_millis() as u64;
                    cumulative_model_wait_ms += model_elapsed_ms;
                    let llm_recorder = LlmRecorder::from_subsystem(&self.telemetry);
                    llm_recorder.record_duration(
                        &turn.model_ref.model_id,
                        &turn.model_ref.provider_id,
                        model_elapsed_ms as f64,
                    );
                    self.event_bus.publish(Event::ModelResponse {
                        session_id: session_id.to_string(),
                        text: response_preview,
                        elapsed_ms: model_elapsed_ms,
                        input_tokens: llm_result.last_input_tokens,
                        output_tokens: llm_result.last_output_tokens,
                    });
                    std::sync::Arc::make_mut(&mut assistant_parts).push(MessagePart::Text {
                        text: llm_result.text_buffer.clone(),
                    });
                }
            }

            // No tool calls — the loop ends here. The agent's text response
            // is the final answer for this turn.
            if llm_result.tool_calls.is_empty() {
                // Sub-agent premature-termination guard: when a sub-agent that
                // was actively calling tools (step > 1 implies prior tool-use
                // steps, otherwise the loop would have broken earlier) produces
                // a SHORT text-only response, it is almost always narration
                // ("Now let me check …") rather than the findings report. The
                // model forgot to call a tool or emit findings, and the loop
                // would silently accept the narration as the deliverable.
                //
                // Inject a one-shot nudge asking the model to produce its
                // complete findings report now, then continue the loop. The
                // next text-only response (the actual findings) terminates the
                // loop normally.
                if agent.mode == crate::agent::AgentMode::Subagent
                    && step > 1
                    && !subagent_summary_nudged
                    && llm_result.text_buffer.len() < SUBAGENT_NARRATION_BYTE_LIMIT
                {
                    subagent_summary_nudged = true;
                    self.event_bus.publish(Event::AgentNotice {
                        session_id: session_id.to_string(),
                        message: "Sub-agent ended with a short text-only \
                                 response after tool work — nudging it to \
                                 produce its findings report."
                            .to_string(),
                    });
                    tracing::info!(
                        session_id = %session_id,
                        step,
                        text_len = llm_result.text_buffer.len(),
                        "sub-agent premature termination detected; injecting \
                         summary nudge"
                    );
                    // Remove the narration text that was prematurely
                    // pushed into `assistant_parts` so the final saved
                    // message contains only the actual findings.
                    {
                        let parts = std::sync::Arc::make_mut(&mut assistant_parts);
                        if matches!(parts.last(), Some(MessagePart::Text { .. })) {
                            parts.pop();
                        }
                    }
                    // Push the assistant's narration into chat history so the
                    // model sees its own last message, then push the nudge as
                    // a user message.
                    let narration = std::mem::take(&mut llm_result.text_buffer);
                    if !narration.is_empty() {
                        Arc::make_mut(&mut chat_messages).push(ChatMessage {
                            role: "assistant".to_string(),
                            content: ChatContent::Text(narration),
                        });
                    }
                    Arc::make_mut(&mut chat_messages).push(ChatMessage {
                        role: "user".to_string(),
                        content: ChatContent::Text(SUBAGENT_SUMMARY_NUDGE.to_string()),
                    });
                    continue;
                }
                break;
            }

            // Tool dispatch phase (kept inline due to closure complexity)
            {
                let _scope = profiler.scope("loop.tool_phase.total");
                // P-17: clear the reused per-step buffers (cheap — the
                // allocations from the previous step are retained).
                assistant_content_parts.clear();
                tool_result_parts.clear();
                // P-18: move `text_buffer` into the `ContentPart::Text` rather
                // than cloning it. The buffer is not referenced after this
                // point (the response-preview and no-tool-nudge paths ran
                // earlier and already consumed whatever they needed).
                if !llm_result.text_buffer.is_empty() {
                    let text = std::mem::take(&mut llm_result.text_buffer);
                    assistant_content_parts.push(ContentPart::Text { text });
                }
                let parallel_tool_calls = turn.session_config.experimental.parallel_tool_calls;
                let mut futures = Vec::new();
                // P-8/P-9: build a single `ToolContext` for this step and clone
                // it per tool call. `active_spec` is read once here (P-9) so
                // the async lock is acquired at most once per step instead of
                // once per tool call. The cloned value is also reused by the
                // auto-spec-task-update block below (P-10).
                let active_spec_id = self.active_spec.read().await.clone();
                let base_tool_ctx = ToolContext {
                    session_id: session_id.to_string(),
                    working_dir: turn.working_dir.clone(),
                    event_bus: self.event_bus.clone(),
                    storage: Some(self.session_manager.storage().clone()),
                    agent_manager: self.agent_manager.get().cloned(),
                    active_model: Some(turn.model_ref.clone()),
                    team_context: turn.team_context.clone(),
                    team_manager: self
                        .team_manager
                        .get()
                        .cloned()
                        .map(|tm| tm as Arc<dyn crate::tool::TeamManagerInterface>),
                    code_index: self.code_index.get().cloned(),
                    bg_service: self.bg_service.get().cloned(),
                    spec_manager: self.spec_manager.get().cloned(),
                    active_spec_id: active_spec_id.clone(),
                    config: Some(std::sync::Arc::clone(&turn.session_config)),
                    cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
                    read_timestamps: self.read_timestamps.clone(),
                    canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
                };
                type ToolExecutionResult = Result<
                    (
                        PendingToolCall,
                        Value,
                        ToolCallStatus,
                        Option<Value>,
                        Option<String>,
                        u64,
                        String,
                        Option<Value>,
                    ),
                    ToolTaskError,
                >;
                let result_profiler = profiler.clone();
                // P-15: collect one `ToolCallBatchEntry` per tool call so a
                // single `Event::ToolCallBatch` can be published at the end of
                // the step, giving consumers an atomic view of all tool calls.
                // The per-call events are still published inside the spawned
                // task as a fallback (PERFPLAN Milestone D risk note).
                let mut batch_entries: Vec<ragent_types::event::ToolCallBatchEntry> = Vec::new();
                let mut handle_tool_execution_result = |result: ToolExecutionResult| {
                    let _scope = result_profiler.scope("loop.tool_phase.handle_result");
                    match result {
                        Ok((
                            tc,
                            input,
                            status,
                            output_value,
                            error,
                            duration_ms,
                            result_content,
                            tool_metadata,
                        )) => {
                            let success = status == ToolCallStatus::Completed;
                            std::sync::Arc::make_mut(&mut assistant_parts).push(
                                MessagePart::ToolCall {
                                    tool: tc.name.clone(),
                                    call_id: tc.id.clone(),
                                    state: ToolCallState {
                                        status,
                                        input,
                                        output: output_value,
                                        error: error.clone(),
                                        duration_ms: Some(duration_ms),
                                    },
                                },
                            );
                            tool_result_parts.push(ContentPart::ToolResult {
                                tool_use_id: tc.id.clone(),
                                content: tool_result_content_for_llm(
                                    &tc.name,
                                    &result_content,
                                    tool_metadata.as_ref(),
                                ),
                            });
                            // P-15: capture the per-call summary for the batch
                            // event. Reuse the already-computed line count and
                            // preview from the spawned task where available;
                            // fall back to computing them here for the batch.
                            let content_line_count = tool_metadata
                                .as_ref()
                                .and_then(|m| m.get("lines"))
                                .and_then(serde_json::Value::as_u64)
                                .map_or_else(|| result_content.lines().count(), |n| n as usize);
                            let batch_content = result_content.clone();
                            batch_entries.push(ragent_types::event::ToolCallBatchEntry {
                                call_id: tc.id.clone(),
                                tool: tc.name.clone(),
                                args: tc.args_json.clone(),
                                error: error.clone(),
                                duration_ms,
                                content: batch_content,
                                content_line_count,
                                metadata: tool_metadata.clone(),
                                success,
                            });
                            if let Some(meta) = tool_metadata.as_ref() {
                                if meta.get("agent_switch").is_some()
                                    || meta.get("agent_restore").is_some()
                                {
                                    agent_switch_requested = true;
                                    return true;
                                }
                                if meta.get("agent_complete").is_some() {
                                    agent_complete_requested = true;
                                    return true;
                                }
                            }
                            false
                        }
                        Err(e) => {
                            warn!(error = %e, "Tool execution task panicked");
                            false
                        }
                    }
                };

                for tc in &llm_result.tool_calls {
                    let _scope = profiler.scope("loop.tool_phase.prepare_call");
                    let input: Value = serde_json::from_str(&tc.args_json).unwrap_or_else(|e| {
                        warn!(error = %e, args = %tc.args_json, "Failed to parse tool call arguments");
                        json!({})
                    });
                    assistant_content_parts.push(ContentPart::ToolUse {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        input: input.clone(),
                    });
                    // Publish the fully-assembled tool arguments so the TUI can
                    // render the call summary regardless of provider-specific
                    // streaming quirks (some local providers omit ToolCallEnd
                    // stream events for the argument payload).
                    self.event_bus.publish(Event::ToolCallArgs {
                        session_id: session_id.to_string(),
                        call_id: tc.id.clone(),
                        tool: tc.name.clone(),
                        args: tc.args_json.clone(),
                    });
                    // P-8/P-9: clone the per-step `ToolContext` rather than
                    // rebuilding it (and re-acquiring the `active_spec` async
                    // lock) for every tool call. `base_tool_ctx` is built once
                    // before the loop; the clone is cheap (Arc refcount bumps
                    // plus a `PathBuf`/`String` clone).
                    let tool_ctx = base_tool_ctx.clone();
                    let tc_clone = tc.clone();
                    let registry = self.tool_registry.clone();
                    let permission_checker = self.permission_checker.clone();
                    let event_bus = self.event_bus.clone();
                    let event_bus_clone = self.event_bus.clone();
                    let session_id_str = session_id.to_string();
                    let session_id_for_perm = session_id.to_string();
                    let hook_working_dir = turn.working_dir.clone();
                    let hook_configs = turn.parsed_hook_configs.clone();
                    let extraction_engine = self.extraction_engine.clone();
                    let storage_clone = self.session_manager.storage().clone();
                    let profiler_clone = profiler.clone();
                    let auto_approve = self.auto_approve;
                    let team_context_cache = self.team_context_cache.clone();
                    let telemetry_clone = Arc::clone(&self.telemetry);
                    let fut = tokio::spawn(async move {
                        let _tool_total_scope =
                            profiler_clone.scope_with(|| format!("tool.total:{}", tc_clone.name));
                        event_bus.publish(Event::ToolCallStart {
                            session_id: session_id_str.clone(),
                            call_id: tc_clone.id.clone(),
                            tool: tc_clone.name.clone(),
                        });
                        let pre_hook_result = {
                            crate::hooks::run_pre_tool_use_hooks(
                                &hook_configs,
                                &hook_working_dir,
                                &tc_clone.name,
                                &tc_clone.args_json,
                                &session_id_str,
                                Some(&event_bus),
                            )
                        };
                        let tool_input = match pre_hook_result {
                            crate::hooks::PreToolUseResult::Allow => {
                                serde_json::from_str(&tc_clone.args_json)
                                    .unwrap_or_else(|_| serde_json::json!({}))
                            }
                            crate::hooks::PreToolUseResult::Deny { reason } => {
                                tracing::info!(tool = %tc_clone.name, reason = %reason, "PreToolUse hook denied tool execution");
                                let err_msg = format!("Permission denied by hook: {}", reason);
                                event_bus.publish(Event::ToolCallEnd {
                                    session_id: session_id_str.clone(),
                                    call_id: tc_clone.id.clone(),
                                    tool: tc_clone.name.clone(),
                                    error: Some(err_msg.clone()),
                                    duration_ms: 0,
                                });
                                let input_val: Value = serde_json::from_str(&tc_clone.args_json)
                                    .unwrap_or_else(|_| serde_json::json!({}));
                                return (
                                    tc_clone.clone(),
                                    input_val,
                                    ToolCallStatus::Error,
                                    None,
                                    Some(err_msg),
                                    0u64,
                                    String::new(),
                                    None,
                                );
                            }
                            crate::hooks::PreToolUseResult::Blocked { reason } => {
                                tracing::info!(tool = %tc_clone.name, reason = %reason, "PreToolUse hook blocked tool execution");
                                let err_msg = format!("Blocked by hook: {}", reason);
                                event_bus.publish(Event::ToolCallEnd {
                                    session_id: session_id_str.clone(),
                                    call_id: tc_clone.id.clone(),
                                    tool: tc_clone.name.clone(),
                                    error: Some(err_msg.clone()),
                                    duration_ms: 0,
                                });
                                let input_val: Value = serde_json::from_str(&tc_clone.args_json)
                                    .unwrap_or_else(|_| serde_json::json!({}));
                                return (
                                    tc_clone.clone(),
                                    input_val,
                                    ToolCallStatus::Error,
                                    None,
                                    Some(err_msg),
                                    0u64,
                                    String::new(),
                                    None,
                                );
                            }
                            crate::hooks::PreToolUseResult::ModifiedInput { input } => input,
                            crate::hooks::PreToolUseResult::NoDecision => {
                                serde_json::from_str(&tc_clone.args_json)
                                    .unwrap_or_else(|_| serde_json::json!({}))
                            }
                        };
                        let _permit = crate::resource::acquire_tool_permit()
                            .await
                            .map_err(|e| anyhow::anyhow!("tool permit: {e}"));
                        let start = Instant::now();
                        let tool_input_for_post_hook = serde_json::to_string(&tool_input)
                            .unwrap_or_else(|_| tc_clone.args_json.clone());
                        let result = registry
                            .get(&tc_clone.name)
                            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tc_clone.name));
                        let result = match result {
                            Ok(tool) => {
                                let perm_category = tool.permission_category();
                                if !perm_category.is_empty() && perm_category != "none" {
                                    let resource =
                                        extract_resource_from_input(&tool_input, &tc_clone.name);
                                    if tc_clone.name == "bash" {
                                        let sub_commands = split_bash_command(&resource);
                                        use ragent_tools_core::bash::is_safe_command;
                                        let all_safe = sub_commands.iter().all(|cmd| {
                                            let cmd_name = extract_command_name(cmd);
                                            is_safe_command(&cmd_name)
                                        });
                                        if all_safe {
                                            tool.execute(tool_input, &tool_ctx).await
                                        } else {
                                            let mut all_approved = true;
                                            for sub_cmd in &sub_commands {
                                                let cmd_name = extract_command_name(sub_cmd);
                                                let permission_action =
                                                    check_permission_with_prompt(
                                                        &permission_checker,
                                                        &event_bus,
                                                        &session_id_for_perm,
                                                        perm_category,
                                                        &cmd_name,
                                                        &tc_clone.name,
                                                        auto_approve,
                                                        Some(&tool_ctx.canonical_cache),
                                                    )
                                                    .await;
                                                match permission_action {
                                                    Ok(
                                                        crate::permission::PermissionAction::Allow,
                                                    ) => continue,
                                                    Ok(
                                                        crate::permission::PermissionAction::Deny,
                                                    ) => {
                                                        all_approved = false;
                                                        break;
                                                    }
                                                    Ok(
                                                        crate::permission::PermissionAction::Ask,
                                                    ) => {
                                                        all_approved = false;
                                                        break;
                                                    }
                                                    Err(_) => {
                                                        all_approved = false;
                                                        break;
                                                    }
                                                }
                                            }
                                            if all_approved {
                                                tool.execute(tool_input, &tool_ctx).await
                                            } else {
                                                Err(anyhow::anyhow!(
                                                    "Permission denied for one or more sub-commands"
                                                ))
                                            }
                                        }
                                    } else {
                                        let permission_action = check_permission_with_prompt(
                                            &permission_checker,
                                            &event_bus,
                                            &session_id_for_perm,
                                            perm_category,
                                            &resource,
                                            &tc_clone.name,
                                            auto_approve,
                                            Some(&tool_ctx.canonical_cache),
                                        )
                                        .await;
                                        match permission_action {
                                            Ok(crate::permission::PermissionAction::Allow) => {
                                                tool.execute(tool_input, &tool_ctx).await
                                            }
                                            Ok(crate::permission::PermissionAction::Deny) => {
                                                Err(anyhow::anyhow!(
                                                    "Permission denied by user or policy"
                                                ))
                                            }
                                            Ok(crate::permission::PermissionAction::Ask) => {
                                                Err(anyhow::anyhow!(
                                                    "Permission check returned Ask (internal error)"
                                                ))
                                            }
                                            Err(e) => Err(e),
                                        }
                                    }
                                } else {
                                    tool.execute(tool_input, &tool_ctx).await
                                }
                            }
                            Err(e) => Err(e),
                        };
                        let duration_ms = start.elapsed().as_millis() as u64;
                        let tool_recorder = ToolRecorder::from_subsystem(&telemetry_clone);
                        tool_recorder.record_invocation(&tc_clone.name);
                        tool_recorder.record_duration(&tc_clone.name, duration_ms as f64);
                        if tc_clone.name.starts_with("team_") {
                            team_context_cache.write().clear();
                        }
                        let output_content = result
                            .as_ref()
                            .map(|o| o.content.clone())
                            .unwrap_or_default();
                        let output_json = result
                            .as_ref()
                            .ok()
                            .and_then(|o| o.metadata.clone())
                            .unwrap_or_else(|| serde_json::json!({"content": output_content}));
                        let success = result.is_ok();
                        let post_hook_result = {
                            crate::hooks::run_post_tool_use_hooks(
                                &hook_configs,
                                &hook_working_dir,
                                &tc_clone.name,
                                &tool_input_for_post_hook,
                                &output_json.to_string(),
                                success,
                                &session_id_str,
                                Some(&event_bus),
                            )
                            .await
                        };
                        let modified_output = match post_hook_result {
                            crate::hooks::PostToolUseResult::Ok { modified_output } => {
                                modified_output
                            }
                            crate::hooks::PostToolUseResult::Flagged { reason } => {
                                tracing::info!(
                                    tool = %tc_clone.name,
                                    reason = %reason,
                                    "PostToolUse hook flagged tool result as policy-violated"
                                );
                                None
                            }
                            crate::hooks::PostToolUseResult::Warn { message } => {
                                tracing::info!(
                                    tool = %tc_clone.name,
                                    message = %message,
                                    "PostToolUse hook emitted warning"
                                );
                                None
                            }
                        };
                        let result = if let Some(modified) = modified_output {
                            if let Some(modified_content) =
                                modified.get("content").and_then(|v| v.as_str())
                            {
                                Ok(crate::tool::ToolOutput {
                                    content: modified_content.to_string(),
                                    metadata: Some(modified.clone()),
                                })
                            } else {
                                result
                            }
                        } else {
                            result
                        };
                        let (output_value, error) = match &result {
                            Ok(output) => {
                                let val = match &output.metadata {
                                    Some(meta) if meta.is_object() => {
                                        let mut obj = meta.clone();
                                        if let Some(map) = obj.as_object_mut() {
                                            map.insert(
                                                "content".to_string(),
                                                json!(output.content),
                                            );
                                        }
                                        obj
                                    }
                                    _ => json!({ "content": output.content }),
                                };
                                (Some(val), None)
                            }
                            Err(e) => (None, Some(format!("{e:#}"))),
                        };
                        if let Some(err_msg) = &error
                            && err_msg.contains("permission denied")
                        {
                            crate::hooks::fire_hooks(
                                &hook_configs,
                                crate::hooks::HookTrigger::OnPermissionDenied,
                                &hook_working_dir,
                                &[("RAGENT_ERROR", err_msg.as_str())],
                            );
                        }
                        let status = if result.is_ok() {
                            ToolCallStatus::Completed
                        } else {
                            ToolCallStatus::Error
                        };
                        // Recovery for a message-window display race: when a
                        // tool call fails with a permission error, the TUI
                        // drains the event queue in the same wake as the
                        // `PermissionReplied` handler and re-renders the
                        // chat — removing the in-flight ToolCall part that
                        // `ToolCallStart` created (the pending prompt is
                        // gone, so the part is no longer protected). The
                        // already-queued `ToolCallArgs` then finds no part
                        // and is buffered in `pending_tool_args`, while the
                        // re-created part from `ToolCallEnd` renders with an
                        // empty input. Republish the args now, after the
                        // part has been re-created, so the buffered args are
                        // applied and the tool's parameters/category icon
                        // appear in the message window.
                        if result.is_err()
                            && error.as_deref().is_some_and(|e| {
                                e.contains("Permission denied") || e.contains("Blocked by hook")
                            })
                        {
                            event_bus.publish(Event::ToolCallArgs {
                                session_id: session_id_str.clone(),
                                call_id: tc_clone.id.clone(),
                                tool: tc_clone.name.clone(),
                                args: tc_clone.args_json.clone(),
                            });
                        }
                        let success = status == ToolCallStatus::Completed;
                        event_bus.publish(Event::ToolCallEnd {
                            session_id: session_id_str.clone(),
                            call_id: tc_clone.id.clone(),
                            tool: tc_clone.name.clone(),
                            error: error.clone(),
                            duration_ms,
                        });
                        let result_content = match &result {
                            Ok(output) => output.content.clone(),
                            Err(e) => format!("Error: {e}"),
                        };
                        let content_line_count = result
                            .as_ref()
                            .ok()
                            .and_then(|o| o.metadata.as_ref())
                            .and_then(|m| m.get("lines"))
                            .and_then(serde_json::Value::as_u64)
                            .map_or_else(|| result_content.lines().count(), |n| n as usize);
                        let result_preview = result_content.clone();
                        let tool_metadata = result.as_ref().ok().and_then(|o| o.metadata.clone());
                        event_bus.publish(Event::ToolResult {
                            session_id: session_id_str.clone(),
                            call_id: tc_clone.id.clone(),
                            tool: tc_clone.name.clone(),
                            content: result_preview,
                            content_line_count,
                            metadata: tool_metadata.clone(),
                            success,
                        });
                        if let Some(engine) = extraction_engine.get() {
                            let sid = session_id_str.clone();
                            engine.on_tool_result(
                                &tc_clone.name,
                                &input,
                                &result_content,
                                success,
                                &sid,
                                &storage_clone,
                                &event_bus_clone,
                                &hook_working_dir,
                            );
                        }
                        (
                            tc_clone,
                            input,
                            status,
                            output_value,
                            error,
                            duration_ms,
                            result_content,
                            tool_metadata,
                        )
                    });
                    if parallel_tool_calls {
                        // Capture a human-readable descriptor for the watchdog
                        // timeout path before `tc` is borrowed/moved.
                        let watchdog_tool_desc = format!("'{}' ({})", tc.name, tc.id);
                        let abort_handle = fut.abort_handle();
                        futures.push(async move {
                            match tokio::time::timeout(TOOL_WATCHDOG_TIMEOUT, fut).await {
                                Ok(result) => (result.map_err(ToolTaskError::Join), None),
                                Err(_) => {
                                    abort_handle.abort();
                                    (Err(ToolTaskError::WatchdogAbort), Some(watchdog_tool_desc))
                                }
                            }
                        });
                    } else {
                        let watchdog_tool_desc = format!("'{}' ({})", tc.name, tc.id);
                        let watchdog_call_id = tc.id.clone();
                        let watchdog_tool_name = tc.name.clone();
                        let abort_handle = fut.abort_handle();
                        let result = match tokio::time::timeout(TOOL_WATCHDOG_TIMEOUT, fut).await {
                            Ok(result) => result.map_err(ToolTaskError::Join),
                            Err(_) => {
                                abort_handle.abort();
                                watchdog_timed_out = true;
                                let msg = watchdog_timeout_msg(&watchdog_tool_desc);
                                warn!("{}", msg);
                                // Close out the tool call in the UI: no `ToolCallEnd`
                                // was published because the spawned task was aborted.
                                self.event_bus.publish(Event::ToolCallEnd {
                                    session_id: session_id.to_string(),
                                    call_id: watchdog_call_id,
                                    tool: watchdog_tool_name,
                                    error: Some(msg.clone()),
                                    duration_ms: TOOL_WATCHDOG_TIMEOUT.as_millis() as u64,
                                });
                                self.event_bus.publish(Event::AgentError {
                                    session_id: session_id.to_string(),
                                    error: msg.clone(),
                                });
                                self.event_bus.publish(Event::AgentNotice {
                                    session_id: session_id.to_string(),
                                    message: msg,
                                });
                                // Stop processing further tool calls for this turn.
                                break;
                            }
                        };
                        if handle_tool_execution_result(result) {
                            break;
                        }
                    }
                }
                if parallel_tool_calls {
                    let results = {
                        let _scope = profiler.scope("loop.tool_phase.join_parallel");
                        futures::future::join_all(futures).await
                    };
                    for (result, stalled_tool) in results {
                        if let Some(ref tool_desc) = stalled_tool {
                            watchdog_timed_out = true;
                            let msg = watchdog_timeout_msg(tool_desc);
                            warn!("{}", msg);
                            // Close out the tool call in the UI: the spawned
                            // task was aborted so no `ToolCallEnd` was emitted
                            // by the normal completion path.
                            self.event_bus.publish(Event::ToolCallEnd {
                                session_id: session_id.to_string(),
                                call_id: "watchdog-parallel".to_string(),
                                tool: "unknown".to_string(),
                                error: Some(msg.clone()),
                                duration_ms: TOOL_WATCHDOG_TIMEOUT.as_millis() as u64,
                            });
                            self.event_bus.publish(Event::AgentError {
                                session_id: session_id.to_string(),
                                error: msg.clone(),
                            });
                            self.event_bus.publish(Event::AgentNotice {
                                session_id: session_id.to_string(),
                                message: msg,
                            });
                            break;
                        }
                        match result {
                            Ok(ok) => {
                                if handle_tool_execution_result(Ok(ok)) {
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "Tool execution task failed to join");
                                break;
                            }
                        }
                    }
                }
                // P-15: publish a single `ToolCallBatch` for this step with
                // all per-call summaries, so consumers can render atomically.
                if !batch_entries.is_empty() {
                    self.event_bus.publish(Event::ToolCallBatch {
                        session_id: session_id_arc.to_string(),
                        step: step as u64,
                        calls: batch_entries,
                    });
                }
                if agent_switch_requested || agent_complete_requested || watchdog_timed_out {
                    break;
                }
                // Auto task status updates (P-10: reuse the `active_spec_id`
                // already read above for the `ToolContext`, and short-circuit
                // when no spec is active or no file-write tool was called).
                {
                    if let Some(ref spec_id_str) = active_spec_id
                        && let Some(ref spec_mgr) = self.spec_manager.get()
                        && llm_result.tool_calls.iter().any(|tc| {
                            matches!(
                                tc.name.as_str(),
                                "write"
                                    | "edit"
                                    | "multiedit"
                                    | "multi_edit"
                                    | "patch"
                                    | "create"
                                    | "append_to_file"
                            )
                        })
                        && let Some(id) = ragent_specs::spec::SpecId::new(spec_id_str)
                        && let Ok(mut spec) = spec_mgr.read_spec(&id).await
                    {
                        let mut updated = false;
                        for task in spec.tasks.iter_mut() {
                            if task.status == ragent_specs::spec::TaskStatus::InProgress {
                                task.status = ragent_specs::spec::TaskStatus::Completed;
                                task.completed_at = Some(
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs(),
                                );
                                updated = true;
                            }
                        }
                        if updated {
                            if let Err(e) = spec_mgr.write_spec(&spec).await {
                                tracing::warn!(error = %e, "Auto task update: failed to write spec");
                            } else {
                                tracing::info!(spec_id = %spec_id_str, "Auto-updated in_progress tasks to completed after file write");
                            }
                        }
                    }
                }
                // Append to chat history (P-17: `std::mem::take` moves the
                // contents into `ChatContent::Parts` while leaving the Vec
                // buffer allocated for reuse on the next step). P-6: mutate the
                // shared `Arc<Vec>` via `Arc::make_mut` so an unchanged
                // history is not cloned.
                Arc::make_mut(&mut chat_messages).push(ChatMessage {
                    role: "assistant".to_string(),
                    content: ChatContent::Parts(std::mem::take(&mut assistant_content_parts)),
                });
                Arc::make_mut(&mut chat_messages).push(ChatMessage {
                    role: "user".to_string(),
                    content: ChatContent::Parts(std::mem::take(&mut tool_result_parts)),
                });
            }

            // Background task injection (sub-agents)
            {
                let _scope = profiler.scope("loop.background.total");
                if let Some(tm) = self.agent_manager.get() {
                    // P-11: skip the lock+scan when no background tasks are
                    // pending. The flag is set by `spawn_background` and
                    // cleared by `drain_completed` when nothing remains.
                    if tm.has_pending_background() {
                        let completed = tm.drain_completed(session_id).await;
                        if !completed.is_empty() {
                            let _scope = profiler.scope("loop.background.inject_completed");
                            // P-17: reuse the hoisted `bg_parts` buffer.
                            bg_parts.clear();
                            for task in &completed {
                                let status_label = match task.status {
                                    crate::task::TaskStatus::Completed => "completed",
                                    crate::task::TaskStatus::Failed => "failed",
                                    crate::task::TaskStatus::Cancelled => "cancelled",
                                    crate::task::TaskStatus::Suspended => "suspended",
                                    crate::task::TaskStatus::Terminating => "terminating",
                                    crate::task::TaskStatus::Running => "running",
                                };
                                let body = task
                                    .result
                                    .as_deref()
                                    .or(task.error.as_deref())
                                    .unwrap_or("(no output)");
                                let mut text = format!(
                                    "[Background Task {status_label}: {} — {}]\n\n{body}",
                                    task.agent_name,
                                    task.id.chars().take(8).collect::<String>()
                                );
                                // The injected body may later be cut by the
                                // generic 12k tool-result truncation; point at
                                // the durable on-disk report so the model can
                                // recover any omitted content with the `read`
                                // tool instead of re-running the sub-agent.
                                if let Some(ref file) = task.output_file {
                                    text.push_str(&format!(
                                        "\n\n(Full untruncated report: {} — read this \
                                         file with the `read` tool if the output above \
                                         appears truncated.)",
                                        file.display()
                                    ));
                                }
                                bg_parts.push(ContentPart::Text { text });
                            }
                            Arc::make_mut(&mut chat_messages).push(ChatMessage {
                                role: "user".to_string(),
                                content: ChatContent::Parts(std::mem::take(&mut bg_parts)),
                            });
                        }
                    }
                }
            }

            // Background shell task injection (M3 / T-023)
            {
                let _scope = profiler.scope("loop.background.bg_shell");
                if let Some(bg) = self.bg_service.get() {
                    if bg.has_pending_completions() {
                        let completed = bg.drain_completed(session_id).await;
                        if !completed.is_empty() {
                            bg_parts.clear();
                            for task in &completed {
                                let exit_str = task
                                    .exit_code
                                    .map(|c| format!("exit={c}"))
                                    .unwrap_or_else(|| "exit=?".to_string());
                                let text = format!(
                                    "[Background Shell Task {}: {} — {} ({})]\n\n{}",
                                    task.status,
                                    task.command,
                                    task.task_id.chars().take(8).collect::<String>(),
                                    exit_str,
                                    if task.tail.is_empty() {
                                        "(no output)"
                                    } else {
                                        &task.tail
                                    }
                                );
                                bg_parts.push(ContentPart::Text { text });
                            }
                            Arc::make_mut(&mut chat_messages).push(ChatMessage {
                                role: "user".to_string(),
                                content: ChatContent::Parts(std::mem::take(&mut bg_parts)),
                            });
                        }
                    }
                }
            }
            // Interim save
            {
                let _scope = profiler.scope("storage.assistant_interim.update");
                // P-12: hash the assistant parts to detect changes since the
                // last interim save. Tool-call input/output are hashed via
                // `serde_json`'s serialised bytes rather than
                // `Value::to_string()`, which re-serialises every tool-call
                // input/output on every step. `serde_json::to_vec` produces a
                // canonical byte form that hashes directly with no
                // intermediate `String` allocation.
                let current_hash = {
                    use rustc_hash::FxHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = FxHasher::default();
                    for part in &*assistant_parts {
                        std::mem::discriminant(part).hash(&mut hasher);
                        match part {
                            MessagePart::Text { text } => text.hash(&mut hasher),
                            MessagePart::ToolCall {
                                tool,
                                call_id,
                                state,
                            } => {
                                tool.hash(&mut hasher);
                                call_id.hash(&mut hasher);
                                // P-12: hash the status discriminant only (the
                                // `ToolCallStatus` enum does not derive `Hash`,
                                // but its discriminant is stable and sufficient
                                // for change detection).
                                std::mem::discriminant(&state.status).hash(&mut hasher);
                                hash_value(&mut hasher, &state.input);
                                if let Some(out) = &state.output {
                                    hash_value(&mut hasher, out);
                                }
                                if let Some(err) = &state.error {
                                    err.hash(&mut hasher);
                                }
                                if let Some(dur) = &state.duration_ms {
                                    dur.hash(&mut hasher);
                                }
                            }
                            MessagePart::Reasoning { text } => text.hash(&mut hasher),
                            MessagePart::Image(img) => {
                                img.mime_type.hash(&mut hasher);
                                img.path.hash(&mut hasher);
                            }
                        }
                    }
                    hasher.finish()
                };
                if last_interim_hash != Some(current_hash) {
                    let mut interim =
                        Message::new(session_id, Role::Assistant, (*assistant_parts).clone());
                    interim.id = assistant_msg_id.clone();
                    // H3: use the FTS-skip variant for the interim save. The
                    // searchable text content of the interim message is either
                    // unchanged (only a tool-call status transition) or the
                    // message is still accumulating deltas and will be
                    // re-synced wholesale on the final save. Rewriting the FTS
                    // index on every stream event (DELETE + re-INSERT) was the
                    // dominant cost of `storage.assistant_interim.update`.
                    let _ = self
                        .storage_op(move |s| s.update_message_parts_skip_fts(&interim))
                        .await;
                    last_interim_hash = Some(current_hash);
                }
            }
        }

        // Watchdog termination: persist the accumulated assistant parts, end the
        // message with the cancelled reason (the closest existing variant to a
        // watchdog-forced stop), and return a fatal error for the run.
        if watchdog_timed_out {
            let total_elapsed_ms = total_start.elapsed().as_millis() as u64;
            let parts_owned =
                std::sync::Arc::try_unwrap(assistant_parts).unwrap_or_else(|arc| (*arc).clone());
            let mut assistant_msg = Message::new(session_id, Role::Assistant, parts_owned);
            assistant_msg.id = assistant_msg_id;
            let msg_id = assistant_msg.id.clone();
            let _ = self
                .storage_op(move |s| s.update_message(&assistant_msg))
                .await;
            self.event_bus.publish(Event::MessageEnd {
                session_id: session_id.to_string(),
                message_id: msg_id,
                reason: FinishReason::Cancelled,
            });
            publish_run_cost_summary(total_elapsed_ms);
            return Err(anyhow::anyhow!(
                "agent run terminated: tool call stalled beyond the {}s watchdog timeout",
                TOOL_WATCHDOG_TIMEOUT.as_secs()
            ));
        }

        // 8. Finalize
        let parts_owned =
            std::sync::Arc::try_unwrap(assistant_parts).unwrap_or_else(|arc| (*arc).clone());
        let mut assistant_msg = Message::new(session_id, Role::Assistant, parts_owned);
        assistant_msg.id = assistant_msg_id;
        // P-20: move the message into the `storage_op` closure and have the
        // closure return it, so we avoid cloning the full `Message` (which
        // includes the parts `Vec`) on the final save. The closure owns the
        // message, persists it, and hands it back to us via the `storage_op`
        // return value. The id is cloned only for the `MessageEnd` event.
        // (`Message` has no `Default`, so we use `std::mem::replace` with a
        // cheap placeholder rather than `std::mem::take`.)
        let msg_id_for_end = assistant_msg.id.clone();
        let moved_msg = std::mem::replace(
            &mut assistant_msg,
            Message::new(session_id, Role::Assistant, Vec::new()),
        );
        let saved_msg = self
            .storage_op(move |s| {
                s.update_message(&moved_msg)?;
                Ok(moved_msg)
            })
            .await?;
        let total_elapsed_ms = total_start.elapsed().as_millis() as u64;
        let other_ms = total_elapsed_ms.saturating_sub(cumulative_model_wait_ms);
        tracing::info!(
            session_id = %session_id, total_ms = total_elapsed_ms, model_wait_ms = cumulative_model_wait_ms, other_ms = other_ms,
            "Agent loop finished - timing breakdown: total={}ms, model_wait={}ms, other={}ms",
            total_elapsed_ms, cumulative_model_wait_ms, other_ms
        );
        let iterations = self.event_bus.current_step(session_id);
        session_recorder.record_session_end();
        session_recorder.record_agent_loop(total_elapsed_ms as f64, iterations);
        publish_run_cost_summary(total_elapsed_ms);
        let end_reason = last_finish_reason.unwrap_or(FinishReason::Stop);
        // Interactive sessions get a visible hint when the provider
        // silently truncated the reply.
        if agent.mode != crate::agent::AgentMode::Subagent
            && matches!(end_reason, FinishReason::Length | FinishReason::Truncation)
        {
            self.event_bus.publish(Event::AgentNotice {
                session_id: session_id.to_string(),
                message: format!(
                    "The provider ended the response without completing it ({}). \
                       The saved reply may be incomplete.",
                    crate::session::finish_reason_label(&end_reason)
                ),
            });
        }
        self.event_bus.publish(Event::MessageEnd {
            session_id: session_id.to_string(),
            message_id: msg_id_for_end,
            reason: end_reason,
        });
        crate::hooks::fire_hooks(
            &turn.parsed_hook_configs,
            crate::hooks::HookTrigger::OnSessionEnd,
            &turn.working_dir,
            &[],
        );
        Ok(saved_msg)
    }

    /// Run the display-only AGENTS.md acknowledgement exchange.
    ///
    /// Streams a one-shot init exchange to the UI so the user sees that the
    /// project guidelines were loaded. The exchanged messages are NOT added to
    /// the persisted `chat_messages` history — this is purely a UI affordance.
    ///
    /// # Errors
    ///
    /// Returns an error if the LLM call fails, the session cannot be resolved,
    /// or the operation is cancelled via `cancel_flag`.
    pub async fn run_init_exchange(
        &self,
        session_id: &str,
        agent: &AgentInfo,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<()> {
        // Resolve working directory.
        let working_dir = self.session_manager.get_session(session_id)?.map_or_else(
            || std::env::current_dir().unwrap_or_default(),
            |s| s.directory,
        );

        // Collect instruction files with discovery info for logging
        let (agents_md, discovery) =
            crate::agent::collect_agents_md_content_with_discovery(&working_dir);

        // Log discovery info to tracing and emit AgentNotice event
        let discovery_msg = discovery.format_summary();
        tracing::info!(
            session_id = %session_id,
            "{}",
            discovery_msg
        );
        self.event_bus.publish(Event::AgentNotice {
            session_id: session_id.to_string(),
            message: discovery_msg.clone(),
        });

        // Check if any instruction files were found
        if discovery.all_discovered_files.is_empty() {
            return Ok(());
        }

        // Skip if an assistant message already exists (init already ran).
        // PERF-010: use the cheap existence check instead of loading the
        // full message history just to test whether an assistant turn
        // has already been recorded.
        let already_done = self
            .session_manager
            .storage()
            .has_assistant_messages(session_id)
            .unwrap_or(false);
        if already_done {
            return Ok(());
        }
        // Resolve model / provider — bail silently if not configured yet.
        let model_ref = match agent.model.as_ref() {
            Some(m) => m,
            None => {
                tracing::debug!(
                    session_id = %session_id,
                    "run_init_exchange: no model configured, skipping"
                );
                return Ok(());
            }
        };
        let provider = match self.provider_registry.get(&model_ref.provider_id) {
            Some(p) => p,
            None => {
                tracing::debug!(
                    session_id = %session_id,
                    provider = %model_ref.provider_id,
                    "run_init_exchange: provider not found, skipping"
                );
                return Ok(());
            }
        };
        let api_key = match self.resolve_api_key(&model_ref.provider_id).await {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "run_init_exchange: API key not available, skipping"
                );
                return Ok(());
            }
        };

        let client = match provider
            .create_client(&api_key, None, &HashMap::new())
            .await
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(session_id=%session_id, error=%e, "run_init_exchange: client creation failed");
                return Ok(());
            }
        };

        // Build a minimal system prompt using the agent's configured prompt.
        // Note: agents_md was already collected above with discovery info
        let (git_status, readme, _, file_tree) =
            crate::agent::collect_prompt_context(&working_dir).await;
        let run_init_config = crate::Config::load().unwrap_or_default();
        let system_prompt = crate::agent::build_system_prompt_with_storage(
            agent,
            &working_dir,
            &file_tree,
            None,
            Some(&git_status),
            Some(&readme),
            Some(&agents_md),
            Some(self.session_manager.storage()),
            Some(&run_init_config.memory),
        );
        // PERF-006: wrap the init-exchange prompt in `Arc<str>` so the
        // `ChatRequest::system` field is satisfied without an intermediate
        // `String` clone.
        let system_prompt: std::sync::Arc<str> = std::sync::Arc::from(system_prompt);
        const INIT_ACK_PROMPT: &str = "AGENTS.md project guidelines have been loaded.\n\n\
                                        Please acknowledge them briefly.";
        let init_messages = vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(INIT_ACK_PROMPT.to_string()),
        }];
        let init_request = ChatRequest {
            model: model_ref.model_id.clone(),
            messages: Arc::new(init_messages),
            tools: Arc::new(Vec::new()),
            temperature: agent.temperature,
            top_p: agent.top_p,
            max_tokens: Some(64),
            system: Some(system_prompt),
            options: (*agent.options).clone(),
            session_id: Some(session_id.to_string()),
            request_id: Some(Uuid::new_v4().to_string()),
            stream_timeout_secs: None,
            thinking: Some(ThinkingConfig::off()),
        };

        let mut ack_text = String::new();

        match client.chat(init_request).await {
            Ok(mut stream) => {
                while let Some(ev) = stream.next().await {
                    if cancel_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    match ev {
                        StreamEvent::TextDelta { text } => {
                            ack_text.push_str(&text);
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "AGENTS.md init exchange failed — skipping acknowledgement"
                );
                self.event_bus.publish(Event::MessageEnd {
                    session_id: session_id.to_string(),
                    message_id: "init".to_string(),
                    reason: FinishReason::Stop,
                });
                return Ok(());
            }
        }

        // Save both the user trigger and the assistant ack to DB so the
        // conversation history is well-formed (alternating user/assistant).
        // Without the user message, history starts with an orphaned Assistant
        // turn which many LLM APIs reject or mishandle, causing the model to
        // ignore tools or the system prompt on the follow-up turn.
        if !ack_text.is_empty() {
            self.event_bus.publish(Event::TextDelta {
                session_id: session_id.to_string(),
                text: ack_text.clone(),
            });
            let init_user_text = INIT_ACK_PROMPT;
            let user_msg = Message::new(
                session_id,
                Role::User,
                vec![MessagePart::Text {
                    text: init_user_text.to_string(),
                }],
            );
            let ack_msg = Message::new(
                session_id,
                Role::Assistant,
                vec![MessagePart::Text { text: ack_text }],
            );
            let _ = self
                .storage_op(move |s| {
                    s.create_message(&user_msg)?;
                    s.create_message(&ack_msg)?;
                    Ok(())
                })
                .await;
        }

        self.event_bus.publish(Event::MessageEnd {
            session_id: session_id.to_string(),
            message_id: "init".to_string(),
            reason: FinishReason::Stop,
        });

        Ok(())
    }

    pub(crate) async fn resolve_api_key(&self, provider_id: &str) -> Result<String> {
        // The router provider is virtual: it delegates to downstream providers
        // and does not need its own API key.
        if provider_id == "router" {
            return Ok(String::new());
        }

        // Ollama does not require an API key for local servers
        if provider_id == "ollama" {
            return Ok(std::env::var("OLLAMA_API_KEY").unwrap_or_default());
        }

        // Copilot: prefer DB-stored device flow token (works for token
        // exchange), then fall back to env var → IDE → gh CLI discovery.
        if provider_id == "copilot" {
            // DB first — device flow tokens stored here work for copilot_internal/v2/token
            if let Ok(Some(key)) = self.storage_op(|s| s.get_provider_auth("copilot")).await
                && !key.is_empty()
            {
                return Ok(key);
            }
            let db_lookup = || -> Option<String> { None }; // already checked above
            if let Some(token) =
                crate::provider::copilot::resolve_copilot_github_token(Some(&db_lookup))
            {
                crate::sanitize::register_secret(&token);
                return Ok(token);
            }
            bail!(
                "No GitHub token found for Copilot. Use /provider to configure, \
                 or authenticate with `gh auth login` then `gh auth refresh -s copilot`."
            );
        }

        // Azure Foundry: also check azure_resource_last_selection for key config
        if provider_id == "azure_foundry" || provider_id == "azure_resource" {
            if let Ok(Some(last)) = self
                .storage_op(|s| s.get_setting("azure_resource_last_selection"))
                .await
            {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&last) {
                    // Direct api_key takes precedence
                    if let Some(key) = parsed.get("api_key").and_then(|v| v.as_str()) {
                        if !key.is_empty() {
                            return Ok(key.to_string());
                        }
                    }
                    // Fall back to api_key_env
                    if let Some(env_var) = parsed.get("api_key_env").and_then(|v| v.as_str()) {
                        if let Ok(key) = std::env::var(env_var)
                            && !key.is_empty()
                        {
                            return Ok(key);
                        }
                    }
                }
            }
        }
        let env_vars = match provider_id {
            "anthropic" => vec!["ANTHROPIC_API_KEY"],
            "openai" => vec!["OPENAI_API_KEY"],
            "gemini" => vec!["GEMINI_API_KEY"],
            "huggingface" => vec!["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN"],
            "generic_openai" => vec!["OPENAI_API_KEY", "GENERIC_OPENAI_API_KEY"],
            "ollama_cloud" => vec!["OLLAMA_API_KEY"],
            "azure_foundry" => vec!["AZURE_AI_FOUNDRY_API_KEY"],
            _ => vec![],
        };

        for var in &env_vars {
            if let Ok(key) = std::env::var(var)
                && !key.is_empty()
            {
                return Ok(key);
            }
        }

        // Check the database for a stored API key
        {
            let pid = provider_id.to_string();
            if let Ok(Some(key)) = self.storage_op(move |s| s.get_provider_auth(&pid)).await
                && !key.is_empty()
            {
                return Ok(key);
            }
        }

        bail!(
            "No API key found for provider '{provider_id}'. Set the appropriate environment variable \
             or run `ragent auth {provider_id} <key>` to store one."
        )
    }
}

/// P-12: hash a [`serde_json::Value`] into the supplied hasher using its
/// serialised bytes, avoiding the `Value::to_string()` allocation that the
/// interim-save hash previously paid for every tool-call input/output on
/// every loop step.
///
/// Falls back to hashing the `Value`'s `Debug` representation if
/// serialisation fails (effectively never for valid `Value`s, but keeps the
/// hash total — every code path contributes — so the change-detection
/// logic stays sound).
fn hash_value<H: std::hash::Hasher>(hasher: &mut H, value: &Value) {
    use std::hash::Hash;
    match serde_json::to_vec(value) {
        Ok(bytes) => bytes.hash(hasher),
        Err(_) => format!("{value:?}").hash(hasher),
    }
}
