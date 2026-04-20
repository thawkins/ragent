//! Message processing pipeline for agent sessions.
//!
//! [`SessionProcessor`] orchestrates the agentic loop: it accepts a user message,
//! streams an LLM response, executes any requested tool calls, and iterates
//! until the model signals completion or the step limit is reached.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{Result, bail};
use futures::StreamExt;
use serde_json::{Value, json};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::agent::AgentInfo;
use crate::event::{Event, EventBus, FinishReason};
use crate::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, StreamEvent};
use crate::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use crate::permission::PermissionChecker;
use crate::provider::ProviderRegistry;
use crate::sanitize::redact_secrets;
use crate::session::SessionManager;
use crate::tool::{McpToolWrapper, TeamContext, ToolContext, ToolRegistry};
use base64::Engine as _;

/// Additional system-prompt guidance injected for Ollama sessions.
pub const OLLAMA_TOOL_GUIDANCE: &str = "\n## Tool Use — Critical Instructions\n\n\
IMPORTANT: When you need to take any action, call the appropriate tool IMMEDIATELY.\n\
Do NOT write text describing what you are going to do — just call the tool.\n\
Do NOT say \'Let me explore...\' or \'I will analyze...\' — instead, call the relevant tool now.\n\n\
When you need file contents, use the `read` tool with arguments like \
`{\"path\":\"src/main.rs\",\"start_line\":1,\"end_line\":100}`.\n\
Prefer small line ranges (100 lines max) for large files; iterate with `start_line`/`end_line`.\n\
Never invent or guess file contents — always read them with the tool.\n\n\
Rule: every response where you need information or need to act MUST start with a tool call.\n\n";

/// Build a system-prompt section for LSP code-intelligence tools listing only
/// the LSP servers that are currently connected. Returns an empty string if no
/// servers are connected so no misleading guidance is injected.
async fn build_lsp_guidance_section(lsp_manager: &crate::lsp::SharedLspManager) -> String {
    use crate::lsp::LspStatus;

    let guard = lsp_manager.read().await;
    let connected: Vec<&crate::lsp::server::LspServer> = guard
        .servers()
        .iter()
        .filter(|s| s.status == LspStatus::Connected)
        .collect();

    if connected.is_empty() {
        return String::new();
    }

    let mut section = String::from(
        "\n## Code Intelligence — LSP Tools\n\n\
        The following Language Server Protocol (LSP) servers are **currently connected**. \
        For source files in these languages, you MUST use LSP tools instead of `grep`/`glob` — \
        they are semantic and understand types, scopes, and cross-file relationships. \
        `grep` does NOT understand code structure.\n\n\
        **Connected servers and their file extensions:**\n",
    );

    for server in &connected {
        let exts: Vec<String> = server
            .config
            .extensions
            .iter()
            .map(|e| format!("`.{e}`"))
            .collect();
        let caps = server
            .capabilities_summary
            .as_deref()
            .unwrap_or("connected");
        section.push_str(&format!(
            "- **{}** ({}) — {}\n",
            server.language,
            exts.join(", "),
            caps
        ));
    }

    section.push_str(
        "\n**Use the right tool for each task:**\n\
        - Find where a symbol is defined → `lsp_definition` (args: `path`, `line`, `column`)\n\
        - Find all usages of a symbol → `lsp_references` (args: `path`, `line`, `column`)\n\
        - Get type info / docs for a symbol → `lsp_hover` (args: `path`, `line`, `column`)\n\
        - List all symbols in a file → `lsp_symbols` (arg: `path`)\n\
        - Show compiler errors and warnings → `lsp_diagnostics` (optional arg: `path`)\n\n\
        **Do NOT use grep for these tasks** when an LSP server is connected for that \
        language. Use `grep` or `glob` only for languages not listed above, \
        or when searching for arbitrary text patterns across many files.\n\n",
    );
    section
}

/// Build a system-prompt section describing the codebase index tools
/// when the index **is** active.
///
/// Uses strong directive language to steer the LLM away from `grep`/`search`
/// for any query that involves code symbols, types, or structure.
fn build_codeindex_guidance_section_active() -> String {
    "\n## Code Intelligence — Codebase Index Tools\n\n\
     A **codebase index** is active for this project. It provides fast, structured \
     search across all indexed source files — symbols, references, dependencies, \
     and documentation.\n\n\
     **MANDATORY — You MUST use codeindex tools instead of grep/search for code symbol queries.**\n\
     When the index is active, `grep` and `search` are the WRONG choice for finding \
     functions, types, structs, enums, traits, or any named code entity. The index \
     is faster, returns structured results with file/line/signature, and understands \
     symbol kinds.\n\n\
     **Decision flow — which tool to use:**\n\
     - \"Where is function X defined?\" → `codeindex_search` (NOT grep)\n\
     - \"Find all structs matching Y\" → `codeindex_symbols` with kind=struct (NOT grep)\n\
     - \"Who calls function Z?\" → `codeindex_references` (NOT grep)\n\
     - \"What does file A import?\" → `codeindex_dependencies` (NOT grep for imports)\n\
     - \"List all functions in file B\" → `codeindex_symbols` with file_path (NOT grep)\n\
     - \"Is the index working?\" → `codeindex_status`\n\
     - \"Re-index after bulk edits\" → `codeindex_reindex`\n\n\
     **When grep/search IS appropriate:**\n\
     - Searching for arbitrary text strings, comments, or prose (not symbols)\n\
     - Finding TODO/FIXME/HACK comments\n\
     - Searching config files, markdown, or non-code text\n\
     - Pattern matching across many files for non-structural content\n\n\
     **Rule of thumb:** If you are looking for a named code entity (function, type, \
     variable, import), use codeindex. If you are searching for a text pattern that \
     is NOT a code symbol, use grep/search.\n\n"
        .to_string()
}

/// Build a system-prompt section for when the codebase index is NOT active.
///
/// Informs the LLM that codeindex tools will return "not available" and
/// that grep/search should be used as fallback. Suggests enabling the index.
fn build_codeindex_guidance_section_disabled() -> String {
    "\n## Code Intelligence — Codebase Index Tools\n\n\
     The codebase index is **not active** for this project. Code index tools \
     (`codeindex_search`, `codeindex_symbols`, `codeindex_references`, \
     `codeindex_dependencies`) will return \"not available\" if called.\n\n\
     Use `grep` or `search` for code lookups in the meantime. You can suggest \
     the user enable the index with `/codeindex on` for faster, structured \
     symbol search.\n\n"
        .to_string()
}

