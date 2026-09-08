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
use crate::error::classify_message;
use crate::event::{Event, EventBus, FinishReason};
use crate::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent, ToolDefinition};
use crate::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use crate::permission::PermissionChecker;
use crate::provider::ProviderRegistry;
use crate::session::SessionManager;
use crate::session::cache::SystemPromptCache;
use crate::session::history::PendingToolCall;
use crate::session::loop_state::LoopSpec;
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
/// aborts it and terminates the agent run (2000 seconds).
const TOOL_WATCHDOG_TIMEOUT: Duration = Duration::from_secs(2000); // ≈ 33m 20s

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
fn watchdog_timeout_msg(tool_name: &str, call_id: &str) -> String {
    format!(
        "Tool call '{tool_name}' ({call_id}) stalled for over {}s \
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

/// Structured identity of a tool call whose watchdog fired, captured BEFORE
/// the per-call future moves `tc` so the drain loop can publish `ToolCallEnd`
/// with the real call id without re-parsing a display string.
type WatchdogIdentity = (String, String);

/// Returns `true` when at least one of `tool_calls` is a file-writing tool
/// whose path argument resolves inside the given spec directory.
///
/// Guards the auto task-completion heuristic so writes elsewhere in the
/// workspace cannot complete spec tasks.
fn writes_in_spec_dir(
    tool_calls: &[crate::session::history::PendingToolCall],
    working_dir: &std::path::Path,
    specs_root: &std::path::Path,
    spec_id: &ragent_specs::spec::SpecId,
) -> bool {
    let spec_dir = specs_root.join(spec_id.as_str());
    tool_calls.iter().any(|tc| {
        if !matches!(
            tc.name.as_str(),
            "write" | "edit" | "multiedit" | "multi_edit" | "patch" | "apply_patch" | "create"
                | "append_to_file"
        ) {
            return false;
        }
        let Ok(args): Result<serde_json::Value, _> = serde_json::from_str(&tc.args_json) else {
            tracing::warn!(
                tool = %tc.name,
                "unparseable args in spec-dir write check"
            );
            return false;
        };
        let mut args_paths = ["path", "file_path"]
            .iter()
            .filter_map(|k| args[k].as_str())
            .chain(
                // The multiedit family carries one path per edit entry.
                args["edits"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|e| ["path", "file_path"].iter().filter_map(|k| e[k].as_str())),
            );
        args_paths
            .any(|p| {
                let resolved = if std::path::Path::new(p).is_absolute() {
                    std::path::PathBuf::from(p)
                } else {
                    working_dir.join(p)
                };
                resolved.starts_with(&spec_dir)
            })
    })
}

/// C5: builds the unknown-tool error message, attaching a "did you mean"
/// hint when a registered tool name is similar to the hallucinated one.
///
/// Similarity is deliberately cheap: case-insensitive equality, prefix match,
/// containment, or a small edit distance against the registry's tool list.
/// Structural matches (equality/prefix/containment) always outrank edit
/// distance, and among equals the smallest distance wins — a `find` over
/// registry order would otherwise let a loose containment match on an
/// early-listed tool beat a minimal-distance match later on.
fn unknown_tool_error(registry: &crate::tool::ToolRegistry, name: &str) -> anyhow::Error {
    let lowered = name.to_lowercase();
    let lowered_len = lowered.chars().count();
    let max_distance = usize::max(2, lowered_len / 3);
    // Rank tiers: 0 = equality, 1 = prefix (either direction), 2 = containment
    // (either direction), 3 = edit distance. Within a tier, smaller edit
    // distance wins; ties keep the first candidate.
    let mut best: Option<(u8, usize, String)> = None;
    for candidate in registry.list() {
        let cand_lower = candidate.to_lowercase();
        let rank = if cand_lower == lowered {
            0
        } else if cand_lower.starts_with(&lowered) || lowered.starts_with(&cand_lower) {
            1
        } else if cand_lower.contains(&lowered) || lowered.contains(&cand_lower) {
            2
        } else {
            // Bail before building the distance matrix when the length
            // difference alone already exceeds the budget.
            let cand_len = cand_lower.chars().count();
            if cand_len.abs_diff(lowered_len) > max_distance {
                continue;
            }
            let distance = edit_distance(&cand_lower, &lowered);
            if distance > max_distance {
                continue;
            }
            3
        };
        let distance = edit_distance(&cand_lower, &lowered);
        let better = match &best {
            None => true,
            Some((best_rank, best_distance, _)) => {
                rank < *best_rank || (rank == *best_rank && distance < *best_distance)
            }
        };
        if better {
            best = Some((rank, distance, candidate));
        }
    }
    match best {
        Some((_, _, candidate)) => {
            anyhow::anyhow!("Unknown tool: '{name}'. Did you mean '{candidate}'?")
        }
        None => anyhow::anyhow!("Unknown tool: '{name}'"),
    }
}

/// Levenshtein edit distance between two tool names.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, a_char) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, b_char) in b.iter().enumerate() {
            let substitution_cost = usize::from(a_char != b_char);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + substitution_cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// B3: validates a tool call's arguments against the tool's declared
/// `parameters_schema()`. Shared by the B1 args-parse gate (where the payload
/// is unparseable and `Value::Null` is validated) and the pre-dispatch gate in
/// the execution task, so both sites report identical corrective reasons.
fn schema_violation(
    registry: &crate::tool::ToolRegistry,
    tool_name: &str,
    input: &Value,
) -> Result<(), String> {
    match registry.get(tool_name) {
        Some(tool) => {
            ragent_tools_core::schema::validate_required_args(&tool.parameters_schema(), input)
        }
        None => Err("tool not registered".to_string()),
    }
}

/// B3: dispatches a validated tool call through the permission layer.
///
/// Extracted from the inline task body so the schema-validation gate and the
/// permission checks live in one readable place. The behaviour is unchanged
/// from the previous inline implementation.
#[allow(clippy::too_many_arguments)]
async fn dispatch_tool_with_permissions(
    tool: &std::sync::Arc<dyn crate::tool::Tool>,
    tool_input: Value,
    tc: &crate::session::history::PendingToolCall,
    tool_ctx: &ToolContext,
    permission_checker: &Arc<parking_lot::RwLock<crate::permission::PermissionChecker>>,
    event_bus: &Arc<EventBus>,
    session_id_for_perm: &str,
    auto_approve: Option<bool>,
    checkpoint_forced: bool,
    checkpoint_timeout_secs: u64,
) -> anyhow::Result<crate::tool::ToolOutput> {
    let perm_category = tool.permission_category();
    if perm_category.is_empty() || perm_category == "none" {
        return tool.execute(tool_input, tool_ctx).await;
    }
    let resource = extract_resource_from_input(&tool_input, &tc.name);
    if tc.name == "bash" {
        let sub_commands = split_bash_command(&resource);
        use ragent_tools_core::bash::is_safe_command;
        let all_safe = sub_commands.iter().all(|cmd| {
            let cmd_name = extract_command_name(cmd);
            is_safe_command(&cmd_name)
        });
        if all_safe {
            return tool.execute(tool_input, tool_ctx).await;
        }
        let mut all_approved = true;
        for sub_cmd in &sub_commands {
            let cmd_name = extract_command_name(sub_cmd);
            let permission_action = check_permission_with_prompt(
                permission_checker,
                event_bus,
                session_id_for_perm,
                perm_category,
                &cmd_name,
                &tc.name,
                auto_approve,
                Some(&tool_ctx.canonical_cache),
                checkpoint_forced,
                checkpoint_timeout_secs,
            )
            .await;
            match permission_action {
                Ok(crate::permission::PermissionAction::Allow) => continue,
                Ok(
                    crate::permission::PermissionAction::Deny
                    | crate::permission::PermissionAction::Ask,
                )
                | Err(_) => {
                    all_approved = false;
                    break;
                }
            }
        }
        return if all_approved {
            tool.execute(tool_input, tool_ctx).await
        } else {
            Err(anyhow::anyhow!(
                "Permission denied for one or more sub-commands"
            ))
        };
    }
    let permission_action = check_permission_with_prompt(
        permission_checker,
        event_bus,
        session_id_for_perm,
        perm_category,
        &resource,
        &tc.name,
        auto_approve,
        Some(&tool_ctx.canonical_cache),
        checkpoint_forced,
        checkpoint_timeout_secs,
    )
    .await;
    match permission_action {
        Ok(crate::permission::PermissionAction::Allow) => tool.execute(tool_input, tool_ctx).await,
        Ok(crate::permission::PermissionAction::Deny) => {
            // T-010 (FR-015): when the denial comes from a forced
            // destructive-action checkpoint (timeout or refusal), surface
            // the checkpoint explanation so the model understands the
            // safe-default intervention.
            if checkpoint_forced {
                Err(anyhow::anyhow!(
                    crate::session::permissions::checkpoint_reason(&tc.name, &resource)
                ))
            } else {
                Err(anyhow::anyhow!("Permission denied by user or policy"))
            }
        }
        Ok(crate::permission::PermissionAction::Ask) => Err(anyhow::anyhow!(
            "Permission check returned Ask (internal error)"
        )),
        Err(e) => Err(e),
    }
}

/// Drives the agentic conversation loop for a single session.
///
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
    /// Optional activity-log store for agent execution events.
    ///
    /// Set once after construction via [`SessionProcessor::set_activity_log`],
    /// matching the `OnceLock` pattern used by `code_index`, `bg_service`,
    /// etc. Recording is gated at call sites by
    /// [`ragent_config::activity_log::is_enabled`] so that `/alog off`
    /// suppresses all writes without unwiring the handle.
    pub activity_log: std::sync::OnceLock<Arc<ragent_storage::ActivityLog>>,
    /// C-001: cached skill registry keyed by the mtimes of the scanned
    /// skill directories plus the `extra_dirs` list. `SkillRegistry::load`
    /// does synchronous `std::fs` walks + `serde_yaml` parses across up to
    /// 7 directories on every turn; caching it here (invalidated when any
    /// contributing directory's mtime changes) eliminates that per-turn
    /// disk I/O.
    pub skill_registry_cache: parking_lot::Mutex<Option<CachedSkillRegistry>>,
    /// Active goal-driven loop runs (spec `agentloop`), keyed by session id.
    ///
    /// Set via [`SessionProcessor::start_loop`] when a loop is launched (TUI
    /// `/loop` or HTTP `POST /loop`), and removed on termination. Plain chat
    /// turns have no entry and behave exactly as before; loop runs enforce
    /// the loop stop conditions (FR-010, FR-013, FR-014) in the agent loop.
    pub active_loops: tokio::sync::RwLock<HashMap<String, crate::session::loop_state::LoopTracker>>,
    /// The [`LoopSpec`] of each active goal-driven loop run, keyed by session
    /// id (spec `agentloop` T-009).
    ///
    /// Maintained alongside [`SessionProcessor::active_loops`]: inserted by
    /// [`SessionProcessor::start_loop`], removed on termination and by
    /// [`SessionProcessor::clear_loop`]. The permission layer consults the
    /// spec before every tool execution to enforce the loop's tool-set
    /// restriction (FR-008/FR-009), read-only constraints (FR-021), and
    /// scope boundaries (FR-022) — denials return observations to the model.
    /// T-003: the stored spec carries the resolved budgets (spec value or
    /// the `loop` config default) so consumers see effective limits.
    pub active_loop_specs:
        tokio::sync::RwLock<HashMap<String, std::sync::Arc<crate::session::loop_state::LoopSpec>>>,
    /// FR-025: set to `true` when the loop telemetry record
    /// ([`SessionRecorder::record_agent_loop`] plus the per-run tool-call
    /// total) has been published for the current loop run, so it is recorded
    /// exactly once per run. Cleared by [`SessionProcessor::start_loop`]
    /// when a new run starts.
    pub loop_telemetry_recorded: std::sync::atomic::AtomicBool,
    /// FR-016 (T-011): the human-interrupt flag for each active goal-driven
    /// loop run, keyed by session id. Raised by
    /// [`SessionProcessor::request_loop_interrupt`] (the TUI `Esc` path) and
    /// consulted at every inter-stage safe point in the agent loop; a set
    /// flag stops the run with [`StopCondition::HumanIntervention`]
    /// (termination status `interrupted`). Created by
    /// [`SessionProcessor::start_loop`], removed on termination and by
    /// [`SessionProcessor::clear_loop`].
    pub active_loop_interrupts: parking_lot::RwLock<HashMap<String, Arc<AtomicBool>>>,
    /// FR-018 (T-012): the pre-loop workspace capture of each goal-driven
    /// loop run, keyed by session id. Armed lazily by
    /// [`SessionProcessor::ensure_pre_loop_capture`] immediately before the
    /// loop's first write action, consulted on termination to compute the
    /// change summary ([`Event::LoopChangeSummary`], FR-019), and consumed
    /// by the rollback flow ([`SessionProcessor::rollback_loop`], FR-020).
    /// Survives termination until rollback or an explicit
    /// [`SessionProcessor::clear_loop`] so the post-loop rollback offer can
    /// still restore it.
    pub active_loop_captures:
        tokio::sync::RwLock<HashMap<String, crate::session::loop_capture::LoopCapture>>,
}

/// C-001: a cached [`crate::skill::SkillRegistry`] plus the inputs used to
/// build it. `working_dir`, `extra_dirs`, and the mtimes of every skill
/// directory are recorded so the cache reloads only when a skill directory
/// changes on disk (a new `SKILL.md` or subdirectory updates its parent's
/// mtime).
#[derive(Clone)]
pub struct CachedSkillRegistry {
    /// The loaded registry.
    pub registry: crate::skill::SkillRegistry,
    /// `(directory, mtime)` pairs for every scanned skill directory.
    pub dir_mtimes: Vec<(PathBuf, std::time::SystemTime)>,
    /// The `extra_dirs` config list used to build the registry.
    pub extra_dirs: Vec<String>,
    /// The working directory the registry was built for.
    pub working_dir: PathBuf,
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
    /// Set the activity-log store.
    ///
    /// Called once after construction (matching the `OnceLock` pattern for
    /// `code_index`, `bg_service`, etc.). Recording is gated at each call
    /// site by [`ragent_config::activity_log::is_enabled`], so the handle
    /// can be wired unconditionally and `/alog off` simply suppresses writes.
    pub fn set_activity_log(&self, log: Arc<ragent_storage::ActivityLog>) {
        let _ = self.activity_log.set(log);
    }

    /// Start a goal-driven loop run for `session_id` (spec `agentloop`).
    ///
    ///
    /// Registers a [`LoopTracker`] initialised from `spec` so the agent
    /// loop enforces the loop stop conditions (FR-010, FR-013, FR-014).
    /// T-003: budget fallbacks are resolved here — when the spec leaves a
    /// budget unset, the loop config defaults (`ragent-config` T-002:
    /// `loop.max_steps`, `loop.cost_limit`) apply, so the pre-request
    /// budget gates in the agent loop always have limits to enforce. An
    /// explicitly configured spec value wins over the config default.
    /// Starting a loop for a session that already has one replaces the
    /// previous tracker. The tracker is removed when the loop terminates
    /// (any stop condition) — plain chat turns have no entry and behave
    /// exactly as before.
    pub async fn start_loop(&self, session_id: &str, mut spec: LoopSpec) {
        // T-003 (FR-013, FR-014): a `None` budget in the spec means "the
        // config default applies" (T-002 semantics); resolve it here so
        // the tracker's budget_breach gates are always armed.
        let loop_defaults = self.load_config_cached().r#loop.clone();
        if spec.max_steps.is_none() {
            spec.max_steps = Some(loop_defaults.max_steps);
        }
        if spec.cost_limit.is_none() {
            spec.cost_limit = loop_defaults.cost_limit;
        }
        // Checkpoint-prompt timeout: a `None` spec value resolves to the
        // config default so `active_loop_specs` always holds the effective
        // value (the permission layer reads it from the spec).
        if spec.checkpoint_timeout_secs.is_none() {
            spec.checkpoint_timeout_secs = Some(loop_defaults.checkpoint_timeout_secs);
        }
        // T-017 (FR-025): a fresh run must publish its own telemetry record,
        // so clear the exactly-once flag here.
        self.loop_telemetry_recorded
            .store(false, std::sync::atomic::Ordering::Release);
        let tracker = crate::session::loop_state::LoopTracker::new(&spec);
        {
            let mut loops = self.active_loops.write().await;
            loops.insert(session_id.to_string(), tracker);
        }
        // T-009: record the spec so the permission layer can enforce the
        // loop's tool-set/scope/constraint restrictions on every tool call.
        let mut specs = self.active_loop_specs.write().await;
        specs.insert(session_id.to_string(), std::sync::Arc::new(spec));
        // T-011 (FR-016): arm a fresh human-interrupt flag for this run.
        self.active_loop_interrupts
            .write()
            .insert(session_id.to_string(), Arc::new(AtomicBool::new(false)));
        // T-012 (FR-018): record the workspace's git state when the session
        // directory sits inside a repository; when it does not, warn that
        // rollback will be snapshot-only so the user can confirm before the
        // loop writes anything. The capture itself is deferred to
        // `ensure_pre_loop_capture` (the first-write safe point).
        let working_dir = self
            .session_manager
            .get_session(session_id)
            .ok()
            .flatten()
            .map(|session| session.directory)
            .unwrap_or_default();
        let git = crate::session::loop_capture::git_state(&working_dir).await;
        match &git {
            Some(state) => tracing::info!(
                session_id = %session_id,
                git = %state.describe(),
                "loop workspace git state recorded (FR-018)"
            ),
            None => {
                tracing::warn!(
                    session_id = %session_id,
                    "loop workspace is not inside a git repository (FR-018); \
                     rollback will be snapshot-only"
                );
                self.event_bus.publish(Event::AgentNotice {
                    session_id: session_id.to_string(),
                    message: "loop: workspace is not inside a git repository \
                              — rollback will be snapshot-only. Confirm to \
                              continue."
                        .to_string(),
                });
            }
        }
        self.active_loop_captures.write().await.insert(
            session_id.to_string(),
            crate::session::loop_capture::LoopCapture {
                snapshot: None,
                git,
            },
        );
    }

    /// Whether a goal-driven loop is active for `session_id`.
    pub async fn loop_active(&self, session_id: &str) -> bool {
        self.active_loops.read().await.contains_key(session_id)
    }

    /// Drop every pending pre-loop capture (FR-020, T-013).
    ///
    /// Called when the user declines a rollback offer: there is no pending
    /// offer left to honour, so the captures are discarded and no later
    /// rollback can resurrect them. A no-op when no captures are pending.
    pub async fn clear_loop_captures(&self) {
        self.active_loop_captures.write().await.clear();
    }

    /// Remove the active loop tracker for `session_id`, if any.
    ///
    /// Called after termination so subsequent plain chat turns do not
    /// consult loop budgets (FR-017: no iteration after a stop condition).
    pub async fn clear_loop(&self, session_id: &str) {
        self.active_loops.write().await.remove(session_id);
        self.active_loop_specs.write().await.remove(session_id);
        self.active_loop_interrupts.write().remove(session_id);
        // T-012: an explicit clear discards the pre-loop capture as well —
        // there is no pending rollback offer to honour.
        self.active_loop_captures.write().await.remove(session_id);
    }

    /// Raise the human-interrupt flag for `session_id`'s active goal-driven
    /// loop (FR-016, T-011).
    ///
    /// Called when the user presses `Esc` while a loop is running: the agent
    /// loop aborts at the next inter-stage safe point and terminates with
    /// termination status `interrupted`, leaving the session persisted and
    /// resumable. A no-op returning `false` when no loop is active for the
    /// session (plain turns cancel through the cancel flag alone, exactly as
    /// before).
    ///
    /// Returns `true` when an active loop's interrupt flag was raised.
    pub fn request_loop_interrupt(&self, session_id: &str) -> bool {
        let flag = self.active_loop_interrupts.read().get(session_id).cloned();
        match flag {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// Arm the pre-loop workspace capture for `session_id`'s loop run
    /// (FR-018, T-012), unless it is already armed.
    ///
    /// Called at the first-write safe point — immediately before the loop's
    /// first write action executes — so the snapshot records the workspace
    /// exactly as the loop found it. A no-op when no loop run exists for the
    /// session or when the capture already holds a snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the storage-backed snapshot write fails; the
    /// caller surfaces the failure as a tool error observation.
    pub async fn ensure_pre_loop_capture(
        &self,
        session_id: &str,
        working_dir: &std::path::Path,
        message_id: &str,
    ) -> Result<(), anyhow::Error> {
        let Some(capture) = self
            .active_loop_captures
            .read()
            .await
            .get(session_id)
            .cloned()
        else {
            return Ok(()); // No loop run: plain chat turn, nothing to capture.
        };
        if capture.snapshot.is_some() {
            return Ok(()); // Already captured before an earlier write action.
        }
        let snapshot = crate::session::loop_capture::capture_pre_loop_snapshot(
            session_id,
            message_id,
            working_dir,
        )
        .await?;
        tracing::info!(
            session_id = %session_id,
            files = snapshot.as_ref().map_or(0, |s| s.files.len()),
            "pre-loop workspace snapshot captured (FR-018)"
        );
        // Only the snapshot slot is updated; the recorded git state stays.
        let mut updated = capture;
        updated.snapshot = snapshot;
        self.active_loop_captures
            .write()
            .await
            .insert(session_id.to_string(), updated);
        Ok(())
    }

    /// Restore the workspace from `session_id`'s pre-loop capture (FR-020,
    /// T-013) and drop the capture.
    ///
    /// Returns `true` when a capture existed and was restored; `false` when
    /// no capture is pending (nothing was written, or rollback already ran).
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot restore fails; the capture is
    /// kept so the caller can retry.
    pub async fn rollback_loop(&self, session_id: &str) -> Result<bool, anyhow::Error> {
        let Some(capture) = self.active_loop_captures.write().await.remove(session_id) else {
            return Ok(false);
        };
        crate::session::loop_capture::rollback_to_capture(capture).await?;
        tracing::info!(
            session_id = %session_id,
            "workspace rolled back to the pre-loop snapshot (FR-020)"
        );
        Ok(true)
    }

    /// Publish [`Event::LoopChangeSummary`] for a terminated loop run
    /// (FR-019), comparing the live workspace against the pre-loop capture.
    ///
    /// A no-op when the session has no capture (the capture is armed lazily
    /// before the first write action, so read-only loops never publish a
    /// summary).
    async fn publish_loop_change_summary(
        &self,
        session_id: &str,
        status: &str,
        iterations: u64,
        working_dir: &std::path::Path,
    ) {
        let Some(capture) = self
            .active_loop_captures
            .read()
            .await
            .get(session_id)
            .cloned()
        else {
            return;
        };
        let dir_owned = working_dir.to_path_buf();
        // A panicked or aborted change-summary computation must not be
        // published as an all-zero summary (misleading UI data); warn and
        // skip the event instead.
        let summary = match tokio::task::spawn_blocking(move || {
            crate::session::loop_capture::compute_change_summary(&capture, &dir_owned)
        })
        .await
        {
            Ok(summary) => summary,
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "loop change summary computation failed; summary not published",
                );
                return;
            }
        };
        tracing::info!(
            session_id = %session_id,
            status,
            modified = summary.modified,
            created = summary.created,
            deleted = summary.deleted,
            diffstat = %summary.diffstat(),
            "loop change summary published (FR-019)"
        );
        self.event_bus.publish(Event::LoopChangeSummary {
            session_id: session_id.to_string(),
            status: status.to_string(),
            iterations,
            files_modified: summary.modified,
            files_created: summary.created,
            files_deleted: summary.deleted,
            diffstat: summary.diffstat(),
            files: summary.files,
        });
    }

    /// Terminate the loop for `session_id` with `condition` (FR-010-FR-013):
    /// stop the tracker, publish [`Event::LoopTerminated`], and remove the
    /// tracker so no further stage can run (FR-017). The published `status`
    /// is the stable termination label (`completed`, `error`,
    /// `budget_exhausted`, `interrupted`).
    ///
    /// Returns the effective stop condition, or `None` when no loop is
    /// active for the session.
    ///
    /// Note: this external path carries no run-start instant, so the
    /// FR-025 telemetry record is not published here; in-run termination
    /// paths (inside [`SessionProcessor::process_user_message`]) record it.
    pub async fn terminate_loop(
        &self,
        session_id: &str,
        condition: crate::session::loop_state::StopCondition,
        verification: Option<String>,
        reason: Option<String>,
    ) -> Option<crate::session::loop_state::StopCondition> {
        self.terminate_loop_inner(
            session_id,
            condition,
            None,
            None,
            verification,
            reason,
            None,
        )
        .await
    }

    /// Shared termination path: take the tracker out of the active map,
    /// optionally tally the final exchange's tokens, apply the stop
    /// condition (first stop wins, FR-017), publish
    /// [`Event::LoopTerminated`] exactly once, and — T-017 (FR-025) —
    /// record the loop telemetry (`iterations` + `duration_ms`) exactly
    /// once per run when `run_start` is supplied.
    ///
    /// T-012 (FR-019): when the run captured a pre-loop workspace snapshot,
    /// a [`Event::LoopChangeSummary`] is published after the termination
    /// event with the modified/created/deleted counts and the diffstat. The
    /// capture survives termination so the rollback offer (T-013) can still
    /// restore it; `clear_loop` discards it.
    async fn terminate_loop_inner(
        &self,
        session_id: &str,
        condition: crate::session::loop_state::StopCondition,
        run_start: Option<Instant>,
        final_tokens: Option<(u64, u64)>,
        verification: Option<String>,
        reason: Option<String>,
        working_dir: Option<&std::path::Path>,
    ) -> Option<crate::session::loop_state::StopCondition> {
        let mut tracker = self.active_loops.write().await.remove(session_id)?;
        self.active_loop_specs.write().await.remove(session_id);
        // T-011 (FR-016): the run is over — drop its interrupt flag.
        self.active_loop_interrupts.write().remove(session_id);
        if let Some((input_tokens, output_tokens)) = final_tokens {
            tracker.record_tokens(input_tokens, output_tokens);
        }
        // First stop condition wins (FR-017); a tracker that already
        // stopped keeps its original condition.
        let effective = tracker.stop(condition);
        let iterations = tracker.steps();
        let tokens = tracker.tokens();
        let status = effective.as_str();
        self.event_bus.publish(Event::LoopTerminated {
            session_id: session_id.to_string(),
            status: status.to_string(),
            iterations: u64::from(iterations),
            verification,
            reason,
        });
        // T-017 (FR-025): record the loop telemetry exactly once per run —
        // the iteration count and total duration via the existing agent-loop
        // instruments, plus the per-iteration tool-call tally published as
        // the per-session tool-call total.
        if run_start.is_some()
            && !self
                .loop_telemetry_recorded
                .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            let elapsed_ms = run_start.map_or(0, |start| {
                u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
            });
            let duration_ms = elapsed_ms as f64;
            let recorder = SessionRecorder::from_subsystem(&self.telemetry);
            recorder.record_agent_loop(duration_ms, u64::from(iterations));
            recorder.record_tool_calls_per_session(tracker.tool_calls());
            tracing::info!(
                session_id = %session_id,
                status,
                iterations,
                duration_ms,
                tool_calls = tracker.tool_calls(),
                tokens,
                "loop telemetry recorded (FR-025)"
            );
        }
        tracing::info!(
            session_id = %session_id,
            status,
            iterations,
            tokens,
            "goal-driven loop terminated"
        );
        // T-012 (FR-019): a run that captured the workspace publishes the
        // change summary alongside the termination event. `working_dir` is
        // supplied only by in-run termination paths; the public external
        // path (`terminate_loop`) has no directory and skips the summary.
        if let Some(dir) = working_dir {
            self.publish_loop_change_summary(session_id, status, u64::from(iterations), dir)
                .await;
        }
        Some(effective)
    }

    /// Shared implementation behind the two loop-termination entry points:
    /// checks activity, logs, and forwards to [`Self::terminate_loop_inner`].
    async fn terminate_loop_with(
        &self,
        session_id: &str,
        condition: crate::session::loop_state::StopCondition,
        reason: String,
        log_line: String,
        run_start: Option<Instant>,
        working_dir: Option<&std::path::Path>,
    ) {
        let active = self.active_loops.read().await.contains_key(session_id);
        if !active {
            return;
        }
        tracing::info!(session_id = %session_id, "{log_line}");
        self.terminate_loop_inner(
            session_id,
            condition,
            run_start,
            None,
            None,
            Some(reason),
            working_dir,
        )
        .await;
    }

    /// Persist the provider-reported input-token figure into the session
    /// state cache so the next turn starts with the same usage value shown
    /// in the TUI. Shared by the compaction and post-LLM call sites.
    fn persist_last_reported_input_tokens(&self, session_id: &str, tokens: u64) {
        let session_state_lock = self
            .session_manager
            .as_ref()
            .session_state_cache(session_id);
        if let Ok(mut guard) = session_state_lock.lock() {
            guard.set_last_reported_input_tokens(tokens);
        } else {
            warn!(
                session_id = %session_id,
                "session state cache lock poisoned; input-token figure not persisted"
            );
        }
    }

    /// Write a mutated loop tracker back to `active_loops` so budget gates
    /// and interrupt handling observe the latest tallies. Shared by the
    /// post-LLM and stage-completion call sites.
    async fn persist_loop_tracker(
        &self,
        session_id: &str,
        tracker: &crate::session::loop_state::LoopTracker,
    ) {
        self.active_loops
            .write()
            .await
            .insert(session_id.to_string(), tracker.clone());
    }

    /// Terminate an active goal-driven loop after a run stage failed
    /// unrecoverably (FR-011: provider transport failure, permission
    /// hard-deny, tool panic, context overflow, watchdog abort).
    ///
    /// Publishes the loop-termination event with termination status `error`
    /// and the surfaced failure reason; the failed stage is not retried —
    /// the caller propagates the error so the turn ends. A no-op when no
    /// loop is active for `session_id` (plain chat turns are unaffected).
    async fn terminate_loop_on_fatal_error(
        &self,
        session_id: &str,
        err: &anyhow::Error,
        run_start: Option<Instant>,
        working_dir: Option<&std::path::Path>,
    ) {
        let reason = format!("unrecoverable error: {err:#}");
        tracing::error!(session_id = %session_id, reason, "fatal loop error detail");
        self.terminate_loop_with(
            session_id,
            crate::session::loop_state::StopCondition::UnrecoverableError,
            reason,
            "loop terminated by unrecoverable error (FR-011); no retry".to_string(),
            run_start,
            working_dir,
        )
        .await;
    }

    /// Terminate an active goal-driven loop because the user interrupted it
    /// (FR-016, T-011: `Esc` during a loop aborts at the next inter-stage
    /// safe point). Publishes [`Event::LoopTerminated`] with termination
    /// status `interrupted`; the caller persists the partial assistant
    /// message and ends the turn normally so the session stays persisted and
    /// resumable. A no-op when no loop is active for `session_id`.
    ///
    /// Token tallies are already persisted by the loop before the safe
    /// point, so no final-token adjustment is passed here.
    async fn terminate_loop_on_interrupt(
        &self,
        session_id: &str,
        run_start: Option<Instant>,
        working_dir: Option<&std::path::Path>,
    ) {
        self.terminate_loop_with(
            session_id,
            crate::session::loop_state::StopCondition::HumanIntervention,
            "interrupted by user (Esc)".to_string(),
            "loop interrupted by user (FR-016); stopping at the safe point".to_string(),
            run_start,
            working_dir,
        )
        .await;
    }

    /// Record an activity-log event if logging is enabled.
    ///
    /// This is a no-op when [`ragent_config::activity_log::is_enabled`]
    /// returns `false` or when no [`ActivityLog`] handle has been wired.
    /// Errors are logged at `warn` level and never propagated — activity
    /// logging is best-effort and must never break the agent loop.
    ///
    /// The SQLite write is off-loaded to a blocking thread via
    /// `tokio::task::spawn_blocking` to avoid stalling the async executor.
    /// The `ActivityLog` owns a `std::sync::Mutex<Connection>` and all its
    /// public methods are synchronous — calling them directly from an async
    /// context would block the tokio worker thread during every INSERT.
    async fn record_activity_event<F>(&self, f: F)
    where
        F: FnOnce(
                &ragent_storage::ActivityLog,
            ) -> std::result::Result<
                ragent_types::activity::ActivityEvent,
                ragent_storage::AppendError,
            > + Send
            + 'static,
    {
        if !ragent_config::activity_log::is_enabled() {
            return;
        }
        if let Some(log) = self.activity_log.get() {
            let log = log.clone();
            match tokio::task::spawn_blocking(move || f(&log)).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => tracing::warn!(error = %e, "activity_log: recording failed"),
                Err(e) => tracing::warn!(error = %e, "activity_log: spawn_blocking failed"),
            }
        }
    }

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
    pub(crate) fn get_cached_tool_definition_bytes(&self) -> Option<u64> {
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
        // Cache miss (or invalid): reload from disk. A broken ragent.json
        // falls back to defaults, but the failure is logged so silent config
        // degradation is diagnosable.
        let cfg = match ragent_config::Config::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!(error = %e, "config load failed; using default config");
                ragent_config::Config::default()
            }
        };
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

    /// C-001: load (and cache) the skill registry for a working directory,
    /// invalidating when any scanned skill directory's mtime changes.
    ///
    /// Returns `None` when no skill directories exist and no skills would be
    /// discovered, so callers can cheaply skip the system-prompt skill section.
    pub fn skill_registry(
        &self,
        working_dir: &std::path::Path,
        extra_dirs: &[String],
    ) -> Option<crate::skill::SkillRegistry> {
        // Collect the candidate skill directories exactly as `discover_skills`
        // does so the cache is invalidated precisely when the scan inputs change.
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(home) = dirs::home_dir() {
            for dir_name in [".agent", ".claude"] {
                dirs.push(home.join(dir_name).join("skills"));
            }
            dirs.push(home.join(".ragent").join("skills"));
        }
        for dir in extra_dirs {
            dirs.push(PathBuf::from(dir));
        }
        for dir_name in [".agent", ".claude"] {
            dirs.push(working_dir.join(dir_name).join("skills"));
        }
        dirs.push(working_dir.join(".ragent").join("skills"));
        // Monorepo: first-level subdirectories of `working_dir`.
        if let Ok(entries) = std::fs::read_dir(working_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path.join(".ragent").join("skills"));
                }
            }
        }

        // Fast path: cached registry still valid (checked before collecting
        // fresh mtimes so a cache hit does no directory stat sweep).
        {
            let guard = self.skill_registry_cache.lock();
            if let Some(cached) = guard.as_ref()
                && cached.working_dir == working_dir
                && cached.extra_dirs == extra_dirs
                && cached.dir_mtimes.iter().all(|(d, mt)| {
                    std::fs::metadata(d)
                        .and_then(|m| m.modified())
                        .ok()
                        .map_or(false, |current| current == *mt)
                })
            {
                return Some(cached.registry.clone());
            }
        }

        // Record mtimes of the directories that actually exist. Needed only
        // to populate a new cache entry, so it runs after the fast path.
        let dir_mtimes: Vec<(PathBuf, std::time::SystemTime)> = dirs
            .into_iter()
            .filter_map(|d| {
                std::fs::metadata(&d)
                    .and_then(|m| m.modified())
                    .ok()
                    .map(|mt| (d, mt))
            })
            .collect();

        // Cache miss or invalid: reload from disk.
        let registry = crate::skill::SkillRegistry::load(working_dir, extra_dirs);
        let mut guard = self.skill_registry_cache.lock();
        *guard = Some(CachedSkillRegistry {
            registry: registry.clone(),
            dir_mtimes,
            extra_dirs: extra_dirs.to_vec(),
            working_dir: working_dir.to_path_buf(),
        });
        Some(registry)
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

        // Activity log: generate a RunId for this turn and record the user
        // message. The run_id is threaded through the loop to link all
        // events (model messages, tool calls, tool results, termination)
        // for this turn.
        let run_id = ragent_types::id::RunId::new();
        let user_content = user_msg.text_content();
        let user_msg_id = user_msg.id.clone();
        let run_id_for_user = run_id.clone();
        self.record_activity_event(move |log| {
            log.record_model_message(&run_id_for_user, "user", &user_content, Some(user_msg_id))
        })
        .await;

        // 2. Prepare LLM client, config, working dir, team context
        // T-007 (FR-011): a stage failure before the loop body (missing
        // model/provider, unusable key, client construction failure) is an
        // unrecoverable provider-stage error — terminate an active loop with
        // status `error` and surface the reason; no retry of the stage.
        let turn = match self
            .prepare_client(session_id, &user_msg.id, agent, &profiler)
            .await
        {
            Ok(turn) => turn,
            Err(e) => {
                self.terminate_loop_on_fatal_error(session_id, &e, None, None)
                    .await;
                return Err(e);
            }
        };

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
        use crate::session::AbortOnDrop;
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
                    // Best-effort persist: a failure silently breaks the
                    // `--include-cost` export, so at least log it.
                    if let Err(e) = storage_for_persist.create_run_cost_summary(&row) {
                        tracing::debug!(error = %e, "run cost summary persist failed");
                    }
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
                session_id,
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
        let (chat_messages_vec, mut last_reported_input_tokens, context_window) = self
            .build_turn_chat_messages(
                session_id,
                agent,
                &turn.model_ref,
                &turn.session_config,
                &profiler,
            )
            .await?;
        // Compression hysteresis for this turn starts false; the LLM stage
        // sets it through `LoopState` when compaction actually runs.
        let mut compressed_this_turn = false;
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
        // T-009 (FR-008): when a goal-driven loop with a configured tool set
        // is active, the loop's tool surface is exactly that set plus the
        // mandatory safety tools — other tools stay visible to the rest of
        // the application but are not offered to this loop's requests.
        let loop_tool_set: Option<std::collections::HashSet<String>> = {
            let specs = self.active_loop_specs.read().await;
            specs
                .get(session_id)
                .filter(|spec| spec.has_tool_set())
                .map(|spec| {
                    let mut allowed: std::collections::HashSet<String> =
                        spec.tool_set.iter().cloned().collect();
                    for name in crate::session::loop_state::LOOP_ALWAYS_ALLOWED_TOOLS {
                        allowed.insert((*name).to_string());
                    }
                    for name in crate::tool::always_allowed_tool_names() {
                        allowed.insert(name.to_string());
                    }
                    allowed
                })
        };
        let tool_definitions: std::sync::Arc<Vec<ToolDefinition>> = if max_steps <= 1 {
            std::sync::Arc::new(Vec::new())
        } else if let Some(allowed) = loop_tool_set {
            let all = self.get_cached_tool_definitions();
            std::sync::Arc::new(
                all.iter()
                    .filter(|def| allowed.contains(def.name.as_str()))
                    .cloned()
                    .collect(),
            )
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
        // M-008: count of non-tool-call assistant parts at the last interim
        // save, so a step that only appended tool-call parts skips the interim
        // rewrite (the sole save gate — see the interim-save block below).
        let mut last_interim_significant_count: Option<usize> = None;
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
        // T-008: take a working copy of the loop tracker (when a goal-driven
        // loop is active) so the FR-017 guard below can consult and update it
        // without holding the `active_loops` lock across an await point.
        // Plain chat turns have no tracker entry and skip the guard entirely.
        let mut loop_tracker: Option<crate::session::loop_state::LoopTracker> =
            self.active_loops.read().await.get(session_id).cloned();
        // T-011 (FR-016): the human-interrupt flag for this session's loop,
        // raised by the TUI `Esc` path
        // ([`SessionProcessor::request_loop_interrupt`]). Plain chat turns
        // have no flag and keep cancelling through `cancel_flag` alone.
        let loop_interrupt_flag = self.active_loop_interrupts.read().get(session_id).cloned();
        let interrupt_requested = |cancel_flag: &AtomicBool| -> bool {
            cancel_flag.load(Ordering::Relaxed)
                || loop_interrupt_flag
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Relaxed))
        };
        // T-011 (FR-016): set when a safe-point interrupt check fired — the
        // post-loop handler persists the partial turn and ends it normally.
        let mut loop_interrupted = false;
        loop {
            // T-008 (FR-010, FR-013, FR-014, FR-017): the stop-flag guard.
            // No stage of this iteration may run once the loop's stop flag is
            // set; a budget breach stops the run BEFORE the LLM request, and
            // the per-iteration step counter is persisted back so the gate is
            // visible to the next iteration and to external terminations.
            if let Some(tracker) = loop_tracker.as_ref() {
                if tracker.is_stopped() {
                    debug!("Loop already stopped; no further stage will run");
                    break;
                }
            }
            // T-011 (FR-016): inter-stage safe point — an interrupt raised
            // since the last stage boundary aborts the loop before any
            // further stage runs, ahead even of the budget gate (first stop
            // wins; the user's intervention takes precedence).
            if loop_tracker.is_some() && interrupt_requested(&cancel_flag) {
                self.terminate_loop_on_interrupt(
                    session_id,
                    Some(total_start),
                    Some(&turn.working_dir),
                )
                .await;
                loop_interrupted = true;
                break;
            }
            if let Some(tracker) = loop_tracker.as_mut() {
                if !tracker.begin_step() {
                    // FR-013/FR-014: the step or token budget was reached —
                    // terminate with `budget_exhausted` before sending
                    // another LLM request.
                    let breach = tracker
                        .budget_breach()
                        .unwrap_or(crate::session::loop_state::StopCondition::BudgetExhausted);
                    let reason = if tracker
                        .cost_limit()
                        .is_some_and(|limit| tracker.tokens() >= limit)
                    {
                        format!(
                            "token cost budget exhausted ({} tokens accumulated)",
                            tracker.tokens()
                        )
                    } else {
                        format!(
                            "step budget exhausted ({}/{} iterations)",
                            tracker.steps(),
                            tracker.max_steps().unwrap_or(0)
                        )
                    };
                    self.terminate_loop_inner(
                        session_id,
                        breach,
                        Some(total_start),
                        None,
                        None,
                        Some(reason),
                        Some(&turn.working_dir),
                    )
                    .await;
                    break;
                }
                // Persist the incremented step counter so the next iteration
                // (and any external termination) sees it.
                self.persist_loop_tracker(session_id, &tracker).await;
            }
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
                // Activity log: record interruption on cancel.
                let run_id_for_cancel = run_id.clone();
                self.record_activity_event(move |log| {
                    log.record_termination(
                        &run_id_for_cancel,
                        ragent_types::activity::TerminationReason::Interrupted,
                    )
                })
                .await;
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
                            self.persist_last_reported_input_tokens(
                                session_id,
                                outcome.compressed_tokens as u64,
                            );
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
                cumulative_model_wait_ms,
                compressed_this_turn,
                compaction_attempted_this_turn,
                last_reported_input_tokens,
                last_finish_reason: last_finish_reason.clone(),
            };
            // T-007 (FR-011): an exhausted LLM-stage failure (transport error
            // after the internal retry budget, permanent API error, context
            // overflow that no compaction path could recover) is unrecoverable
            // — terminate an active loop with status `error`, surface the
            // reason, and stop without another request.
            let mut llm_result = match self
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
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    self.terminate_loop_on_fatal_error(
                        session_id,
                        &e,
                        Some(total_start),
                        Some(&turn.working_dir),
                    )
                    .await;
                    return Err(e);
                }
            };
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
                self.persist_last_reported_input_tokens(session_id, llm_result.last_input_tokens);
            }
            // T-008 (FR-014): tally this exchange's tokens into the loop
            // tracker so the next iteration's budget gate sees the updated
            // cost (the gate fires BEFORE the next request).
            if let Some(tracker) = loop_tracker.as_mut() {
                tracker.record_tokens(llm_result.last_input_tokens, llm_result.last_output_tokens);
                self.persist_loop_tracker(session_id, tracker).await;
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
                    // Activity log: record the assistant's text response.
                    let assistant_text = llm_result.text_buffer.clone();
                    let assistant_msg_id_for_log = assistant_msg_id.clone();
                    let run_id_for_msg = run_id.clone();
                    self.record_activity_event(move |log| {
                        log.record_model_message(
                            &run_id_for_msg,
                            "assistant",
                            &assistant_text,
                            Some(assistant_msg_id_for_log),
                        )
                    })
                    .await;
                }
            }

            // F9: text-format tool-call recovery. Some providers and models
            // cannot emit native tool-call fields and narrate the invocation
            // as text (markup dialects or bare tool-call JSON). When the step
            // produced NO native tool calls, attempt a conservative extraction
            // from the text buffer so the call is executed instead of left as
            // prose. Ordinary prose never matches (markup tags must be
            // literally present, or the whole response must be the JSON).
            // Recovered spans are blanked from the buffer so the model does
            // not see its own narration duplicated alongside the ToolUse part
            // on the next round-trip.
            if llm_result.tool_calls.is_empty() && !llm_result.text_buffer.is_empty() {
                let (recovered, spans) =
                    crate::session::text_toolcalls::extract_text_tool_calls_with_spans(
                        &llm_result.text_buffer,
                    );
                if !recovered.is_empty() {
                    tracing::info!(
                        session_id = %session_id,
                        count = recovered.len(),
                        "recovered tool call(s) from text-format output"
                    );
                    self.event_bus.publish(Event::AgentNotice {
                        session_id: session_id.to_string(),
                        message: format!(
                            "Recovered {} tool call(s) from text-format output.",
                            recovered.len()
                        ),
                    });
                    llm_result.tool_calls = recovered;
                    crate::session::text_toolcalls::blank_spans(
                        &mut llm_result.text_buffer,
                        &spans,
                    );
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
                // T-005 (FR-007, FR-010): the verification gate. A
                // no-tool-call response from a loop with a verification
                // command does not complete the run directly — the command's
                // exit status decides. Success terminates with `completed`;
                // failure with steps remaining appends the failure output as
                // an observation and continues; failure with the budget
                // breached ends the run as `budget_exhausted`.
                // Bind the spec outside the `if let` scrutinee so the
                // `active_loop_specs` read guard drops immediately (clippy
                // significant_drop_in_scrutinee).
                let loop_spec = self.active_loop_specs.read().await.get(session_id).cloned();
                if let Some(spec) = loop_spec {
                    if let Some(verify_cmd) = spec.verify_cmd.clone() {
                        let outcome = crate::session::verification::run_verification_command(
                            &verify_cmd,
                            &turn.working_dir,
                        )
                        .await
                        .unwrap_or_else(|e| {
                            crate::session::verification::VerificationOutcome::Failure {
                                reason: format!("gate error: {e}"),
                                output: String::new(),
                            }
                        });
                        let label = outcome.label();
                        let output = outcome.output().to_string();
                        tracing::info!(
                            session_id = %session_id,
                            passed = outcome.passed(),
                            "verification gate ran"
                        );
                        if outcome.passed() {
                            self.terminate_loop_inner(
                                session_id,
                                crate::session::loop_state::StopCondition::GoalAchieved,
                                Some(total_start),
                                Some((llm_result.last_input_tokens, llm_result.last_output_tokens)),
                                Some(label),
                                None,
                                Some(&turn.working_dir),
                            )
                            .await;
                            break;
                        }
                        // Failure: with no step remaining the loop cannot
                        // retry the verification, so the run is a budget
                        // exhaustion (FR-007); otherwise the failure output
                        // becomes the next observation and the loop continues.
                        let steps_remaining = loop_tracker
                            .as_ref()
                            .is_none_or(|tracker| tracker.budget_breach().is_none());
                        if !steps_remaining {
                            // A silent failure still needs an informative
                            // verification field, so fall back to the label.
                            let verification_summary = if output.is_empty() {
                                label.clone()
                            } else {
                                output.clone()
                            };
                            let reason = format!(
                                "verification command failed and no steps remain ({label})"
                            );
                            self.terminate_loop_inner(
                                session_id,
                                crate::session::loop_state::StopCondition::BudgetExhausted,
                                Some(total_start),
                                None,
                                Some(verification_summary),
                                Some(reason),
                                Some(&turn.working_dir),
                            )
                            .await;
                            break;
                        }
                        let observation =
                            crate::session::verification::verification_failure_observation(
                                &verify_cmd,
                                &outcome,
                            );
                        self.event_bus.publish(Event::AgentNotice {
                            session_id: session_id.to_string(),
                            message: format!(
                                "Verification gate failed - {label}. Appending \
                                 the failure output as an observation and \
                                 continuing the loop."
                            ),
                        });
                        Arc::make_mut(&mut chat_messages).push(ChatMessage {
                            role: "user".to_string(),
                            content: ChatContent::Text(observation),
                        });
                        // The verification observation is a successful
                        // observation append: reset the consecutive-failure
                        // counter so unrelated recoverable failures do not
                        // accumulate against the retry allowance (FR-012).
                        if let Some(tracker) = loop_tracker.as_mut() {
                            tracker.record_success();
                            self.persist_loop_tracker(session_id, &tracker).await;
                        }
                        continue;
                    }
                }
                // Goal-driven loop stop condition 1 (FR-010): a no-tool-call
                // response with no verification command means the goal was
                // reached — terminate with `GoalAchieved`, publish
                // `Event::LoopTerminated`, and stop.
                self.terminate_loop_inner(
                    session_id,
                    crate::session::loop_state::StopCondition::GoalAchieved,
                    Some(total_start),
                    Some((llm_result.last_input_tokens, llm_result.last_output_tokens)),
                    None,
                    None,
                    Some(&turn.working_dir),
                )
                .await;
                break;
            }

            // T-011 (FR-016): safe point between the LLM response and the
            // tool phase — an interrupt raised while the model was responding
            // aborts the run before any tool executes.
            if loop_tracker.is_some() && interrupt_requested(&cancel_flag) {
                self.terminate_loop_on_interrupt(
                    session_id,
                    Some(total_start),
                    Some(&turn.working_dir),
                )
                .await;
                loop_interrupted = true;
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
                let allowed_roots = turn
                    .session_config
                    .dirs
                    .allowed_roots
                    .iter()
                    .map(std::path::PathBuf::from)
                    .collect();
                let base_tool_ctx = ToolContext {
                    session_id: session_id.to_string(),
                    working_dir: turn.working_dir.clone(),
                    event_bus: self.event_bus.clone(),
                    storage: Some(self.session_manager.storage().clone()),
                    agent_manager: self.agent_manager.get().cloned(),
                    active_model: Some(turn.model_ref.clone()),
                    provider_registry: Some(Arc::clone(&self.provider_registry)),
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
                    allowed_roots,
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
                // Builds the synthetic error result shared by the B1
                // args-parse gate and the sequential/parallel join-failure
                // paths (C1). One construction site keeps the 8-field tuple
                // consistent as fields evolve.
                let synthetic_error_result =
                    |tc: &PendingToolCall, msg: &str| -> ToolExecutionResult {
                        Ok((
                            tc.clone(),
                            Value::Null,
                            ToolCallStatus::Error,
                            None,
                            Some(msg.to_string()),
                            0u64,
                            format!("Error: {msg}"),
                            None,
                        ))
                    };
                let result_profiler = profiler.clone();
                // P-15: collect one `ToolCallBatchEntry` per tool call so a
                // single `Event::ToolCallBatch` can be published at the end of
                // the step, giving consumers an atomic view of all tool calls.
                // The per-call events are still published inside the spawned
                // task as a fallback (PERFPLAN Milestone D risk note).
                let mut batch_entries: Vec<ragent_types::event::ToolCallBatchEntry> = Vec::new();
                // T-007 (FR-011): set when a tool task panicked or failed to
                // join — an unrecoverable failure that terminates an active
                // loop after the tool phase. Panics discovered OUTSIDE the
                // result handler are staged here first: the closure captures
                // `tool_panic` mutably, so direct writes elsewhere are
                // rejected by the borrow checker.
                let mut tool_panic: Option<String> = None;
                let mut staged_tool_panic: Option<String> = None;
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
                            // M-007: the batch event carries a truncated display
                            // preview too (the full content is preserved in
                            // `assistant_parts` and the activity log).
                            const BATCH_PREVIEW_CHARS: usize = 2000;
                            let batch_content =
                                truncate_preview(&result_content, BATCH_PREVIEW_CHARS);
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
                            tool_panic = Some(e.to_string());
                            false
                        }
                    }
                };

                for tc in &llm_result.tool_calls {
                    let _scope = profiler.scope("loop.tool_phase.prepare_call");
                    // C4: an interrupt raised mid-phase stops further
                    // dispatch. Not-yet-started calls have no `ToolUse`
                    // record in the assistant parts, so breaking here keeps
                    // the conversation history consistent (no orphaned
                    // tool_use); in-flight tasks complete under the
                    // watchdog as before.
                    if interrupt_requested(&cancel_flag) {
                        tracing::info!(
                            session_id = %session_id,
                            "interrupt during tool phase; skipping remaining tool calls"
                        );
                        break;
                    }
                    // B1: parse the arguments ONCE per call. A malformed args
                    // JSON used to be silently coerced to `{}` (six parse
                    // sites), so the tool executed with empty arguments and
                    // the model only saw a misleading "missing parameter X"
                    // error. The parse result is now computed once and reused
                    // through every guard/hook stage; a parse failure
                    // short-circuits execution with a corrective,
                    // LLM-visible error so the model can resend valid JSON.
                    let parsed_input: Result<Value, String> =
                        serde_json::from_str(&tc.args_json).map_err(|e| e.to_string());
                    let input: Value = match &parsed_input {
                        Ok(value) => value.clone(),
                        Err(e) => {
                            warn!(
                                error = %e,
                                args = %tc.args_json,
                                "Failed to parse tool call arguments"
                            );
                            // B3: even a parse failure is validated against
                            // the schema so a non-object payload gets the
                            // same corrective treatment.
                            let schema_error = schema_violation(
                                &self.tool_registry,
                                &tc.name,
                                &Value::Null,
                            );
                            let err_msg = format!(
                                "Invalid arguments JSON for tool '{}': {e}. \
                                 Arguments must be a single valid JSON object; \
                                 resend the complete tool call with corrected \
                                 JSON.{}",
                                tc.name,
                                schema_error
                                    .err()
                                    .map(|reason| format!(" ({reason})"))
                                    .unwrap_or_default()
                            );
                            let synthetic = Ok((
                                tc.clone(),
                                Value::Null,
                                ToolCallStatus::Error,
                                None,
                                Some(err_msg.clone()),
                                0u64,
                                format!("Error: {err_msg}"),
                                None,
                            ));
                            if handle_tool_execution_result(synthetic) {
                                break;
                            }
                            continue;
                        }
                    };
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
                    // Activity log: record the tool call.
                    let tc_id = tc.id.clone();
                    let tc_name = tc.name.clone();
                    let tc_args = tc.args_json.clone();
                    let run_id_for_tc = run_id.clone();
                    self.record_activity_event(move |log| {
                        log.record_tool_call(&run_id_for_tc, tc_id, tc_name, tc_args)
                    })
                    .await;
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
                    let session_id_str = session_id.to_string();
                    let session_id_for_perm = session_id.to_string();
                    let hook_working_dir = turn.working_dir.clone();
                    let hook_configs = turn.parsed_hook_configs.clone();
                    let extraction_engine = self.extraction_engine.clone();
                    let storage_clone = self.session_manager.storage().clone();
                    let profiler_clone = profiler.clone();
                    let team_context_cache = self.team_context_cache.clone();
                    let telemetry_clone = Arc::clone(&self.telemetry);
                    // T-009 (FR-008/FR-009/FR-021/FR-022): the loop
                    // restriction guard runs inside the task, before hooks
                    // and the permission checker, so a restricted loop
                    // cannot execute a call outside its tool set or scope.
                    let loop_spec = self.active_loop_specs.read().await.get(session_id).cloned();
                    // T-010 (FR-015): when the loop has destructive-action
                    // checkpoints enabled, `auto_approve` becomes `None` so
                    // `check_permission_with_prompt` forces the checkpoint
                    // prompt for destructive calls even under allow rules and
                    // in auto-approve mode (FR-024). Plain turns keep
                    // `Some(self.auto_approve)`.
                    let loop_checkpoint = loop_spec.as_ref().is_some_and(|spec| spec.checkpoints);
                    let auto_approve = if loop_checkpoint {
                        None
                    } else {
                        Some(self.auto_approve)
                    };
                    let checkpoint_timeout_secs = loop_spec
                        .as_ref()
                        .and_then(|spec| spec.checkpoint_timeout_secs)
                        .map_or_else(
                            || u64::from(self.load_config_cached().r#loop.checkpoint_timeout_secs),
                            u64::from,
                        );
                    // T-012 (FR-018): capture the workspace BEFORE the first
                    // write action executes. The check is cheap when already
                    // captured (a map read); when armed, the storage-backed
                    // snapshot runs synchronously here so no write can slip
                    // through before the capture. Failures surface as a tool
                    // error observation instead of executing the call.
                    if loop_tracker.is_some()
                        && crate::session::loop_state::LOOP_WRITE_TOOLS.contains(&tc.name.as_str())
                    {
                        if let Err(e) = self
                            .ensure_pre_loop_capture(session_id, &turn.working_dir, &tc.id)
                            .await
                        {
                            let err_msg = format!("pre-loop workspace snapshot failed: {e:#}");
                            tracing::error!(
                                session_id = %session_id,
                                reason = %err_msg,
                                "loop terminated: workspace capture failed (FR-018)"
                            );
                            self.terminate_loop_inner(
                                session_id,
                                crate::session::loop_state::StopCondition::UnrecoverableError,
                                Some(total_start),
                                None,
                                None,
                                Some(format!("unrecoverable error: {err_msg}")),
                                Some(&turn.working_dir),
                            )
                            .await;
                            return Err(anyhow::anyhow!(err_msg));
                        }
                    }
                    // B2: the loop restriction must run even when the args
                    // JSON failed to parse — fail closed. Unparseable args
                    // used to be coerced to `{}`, letting path-based scope
                    // checks pass vacuously.
                    let loop_restriction_input = match &parsed_input {
                        Ok(value) => value.clone(),
                        Err(_) => Value::Null,
                    };
                    let fut = tokio::spawn(async move {
                        let _tool_total_scope =
                            profiler_clone.scope_with(|| format!("tool.total:{}", tc_clone.name));
                        event_bus.publish(Event::ToolCallStart {
                            session_id: session_id_str.clone(),
                            call_id: tc_clone.id.clone(),
                            tool: tc_clone.name.clone(),
                        });
                        event_bus.increment_tool_calls(&session_id_str);
                        // T-010 (FR-015): the destructive-action checkpoint
                        // applies per call — only when the loop has
                        // checkpoints enabled AND this invocation is marked
                        // destructive (deletion, config write, dependency
                        // installation, destructive git). Plain write tools
                        // keep the normal permission path.
                        let checkpoint_forced = loop_checkpoint
                            && crate::session::permissions::is_destructive_tool(
                                &tc_clone.name,
                                &loop_restriction_input,
                            );
                        // T-009: per-loop tool-set / scope / read-only
                        // restriction (FR-008, FR-009, FR-021, FR-022).
                        // A violation returns a denial observation to the
                        // model instead of executing the tool. This check is
                        // unconditional: it applies in auto-approve / YOLO
                        // mode too (FR-024 — the permission layer is always
                        // in the path).
                        if let Some(spec) = &loop_spec {
                            // B2: unparseable args fail closed — a denied call
                            // is returned to the model instead of silently
                            // evaluating the restriction against `{}`.
                            let early_input = loop_restriction_input.clone();
                            if let Some(reason) = spec.deny_reason(&tc_clone.name, &early_input) {
                                tracing::info!(
                                    tool = %tc_clone.name,
                                    reason = %reason,
                                    "loop restriction denied tool execution"
                                );
                                event_bus.publish(Event::ToolCallEnd {
                                    session_id: session_id_str.clone(),
                                    call_id: tc_clone.id.clone(),
                                    tool: tc_clone.name.clone(),
                                    error: Some(reason.clone()),
                                    duration_ms: 0,
                                });
                                return (
                                    tc_clone.clone(),
                                    early_input,
                                    ToolCallStatus::Error,
                                    None,
                                    Some(reason),
                                    0u64,
                                    String::new(),
                                    None,
                                );
                            }
                        }
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
                        let tool_input: Value = match pre_hook_result {
                            crate::hooks::PreToolUseResult::Allow => {
                                // Unparseable args never reach here: the B1
                                // gate short-circuits before the task runs.
                                parsed_input.unwrap_or(Value::Null)
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
                                let input_val = parsed_input.unwrap_or(Value::Null);
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
                                let input_val = parsed_input.unwrap_or(Value::Null);
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
                                parsed_input.unwrap_or(Value::Null)
                            }
                        };
                        let _permit = match crate::resource::acquire_tool_permit().await {
                            Ok(permit) => permit,
                            Err(e) => {
                                let err_msg = format!("tool permit acquisition failed: {e}");
                                // C3: close out the UI tool call — no
                                // `ToolCallEnd` is published on this early
                                // return, which used to leave the TUI spinner
                                // stuck on the in-flight call.
                                event_bus.publish(Event::ToolCallEnd {
                                    session_id: session_id_str.clone(),
                                    call_id: tc_clone.id.clone(),
                                    tool: tc_clone.name.clone(),
                                    error: Some(err_msg.clone()),
                                    duration_ms: 0,
                                });
                                return (
                                    tc_clone.clone(),
                                    tool_input,
                                    ToolCallStatus::Error,
                                    None,
                                    Some(err_msg),
                                    0u64,
                                    String::new(),
                                    None,
                                );
                            }
                        };
                        let start = Instant::now();
                        let tool_input_for_post_hook = serde_json::to_string(&tool_input)
                            .unwrap_or_else(|_| tc_clone.args_json.clone());
                        let result = registry
                            .get(&tc_clone.name)
                            .ok_or_else(|| unknown_tool_error(&registry, &tc_clone.name));
                        let result = match result {
                            Ok(tool) => {
                                // B3: central required-argument validation.
                                // The tool's declared `parameters_schema()`
                                // has always been shown to the model but never
                                // enforced; required-field presence and
                                // primitive types are now checked before
                                // permissions so the model gets a corrective
                                // error naming the offending parameter.
                                if let Err(schema_err) = schema_violation(
                                    &registry,
                                    &tc_clone.name,
                                    &tool_input,
                                ) {
                                    Err(anyhow::anyhow!("Invalid tool arguments: {schema_err}"))
                                } else {
                                    dispatch_tool_with_permissions(
                                        &tool,
                                        tool_input,
                                        &tc_clone,
                                        &tool_ctx,
                                        &permission_checker,
                                        &event_bus,
                                        &session_id_for_perm,
                                        auto_approve,
                                        checkpoint_forced,
                                        checkpoint_timeout_secs,
                                    )
                                    .await
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
                        // M-007: publish only a short display preview on the
                        // event (the TUI logs ~2500 chars and the LLM gets the
                        // full content via `tool_result_content_for_llm`), so a
                        // large result is not cloned in full into the event and
                        // the activity-log records.
                        const TOOL_RESULT_EVENT_PREVIEW_CHARS: usize = 2000;
                        let result_preview =
                            truncate_preview(&result_content, TOOL_RESULT_EVENT_PREVIEW_CHARS);
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
                                &event_bus,
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
                        // Capture the real call identity for the watchdog and
                        // panic paths BEFORE `tc` is borrowed/moved. C2 used
                        // to publish a placeholder `watchdog-parallel` call id
                        // and C1 dropped sibling results on the first failure.
                        let parallel_call_id = tc.id.clone();
                        let parallel_tool_name = tc.name.clone();
                        let parallel_args_json = tc.args_json.clone();
                        let watchdog_identity: WatchdogIdentity =
                            (tc.id.clone(), tc.name.clone());
                        let abort_handle = fut.abort_handle();
                        futures.push(async move {
                            match tokio::time::timeout(TOOL_WATCHDOG_TIMEOUT, fut).await {
                                Ok(Ok(ok)) => (Ok(ok), None, None),
                                Ok(Err(join_err)) => {
                                    // C1: the tool task panicked or failed to
                                    // join. Synthesise an error result so the
                                    // tool_use record is not orphaned in the
                                    // conversation history, and surface the
                                    // panic so loop sessions still terminate.
                                    let msg = format!("Tool task failed: {join_err}");
                                    let synthetic_tc = crate::session::history::PendingToolCall {
                                        id: parallel_call_id,
                                        name: parallel_tool_name,
                                        args_json: parallel_args_json,
                                    };
                                    (
                                        synthetic_error_result(&synthetic_tc, &msg),
                                        None,
                                        Some(join_err.to_string()),
                                    )
                                }
                                Err(_) => {
                                    abort_handle.abort();
                                    (
                                        Err(ToolTaskError::WatchdogAbort),
                                        Some(watchdog_identity),
                                        None,
                                    )
                                }
                            }
                        });
                    } else {
                        let watchdog_identity: WatchdogIdentity = (tc.id.clone(), tc.name.clone());
                        let abort_handle = fut.abort_handle();
                        let result = match tokio::time::timeout(TOOL_WATCHDOG_TIMEOUT, fut).await {
                            Ok(result) => result.map_err(ToolTaskError::Join),
                            Err(_) => {
                                abort_handle.abort();
                                watchdog_timed_out = true;
                                let msg = watchdog_timeout_msg(
                                    &watchdog_identity.1,
                                    &watchdog_identity.0,
                                );
                                warn!("{}", msg);
                                // Close out the tool call in the UI: no `ToolCallEnd`
                                // was published because the spawned task was aborted.
                                self.event_bus.publish(Event::ToolCallEnd {
                                    session_id: session_id.to_string(),
                                    call_id: watchdog_identity.0,
                                    tool: watchdog_identity.1,
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
                        match result {
                            Ok(ok) => {
                                if handle_tool_execution_result(Ok(ok)) {
                                    break;
                                }
                            }
                            Err(join_err) => {
                                // C1: a panicked/failed tool task must not
                                // orphan its tool_use record. Synthesise an
                                // error result (rendered through the normal
                                // handler) and stage the panic so active
                                // loops still terminate (T-007).
                                let msg = format!("Tool task failed: {join_err}");
                                warn!(error = %join_err, "Tool execution task panicked");
                                staged_tool_panic = Some(join_err.to_string());
                                self.event_bus.publish(Event::ToolCallEnd {
                                    session_id: session_id.to_string(),
                                    call_id: tc.id.clone(),
                                    tool: tc.name.clone(),
                                    error: Some(msg.clone()),
                                    duration_ms: 0,
                                });
                                let synthetic = Ok((
                                    tc.clone(),
                                    Value::Null,
                                    ToolCallStatus::Error,
                                    None,
                                    Some(msg.clone()),
                                    0u64,
                                    format!("Error: {msg}"),
                                    None,
                                ));
                                if handle_tool_execution_result(synthetic) {
                                    break;
                                }
                            }
                        }
                    }
                }
                if parallel_tool_calls {
                    let results = {
                        let _scope = profiler.scope("loop.tool_phase.join_parallel");
                        futures::future::join_all(futures).await
                    };
                    for (result, stalled_tool, panic_msg) in results {
                        if let Some((call_id, tool_name)) = stalled_tool {
                            watchdog_timed_out = true;
                            let msg = watchdog_timeout_msg(&tool_name, &call_id);
                            warn!("{}", msg);
                            // C2: close out the stalled call with its REAL
                            // call id (a placeholder id left the TUI part
                            // open) and keep draining sibling results so
                            // completed calls are not dropped.
                            self.event_bus.publish(Event::ToolCallEnd {
                                session_id: session_id.to_string(),
                                call_id,
                                tool: tool_name,
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
                            continue;
                        }
                        if let Some(panic) = panic_msg {
                            staged_tool_panic = Some(panic);
                        }
                        match result {
                            Ok(ok) => {
                                if handle_tool_execution_result(Ok(ok)) {
                                    break;
                                }
                            }
                            Err(e) => {
                                // Defense-in-depth: the wrapper converts every
                                // join `Err` into `Ok(synthetic)` + a panic
                                // message, and the only remaining `Err` here is
                                // `WatchdogAbort`, which is paired with
                                // `stalled_tool` and handled above. If this arm
                                // ever fires, log LOUDLY — it means the wrapper
                                // contract was broken.
                                warn!(error = %e, "unreachable: tool task returned Err without a stalled-tool identity; wrapper contract violated");
                                staged_tool_panic = Some(e.to_string());
                                // C2: keep draining sibling results.
                                continue;
                            }
                        }
                    }
                }
                // Merge any staged panic from the parallel drain paths.
                if tool_panic.is_none() {
                    tool_panic = staged_tool_panic.take();
                }
                // P-15: publish a single `ToolCallBatch` for this step with
                // all per-call summaries, so consumers can render atomically.
                if !batch_entries.is_empty() {
                    // Activity log: record each tool result from the batch.
                    if ragent_config::activity_log::is_enabled() {
                        if let Some(log) = self.activity_log.get() {
                            let log = log.clone();
                            let entries: Vec<_> = batch_entries
                                .iter()
                                .map(|e| {
                                    (
                                        e.call_id.clone(),
                                        e.tool.clone(),
                                        e.success,
                                        e.content.clone(),
                                    )
                                })
                                .collect();
                            let run_id_for_results = run_id.clone();
                            tokio::task::spawn_blocking(move || {
                                for (call_id, tool, success, content) in entries {
                                    if let Err(e) = log.record_tool_result(
                                        &run_id_for_results,
                                        call_id,
                                        tool,
                                        success,
                                        content,
                                    ) {
                                        tracing::warn!(
                                            error = %e,
                                            "activity_log: record_tool_result failed"
                                        );
                                    }
                                }
                            })
                            .await
                            .ok();
                        }
                    }
                    // T-017 (FR-025): tally this iteration's tool calls into
                    // the loop tracker so the run's tool-call total is
                    // published with the loop telemetry on termination. The
                    // tool-dispatch phase only runs when the response carried
                    // tool calls, so a no-tool-call step cannot double-count.
                    if let Some(tracker) = loop_tracker.as_mut() {
                        tracker.record_tool_calls(llm_result.tool_calls.len() as u64);
                        self.persist_loop_tracker(session_id, &tracker).await;
                    }
                    self.event_bus.publish(Event::ToolCallBatch {
                        session_id: session_id_arc.to_string(),
                        step: step as u64,
                        calls: batch_entries.clone(),
                    });
                }
                if agent_switch_requested || agent_complete_requested || watchdog_timed_out {
                    break;
                }
                // T-007 (FR-011, FR-012): classify this step's tool results.
                // A panic/join failure or a permission hard-deny terminates an
                // active loop immediately (FR-011, no retry); recoverable
                // failures were appended as observations above and only
                // terminate the loop once the consecutive count exceeds the
                // retry allowance (FR-012). Successful steps reset the
                // consecutive-failure counter.
                if let Some(tracker) = loop_tracker.as_mut() {
                    let step_errors: Vec<String> = batch_entries
                        .iter()
                        .filter(|entry| !entry.success)
                        .filter_map(|entry| entry.error.clone())
                        .collect();
                    let hard_failure = tool_panic.clone().or_else(|| {
                        step_errors
                            .iter()
                            .find(|msg| classify_message(msg).is_unrecoverable())
                            .cloned()
                    });
                    if let Some(failure) = hard_failure {
                        let reason = format!("unrecoverable tool failure: {failure}");
                        tracing::error!(
                            session_id = %session_id,
                            reason,
                            "loop terminated by unrecoverable tool failure (FR-011); no retry"
                        );
                        self.terminate_loop_inner(
                            session_id,
                            crate::session::loop_state::StopCondition::UnrecoverableError,
                            Some(total_start),
                            None,
                            None,
                            Some(reason),
                            Some(&turn.working_dir),
                        )
                        .await;
                        break;
                    }
                    if step_errors.is_empty() {
                        tracker.record_success();
                    } else if tracker.record_failure() {
                        let reason = format!(
                            "retry allowance exceeded ({} consecutive recoverable \
                             tool failures: {})",
                            tracker.consecutive_failures(),
                            step_errors.join("; ")
                        );
                        tracing::error!(
                            session_id = %session_id,
                            reason,
                            "loop terminated at the retry allowance (FR-012)"
                        );
                        self.terminate_loop_inner(
                            session_id,
                            crate::session::loop_state::StopCondition::UnrecoverableError,
                            Some(total_start),
                            None,
                            None,
                            Some(reason),
                            Some(&turn.working_dir),
                        )
                        .await;
                        break;
                    }
                    // Persist the updated failure counter so the next
                    // iteration (and any external termination) sees it.
                    self.persist_loop_tracker(session_id, &tracker).await;
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
                        // Only auto-complete tasks when a write tool actually
                        // touched a path inside this spec's own directory: an
                        // unrelated write elsewhere in the workspace (docs,
                        // scratch files, snapshots) must not silently complete
                        // spec tasks and corrupt PLAN.md.
                        && writes_in_spec_dir(&llm_result.tool_calls, &turn.working_dir, spec_mgr.root(), &id)
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
                // M-008: avoid rewriting the full SQLite row when the only
                // change is that tool-call parts were appended. Tool-call
                // parts are carried in the transcript (`chat_messages`) and
                // finalised on the final save, so an interim rewrite that
                // only adds them is wasted work.
                //
                // Invariant: non-tool-call parts are only pushed (stream
                // deltas) or popped (sub-agent narration nudge) — never
                // mutated in place — so an unchanged non-tool-call count
                // implies unchanged persisted content. That makes the count
                // alone a sufficient save gate; hashing the serialised parts
                // (the previous P-12 gate) re-serialised every tool-call
                // input/output on every step, which is exactly the cost this
                // interim save exists to avoid.
                let significant_count = assistant_parts
                    .iter()
                    .filter(|p| !matches!(p, MessagePart::ToolCall { .. }))
                    .count();
                if last_interim_significant_count != Some(significant_count) {
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
                    last_interim_significant_count = Some(significant_count);
                }
            }
        }

        // T-011 (FR-016): a human interrupt stops the loop with `interrupted`
        // (terminated at the safe point above); persist the partial assistant
        // message and end the turn normally so the session stays persisted
        // and resumable.
        if loop_interrupted {
            let total_elapsed_ms = total_start.elapsed().as_millis() as u64;
            let parts_owned =
                std::sync::Arc::try_unwrap(assistant_parts).unwrap_or_else(|arc| (*arc).clone());
            let mut assistant_msg = Message::new(session_id, Role::Assistant, parts_owned);
            assistant_msg.id = assistant_msg_id;
            let interrupted_id = assistant_msg.id.clone();
            self.storage_op(move |s| s.update_message(&assistant_msg))
                .await?;
            self.event_bus.publish(Event::MessageEnd {
                session_id: session_id.to_string(),
                message_id: interrupted_id,
                reason: FinishReason::Cancelled,
            });
            // Activity log: record the user interruption.
            let run_id_for_interrupt = run_id.clone();
            self.record_activity_event(move |log| {
                log.record_termination(
                    &run_id_for_interrupt,
                    ragent_types::activity::TerminationReason::Interrupted,
                )
            })
            .await;
            publish_run_cost_summary(total_elapsed_ms);
            return Ok(Message::new(session_id, Role::Assistant, vec![]));
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
            // Activity log: record interruption on watchdog timeout.
            let run_id_for_watchdog = run_id.clone();
            self.record_activity_event(move |log| {
                log.record_termination(
                    &run_id_for_watchdog,
                    ragent_types::activity::TerminationReason::Interrupted,
                )
            })
            .await;
            publish_run_cost_summary(total_elapsed_ms);
            // T-007 (FR-011): a watchdog abort is an unrecoverable tool-stage
            // failure — terminate an active loop with status `error`.
            let watchdog_err = anyhow::anyhow!(
                "agent run terminated: tool call stalled beyond the {}s watchdog timeout",
                TOOL_WATCHDOG_TIMEOUT.as_secs()
            );
            self.terminate_loop_on_fatal_error(
                session_id,
                &watchdog_err,
                Some(total_start),
                Some(&turn.working_dir),
            )
            .await;
            return Err(watchdog_err);
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
        // Activity log: record normal completion.
        let run_id_for_complete = run_id.clone();
        self.record_activity_event(move |log| {
            log.record_termination(
                &run_id_for_complete,
                ragent_types::activity::TerminationReason::Completed,
            )
        })
        .await;
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
            Ok(mut stream) => loop {
                let ev =
                    match tokio::time::timeout(std::time::Duration::from_secs(60), stream.next())
                        .await
                    {
                        Ok(Some(event)) => event,
                        Ok(None) => break,
                        Err(_) => {
                            tracing::warn!(
                                session_id = %session_id,
                                "AGENTS.md init exchange stream stalled — no data for 60s"
                            );
                            break;
                        }
                    };
                if cancel_flag.load(Ordering::Relaxed) {
                    break;
                }
                match ev {
                    StreamEvent::TextDelta { text } => {
                        ack_text.push_str(&text);
                    }
                    _ => {}
                }
            },
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
        // exchange), then fall back to env var → IDE discovery.
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
                 or set the GITHUB_COPILOT_TOKEN environment variable."
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
            "ollama_cloud" => vec!["OLLAMA_CLOUD_API_KEY", "OLLAMA_API_KEY"],
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

/// M-007: shared tool-result display preview — trim, truncate to `max_chars`
/// characters on a char boundary, and append `…` when truncated.
///
/// The `len() <= max` fast path exploits the fact that a string's byte length
/// is an upper bound on its character count, so short ASCII strings (the
/// common case) skip the full char scan entirely.
fn truncate_preview(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.len() <= max_chars {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max_chars).collect();
    out.push('…');
    out
}
