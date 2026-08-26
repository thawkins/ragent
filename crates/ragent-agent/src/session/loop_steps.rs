//! Extracted setup and loop steps for `process_user_message` (T6.5).
//!
//! These methods split the 2,273-line `process_user_message` into named
//! steps per the REMPLAN.md plan. Each step is `≤ ~400 lines`.
//! `process_user_message` in `processor.rs` becomes a thin orchestrator
//! that calls them in order.
//!
//! ## Step order
//!
//! 1. [`Self::prepare_client`] — resolve model / provider / API key / client.
//! 2. [`Self::build_turn_system_prompt`] — assemble the system prompt.
//! 3. [`Self::build_turn_chat_messages`] — load history, optionally compress,
//!    convert to `ChatMessage`s.
//! 4. [`Self::run_inline_init_acknowledgement`] — AGENTS.md init exchange.
//! 5. (loop) [`Self::call_llm_step`] — call the LLM with retry + stream
//!    event handling.
//! 6. (loop) [`Self::dispatch_tool_calls`] — execute tool calls and collect
//!    results.
//! 7. [`Self::finalize_assistant_message`] — final save + timing + hooks.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{Result, bail};
use futures::StreamExt;
use tracing::debug;
use uuid::Uuid;

use crate::agent::AgentInfo;
use crate::event::{Event, EventBus, FinishReason};
use crate::llm::{ChatContent, ChatMessage, ChatRequest, StreamEvent, ToolDefinition};
use crate::message::{Message, MessagePart, Role};
use crate::session::history::{
    PendingToolCall, chat_request_payload_bytes, history_to_chat_messages, history_version_of,
    is_permanent_llm_api_error, is_token_overflow_error_message, should_retry_stream_error,
    stream_has_meaningful_partial_output,
};
use crate::session::processor::SessionProcessor;
use crate::session::prompt_builders::{
    TOOL_CALLING_GUIDANCE, build_codeindex_guidance_section_active,
    build_codeindex_guidance_section_disabled, build_detailed_tool_reference_section,
    build_tool_reference_section,
};
use crate::session::stream_buffer::StreamBuffer;
use crate::tool::TeamContext;

// Re-imports for pub(crate) items used within the steps.

/// Immutable per-turn context created by [`SessionProcessor::prepare_client`].
pub(crate) struct TurnClient {
    /// The agent's model reference (provider id + model id).
    pub model_ref: crate::agent::ModelRef,
    /// The created LLM client.
    pub client: Arc<dyn crate::llm::LlmClient>,
    /// The per-turn config wrapped in `Arc` for cheap cloning into `ToolContext`.
    pub session_config: Arc<ragent_config::Config>,
    /// Parsed hook configs derived from `session_config.hooks`.
    pub parsed_hook_configs: Vec<crate::hooks::HookConfig>,
    /// The session's working directory.
    pub working_dir: PathBuf,
    /// Team context for this session, if it participates in a team.
    pub team_context: Option<Arc<TeamContext>>,
}

/// Mutable state shared across loop iterations.
pub(crate) struct LoopState {
    /// Provider-facing chat messages (mutated each step: assistant + tool results appended).
    ///
    /// P-6: held as `Arc<Vec<ChatMessage>>` so the per-retry `ChatRequest`
    /// can share the history by refcount bump instead of cloning the entire
    /// `Vec` (including all tool-result `ContentPart`s) on every attempt.
    /// Mutation is performed via `Arc::make_mut`, which clones only when
    /// another `Arc` reference is still live — so an unchanged history is
    /// shared for free.
    pub chat_messages: Arc<Vec<ChatMessage>>,
    /// Accumulated assistant message parts for the final save (COW via `Arc`).
    pub assistant_parts: Arc<Vec<MessagePart>>,
    /// Set when a tool returns `agent_switch` / `agent_restore` metadata.
    pub agent_switch_requested: bool,
    /// Set when a tool returns `agent_complete` metadata.
    pub agent_complete_requested: bool,
    /// Content hash of `assistant_parts` from the last interim save.
    pub last_interim_hash: Option<u64>,
    /// Cumulative milliseconds spent waiting for the LLM.
    pub cumulative_model_wait_ms: u64,
    /// Hysteresis flag: compression already ran this turn.
    pub compressed_this_turn: bool,
    /// Hysteresis flag: a compaction attempt (success or failure) already ran
    /// this turn. Prevents repeated user-visible "compaction skipped" notices
    /// and wasted re-serialisation when the runner bails out.
    pub compaction_attempted_this_turn: bool,
    /// Last LLM-reported input token count (0 if provider omits usage).
    pub last_reported_input_tokens: u64,
    /// Finish signal observed most recently this turn (the loop re-checks it
    /// on every step since a single step can contain several retry attempts).
    pub last_finish_reason: Option<FinishReason>,
}

/// Result of [`SessionProcessor::call_llm_step`].
pub(crate) struct LlmStepResult {
    pub text_buffer: String,
    pub reasoning_buffer: String,
    pub tool_calls: Vec<PendingToolCall>,
    pub last_input_tokens: u64,
    pub last_output_tokens: u64,
    pub llm_request_start: Instant,
    /// `true` if the step should break the main loop (fatal error).
    pub should_break: bool,
}