/// Build a concise system-prompt section listing every registered tool by name and description.
///
/// Injected into every session's system prompt so the model always knows the exact tool names
/// available. This prevents hallucinated tool names (e.g. calling "search" instead of "grep").
fn build_tool_reference_section(registry: &crate::tool::ToolRegistry) -> String {
    let defs = registry.definitions();
    if defs.is_empty() {
        return String::new();
    }
    let mut section = String::from(
        "## Available Tools\n\nYou have access to the following tools. \
        Use ONLY these exact tool names — do not invent or guess tool names.\n\n",
    );
    for def in &defs {
        // Truncate long descriptions to keep the prompt compact.
        let desc = if def.description.len() > 120 {
            format!("{}…", &def.description[..120])
        } else {
            def.description.clone()
        };
        section.push_str(&format!("- `{}` — {}\n", def.name, desc));
    }
    section.push('\n');
    section
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
    pub permission_checker: Arc<tokio::sync::RwLock<PermissionChecker>>,
    /// Bus for broadcasting session and processing events.
    pub event_bus: Arc<EventBus>,
    /// Optional task manager for sub-agent spawning (F13/F14).
    /// Uses `OnceLock` to break the circular dependency with `TaskManager`.
    pub task_manager: std::sync::OnceLock<Arc<crate::task::TaskManager>>,
    /// Optional LSP manager for code-intelligence tool context.
    /// Uses `OnceLock` so it can be set after the processor is constructed
    /// (the `LspManager` is created after the processor in `run_tui`).
    pub lsp_manager: std::sync::OnceLock<crate::lsp::SharedLspManager>,
    /// Optional team manager for spawning and coordinating teammate sessions.
    /// Uses `OnceLock` to break the circular dependency with `TeamManager`.
    pub team_manager: std::sync::OnceLock<Arc<crate::team::TeamManager>>,
    /// Optional MCP client for dynamic MCP tool registration.
    /// Set once after startup via [`SessionProcessor::set_mcp_client`].
    pub mcp_client: std::sync::OnceLock<Arc<tokio::sync::RwLock<crate::mcp::McpClient>>>,
    /// Optional code index for codebase search and symbol lookup.
    /// Uses `OnceLock` so it can be set after the processor is constructed.
    pub code_index: std::sync::OnceLock<Arc<ragent_code::CodeIndex>>,
    /// LLM stream configuration (timeouts, retries, backoff).
    pub stream_config: crate::config::StreamConfig,
    /// Memory extraction engine for automatic memory candidate generation.
    pub extraction_engine: std::sync::OnceLock<Arc<crate::memory::ExtractionEngine>>,
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
    }

    /// Run a blocking storage operation on a dedicated thread to avoid
    /// stalling the Tokio runtime.
    async fn storage_op<F, T>(&self, f: F) -> Result<T>
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
    /// use ragent_core::session::processor::SessionProcessor;
    /// use ragent_core::agent::AgentInfo;
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
        // 1. Store user message (off async thread)
        {
            let msg = user_msg.clone();
            self.storage_op(move |s| s.create_message(&msg)).await?;
        }

        self.event_bus.publish(Event::MessageStart {
            session_id: session_id.to_string(),
            message_id: user_msg.id.clone(),
        });

        // Helper: publish error + message-end so the TUI always resets.
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

        // 2. Resolve model and create LLM client
        let model_ref = if let Some(m) = agent.model.as_ref() {
            m
        } else {
            let err = format!("Agent '{}' has no model configured", agent.name);
            publish_error(&self.event_bus, session_id, &user_msg.id, &err);
            bail!("{err}");
        };

        let provider = if let Some(p) = self.provider_registry.get(&model_ref.provider_id) {
            p
        } else {
            let err = format!("Provider '{}' not found", model_ref.provider_id);
            publish_error(&self.event_bus, session_id, &user_msg.id, &err);
            bail!("{err}");
        };

        // Try to get API key from environment or storage
        let api_key = match self.resolve_api_key(&model_ref.provider_id).await {
            Ok(k) => k,
            Err(e) => {
                let err = e.to_string();
                publish_error(&self.event_bus, session_id, &user_msg.id, &err);
                return Err(e);
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
                let cfg = crate::config::Config::load().ok();
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
            _ => None,
        };

        tracing::info!(
            provider = %model_ref.provider_id,
            model = %model_ref.model_id,
            api_base = %crate::sanitize::redact_secrets(&format!("{base_url:?}")),
            "creating LLM client"
        );

        let client = match provider
            .create_client(&api_key, base_url.as_deref(), &HashMap::new())
            .await
        {
            Ok(c) => c,
            Err(e) => {
                let err = e.to_string();
                publish_error(&self.event_bus, session_id, &user_msg.id, &err);
                return Err(e);
            }
        };

        // 3. Build system prompt
        let working_dir = self.session_manager.get_session(session_id)?.map_or_else(
            || std::env::current_dir().unwrap_or_default(),
            |s| s.directory,
        );
        let team_context_for_session = resolve_team_context_for_session(session_id, &working_dir);

        // Load config once for hooks and other config-dependent features
        let session_config = crate::config::Config::load().unwrap_or_default();

        // Fire on_session_start hook when this is the first message in the session
        let has_prior_messages = self
            .session_manager
            .get_messages(session_id)
            .map(|msgs| {
                msgs.iter()
                    .any(|m| m.role == crate::message::Role::Assistant)
            })
            .unwrap_or(false);
        if !has_prior_messages {
            crate::hooks::fire_hooks(
                &session_config.hooks,
                crate::hooks::HookTrigger::OnSessionStart,
                &working_dir,
                &[],
            );
        }

        // Load skill registry for system prompt injection
        let skill_dirs = session_config.skill_dirs.clone();
        let skill_registry = crate::skill::SkillRegistry::load(&working_dir, &skill_dirs);
        let (git_status, readme, agents_md, file_tree) =
            crate::agent::collect_prompt_context(&working_dir).await;
        let mut system_prompt = crate::agent::build_system_prompt_with_storage(
            agent,
            &working_dir,
            &file_tree,
            Some(&skill_registry),
            Some(&git_status),
            Some(&readme),
            Some(&agents_md),
            Some(self.session_manager.storage()),
            Some(&session_config.memory),
        ); // Inject a tool reference listing so the model knows the exact tool names.
        // This is critical for models (especially via Ollama) that may hallucinate
        // tool names like "search" instead of the actual "grep" tool.
        let tool_reference = build_tool_reference_section(&self.tool_registry);
        system_prompt.push_str(&tool_reference);

        // Inject LSP guidance only for the servers that are actually connected.
        // This avoids telling the model it can use rust-analyzer when none is running.
        if let Some(lsp) = self.lsp_manager.get() {
            let lsp_guidance = build_lsp_guidance_section(lsp).await;
            system_prompt.push_str(&lsp_guidance);
        }

        // Inject codebase index guidance. When the index is active, emit strong
        // directives to use codeindex over grep/search. When disabled, inform the
        // model that codeindex tools will return "not available" and suggest grep.
        let code_index_active = self.code_index.get().is_some();
        if code_index_active {
            system_prompt.push_str(&build_codeindex_guidance_section_active());
        } else {
            system_prompt.push_str(&build_codeindex_guidance_section_disabled());
        }

        if matches!(model_ref.provider_id.as_str(), "ollama" | "ollama_cloud") {
            system_prompt.push_str(OLLAMA_TOOL_GUIDANCE);
        }

        // Inject team-lead task distribution guidelines when this session is
        // running as a team lead.  These rules help the LLM spawn a consistent
        // number of teammates and avoid overloading a single teammate with an
        // unbounded list of items — which causes context-window overflows.
        if team_context_for_session
            .as_deref()
            .is_some_and(|tc| tc.is_lead)
        {
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
        } else if team_context_for_session.is_some() {
            // Inject teammate workflow guidelines when this session is running as a teammate.
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

        // 4. Build chat messages from history with context window awareness
        let history = self.session_manager.get_messages(session_id)?;

        // Get model's context window size for potential compaction
        let context_window = self
            .provider_registry
            .get(&model_ref.provider_id)
            .map(|p| {
                p.default_models()
                    .into_iter()
                    .find(|m| m.id == model_ref.model_id)
            })
            .flatten()
            .map(|m| m.context_window)
            .unwrap_or(128_000); // Default fallback                            
        // Compact history if needed to prevent context window overflow
        // This ensures tool calls are atomic and not split across boundaries
        let compacted_history = compact_history_with_atomic_tool_calls(
            &history,
            context_window,
            8192, // Default max output tokens
        );

        let mut chat_messages = history_to_chat_messages(&compacted_history); // 4b. AGENTS.md init exchange — on the first message of a session,
        // prompt the model to acknowledge project guidelines so its output
        // appears in the message window.
        // Note: history already contains the user message we just stored,
        // so we check for the absence of any assistant messages instead.
        // The init exchange is display-only: it streams to the TUI but is
        // NOT added to chat_messages so the actual LLM call isn't confused.
        // Skip for subagent/teammate sessions — the guidelines are already
        // embedded in the system prompt and the extra LLM round-trip adds
        // significant latency to team operations.
        let has_tools = agent.max_steps.is_none_or(|s| s > 1);
        let has_prior_exchange = history.iter().any(|m| m.role == Role::Assistant);
        let is_subagent = agent.mode == crate::agent::AgentMode::Subagent;
        if !has_prior_exchange && has_tools && !is_subagent {
            let agents_md_path = working_dir.join("AGENTS.md");
            if agents_md_path.is_file() {
                let init_text = "AGENTS.md project guidelines have been loaded. \
                                 Please acknowledge them briefly.";

                // Only send the init prompt — exclude the user's real message
                // so the model doesn't try to answer it without tools.
                let init_messages = vec![ChatMessage {
                    role: "user".to_string(),
                    content: ChatContent::Text(init_text.to_string()),
                }];

                let init_request = ChatRequest {
                    model: model_ref.model_id.clone(),
                    messages: init_messages,
                    tools: Vec::new(),
                    temperature: agent.temperature,
                    top_p: agent.top_p,
                    max_tokens: Some(200),
                    system: Some(system_prompt.clone()),
                    options: agent.options.clone(),
                    session_id: Some(session_id.to_string()),
                    request_id: Some(Uuid::new_v4().to_string()),
                    stream_timeout_secs: None,
                };

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
                        // Signal end of init message so the TUI separates it from
                        // the actual response.
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
                        // Still set force_new_message via the "init" MessageEnd so
                        // the main agent loop starts in a fresh message block.
                        self.event_bus.publish(Event::MessageEnd {
                            session_id: session_id.to_string(),
                            message_id: "init".to_string(),
                            reason: FinishReason::Stop,
                        });
                    }
                }
            }
        }

        // 5. Agent loop
        let max_steps = agent.max_steps.unwrap_or(500) as usize;
        // Reset step counter for this session so warnings are relative to this run.
        self.event_bus.set_step(session_id, 0);
        // Single-step agents (e.g. "chat") don't use tools — omit definitions
        // so providers aren't confused by unused tool schemas.
        let tool_definitions = if max_steps <= 1 {
            Vec::new()
        } else {
            self.tool_registry.definitions()
        };
        let mut assistant_parts: Vec<MessagePart> = Vec::new();
        let mut agent_switch_requested = false;
        let mut task_complete_requested = false;
        let mut task_completeness_nudged = false;

        // Timing tracking for the agent loop
        let total_start = Instant::now();
        let mut cumulative_model_wait_ms: u64 = 0;

        // Pre-create a placeholder assistant message so that partial progress
        // is visible in the output view (e.g. teammate inspection) even before
        // the agent loop finishes.  We update it incrementally after each step.
        let assistant_msg_id = {
            let placeholder = Message::new(session_id, Role::Assistant, vec![]);
            let id = placeholder.id.clone();
            self.storage_op(move |s| s.create_message(&placeholder))
                .await?;
            id
        };

        loop {
            self.event_bus
                .set_step(session_id, self.event_bus.current_step(session_id) + 1);
            let step = self.event_bus.current_step(session_id) as usize;
            if step > max_steps {
                warn!("Reached max steps ({}), stopping agent loop", max_steps);
                self.event_bus.publish(Event::AgentError {
                    session_id: session_id.to_string(),
                    error: format!("Reached maximum steps ({max_steps})"),
                });
                break;
            }

            // Check if the user cancelled (e.g. pressed ESC)
            if cancel_flag.load(Ordering::Relaxed) {
                warn!("Agent loop cancelled by user at step {}", step);
                // Calculate timing breakdown before returning
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
                // Save partial progress (update the pre-created placeholder).
                let mut assistant_msg = Message::new(session_id, Role::Assistant, assistant_parts);
                assistant_msg.id = assistant_msg_id;
                let cancelled_id = assistant_msg.id.clone();
                self.storage_op(move |s| s.update_message(&assistant_msg))
                    .await?;
                self.event_bus.publish(Event::MessageEnd {
                    session_id: session_id.to_string(),
                    message_id: cancelled_id,
                    reason: FinishReason::Cancelled,
                });
                // Return a fresh placeholder since assistant_msg was moved
                return Ok(Message::new(session_id, Role::Assistant, vec![]));
            }

            debug!("Agent loop step {}/{}", step, max_steps);

            // Log which tools are being sent with this request
            if !tool_definitions.is_empty() {
                let tool_names: Vec<String> =
                    tool_definitions.iter().map(|t| t.name.clone()).collect();
                self.event_bus.publish(Event::ToolsSent {
                    session_id: session_id.to_string(),
                    tools: tool_names,
                });
            }

            // Call LLM with retry on transient failures (connection errors, stream stalls)
            let max_retries = self.stream_config.max_retries;
            let backoff_secs = self.stream_config.retry_backoff_secs;
            let llm_request_start = std::time::Instant::now();
            let mut text_buffer = String::new();
            let mut reasoning_buffer = String::new();
            let mut tool_calls: Vec<PendingToolCall> = Vec::new();
            let mut _finish_reason = FinishReason::Stop;
            let mut last_input_tokens: u64 = 0;
            let mut last_output_tokens: u64 = 0;

            'retry: for attempt in 0..=max_retries {
                if attempt > 0 {
                    // Reset buffers for retry
                    text_buffer.clear();
                    reasoning_buffer.clear();
                    tool_calls.clear();
                    _finish_reason = FinishReason::Stop;
                    last_input_tokens = 0;
                    last_output_tokens = 0;

                    let wait_secs = attempt as u64 * backoff_secs;
                    self.event_bus.publish(Event::AgentError {
                        session_id: session_id.to_string(),
                        error: format!(
                            "Retrying LLM request (attempt {}/{}), waiting {}s...",
                            attempt + 1,
                            max_retries + 1,
                            wait_secs
                        ),
                    });
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                }

                // Build request (fresh for each attempt)
                let attempt_request = ChatRequest {
                    model: model_ref.model_id.clone(),
                    messages: chat_messages.clone(),
                    tools: tool_definitions.clone(),
                    temperature: agent.temperature,
                    top_p: agent.top_p,
                    max_tokens: None,
                    system: Some(system_prompt.clone()),
                    options: agent.options.clone(),
                    session_id: Some(session_id.to_string()),
                    request_id: Some(Uuid::new_v4().to_string()),
                    stream_timeout_secs: Some(self.stream_config.timeout_secs),
                };

                let mut stream = match client.chat(attempt_request).await {
                    Ok(s) => s,
                    Err(e) => {
                        debug!(
                            "LLM call failed (attempt {}): {}",
                            attempt + 1,
                            redact_secrets(&e.to_string())
                        );
                        if attempt < max_retries {
                            self.event_bus.publish(Event::AgentError {
                                session_id: session_id.to_string(),
                                error: format!("{} — will retry", e),
                            });
                            continue 'retry;
                        }
                        self.event_bus.publish(Event::AgentError {
                            session_id: session_id.to_string(),
                            error: e.to_string(),
                        });
                        let error_msg = e.to_string();
                        crate::hooks::fire_hooks(
                            &session_config.hooks,
                            crate::hooks::HookTrigger::OnError,
                            &working_dir,
                            &[("RAGENT_ERROR", &error_msg)],
                        );
                        bail!("LLM call failed after {} attempts: {e}", max_retries + 1);
                    }
                };

                let mut had_retryable_error = false;
                while let Some(event) = stream.next().await {
                    match event {
                        StreamEvent::TextDelta { text } => {
                            self.event_bus.publish(Event::TextDelta {
                                session_id: session_id.to_string(),
                                text: text.clone(),
                            });
                            text_buffer.push_str(&text);
                        }
                        StreamEvent::ReasoningStart => {}
                        StreamEvent::ReasoningDelta { text } => {
                            self.event_bus.publish(Event::ReasoningDelta {
                                session_id: session_id.to_string(),
                                text: text.clone(),
                            });
                            reasoning_buffer.push_str(&text);
                        }
                        StreamEvent::ReasoningEnd => {}
                        StreamEvent::ToolCallStart { id, name } => {
                            self.event_bus.publish(Event::ToolCallStart {
                                session_id: session_id.to_string(),
                                call_id: id.clone(),
                                tool: name.clone(),
                            });
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
                            if let Some(tc) = tool_calls.iter().find(|t| t.id == id) {
                                self.event_bus.publish(Event::ToolCallArgs {
                                    session_id: session_id.to_string(),
                                    call_id: tc.id.clone(),
                                    tool: tc.name.clone(),
                                    args: tc.args_json.clone(),
                                });
                            }
                        }
                        StreamEvent::Usage {
                            input_tokens,
                            output_tokens,
                        } => {
                            last_input_tokens = input_tokens;
                            last_output_tokens = output_tokens;
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
                                redact_secrets(&message)
                            );
                            let is_retryable = is_retryable_stream_error(&message);
                            if is_retryable && attempt < max_retries {
                                self.event_bus.publish(Event::AgentError {
                                    session_id: session_id.to_string(),
                                    error: format!("{} — will retry", message),
                                });
                                had_retryable_error = true;
                            } else {
                                self.event_bus.publish(Event::AgentError {
                                    session_id: session_id.to_string(),
                                    error: message,
                                });
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
                            _finish_reason = reason;
                        }
                    }
                }

                if had_retryable_error {
                    continue 'retry;
                }
                break;
            }

            // Collect parts from this turn
            if !reasoning_buffer.is_empty() {
                assistant_parts.push(MessagePart::Reasoning {
                    text: reasoning_buffer.clone(),
                });
            }
            if !text_buffer.is_empty() {
                // Log the model response text
                let response_preview = if text_buffer.len() > 200 {
                    let mut end = 200;
                    while end > 0 && !text_buffer.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}…", &text_buffer[..end])
                } else {
                    text_buffer.clone()
                };
                let model_elapsed_ms = llm_request_start.elapsed().as_millis() as u64;
                cumulative_model_wait_ms += model_elapsed_ms;
                self.event_bus.publish(Event::ModelResponse {
                    session_id: session_id.to_string(),
                    text: response_preview,
                    elapsed_ms: model_elapsed_ms,
                    input_tokens: last_input_tokens,
                    output_tokens: last_output_tokens,
                });
                assistant_parts.push(MessagePart::Text {
                    text: text_buffer.clone(),
                });
            }

            // Execute tool calls if any were emitted, regardless of finish_reason.
            // Some Ollama models send tool calls but set done_reason to "stop" rather
            // than "tool_calls", so we cannot rely on finish_reason alone.
            if tool_calls.is_empty() {
                // No tool calls — check whether an Ollama model wrote planning text
                // instead of calling a tool, and inject a nudge to make it act.
                let is_ollama = matches!(model_ref.provider_id.as_str(), "ollama" | "ollama_cloud");
                let trimmed_text = text_buffer.trim();
                // Detect "stall" responses: model output is only dots/whitespace,
                // indicating it was thinking out loud and didn't produce tool calls.
                let looks_like_stall = !trimmed_text.is_empty()
                    && !tool_definitions.is_empty()
                    && trimmed_text
                        .chars()
                        .all(|c| c == '.' || c == ' ' || c == '\n');
                let looks_like_planning = !text_buffer.is_empty()
                    && !tool_definitions.is_empty()
                    && (text_buffer.contains("Let me")
                        || text_buffer.contains("I'll")
                        || text_buffer.contains("I will")
                        || text_buffer.contains("I'm going to")
                        || text_buffer.contains("let me")
                        || text_buffer.contains("start by")
                        || text_buffer.contains("begin by")
                        || text_buffer.contains("First,")
                        || text_buffer.contains("First I")
                        || text_buffer.contains("exploring")
                        || text_buffer.contains("examine")
                        || text_buffer.contains("analyze"));
                // Only nudge on early steps to avoid infinite loops.
                // Stall responses (dots-only) are nudged for any provider; planning
                // text nudges are limited to Ollama which commonly narrates before acting.
                let should_nudge_stall = looks_like_stall && step <= 8;
                let should_nudge_planning = is_ollama && looks_like_planning && step <= 3;

                // Task completeness check: if the user requested a file output
                // (e.g. "create hugplan.md") but no write tool was used, nudge
                // the model to finish. This fires once, at any step, for all
                // providers, to catch cases where sub-agent results were consumed
                // but the model stopped without producing the requested artefact.
                let should_nudge_incomplete = !task_completeness_nudged
                    && !tool_definitions.is_empty()
                    && detect_incomplete_file_task(&user_msg.text_content(), &assistant_parts);

                if should_nudge_stall || should_nudge_planning || should_nudge_incomplete {
                    let reason = if should_nudge_stall {
                        "stall (dots-only output)"
                    } else if should_nudge_planning {
                        "planning text without tool calls"
                    } else {
                        task_completeness_nudged = true;
                        "task incomplete — file output requested but not created"
                    };
                    tracing::info!(
                        session_id = %session_id,
                        step,
                        "Model produced {reason} — injecting nudge to continue"
                    );
                    chat_messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: ChatContent::Text(text_buffer.clone()),
                    });
                    let nudge_text = if should_nudge_incomplete {
                        "You have not completed the requested task. The user asked \
                         you to create or write a file, but no file-writing tool \
                         was called. Please use the appropriate tool (e.g. \
                         write_file, create_file) to produce the requested output \
                         now."
                    } else {
                        "Please continue — use tool calls to proceed with the task. \
                         Do not output dots or placeholder text."
                    };
                    chat_messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: ChatContent::Text(nudge_text.to_string()),
                    });
                    text_buffer = String::new();
                    reasoning_buffer = String::new();
                    _finish_reason = FinishReason::Stop;
                    continue;
                }
                break;
            }

            // Build assistant message content for history
            let mut assistant_content_parts: Vec<ContentPart> = Vec::new();
            if !text_buffer.is_empty() {
                assistant_content_parts.push(ContentPart::Text {
                    text: text_buffer.clone(),
                });
            }

            // Execute tool calls in parallel, bounded by tool semaphore
            let mut tool_result_parts: Vec<ContentPart> = Vec::new();

            let mut futures = Vec::new();

            for tc in &tool_calls {
                let input: Value = serde_json::from_str(&tc.args_json).unwrap_or_else(|e| {
                    warn!(error = %e, args = %tc.args_json, "Failed to parse tool call arguments");
                    json!({})
                });

                assistant_content_parts.push(ContentPart::ToolUse {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input: input.clone(),
                });

                let tool_ctx = ToolContext {
                    session_id: session_id.to_string(),
                    working_dir: working_dir.clone(),
                    event_bus: self.event_bus.clone(),
                    storage: Some(self.session_manager.storage().clone()),
                    task_manager: self.task_manager.get().cloned(),
                    lsp_manager: self.lsp_manager.get().cloned(),
                    active_model: Some(model_ref.clone()),
                    team_context: team_context_for_session.clone(),
                    team_manager: self
                        .team_manager
                        .get()
                        .cloned()
                        .map(|tm| tm as Arc<dyn crate::tool::TeamManagerInterface>),
                    code_index: self.code_index.get().cloned(),
                };

                let tc_clone = tc.clone();
                let registry = self.tool_registry.clone();
                let event_bus = self.event_bus.clone();
                let event_bus_clone = self.event_bus.clone();
                let session_id_str = session_id.to_string();
                let hook_working_dir = working_dir.clone();
                let hook_configs = session_config.hooks.clone();
                let extraction_engine = self.extraction_engine.clone();
                let storage_clone = self.session_manager.storage().clone();

                // Spawn each tool execution as a future — the tool semaphore
                // inside the spawned task bounds concurrency.
                let fut = tokio::spawn(async move {
                    // Check PreToolUse hooks first
                    let pre_hook_result = crate::hooks::run_pre_tool_use_hooks(
                        &hook_configs,
                        &hook_working_dir,
                        &tc_clone.name,
                        &tc_clone.args_json,
                    );

                    // Apply hook result
                    let tool_input = match pre_hook_result {
                        crate::hooks::PreToolUseResult::Allow => {
                            // Hook approved - skip the UI prompt but still execute
                            serde_json::from_str(&tc_clone.args_json)
                                .unwrap_or_else(|_| serde_json::json!({}))
                        }
                        crate::hooks::PreToolUseResult::Deny { reason } => {
                            // Hook denied - return error without executing
                            tracing::info!(
                                tool = %tc_clone.name,
                                reason = %reason,
                                "PreToolUse hook denied tool execution"
                            );
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
                        crate::hooks::PreToolUseResult::ModifiedInput { input } => {
                            // Hook modified the input - use the modified input
                            tracing::debug!(
                                tool = %tc_clone.name,
                                "PreToolUse hook modified tool input"
                            );
                            input
                        }
                        crate::hooks::PreToolUseResult::NoDecision => {
                            // No hook decision - use original input
                            serde_json::from_str(&tc_clone.args_json)
                                .unwrap_or_else(|_| serde_json::json!({}))
                        }
                    };

                    let _permit = crate::resource::acquire_tool_permit()
                        .await
                        .map_err(|e| anyhow::anyhow!("tool permit: {e}"));
                    let start = Instant::now();

                    // D3 fix: Serialize tool_input before it's moved for PostToolUse hooks
                    let tool_input_for_post_hook = serde_json::to_string(&tool_input)
                        .unwrap_or_else(|_| tc_clone.args_json.clone());

                    let result = registry
                        .get(&tc_clone.name)
                        .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tc_clone.name));
                    let result = match result {
                        Ok(tool) => tool.execute(tool_input, &tool_ctx).await,
                        Err(e) => Err(e),
                    };
                    let duration_ms = start.elapsed().as_millis() as u64;

                    // Run PostToolUse hooks after execution
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

                    let modified_output = crate::hooks::run_post_tool_use_hooks(
                        &hook_configs,
                        &hook_working_dir,
                        &tc_clone.name,
                        &tool_input_for_post_hook, // Use modified input here
                        &output_json.to_string(),
                        success,
                    )
                    .await;

                    // If hook modified the output, create a new ToolOutput with the modified content
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
                            // Merge metadata into the output value so the
                            // renderer can access line counts, summaries, etc.
                            let val = match &output.metadata {
                                Some(meta) if meta.is_object() => {
                                    let mut obj = meta.clone();
                                    obj.as_object_mut()
                                        .unwrap()
                                        .insert("content".to_string(), json!(output.content));
                                    obj
                                }
                                _ => json!({ "content": output.content }),
                            };
                            (Some(val), None)
                        }
                        Err(e) => (None, Some(format!("{e:#}"))),
                    };

                    // Fire on_permission_denied hook when a tool returns a permission error
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

                    // Use metadata "lines" field when available (e.g. write/edit
                    // tools report the actual file line count there), otherwise
                    // fall back to counting lines in the result content.
                    let content_line_count = result
                        .as_ref()
                        .ok()
                        .and_then(|o| o.metadata.as_ref())
                        .and_then(|m| m.get("lines"))
                        .and_then(serde_json::Value::as_u64)
                        .map_or_else(|| result_content.lines().count(), |n| n as usize);

                    // Log the tool result (truncate at a char boundary)
                    let result_preview = if result_content.len() > 200 {
                        let end = result_content
                            .char_indices()
                            .map(|(i, _)| i)
                            .take_while(|&i| i <= 200)
                            .last()
                            .unwrap_or(0);
                        format!("{}…", &result_content[..end])
                    } else {
                        result_content.clone()
                    };
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

                    // ── Memory extraction hook ────────────��────────────
                    // After the tool result is processed, invoke the
                    // extraction engine (if initialised) to propose memory
                    // candidates from the tool usage.
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
                    } // Return all the info we need to reconstruct state
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

                futures.push(fut);
            }

            // Wait for all tool calls to complete (concurrency bounded by semaphore)
            let results = futures::future::join_all(futures).await;

            // Process results in order
            for result in results {
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
                        assistant_parts.push(MessagePart::ToolCall {
                            tool: tc.name.clone(),
                            call_id: tc.id.clone(),
                            state: ToolCallState {
                                status,
                                input,
                                output: output_value,
                                error,
                                duration_ms: Some(duration_ms),
                            },
                        });

                        tool_result_parts.push(ContentPart::ToolResult {
                            tool_use_id: tc.id.clone(),
                            content: result_content,
                        });

                        // Check if a tool requested an agent switch or restore
                        if let Some(meta) = tool_metadata.as_ref() {
                            if meta.get("agent_switch").is_some()
                                || meta.get("agent_restore").is_some()
                            {
                                agent_switch_requested = true;
                                break;
                            }
                            if meta.get("task_complete").is_some() {
                                task_complete_requested = true;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Tool execution task panicked");
                    }
                }
            }

            // If an agent switch or task completion was requested, exit the main loop too
            if agent_switch_requested || task_complete_requested {
                break;
            }

            // Add assistant message with tool uses to chat history
            chat_messages.push(ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Parts(assistant_content_parts),
            });

            // Add tool results to chat history
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(tool_result_parts),
            });

            // Inject completed background task results (F14 result injection)
            if let Some(tm) = self.task_manager.get() {
                let completed = tm.drain_completed(session_id).await;
                if !completed.is_empty() {
                    let mut bg_parts: Vec<ContentPart> = Vec::new();
                    for task in &completed {
                        let status_label = match task.status {
                            crate::task::TaskStatus::Completed => "completed",
                            crate::task::TaskStatus::Failed => "failed",
                            crate::task::TaskStatus::Cancelled => "cancelled",
                            crate::task::TaskStatus::Running => "running", // shouldn't happen
                        };
                        let body = task
                            .result
                            .as_deref()
                            .or(task.error.as_deref())
                            .unwrap_or("(no output)");
                        let text = format!(
                            "[Background Task {status_label}: {} — {}]\n\n{body}",
                            task.agent_name,
                            &task.id[..8.min(task.id.len())]
                        );
                        bg_parts.push(ContentPart::Text { text });
                    }
                    chat_messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: ChatContent::Parts(bg_parts),
                    });
                }
            }

            // Persist intermediate progress so that output inspectors (e.g.
            // the teammate output overlay) can show steps while the agent is
            // still running.  Fire-and-forget on a blocking thread.
            {
                let mut interim =
                    Message::new(session_id, Role::Assistant, assistant_parts.clone());
                interim.id = assistant_msg_id.clone();
                let _ = self.storage_op(move |s| s.update_message(&interim)).await;
            }
        }

        // 6. Final save of assistant message (update the pre-created placeholder).
        let mut assistant_msg = Message::new(session_id, Role::Assistant, assistant_parts);
        assistant_msg.id = assistant_msg_id;
        {
            let msg = assistant_msg.clone();
            self.storage_op(move |s| s.update_message(&msg)).await?;
        }

        // Calculate and log timing breakdown
        let total_elapsed_ms = total_start.elapsed().as_millis() as u64;
        let other_ms = total_elapsed_ms.saturating_sub(cumulative_model_wait_ms);
        tracing::info!(
            session_id = %session_id,
            total_ms = total_elapsed_ms,
            model_wait_ms = cumulative_model_wait_ms,
            other_ms = other_ms,
            "Agent loop finished - timing breakdown: total={}ms, model_wait={}ms, other={}ms",
            total_elapsed_ms,
            cumulative_model_wait_ms,
            other_ms
        );

        self.event_bus.publish(Event::MessageEnd {
            session_id: session_id.to_string(),
            message_id: assistant_msg.id.clone(),
            reason: FinishReason::Stop,
        });

        crate::hooks::fire_hooks(
            &session_config.hooks,
            crate::hooks::HookTrigger::OnSessionEnd,
            &working_dir,
            &[],
        );

        Ok(assistant_msg)
    }

    /// Run the AGENTS.md acknowledgement exchange for a session at startup.
    ///
    /// Checks whether AGENTS.md exists in the session's working directory and,
    /// if the session has no prior assistant messages, sends a lightweight
    /// "please acknowledge" prompt to the model and streams the reply to the
    /// TUI via [`Event::TextDelta`] events.  The acknowledgement text is then
    /// saved to the session history so that subsequent calls to
    /// [`process_user_message`] skip the init exchange (via the
    /// `has_prior_exchange` guard).
    ///
    /// Errors are non-fatal: if the provider is not configured or the API call
    /// fails, the function returns `Ok(())` and logs a warning.
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

        let agents_md_path = working_dir.join("AGENTS.md");
        if !agents_md_path.is_file() {
            return Ok(());
        }

        // Skip if an assistant message already exists (init already ran).
        let already_done = self
            .session_manager
            .get_messages(session_id)
            .map(|msgs| msgs.iter().any(|m| m.role == Role::Assistant))
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
        let (git_status, readme, agents_md, file_tree) =
            crate::agent::collect_prompt_context(&working_dir).await;
        let run_init_config = crate::config::Config::load().unwrap_or_default();
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
        let init_text = "AGENTS.md project guidelines have been loaded. \
                         Please acknowledge them briefly.";
        let init_messages = vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(init_text.to_string()),
        }];
        let init_request = ChatRequest {
            model: model_ref.model_id.clone(),
            messages: init_messages,
            tools: Vec::new(),
            temperature: agent.temperature,
            top_p: agent.top_p,
            max_tokens: Some(200),
            system: Some(system_prompt),
            options: agent.options.clone(),
            session_id: Some(session_id.to_string()),
            request_id: Some(Uuid::new_v4().to_string()),
            stream_timeout_secs: None,
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
                            self.event_bus.publish(Event::TextDelta {
                                session_id: session_id.to_string(),
                                text,
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
            let init_user_text = "AGENTS.md project guidelines have been loaded. \
                                  Please acknowledge them briefly.";
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

    async fn resolve_api_key(&self, provider_id: &str) -> Result<String> {
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

        // Check common environment variable names
        let env_vars = match provider_id {
            "anthropic" => vec!["ANTHROPIC_API_KEY"],
            "openai" => vec!["OPENAI_API_KEY"],
            "gemini" => vec!["GEMINI_API_KEY"],
            "huggingface" => vec!["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN"],
            "generic_openai" => vec!["OPENAI_API_KEY", "GENERIC_OPENAI_API_KEY"],
            "ollama_cloud" => vec!["OLLAMA_API_KEY"],
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

#[derive(Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    args_json: String,
}

/// Resolve team identity for the given `session_id`, if that session currently
/// participates in a team as lead or teammate.
fn resolve_team_context_for_session(
    session_id: &str,
    working_dir: &std::path::Path,
) -> Option<Arc<TeamContext>> {
    for (_name, dir, _) in crate::team::TeamStore::list_teams(working_dir) {
        let Ok(store) = crate::team::TeamStore::load(&dir) else {
            continue;
        };
        if store.config.status != crate::team::TeamStatus::Active {
            continue;
        }
        if store.config.lead_session_id == session_id {
            return Some(Arc::new(TeamContext {
                team_name: store.config.name,
                agent_id: "lead".to_string(),
                is_lead: true,
            }));
        }
        if let Some(member) = store
            .config
            .members
            .iter()
            .find(|m| m.session_id.as_deref() == Some(session_id))
        {
            return Some(Arc::new(TeamContext {
                team_name: store.config.name.clone(),
                agent_id: member.agent_id.clone(),
                is_lead: false,
            }));
        }
    }
    None
}

fn history_to_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    let mut chat_messages = Vec::new();

    for msg in messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        let content = if msg.parts.len() == 1 {
            match &msg.parts[0] {
                MessagePart::Text { text } => ChatContent::Text(text.clone()),
                // Image parts must go through Parts() to get the image_url block.
                MessagePart::Image { .. } => parts_to_chat_content(&msg.parts),
                _ => parts_to_chat_content(&msg.parts),
            }
        } else {
            parts_to_chat_content(&msg.parts)
        };

        chat_messages.push(ChatMessage {
            role: role.to_string(),
            content,
        });

        // If this assistant message contains tool calls, emit a follow-up
        // user message with the corresponding tool results so the LLM sees
        // matching tool_use / tool_result pairs.
        if msg.role == Role::Assistant {
            let tool_results: Vec<ContentPart> = msg
                .parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::ToolCall { call_id, state, .. } => {
                        let result_text = state
                            .output
                            .as_ref()
                            .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                            .or_else(|| state.error.clone())
                            .unwrap_or_default();
                        Some(ContentPart::ToolResult {
                            tool_use_id: call_id.clone(),
                            content: result_text,
                        })
                    }
                    _ => None,
                })
                .collect();

            if !tool_results.is_empty() {
                chat_messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: ChatContent::Parts(tool_results),
                });
            }
        }
    }

    chat_messages
}

/// Compacts message history to fit within a context window while ensuring
/// tool call atomicity - tool calls and their results are never split across
/// compaction boundaries.
///
/// This function trims messages from the beginning (oldest first) until the
/// estimated token count is below the threshold. It ensures that when a
/// message containing tool calls is retained, the corresponding tool result
/// messages are also retained, preventing incomplete tool call pairs.
fn compact_history_with_atomic_tool_calls(
    messages: &[Message],
    context_window: usize,
    _max_output_tokens: usize,
) -> Vec<Message> {
    if messages.is_empty() {
        return Vec::new();
    }

    // Rough token estimation: ~4 chars per token on average
    const CHARS_PER_TOKEN: usize = 4;

    // Calculate estimated tokens for a message
    let estimate_tokens = |msg: &Message| -> usize {
        let text_len: usize = msg
            .parts
            .iter()
            .map(|p| match p {
                MessagePart::Text { text } => text.len(),
                MessagePart::ToolCall { tool, state, .. } => {
                    tool.len()
                        + state
                            .output
                            .as_ref()
                            .and_then(|v| v.as_str())
                            .map(|s| s.len())
                            .unwrap_or(0)
                        + state.error.as_ref().map(|s| s.len()).unwrap_or(0)
                }
                MessagePart::Image { .. } => 1000, // Rough estimate for image
                MessagePart::Reasoning { text } => text.len(),
            })
            .sum();
        text_len / CHARS_PER_TOKEN + 10 // Base overhead per message
    };
    // Build a map of tool call IDs to their result indices
    let mut tool_call_indices: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (idx, msg) in messages.iter().enumerate() {
        if msg.role == Role::Assistant {
            for part in &msg.parts {
                if let MessagePart::ToolCall { call_id, .. } = part {
                    tool_call_indices.insert(call_id.clone(), idx);
                }
            }
        }
    }

    // Calculate total estimated tokens
    let total_tokens: usize = messages.iter().map(estimate_tokens).sum();
    let max_tokens = context_window.saturating_sub(1000); // Leave headroom

    if total_tokens <= max_tokens {
        return messages.to_vec();
    }

    // Need to compact - identify which messages must be kept together
    // Messages with tool calls must keep their corresponding results
    let mut must_keep: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for (idx, msg) in messages.iter().enumerate() {
        if msg.role == Role::Assistant {
            let has_tool_calls = msg
                .parts
                .iter()
                .any(|p| matches!(p, MessagePart::ToolCall { .. }));
            if has_tool_calls {
                must_keep.insert(idx);
                // The result is in the user message that follows
                if idx + 1 < messages.len() {
                    must_keep.insert(idx + 1);
                }
            }
        }
    }

    // Trim from the beginning, respecting atomic groups
    let mut trimmed = messages.to_vec();
    let mut current_tokens: usize = total_tokens;

    while current_tokens > max_tokens && trimmed.len() > 2 {
        // Always keep at least the last 2 messages (user query + context)
        let to_remove = 0; // Try removing the oldest message

        // Check if removing this would break atomicity
        let would_break_atomicity = must_keep.contains(&to_remove)
            || (to_remove > 0 && must_keep.contains(&(to_remove - 1)));

        if would_break_atomicity {
            // Skip this message and try the next
            break;
        }

        let removed_tokens = estimate_tokens(&trimmed[to_remove]);
        trimmed.remove(to_remove);

        // Update indices in must_keep
        let mut new_must_keep: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for &idx in &must_keep {
            if idx > to_remove {
                new_must_keep.insert(idx - 1);
            } else if idx != to_remove {
                new_must_keep.insert(idx);
            }
        }
        must_keep = new_must_keep;

        current_tokens = current_tokens.saturating_sub(removed_tokens);
    }

    tracing::debug!(
        original_count = messages.len(),
        trimmed_count = trimmed.len(),
        original_tokens = total_tokens,
        final_tokens = current_tokens,
        "Compacted message history"
    );

    trimmed
}