impl SessionProcessor {
    /// Resolve the model, provider, API key, base URL, and construct the LLM
    /// client for the turn. Also loads the config once (PERF-001), wraps it in
    /// `Arc` (PERF-009), and resolves the working directory + team context.
    ///
    /// On any failure this publishes an `AgentError` + `MessageEnd` event via
    /// `publish_error` and returns `Err`.
    pub(crate) async fn prepare_client(
        &self,
        session_id: &str,
        user_msg_id: &str,
        agent: &AgentInfo,
        profiler: &Arc<crate::session::profiler::AgentLoopProfiler>,
    ) -> Result<TurnClient> {
        let publish_error = |bus: &EventBus, sid: &str, msg_id: &str, err: &str| {
            bus.publish(Event::AgentError {
                session_id: sid.to_string(),
                error: err.to_string(),
            });
            bus.publish(Event::MessageEnd {
                session_id: sid.to_string(),
                message_id: msg_id.to_string(),
                reason: FinishReason::Stop,
            });
        };

        // PERF-001 / P-2: load the config once for the whole turn. The
        // `load_config_cached` helper caches the resolved config keyed by
        // the mtimes of every contributing file, so a session whose config
        // has not changed pays zero disk I/O on subsequent turns.
        let cfg = {
            let _scope = profiler.scope("config.load");
            self.load_config_cached()
        };

        let model_ref = {
            let _scope = profiler.scope("llm.resolve_model");
            if let Some(m) = agent.model.as_ref() {
                m.clone()
            } else {
                let err = format!("Agent '{}' has no model configured", agent.name);
                publish_error(&self.event_bus, session_id, user_msg_id, &err);
                bail!("{err}");
            }
        };

        let provider = {
            let _scope = profiler.scope("llm.resolve_provider");
            match self.provider_registry.get(&model_ref.provider_id) {
                Some(p) => p,
                None => {
                    let err = format!("Provider '{}' not found", model_ref.provider_id);
                    publish_error(&self.event_bus, session_id, user_msg_id, &err);
                    bail!("{err}");
                }
            }
        };

        let api_key = {
            let _scope = profiler.scope("llm.resolve_api_key");
            match self.resolve_api_key(&model_ref.provider_id).await {
                Ok(k) => k,
                Err(e) => {
                    let err = e.to_string();
                    publish_error(&self.event_bus, session_id, user_msg_id, &err);
                    return Err(e);
                }
            }
        };

        // For Copilot, pass the stored plan-specific API base URL
        let base_url = match model_ref.provider_id.as_str() {
            "copilot" => self
                .storage_op(|s| Ok(s.get_setting("copilot_api_base").ok().flatten()))
                .await
                .ok()
                .flatten(),
            "generic_openai" => {
                let cfg = Some(cfg.clone());
                self.storage_op(|s| Ok(s.get_setting("generic_openai_api_base").ok().flatten()))
                    .await
                    .ok()
                    .flatten()
                    .filter(|s: &String| !s.trim().is_empty())
                    .or_else(|| {
                        cfg.and_then(|c| c.provider.get("generic_openai").cloned())
                            .and_then(|p| p.api.and_then(|a| a.base_url))
                    })
                    .or_else(|| {
                        std::env::var("GENERIC_OPENAI_API_BASE")
                            .ok()
                            .filter(|s| !s.trim().is_empty())
                    })
            }
            "azure_foundry" => {
                let cfg = Some(cfg.clone());
                self.storage_op(|s| Ok(s.get_setting("azure_foundry_api_base").ok().flatten()))
                    .await
                    .ok()
                    .flatten()
                    .filter(|s: &String| !s.trim().is_empty())
                    .or_else(|| {
                        cfg.and_then(|c| c.provider.get("azure_foundry").cloned())
                            .and_then(|p| p.api.and_then(|a| a.base_url))
                    })
                    .or_else(|| {
                        std::env::var("AZURE_AI_FOUNDRY_BASE")
                            .ok()
                            .filter(|s| !s.trim().is_empty())
                    })
            }
            "azure_resource" => self
                .storage_op(|s| {
                    Ok(s.get_setting("azure_resource_last_selection")
                        .ok()
                        .flatten())
                })
                .await
                .ok()
                .flatten()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                .and_then(|parsed| {
                    parsed
                        .get("endpoint")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .filter(|s| !s.trim().is_empty()),
            _ => None,
        };
        tracing::info!(
            provider = %model_ref.provider_id,
            model = %model_ref.model_id,
            endpoint = %crate::sanitize::redact_secrets(&format!("{base_url:?}")),
            "creating LLM client"
        );
        let client = {
            let _scope = profiler.scope("llm.create_client");
            let mut options: HashMap<String, serde_json::Value> = HashMap::new();
            options.insert(
                "model_id".to_string(),
                serde_json::Value::String(model_ref.model_id.clone()),
            );

            // Merge provider-specific config options (auto_start, device,
            // models_path) for providers that consume them.
            // PERF-001: reuse the `cfg` loaded once at the top of the turn.
            if let Some(provider_cfg) = cfg.provider.get(&model_ref.provider_id) {
                if let Some(auto_start) = provider_cfg.options.get("auto_start") {
                    options.insert("auto_start".to_string(), auto_start.clone());
                }
                if let Some(device) = provider_cfg.options.get("device") {
                    options.insert("device".to_string(), device.clone());
                }
                if let Some(models_path) = provider_cfg.options.get("models_path") {
                    options.insert("models_path".to_string(), models_path.clone());
                }
            }

            // H2: reuse a warm per-(provider,model,key) client across turns so
            // the connection pool (TLS, keep-alive) built by the provider's
            // `create_client` is amortised over the whole session instead of
            // being re-created on every turn.
            let cache_key = format!("{}/{}", model_ref.provider_id, model_ref.model_id);
            if let Some(cached) = self.llm_client_cache.read().get(&cache_key).map(Arc::clone) {
                cached
            } else {
                match provider
                    .create_client(&api_key, base_url.as_deref(), &options)
                    .await
                {
                    Ok(c) => {
                        let arc: Arc<dyn crate::llm::LlmClient> = Arc::from(c);
                        // R-21: Bound the LLM client cache so cycling
                        // through many models does not accumulate one
                        // `reqwest::Client` (connection pool, TLS state)
                        // per model for the session lifetime.
                        {
                            let mut cache = self.llm_client_cache.write();
                            const MAX_LLM_CLIENTS: usize = 8;
                            if cache.len() >= MAX_LLM_CLIENTS {
                                // Evict an arbitrary entry (HashMap has no
                                // ordering, so this is effectively random).
                                if let Some(key) = cache.keys().next().cloned() {
                                    cache.remove(&key);
                                }
                            }
                            cache.insert(cache_key, arc.clone());
                        }
                        arc
                    }
                    Err(e) => {
                        let err = e.to_string();
                        publish_error(&self.event_bus, session_id, user_msg_id, &err);
                        return Err(e);
                    }
                }
            }
        };

        // Resolve working directory.
        let working_dir = {
            let _scope = profiler.scope("session.resolve_working_dir");
            self.session_manager.get_session(session_id)?.map_or_else(
                || std::env::current_dir().unwrap_or_default(),
                |s| s.directory,
            )
        };

        // Resolve team context with 5-second cache (M8-T1).
        let team_context = {
            let _scope = profiler.scope("team.resolve_context");
            const TEAM_CONTEXT_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5);
            let session_key = session_id.to_string();
            let cached = {
                let cache = self.team_context_cache.read();
                cache.get(&session_key).cloned()
            };
            if let Some((ctx, fetched_at)) = cached
                && fetched_at.elapsed() < TEAM_CONTEXT_CACHE_TTL
            {
                Some(Arc::new(ctx))
            } else {
                let resolved = crate::session::history::resolve_team_context_for_session(
                    session_id,
                    &working_dir,
                );
                if let Some(arc) = resolved.as_ref() {
                    let mut cache = self.team_context_cache.write();
                    cache.insert(session_key, ((**arc).clone(), std::time::Instant::now()));
                } else {
                    let mut cache = self.team_context_cache.write();
                    cache.remove(&session_key);
                }
                resolved
            }
        };

        // PERF-009 / P-2: `load_config_cached` already returns an `Arc<Config>`,
        // so we reuse it directly instead of re-wrapping.
        let session_config: Arc<ragent_config::Config> = cfg;
        let parsed_hook_configs = crate::hooks::parse_hook_configs(&session_config.hooks);

        // Fire on_session_start hook when this is the first message.
        let has_prior_messages = {
            let _scope = profiler.scope("history.check_prior_assistant");
            self.session_manager
                .storage()
                .has_assistant_messages(session_id)
                .unwrap_or(false)
        };
        if !has_prior_messages {
            let _scope = profiler.scope("hooks.on_session_start");
            crate::hooks::fire_hooks(
                &parsed_hook_configs,
                crate::hooks::HookTrigger::OnSessionStart,
                &working_dir,
                &[],
            );
        }

        Ok(TurnClient {
            model_ref,
            client,
            session_config,
            parsed_hook_configs,
            working_dir,
            team_context,
        })
    }

    /// Assemble the per-turn system prompt: base prompt + tool reference +
    /// question/codeindex guidance + team guidelines + active spec context.
    /// Returns the prompt frozen into an `Arc<str>` (PERF-006).
    pub(crate) async fn build_turn_system_prompt(
        &self,
        agent: &AgentInfo,
        session_config: &ragent_config::Config,
        working_dir: &std::path::Path,
        team_context: Option<&Arc<TeamContext>>,
        profiler: &Arc<crate::session::profiler::AgentLoopProfiler>,
    ) -> Result<Arc<str>> {
        // Load skill registry for system prompt injection
        let skill_dirs = session_config.skill_dirs.clone();
        let skill_registry = {
            let _scope = profiler.scope("skills.load_registry");
            crate::skill::SkillRegistry::load(working_dir, &skill_dirs)
        };
        let (git_status, readme, agents_md, file_tree) = {
            let _scope = profiler.scope("prompt.collect_context");
            crate::agent::collect_prompt_context(working_dir).await
        };
        let memory_section = {
            let _scope = profiler.scope("prompt.build_memory_section");
            let wd = working_dir.to_path_buf();
            let storage = Arc::clone(self.session_manager.storage());
            let memory_cfg = session_config.memory.clone();
            tokio::task::spawn_blocking(move || {
                crate::agent::build_memory_prompt_section(&wd, Some(&storage), Some(&memory_cfg))
            })
            .await
            .unwrap_or_default()
        };
        // JCODEPLAN M8 (T-070): surface active durable initiatives on every
        // turn so the agent stays aware of long-term goals across sessions.
        let initiatives_section = {
            let _scope = profiler.scope("prompt.build_initiatives_section");
            let wd = working_dir.to_path_buf();
            let storage = Arc::clone(self.session_manager.storage());
            tokio::task::spawn_blocking(move || {
                crate::tool::initiative::build_initiatives_prompt_section(&storage, &wd)
            })
            .await
            .unwrap_or_default()
        };
        let mut system_prompt = {
            let _scope = profiler.scope("prompt.build_system_prompt");
            crate::agent::build_system_prompt_with_storage_and_memory(
                agent,
                working_dir,
                &file_tree,
                Some(&skill_registry),
                Some(&git_status),
                Some(&readme),
                Some(&agents_md),
                Some(self.session_manager.storage()),
                Some(&session_config.memory),
                Some(&memory_section),
            )
        };

        let is_subagent = agent.mode == crate::agent::AgentMode::Subagent;
        let tool_reference = if is_subagent {
            let cache = self.system_prompt_cache();
            cache
                .get_tool_reference(&self.tool_registry, |registry| {
                    build_detailed_tool_reference_section(registry)
                })
                .unwrap_or_else(|| build_detailed_tool_reference_section(&self.tool_registry))
        } else {
            let cache = self.system_prompt_cache();
            cache
                .get_tool_reference(&self.tool_registry, |registry| {
                    build_tool_reference_section(registry)
                })
                .unwrap_or_default()
        };
        if !initiatives_section.is_empty() {
            system_prompt.push_str(&initiatives_section);
        }
        system_prompt.push_str(&tool_reference);

        system_prompt.push_str(
            "\n## Question Tool Usage\n\
             When you need to ask the user a question, use the `question` tool. \
             If the answer should be one of a fixed set of choices, provide the `options` \
             parameter as an array of strings. The user will see a multiple-choice dialog \
             instead of a free-text input, which is faster and less error-prone.\n\n\
             Example — multiple choice:\n\
             ```\n\
             question(question: \"Which build profile?\", options: [\"Debug\", \"Release\", \"Check only\"])\n\
             ```\n\n\
             Example — free-text input (no options):\n\
             ```\n\
             question(question: \"What is your name?\")\n\
             ```\n\n",
        );

        let code_index_active = self.code_index.get().is_some();
        let codeindex_section = {
            let cache = self.system_prompt_cache();
            cache
                .get_codeindex_guidance(code_index_active, |is_active| {
                    if is_active {
                        build_codeindex_guidance_section_active()
                    } else {
                        build_codeindex_guidance_section_disabled()
                    }
                })
                .unwrap_or_default()
        };
        system_prompt.push_str(&codeindex_section);

        system_prompt.push_str(TOOL_CALLING_GUIDANCE);

        if team_context.map(|tc| tc.is_lead).unwrap_or(false) {
            system_prompt.push_str(
                "\n## Team Lead — Task Distribution Rules\n\n\
                 When you receive a request that involves a list of N independent items \
                 (e.g. N competitors, N modules, N documents), ALWAYS spawn **exactly one \
                 teammate per item** — never assign multiple items from the list to a single \
                 teammate.\n\n\
                 **Why:** Each teammate has a finite context window.  Assigning all items \
                 to one teammate will overflow its context and cause it to fail.\n\n\
                 **Rules:**\n\
                 1. **Count first.** Before spawning, enumerate the items to process.\n\
                 2. **One teammate per item.** Spawn one `team_spawn` call per item in the \
                    same response turn (all in parallel).\n\
                 3. **Bounded prompt per teammate.** Each teammate's `prompt` must reference \
                    **only its one assigned item** — never a list.  Keep the prompt under \
                    ~500 words; link to files rather than pasting large content.\n\
                 4. **Pre-assign tasks.** When spawning, always include `task_id` parameter \
                    to pre-claim the work item on the teammate's behalf. This ensures they \
                    start with a claimed task and can focus on work instead of claiming.\n\
                    **IMPORTANT:** Only spawn teammates for tasks that are claimable (no \
                    unsatisfied dependencies). If a task has blockers, wait for its dependencies \
                    to complete first, then spawn its teammate later.\n\
                 5. **Then wait.** After all spawns, call `team_wait` once to block until \
                    all teammates report idle or complete.\n\
                 6. **Synthesise.** Read each teammate's output and combine results yourself.\n\
                 7. **Iterate if needed.** If you have more items than available teammates, \
                    distribute in waves — spawn a batch, wait, synthesise, then spawn the \
                    next batch with freshly-idle teammates.\n\n\
                 **Example — analysing 3 competitors A, B, C:**\n\
                 ```\n\
                 team_spawn(teammate_name: \"analyst-A\", task_id: \"s1\", prompt: \"Analyse competitor A only …\")\n\
                 team_spawn(teammate_name: \"analyst-B\", task_id: \"s2\", prompt: \"Analyse competitor B only …\")\n\
                 team_spawn(teammate_name: \"analyst-C\", task_id: \"s3\", prompt: \"Analyse competitor C only …\")\n\
                 team_wait()\n\
                 ```\n\
                 Never: `team_spawn(prompt: \"Analyse competitors A, B, and C …\")`\n\n\
                 **Critical:** The `team_spawn` tool **rejects multi-item prompts**. If your \
                 prompt contains patterns like \"1.\", \"2.\", \"- Item\", or \"and\" joining multiple \
                 items, the spawn will fail. This is intentional — it forces correct distribution.\n\n",
            );
        } else if team_context.is_some() {
            system_prompt.push_str(
                "\n## Teammate — Task Workflow\n\n\
                 You are a member of a team. Always follow this workflow:\n\n\
                 **CRITICAL:** Before starting any work:\n\
                 1. Call `team_task_claim` to claim your assigned task. This returns the task ID \
                    and details.\n\
                 2. Perform the work described in the task.\n\
                 3. Call `team_task_complete(task_id)` with the task ID you claimed in step 1 — \
                    **never guess or try to complete a different task**.\n\
                 4. Call `team_idle` to signal you are done and ready for new assignments.\n\n\
                 **Do NOT:**\n\
                 - Start work without calling `team_task_claim` first\n\
                 - Try to complete a task without its task_id\n\
                 - Complete a task that you did not claim\n\
                 - Go idle while you still have an uncompleted task assigned to you\n\n\
                 If `team_task_claim` returns \"already has task\", complete that task first \
                 (step 3–4 above), then call `team_task_claim` again.\n\n",
            );
        }

        // Inject active spec context into the system prompt.
        let active_spec_opt = self.active_spec.read().await.clone();
        if let Some(ref active_spec_id) = active_spec_opt
            && let Some(ref spec_mgr) = self.spec_manager.get()
            && let Some(spec_id) = ragent_specs::spec::SpecId::new(active_spec_id)
        {
            // P-24: check the SystemPromptCache for a pre-rendered spec
            // section keyed by (spec_id, spec.modified_at). A cache hit
            // skips the disk read and the re-render of the requirements +
            // tasks sections on every turn.
            let cache = self.system_prompt_cache();
            match spec_mgr.read_spec(&spec_id).await {
                Ok(spec) => {
                    if let Some(cached) = cache.get_spec_section(active_spec_id, spec.modified_at) {
                        system_prompt.push_str(&cached);
                    } else {
                        let mut spec_section = format!(
                            "\n## Active Specification: {}\n\n\
                             **Status:** {}\n\
                             **Title:** {}\n\n\
                             ### Requirements\n\n",
                            spec.id,
                            spec.status.as_str(),
                            spec.title
                        );
                        for req in &spec.requirements {
                            spec_section.push_str(&format!(
                                "- `{}` ({:?}) — {}\n",
                                req.id, req.template, req.text
                            ));
                        }
                        spec_section.push_str("\n### Tasks\n\n");
                        for task in &spec.tasks {
                            spec_section.push_str(&format!(
                                "- `{}` — {} ({})\n",
                                task.id,
                                task.title,
                                task.status.as_str()
                            ));
                        }
                        spec_section.push_str(
                            "\nWhen implementing this spec, use the spec_task_update tool to mark tasks as completed.\n",
                        );
                        // P-24: cache the rendered section for subsequent turns.
                        cache.store_spec_section(
                            active_spec_id.clone(),
                            spec.modified_at,
                            spec_section.clone(),
                        );
                        system_prompt.push_str(&spec_section);
                    }
                    tracing::info!(spec_id = %active_spec_id, "Injected active spec into system prompt");
                }
                Err(e) => {
                    tracing::warn!(spec_id = %active_spec_id, error = %e, "Failed to read active spec for prompt injection");
                }
            }
        }

        Ok(Arc::from(system_prompt))
    }

    /// Load the session history and convert to provider-facing `ChatMessage`s
    /// with the per-session version cache (FR-006 / PERF-007).
    ///
    /// Returns `(chat_messages, compressed_this_turn, last_reported_input_tokens, context_window)`.
    pub(crate) async fn build_turn_chat_messages(
        &self,
        session_id: &str,
        _agent: &AgentInfo,
        model_ref: &crate::agent::ModelRef,
        _session_config: &ragent_config::Config,
        profiler: &Arc<crate::session::profiler::AgentLoopProfiler>,
    ) -> Result<(Vec<ChatMessage>, bool, u64, usize)> {
        let history = {
            let _scope = profiler.scope("history.load");
            // P-1: route `get_messages` through `storage_op` so the SQLite
            // read runs on a dedicated blocking thread instead of stalling
            // the async runtime.
            let sid = session_id.to_string();
            self.storage_op(move |s| s.get_messages(&sid)).await?
        };

        let compressed_this_turn = false;
        let last_reported_input_tokens: u64 = {
            let session_state_lock = self
                .session_manager
                .as_ref()
                .session_state_cache(session_id);
            match session_state_lock.lock() {
                Ok(guard) => guard.last_reported_input_tokens(),
                Err(_) => {
                    tracing::warn!(session_id, "session_state cache lock poisoned");
                    0
                }
            }
        };

        // P-3: resolve the model's context window. Some providers (notably the
        // virtual Model Router) report `0` because the real window belongs to
        // the downstream model; treat non-positive windows as "unknown" and
        // fall back to a sensible default so compaction does not fire on the
        // first turn (`context_window - buffer` underflowing to 0).
        let context_window = self
            .provider_registry
            .get(&model_ref.provider_id)
            .and_then(|p| {
                p.default_models()
                    .into_iter()
                    .find(|m| m.id == model_ref.model_id)
            })
            .map(|m| m.context_window)
            .filter(|w| *w > 0)
            .unwrap_or(128_000);

        let history_version = history_version_of(&history);
        let cached: Option<Vec<ChatMessage>> = {
            let session_state_lock = self
                .session_manager
                .as_ref()
                .session_state_cache(session_id);
            let mut state_guard = session_state_lock
                .lock()
                .map_err(|_| anyhow::anyhow!("session_state cache lock poisoned"))?;
            state_guard
                .cached_chat_messages_for_version(history_version)
                .map(|c| c.to_vec())
        };
        let chat_messages = match cached {
            Some(messages) => messages,
            None => {
                let built = history_to_chat_messages(&history).await;
                let session_state_lock = self
                    .session_manager
                    .as_ref()
                    .session_state_cache(session_id);
                if let Ok(mut state_guard) = session_state_lock.lock() {
                    state_guard.store_chat_messages(built.clone(), None);
                }
                built
            }
        };

        // P-3: return `context_window` so the orchestrator can reuse it
        // instead of resolving the identical value a second time.
        Ok((
            chat_messages,
            compressed_this_turn,
            last_reported_input_tokens,
            context_window,
        ))
    }

    /// Run the display-only AGENTS.md acknowledgement exchange (streams to the
    /// TUI but is NOT added to `chat_messages`). Skips subagent/teammate
    /// sessions and sessions that already have an assistant turn.
    pub(crate) async fn run_inline_init_acknowledgement(
        &self,
        session_id: &str,
        agent: &AgentInfo,
        history: &[Message],
        system_prompt: &Arc<str>,
        client: &Arc<dyn crate::llm::LlmClient>,
        model_ref: &crate::agent::ModelRef,
        working_dir: &std::path::Path,
    ) -> Result<()> {
        let has_tools = agent.max_steps.is_none_or(|s| s > 1);
        let has_prior_exchange = history.iter().any(|m| m.role == Role::Assistant);
        let is_subagent = agent.mode == crate::agent::AgentMode::Subagent;
        if !has_prior_exchange && has_tools && !is_subagent {
            let agents_md_path = working_dir.join("AGENTS.md");
            if agents_md_path.is_file() {
                let init_text = "AGENTS.md project guidelines have been loaded.\n\n\
                                 Please acknowledge them briefly.";
                let init_messages = vec![ChatMessage {
                    role: "user".to_string(),
                    content: ChatContent::Text(init_text.to_string()),
                }];
                let init_request = ChatRequest {
                    model: model_ref.model_id.clone(),
                    messages: Arc::new(init_messages),
                    tools: Arc::new(Vec::new()),
                    temperature: agent.temperature,
                    top_p: agent.top_p,
                    max_tokens: Some(200),
                    system: Some(std::sync::Arc::clone(system_prompt)),
                    options: (*agent.options).clone(),
                    session_id: Some(session_id.to_string()),
                    request_id: Some(Uuid::new_v4().to_string()),
                    stream_timeout_secs: None,
                    thinking: agent.thinking.clone(),
                };
                self.event_bus.publish(Event::RequestStarted {
                    session_id: session_id.to_string(),
                    outbound_bytes: chat_request_payload_bytes(&init_request),
                });
                match client.chat(init_request).await {
                    Ok(mut stream) => {
                        while let Some(ev) = stream.next().await {
                            match ev {
                                StreamEvent::TextDelta { text } => {
                                    self.event_bus.publish(Event::TextDelta {
                                        session_id: session_id.to_string(),
                                        text: text.clone(),
                                    });
                                }
                                StreamEvent::Usage {
                                    input_tokens,
                                    output_tokens,
                                } => {
                                    self.event_bus.publish(Event::TokenUsage {
                                        session_id: session_id.to_string(),
                                        input_tokens,
                                        output_tokens,
                                    });
                                }
                                _ => {}
                            }
                        }
                        self.event_bus.publish(Event::MessageEnd {
                            session_id: session_id.to_string(),
                            message_id: "init".to_string(),
                            reason: FinishReason::Stop,
                        });
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
                    }
                }
            }
        }
        Ok(())
    }

    /// Call the LLM with retry on transient failures, process stream events,
    /// and collect the response buffers. This is the largest single step
    /// in the agent loop.
    ///
    /// Returns the text/reasoning/tool-call buffers and token counts, or
    /// `should_break` signal for the orchestrator.
    pub(crate) async fn call_llm_step(
        &self,
        session_id: &str,
        agent: &AgentInfo,
        turn: &TurnClient,
        loop_state: &mut LoopState,
        tool_definitions: &Arc<Vec<ToolDefinition>>,
        system_prompt: &Arc<str>,
        context_window: usize,
        llm_request_start: Instant,
        profiler: &Arc<crate::session::profiler::AgentLoopProfiler>,
    ) -> Result<LlmStepResult> {
        let max_retries = self.stream_config.max_retries;
        let backoff_secs = self.stream_config.retry_backoff_secs;
        let mut text_buffer = String::new();
        let mut reasoning_buffer = String::new();
        let mut tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut attempted_truncation_continuation = false;
        let mut last_input_tokens: u64 = 0;
        let mut last_output_tokens: u64 = 0;

        let _scope = profiler.scope("loop.llm.total");
        let llm_recorder = crate::telemetry::LlmRecorder::from_subsystem(&self.telemetry);
        'retry: for attempt in 0..=max_retries {
            let mut saw_completed_tool_call = false;
            // Truncation-continuation pass (background sub-agents only).
            //
            // The default sub-agent path keeps buffers from the previous
            // attempt when retrying, so output is NOT cleared when this flag
            // is set: the continuation response Appends to the text recorded
            // so far rather than replacing it. The flag is cleared below as
            // soon as the continuation attempt consumed it, so a following
            // ordinary retry clears buffers as before.
            let is_truncation_continuation = attempted_truncation_continuation;
            if attempt > 0 && !is_truncation_continuation {
                text_buffer.clear();
                reasoning_buffer.clear();
                tool_calls.clear();
                last_input_tokens = 0;
                last_output_tokens = 0;
                saw_completed_tool_call = false;

                let wait_secs = attempt as u64 * backoff_secs;
                self.event_bus.publish(Event::AgentNotice {
                    session_id: session_id.to_string(),
                    message: format!(
                        "Retrying LLM request (attempt {}/{}), waiting {}s...",
                        attempt + 1,
                        max_retries + 1,
                        wait_secs
                    ),
                });
                {
                    let _scope = profiler.scope("loop.llm.backoff_sleep");
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                }
            }

            // Build request (fresh for each attempt)
            // P-6: share the history by refcount bump instead of cloning
            // the entire `Vec<ChatMessage>` (including all tool-result
            // `ContentPart`s) on every retry attempt.
            let attempt_messages: Arc<Vec<ChatMessage>> = if is_truncation_continuation {
                // Append an out-of-band user-side message to the request for
                // this attempt, asking the model (invisible to the caller)
                // to continue exactly from where it stopped. The shared
                // history itself is only mutated inside the request copy,
                // so the main loop's `chat_messages` stay unchanged.
                let mut msgs = (*loop_state.chat_messages).clone();
                msgs.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: ChatContent::Text(text_buffer.clone()),
                });
                msgs.push(ChatMessage {
                    role: "user".to_string(),
                    content: ChatContent::Text(
                        "Your previous response was cut off mid-message. \
                         Continue EXACTLY from where you stopped, outputting only the remaining \
                         continuation text (no preamble, no recap, no repeating what you already \
                         wrote, no formatting restarts). If your previous output cannot be \
                         continued faithfully, restart the full answer from the beginning."
                            .to_string(),
                    ),
                });
                Arc::new(msgs)
            } else {
                Arc::clone(&loop_state.chat_messages)
            };
            let attempt_request = ChatRequest {
                model: turn.model_ref.model_id.clone(),
                messages: attempt_messages,
                tools: tool_definitions.clone(),
                temperature: agent.temperature,
                top_p: agent.top_p,
                max_tokens: None,
                system: Some(std::sync::Arc::clone(system_prompt)),
                options: (*agent.options).clone(),
                session_id: Some(session_id.to_string()),
                request_id: Some(Uuid::new_v4().to_string()),
                stream_timeout_secs: Some(self.stream_config.initial_response_timeout_secs),
                thinking: agent.thinking.clone(),
            };
            self.event_bus.publish(Event::RequestStarted {
                session_id: session_id.to_string(),
                outbound_bytes: chat_request_payload_bytes(&attempt_request),
            });
            llm_recorder.record_request(&turn.model_ref.model_id, &turn.model_ref.provider_id);
            // H4: surface provider progress so long `create_stream` waits do not
            // feel like hangs. A background task publishes an AgentNotice every 30s
            // after the first 30s, until the first stream event arrives.
            let first_event_arrived = Arc::new(AtomicBool::new(false));
            // R-1: Wrap the notice task in an `AbortOnDrop` guard so it is
            // aborted on every scope exit — including the `continue 'retry`
            // and `bail!` error paths below — not just the success path.
            struct AbortOnDrop(tokio::task::JoinHandle<()>);
            impl Drop for AbortOnDrop {
                fn drop(&mut self) {
                    self.0.abort();
                }
            }
            let notice_handle = {
                let event_bus = Arc::clone(&self.event_bus);
                let session_id = session_id.to_string();
                let first_event_arrived = Arc::clone(&first_event_arrived);
                let provider_id = turn.model_ref.provider_id.clone();
                AbortOnDrop(tokio::spawn(async move {
                    let interval = std::time::Duration::from_secs(30);
                    let mut elapsed = 0u64;
                    loop {
                        tokio::time::sleep(interval).await;
                        elapsed += 30;
                        if first_event_arrived.load(Ordering::Relaxed) {
                            break;
                        }
                        let message = if elapsed == 30 && provider_id == "ollama" {
                            "Waiting for model response (Ollama may be loading the model)..."
                                .to_string()
                        } else {
                            format!("Waiting for model response... ({elapsed}s)")
                        };
                        event_bus.publish(Event::AgentNotice {
                            session_id: session_id.clone(),
                            message,
                        });
                    }
                }))
            };
            let mut stream = {
                let _scope = profiler.scope("loop.llm.create_stream");
                match turn.client.chat(attempt_request).await {
                    Ok(s) => s,
                    Err(e) => {
                        let error_message = e.to_string();
                        debug!(
                            "LLM call failed (attempt {}): {}",
                            attempt + 1,
                            crate::sanitize::redact_secrets(&error_message)
                        );
                        if attempt < max_retries && !is_permanent_llm_api_error(&error_message) {
                            // FR-004 / FR-008: emergency overflow compaction is
                            // always eligible regardless of `compaction.auto`.
                            // When `auto` is false the pre-send path is skipped
                            // (FR-008) and the runner relies solely on this
                            // emergency path; when `auto` is true and pre-send
                            // already compacted this turn, the
                            // `compressed_this_turn` guard prevents a second
                            // compaction.
                            if is_token_overflow_error_message(&error_message)
                                && !loop_state.compaction_attempted_this_turn
                            {
                                // FR-004: emergency overflow compaction. The
                                // `chat()` call failed before any assistant
                                // tokens were produced, so run the summarisation
                                // runner with reason "overflow" and retry the
                                // turn once with the compacted history.
                                loop_state.compaction_attempted_this_turn = true;
                                let compact_result = crate::compaction::emergency_compact(
                                    session_id,
                                    Arc::make_mut(&mut loop_state.chat_messages),
                                    &turn.model_ref.model_id,
                                    context_window,
                                    0,
                                    &turn.session_config.compaction,
                                    &turn.client,
                                    &self.event_bus,
                                    &self.stream_config,
                                )
                                .await;
                                match compact_result {
                                    Ok(outcome) => {
                                        loop_state.compressed_this_turn = true;
                                        loop_state.last_reported_input_tokens =
                                            outcome.compressed_tokens as u64;
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
                                            "emergency overflow compaction applied"
                                        );
                                        self.event_bus.publish(Event::AgentNotice {
                                            session_id: session_id.to_string(),
                                            message: format!(
                                                "{error_message} — emergency-compacted, will retry"
                                            ),
                                        });
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            error = %e,
                                            "emergency overflow compaction failed; surfacing original error"
                                        );
                                        self.event_bus.publish(Event::AgentError {
                                            session_id: session_id.to_string(),
                                            error: error_message.clone(),
                                        });
                                        bail!(
                                            "LLM call failed after {} attempts: {}",
                                            max_retries + 1,
                                            error_message
                                        );
                                    }
                                }
                            } else {
                                self.event_bus.publish(Event::AgentNotice {
                                    session_id: session_id.to_string(),
                                    message: format!("{error_message} — will retry"),
                                });
                            }
                            continue 'retry;
                        }
                        self.event_bus.publish(Event::AgentError {
                            session_id: session_id.to_string(),
                            error: error_message.clone(),
                        });
                        crate::hooks::fire_hooks(
                            &turn.parsed_hook_configs,
                            crate::hooks::HookTrigger::OnError,
                            &turn.working_dir,
                            &[("RAGENT_ERROR", &error_message)],
                        );
                        bail!(
                            "LLM call failed after {} attempts: {}",
                            max_retries + 1,
                            error_message
                        );
                    }
                }
            };
            // Stop the H4 progress-notice task immediately now that the stream
            // has been created. The `AbortOnDrop` guard (R-1) also aborts on
            // scope exit, but we cancel here for promptness so the notice task
            // does not fire during stream processing.
            notice_handle.0.abort();

            let mut had_retryable_error = false;
            let mut first_stream_event_pending = true;
            let mut saw_finish_reason: Option<FinishReason> = None;
            let mut fatal_stream_error: Option<String> = None;
            let mut stream_buffer = StreamBuffer::new();
            {
                let _scope = profiler.scope("loop.llm.stream");
                loop {
                    let wait_started = Instant::now();
                    // Per-chunk stall safety net. The H1 optimisation assumed
                    // every provider wraps `stream.next()` in its own timeout,
                    // but only Ollama does — all other providers (OpenAI,
                    // Anthropic, Gemini, Bedrock, Copilot, Azure, Router, etc.)
                    // have an unguarded `stream.next().await` that hangs forever
                    // on a stalled connection (low CPU, no progress). We wrap the
                    // poll in `stream_config.timeout_secs` (default 120s) and
                    // synthesise a retryable `StreamEvent::Error` on timeout so
                    // the existing retry logic can re-attempt the call.
                    let stall_secs = agent
                        .stall_timeout_secs
                        .unwrap_or(self.stream_config.timeout_secs);
                    let next_event = match tokio::time::timeout(
                        std::time::Duration::from_secs(stall_secs),
                        stream.next(),
                    )
                    .await
                    {
                        Ok(event) => event,
                        Err(_) => {
                            debug!(
                                stall_secs,
                                "Stream stalled — no data for {stall_secs}s, \
                                 treating as retryable error"
                            );
                            Some(StreamEvent::Error {
                                message: format!(
                                    "stream stalled — no data received for {stall_secs}s"
                                ),
                            })
                        }
                    };
                    if first_stream_event_pending {
                        if next_event.is_some() {
                            first_event_arrived.store(true, Ordering::Relaxed);
                            profiler.record_duration(
                                "loop.llm.first_event_wait",
                                wait_started.elapsed(),
                            );
                        }
                        first_stream_event_pending = false;
                    }
                    let Some(event) = next_event else {
                        let text = stream_buffer.drain_text();
                        if !text.is_empty() {
                            self.event_bus.publish(Event::TextDelta {
                                session_id: session_id.to_string(),
                                text,
                            });
                        }
                        let reasoning = stream_buffer.drain_reasoning();
                        if !reasoning.is_empty() {
                            self.event_bus.publish(Event::ReasoningDelta {
                                session_id: session_id.to_string(),
                                text: reasoning,
                            });
                        }
                        break;
                    };
                    match event {
                        StreamEvent::TextDelta { text } => {
                            if let Some(flushed) = stream_buffer.push_text(&text) {
                                self.event_bus.publish(Event::TextDelta {
                                    session_id: session_id.to_string(),
                                    text: flushed,
                                });
                                stream_buffer.reset_timer();
                            }
                            text_buffer.push_str(&text);
                        }
                        StreamEvent::ReasoningStart => {}
                        StreamEvent::ReasoningDelta { text } => {
                            if let Some(flushed) = stream_buffer.push_reasoning(&text) {
                                self.event_bus.publish(Event::ReasoningDelta {
                                    session_id: session_id.to_string(),
                                    text: flushed,
                                });
                                stream_buffer.reset_timer();
                            }
                            reasoning_buffer.push_str(&text);
                        }
                        StreamEvent::ReasoningEnd => {}
                        StreamEvent::ToolCallStart { id, name } => {
                            let text = stream_buffer.drain_text();
                            if !text.is_empty() {
                                self.event_bus.publish(Event::TextDelta {
                                    session_id: session_id.to_string(),
                                    text,
                                });
                            }
                            let reasoning = stream_buffer.drain_reasoning();
                            if !reasoning.is_empty() {
                                self.event_bus.publish(Event::ReasoningDelta {
                                    session_id: session_id.to_string(),
                                    text: reasoning,
                                });
                            }
                            stream_buffer.reset_timer();
                            tool_calls.push(PendingToolCall {
                                id,
                                name,
                                args_json: String::new(),
                            });
                        }
                        StreamEvent::ToolCallDelta { id, args_json } => {
                            if let Some(tc) = tool_calls.iter_mut().find(|t| t.id == id) {
                                tc.args_json.push_str(&args_json);
                            }
                        }
                        StreamEvent::ToolCallEnd { id } => {
                            if let Some(_tc) = tool_calls.iter().find(|t| t.id == id) {
                                saw_completed_tool_call = true;
                            }
                        }
                        StreamEvent::Usage {
                            input_tokens,
                            output_tokens,
                        } => {
                            last_input_tokens = input_tokens;
                            last_output_tokens = output_tokens;
                            // Persist the provider-reported input tokens in the
                            // per-session state cache so the next turn's start-of-turn
                            // compaction check can use the same usage value shown in
                            // the TUI status bar instead of resetting to zero.
                            {
                                let session_state_lock = self
                                    .session_manager
                                    .as_ref()
                                    .session_state_cache(session_id);
                                if let Ok(mut guard) = session_state_lock.lock() {
                                    guard.set_last_reported_input_tokens(input_tokens);
                                }
                            }
                            llm_recorder.record_usage(
                                &turn.model_ref.model_id,
                                &turn.model_ref.provider_id,
                                input_tokens,
                                output_tokens,
                            );
                            self.event_bus.publish(Event::TokenUsage {
                                session_id: session_id.to_string(),
                                input_tokens,
                                output_tokens,
                            });
                        }
                        StreamEvent::Error { message } => {
                            debug!(
                                "Stream error (attempt {}): {}",
                                attempt + 1,
                                crate::sanitize::redact_secrets(&message)
                            );
                            let has_meaningful_partial_output =
                                stream_has_meaningful_partial_output(
                                    &text_buffer,
                                    &reasoning_buffer,
                                    saw_completed_tool_call,
                                );
                            let is_emergency_overflow: bool = {
                                attempt < max_retries
                                    && !loop_state.compaction_attempted_this_turn
                                    && !has_meaningful_partial_output
                                    && is_token_overflow_error_message(&message)
                            };
                            if is_emergency_overflow {
                                // FR-004 / FR-008: emergency overflow compaction.
                                // The stream failed with a context-overflow error
                                // before any assistant tokens were produced, so
                                // run the OpenCode-derived summarisation runner
                                // with reason "overflow" and retry the turn once
                                // with the compacted history. This path is
                                // eligible regardless of `compaction.auto` (when
                                // `auto` is false the runner relies solely on
                                // emergency summarisation — FR-008). The
                                // `compaction_attempted_this_turn` guard ensures
                                // only a single compaction attempt per turn, so a
                                // skipped emergency compaction is not retried.
                                loop_state.compaction_attempted_this_turn = true;
                                let compact_result = crate::compaction::emergency_compact(
                                    session_id,
                                    Arc::make_mut(&mut loop_state.chat_messages),
                                    &turn.model_ref.model_id,
                                    context_window,
                                    0,
                                    &turn.session_config.compaction,
                                    &turn.client,
                                    &self.event_bus,
                                    &self.stream_config,
                                )
                                .await;
                                match compact_result {
                                    Ok(outcome) => {
                                        loop_state.compressed_this_turn = true;
                                        loop_state.last_reported_input_tokens =
                                            outcome.compressed_tokens as u64;
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
                                            "emergency overflow compaction applied (stream error)"
                                        );
                                        self.event_bus.publish(Event::AgentNotice {
                                            session_id: session_id.to_string(),
                                            message: format!(
                                                "{message} — emergency-compacted, will retry"
                                            ),
                                        });
                                        had_retryable_error = true;
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            error = %e,
                                            "emergency overflow compaction failed; surfacing original error"
                                        );
                                        self.event_bus.publish(Event::AgentError {
                                            session_id: session_id.to_string(),
                                            error: message.clone(),
                                        });
                                        fatal_stream_error = Some(message);
                                    }
                                }
                            } else if should_retry_stream_error(
                                &message,
                                attempt,
                                max_retries,
                                has_meaningful_partial_output,
                            ) {
                                self.event_bus.publish(Event::AgentNotice {
                                    session_id: session_id.to_string(),
                                    message: format!("{} — will retry", message),
                                });
                                had_retryable_error = true;
                            } else if crate::session::history::is_retryable_stream_error(&message)
                                && has_meaningful_partial_output
                            {
                                self.event_bus.publish(Event::AgentNotice {
                                    session_id: session_id.to_string(),
                                    message: format!(
                                        "{} — keeping partial output from this attempt",
                                        message
                                    ),
                                });
                            } else {
                                self.event_bus.publish(Event::AgentError {
                                    session_id: session_id.to_string(),
                                    error: message.clone(),
                                });
                                fatal_stream_error = Some(message);
                            }
                        }
                        StreamEvent::RateLimit {
                            requests_used_pct,
                            tokens_used_pct,
                        } => {
                            let percent = requests_used_pct.or(tokens_used_pct);
                            if let Some(pct) = percent {
                                self.event_bus.publish(Event::QuotaUpdate {
                                    session_id: session_id.to_string(),
                                    percent: pct,
                                });
                            }
                        }
                        StreamEvent::Finish { reason } => {
                            saw_finish_reason = Some(reason);
                        }
                    }
                }
            }

            if had_retryable_error {
                // A retry loop due to a transient error; the truncation
                // continuation flag is only consumed when we have real
                // text output to append to, so leave it armed if the
                // buffer is still empty.
                if text_buffer.trim().is_empty() {
                    attempted_truncation_continuation = false;
                }
                continue 'retry;
            }

            // Propagate the finish reason observed for this attempt into the
            // shared loop state so the caller (processor / sub-agent runner)
            // can flag truncated results.
            if let Some(reason) = saw_finish_reason.clone() {
                loop_state.last_finish_reason = Some(reason.clone());
            } else if tool_calls.is_empty() {
                // Stream ended silently — the provider never emitted an
                // explicit `Finish` signal. This is the signature of an
                // output-truncating provider: the response simply stops.
                // Mark it so the task-layer can retry / flag the result.
                loop_state.last_finish_reason = Some(FinishReason::Truncation);
            } else {
                loop_state.last_finish_reason = None;
            }

            // Truncation-continuation kick: for background sub-agents, the
            // only way to recover from a provider-side silent cut is to ask
            // the model to continue. We do it once per step (bounded by the
            // existing retry budget) and only when there is something
            // non-empty to continue from. This path only makes sense when
            // the reply was text-only (no pending tool calls).
            if matches!(
                loop_state.last_finish_reason,
                Some(FinishReason::Truncation)
            ) && agent.mode == crate::agent::AgentMode::Subagent
                && tool_calls.is_empty()
                && !text_buffer.trim().is_empty()
                && !attempted_truncation_continuation
            {
                attempted_truncation_continuation = true;
                self.event_bus.publish(Event::AgentNotice {
                    session_id: session_id.to_string(),
                    message: "Provider truncated the reply without an explicit \
                               finish signal — asking the model to continue from \
                               where it stopped…"
                        .to_string(),
                });
                tracing::warn!(
                    session_id = %session_id,
                    "silent end-of-stream detected for sub-agent; retrying with truncation continuation"
                );
                continue 'retry;
            }

            if matches!(
                loop_state.last_finish_reason,
                Some(FinishReason::Truncation)
            ) && !tool_calls.is_empty()
            {
                // A truncation observed on a step that produced tool calls is
                // most likely a pre-tool preamble cut — the tool phase will
                // advance the conversation anyway, so no continuation is
                // required. Log it for diagnostics, then normalise the
                // recorded reason back to `ToolUse` so the final message is
                // not mislabelled as truncated when the run ends.
                tracing::info!(
                    session_id = %session_id,
                    "silent end-of-stream before tool calls; continuing via tool phase"
                );
                loop_state.last_finish_reason = Some(FinishReason::ToolUse);
            }

            if let Some(error) = fatal_stream_error {
                let error_message = error.clone();
                crate::hooks::fire_hooks(
                    &turn.parsed_hook_configs,
                    crate::hooks::HookTrigger::OnError,
                    &turn.working_dir,
                    &[("RAGENT_ERROR", &error_message)],
                );
                bail!(
                    "LLM stream failed (attempt {}): {}",
                    attempt + 1,
                    error_message
                );
            }

            break;
        }

        // Capture the LLM-reported input token count for the next
        // iteration's threshold check.
        if last_input_tokens > 0 {
            loop_state.last_reported_input_tokens = last_input_tokens;
        }

        Ok(LlmStepResult {
            text_buffer,
            reasoning_buffer,
            tool_calls,
            last_input_tokens,
            last_output_tokens,
            llm_request_start,
            should_break: false,
        })
    }

    /// Final save of the assistant message, timing breakdown, and
    /// on_session_end hook.
    pub(crate) async fn finalize_assistant_message(
        &self,
        session_id: &str,
        loop_state: &mut LoopState,
        assistant_msg_id: &str,
        total_start: Instant,
        parsed_hook_configs: &[crate::hooks::HookConfig],
        working_dir: &std::path::Path,
    ) -> Result<Message> {
        // Final save (update the pre-created placeholder).
        let parts_owned =
            std::sync::Arc::try_unwrap(std::mem::take(&mut loop_state.assistant_parts))
                .unwrap_or_else(|arc| (*arc).clone());
        let mut assistant_msg = Message::new(session_id, Role::Assistant, parts_owned);
        assistant_msg.id = assistant_msg_id.to_string();
        {
            let msg = assistant_msg.clone();
            self.storage_op(move |s| s.update_message(&msg)).await?;
        }

        // Calculate and log timing breakdown
        let total_elapsed_ms = total_start.elapsed().as_millis() as u64;
        let other_ms = total_elapsed_ms.saturating_sub(loop_state.cumulative_model_wait_ms);
        tracing::info!(
            session_id = %session_id,
            total_ms = total_elapsed_ms,
            model_wait_ms = loop_state.cumulative_model_wait_ms,
            other_ms = other_ms,
            "Agent loop finished - timing breakdown: total={}ms, model_wait={}ms, other={}ms",
            total_elapsed_ms,
            loop_state.cumulative_model_wait_ms,
            other_ms
        );

        self.event_bus.publish(Event::MessageEnd {
            session_id: session_id.to_string(),
            message_id: assistant_msg.id.clone(),
            reason: FinishReason::Stop,
        });

        crate::hooks::fire_hooks(
            parsed_hook_configs,
            crate::hooks::HookTrigger::OnSessionEnd,
            working_dir,
            &[],
        );

        Ok(assistant_msg)
    }
}