/// Determines whether a stream error message represents a transient failure
/// that should be retried rather than treated as fatal.
///
/// Retryable errors include stream stalls, body decoding failures, connection
/// resets, and protocol errors that are typically caused by transient network
/// conditions.
fn is_retryable_stream_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("stall")
        || lower.contains("error decoding response body")
        || lower.contains("connection reset")
        || lower.contains("connection closed")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
        || lower.contains("incomplete message")
        || lower.contains("stream ended unexpectedly")
        || lower.contains("h2 protocol error")
        || lower.contains("http2 error")
}

fn parts_to_chat_content(parts: &[MessagePart]) -> ChatContent {
    let content_parts: Vec<ContentPart> = parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => Some(ContentPart::Text { text: text.clone() }),
            MessagePart::ToolCall {
                tool,
                call_id,
                state,
            } => Some(ContentPart::ToolUse {
                id: call_id.clone(),
                name: tool.clone(),
                input: state.input.clone(),
            }),
            MessagePart::Reasoning { .. } => None,
            MessagePart::Image { mime_type, path } => {
                // Read the file and encode as a base64 data URI.
                match std::fs::read(path) {
                    Ok(bytes) => {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        Some(ContentPart::ImageUrl {
                            url: format!("data:{mime_type};base64,{b64}"),
                        })
                    }
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "failed to read image attachment");
                        None
                    }
                }
            }
        })
        .collect();
    ChatContent::Parts(content_parts)
}

/// Detects whether the user's original message requested file creation or
/// writing, and whether any file-writing tool was actually executed during
/// the session so far.
///
/// Returns `true` when the user appears to have asked for a file to be
/// created/written but no write tool has been called — indicating the task
/// is incomplete.
///
/// This is intentionally conservative: it only triggers on clear
/// verb+filename patterns and only checks for common file-output tools.
pub fn detect_incomplete_file_task(user_text: &str, assistant_parts: &[MessagePart]) -> bool {
    let lower = user_text.to_lowercase();

    // 1. Check if user message contains a file-output request.
    //    Look for action verbs near file-like tokens (word.ext patterns).
    let has_file_action_verb = lower.contains("create ")
        || lower.contains("produce ")
        || lower.contains("write ")
        || lower.contains("generate ")
        || lower.contains("make ")
        || lower.contains("save ")
        || lower.contains("output ");

    if !has_file_action_verb {
        return false;
    }

    // Look for something that resembles a filename (word.ext) in the user text.
    let has_filename = user_text.split_whitespace().any(|word| {
        let cleaned = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '.' && c != '_' && c != '-' && c != '/'
        });
        if let Some(dot_pos) = cleaned.rfind('.') {
            let ext = &cleaned[dot_pos + 1..];
            // Extension must be 1-10 alphanumeric chars and have content before the dot
            dot_pos > 0
                && !ext.is_empty()
                && ext.len() <= 10
                && ext.chars().all(|c| c.is_alphanumeric())
        } else {
            false
        }
    });

    if !has_filename {
        return false;
    }

    // 2. Check if any file-writing tool was executed in assistant_parts.
    let write_tools = [
        "write_file",
        "create_file",
        "write_new_file",
        "edit_file",
        "patch_file",
        "append_file",
        "save_file",
    ];

    let has_write_tool = assistant_parts.iter().any(|part| {
        if let MessagePart::ToolCall { tool, .. } = part {
            write_tools.iter().any(|w| tool == w)
        } else {
            false
        }
    });

    // Incomplete if user asked for file creation but no write tool was used.
    !has_write_tool
}
