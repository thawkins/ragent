//! ragent CLI binary.
//!
//! Entry point for the ragent terminal AI coding agent. Parses CLI arguments,
//! loads configuration, initialises storage and the event bus, and dispatches
//! to the requested sub-command (TUI, headless run, HTTP server, session
//! management, auth, or config display).

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ragent_agent as ragent_core;
use tracing_subscriber::EnvFilter;

use ragent_core::{
    agent,
    config::Config,
    event::EventBus,
    memory::BlockStorage,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};

/// small CLI demo for orchestration
///
/// # Errors
///
/// Returns an error if job execution fails.
async fn run_orchestration_example() -> anyhow::Result<()> {
    tracing::info!("Running orchestration example");
    let registry = ragent_core::orchestrator::AgentRegistry::new();

    use futures::future::FutureExt;
    use ragent_core::orchestrator::{Coordinator, JobDescriptor, Responder};
    use std::sync::Arc;
    use tokio::time::Duration;
    use tokio::time::sleep;

    let responder_a: Responder =
        Arc::new(|payload: String| async move { format!("demo-a: {payload}") }.boxed());
    let responder_b: Responder = Arc::new(|payload: String| {
        async move {
            sleep(Duration::from_millis(30)).await;
            format!("demo-b: {payload}")
        }
        .boxed()
    });

    registry
        .register("demo-a", vec!["demo".to_string()], Some(responder_a))
        .await;
    registry
        .register("demo-b", vec!["demo".to_string()], Some(responder_b))
        .await;

    let coord = Coordinator::new(registry.clone());
    let desc = JobDescriptor {
        id: "demo-job".to_string(),
        required_capabilities: vec!["demo".to_string()],
        payload: "payload".to_string(),
    };

    let res = coord.start_job_sync(desc).await?;
    println!("Orchestration sync result:\n{res}");

    Ok(())
}

/// Top-level CLI arguments parsed by clap.
#[derive(Parser)]
#[command(name = "ragent", about = "An Rust AI coding agent for the terminal")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Override model (provider/model format)
    #[arg(long, global = true)]
    model: Option<String>,

    /// Override agent
    #[arg(long, global = true, default_value = "general")]
    agent: String,

    /// Log level
    #[arg(long, global = true, default_value = "warn")]
    log_level: String,

    /// Disable TUI, use plain stdout
    #[arg(long, global = true)]
    no_tui: bool,

    /// Auto-approve all permissions
    #[arg(long, alias = "no-prompt", global = true)]
    yes: bool,

    /// Show the log panel in the TUI
    #[arg(long, global = true)]
    log: bool,

    /// Path to config file
    #[arg(long, global = true)]
    config: Option<String>,

    /// Maximum number of agentic loop steps (default: 500)
    #[arg(long, global = true)]
    maxsteps: Option<u32>,

    /// Disable automatic git context injection
    #[arg(long, global = true)]
    no_git_context: bool,

    /// Disable automatic README context injection
    #[arg(long, global = true)]
    no_readme_context: bool,
}

/// Available top-level sub-commands.
#[derive(Subcommand)]
enum Commands {
    /// Execute agent with prompt
    Run {
        /// The prompt to send
        prompt: String,
    },
    /// Start HTTP server only
    Serve {
        /// Address to bind to
        #[arg(long, default_value = "127.0.0.1:3000")]
        addr: String,
    },
    /// Run a small orchestration example (demonstrates multi-agent coordinator)
    Orchestrate,
    /// Manage sessions
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// Manage persistent memory (export, import, list)
    Memory {
        #[command(subcommand)]
        command: MemoryCommands,
    },
    /// Configure provider authentication
    Auth {
        /// Provider name
        provider: String,
        /// API key
        key: String,
    },
    /// List available models
    Models {
        /// Filter by provider (e.g. "ollama", "openai", "anthropic")
        #[arg(short, long)]
        provider: Option<String>,
        /// Discover models from a remote Ollama server URL
        #[arg(long)]
        ollama_url: Option<String>,
    },
    /// Show resolved configuration
    Config,
}

/// Sub-commands for the `session` namespace.
#[derive(Subcommand)]
enum SessionCommands {
    /// List all sessions
    List,
    /// Resume an existing session
    Resume {
        /// Session ID
        id: String,
    },
    /// Export a session
    Export {
        /// Session ID
        id: String,
    },
    /// Import a session from file
    Import {
        /// Path to session file
        file: String,
    },
}

/// Sub-commands for the `memory` namespace.
#[derive(Subcommand)]
enum MemoryCommands {
    /// Export structured memories and blocks to JSON on stdout
    Export,
    /// Import memories from a JSON file or external format
    Import {
        /// Path to import file or directory
        path: String,
        /// Input format: "ragent" (default), "cline", or "claude-code"
        #[arg(long, default_value = "ragent")]
        format: String,
        /// Preview import without writing (dry run)
        #[arg(long)]
        dry_run: bool,
    },
    /// List all memory blocks and structured memory statistics
    List,
}

/// Return the platform data directory for ragent (e.g. `~/.local/share/ragent`).
fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ragent")
}

/// Parse CLI args, set up infrastructure, and dispatch to the selected command.
///
/// # Errors
///
/// Returns an error on configuration, storage, network, or I/O failures.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_git_context {
        ragent_core::agent::disable_git_prompt_context();
    }
    if cli.no_readme_context {
        ragent_core::agent::disable_readme_prompt_context();
    }

    // Initialize tracing.
    //
    // For TUI mode: install a channel-based layer so all tracing output is
    // captured by the log panel rather than written directly to stdout (which
    // would corrupt the alternate-screen rendering).
    //
    // For non-TUI modes (--no-tui, headless run, server, etc.): fall back to
    // the normal fmt subscriber so logs appear in the terminal as usual.
    let filter = EnvFilter::try_new(&cli.log_level).unwrap_or_else(|_| EnvFilter::new("warn"));
    let tui_will_run = !cli.no_tui
        && matches!(
            cli.command,
            None | Some(Commands::Session {
                command: SessionCommands::Resume { .. }
            })
        );
    let tui_log_rx = if tui_will_run {
        use tracing_subscriber::prelude::*;
        let (tx, rx) = ragent_tui::tracing_layer::tui_log_channel(512);
        tracing_subscriber::registry()
            .with(filter)
            .with(ragent_tui::tracing_layer::TuiTracingLayer::new(tx))
            .init();
        Some(rx)
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();
        None
    };
    tracing::info!(log_level = %cli.log_level, tui_mode = tui_will_run, "Tracing initialized");

    // Load config
    let config = if let Some(ref path_str) = cli.config {
        let path = PathBuf::from(path_str);
        tracing::info!(config_path = %path.display(), "Loading config from file");
        let content = std::fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!("Failed to read config file '{}': {}", path.display(), e)
        })?;

        serde_json::from_str(&content).map_err(|e| {
            let line = e.line();
            let column = e.column();
            let problematic_line = content
                .lines()
                .nth(line.saturating_sub(1))
                .unwrap_or("<line not found>");

            anyhow::anyhow!(
                "Failed to parse config file '{}':\n\
                           Error at line {}, column {}:\n\
                           {}\n\
                           Problematic line:\n\
                           {}\n\
                           {}^\n\
                           Parse error: {}",
                path.display(),
                line,
                column,
                "─".repeat(80),
                problematic_line,
                " ".repeat(column.saturating_sub(1)),
                e
            )
        })?
    } else {
        Config::load()?
    };
    tracing::info!("Configuration loaded successfully");

    let internal_llm_service =
        ragent_core::internal_llm::InternalLlmService::from_config(config.internal_llm.clone())?
            .map(Arc::new);
    let auto_extract_config = config.memory.auto_extract.clone();
    tracing::debug!(
        internal_llm_enabled = internal_llm_service.is_some(),
        "Internal LLM service initialized"
    );
    if let Some(service) = &internal_llm_service {
        let snapshot = service.status_snapshot();
        if let Some(runtime) = snapshot.runtime {
            tracing::info!(
                model_id = %snapshot.model_id,
                backend = %snapshot.backend,
                lifecycle = ?runtime.lifecycle,
                execution_device = %runtime.settings.execution_device,
                quantized_runtime = %runtime.settings.quantized_runtime,
                requested_threads = runtime.settings.requested_threads,
                effective_threads = runtime.settings.effective_threads,
                requested_gpu_layers = runtime.settings.requested_gpu_layers,
                effective_gpu_layers = runtime.settings.effective_gpu_layers,
                gpu_offload = %runtime.settings.gpu_offload,
                threading = %runtime.settings.threading,
                "Internal LLM runtime settings"
            );
            if runtime.settings.requested_gpu_layers > runtime.settings.effective_gpu_layers {
                tracing::warn!(
                    requested_gpu_layers = runtime.settings.requested_gpu_layers,
                    effective_gpu_layers = runtime.settings.effective_gpu_layers,
                    gpu_offload = %runtime.settings.gpu_offload,
                    "Internal LLM gpu_layers setting is not supported by the current runtime"
                );
            }
        }
    }

    // Initialize storage
    let db_path = data_dir().join("ragent.db");
    tracing::info!(db_path = %db_path.display(), "Opening database");
    let storage = Arc::new(Storage::open(&db_path)?);
    tracing::info!("Storage initialized successfully");

    // Seed the secret registry from stored provider credentials so that
    // redact_secrets() can mask them by exact match in all log output.
    if let Err(e) = storage.seed_secret_registry() {
        tracing::warn!(error = %e, "Failed to seed secret registry from database");
    }

    // Also seed from well-known environment variables.
    for var in [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GENERIC_OPENAI_API_KEY",
        "OLLAMA_API_KEY",
    ] {
        if let Ok(val) = std::env::var(var) {
            ragent_core::sanitize::register_secret(&val);
        }
    }
    // Create event bus
    let event_bus = Arc::new(EventBus::new(2048));
    tracing::debug!(capacity = 2048, "Event bus created");

    // Create registries
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let provider_count = provider_registry.list().len();
    let tool_count = tool_registry.list().len();
    tracing::info!(
        providers = provider_count,
        tools = tool_count,
        "Registries created"
    );

    let hidden_tools = config.effective_hidden_tools();
    if !hidden_tools.is_empty() {
        tracing::debug!(hidden_tools = ?hidden_tools, "Hiding tools from registry");
        tool_registry.set_hidden(&hidden_tools);
    }
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(
        config
            .permission
            .clone()
            .into_iter()
            .map(Into::into)
            .collect(),
    )));

    // Resolve the active agent
    let agent_name = &cli.agent;
    tracing::info!(agent = %agent_name, "Resolving agent");
    let mut resolved_agent = agent::resolve_agent(agent_name, &config)?;
    tracing::info!(agent = %resolved_agent.name, model = ?resolved_agent.model, "Agent resolved");

    // Apply CLI --maxsteps override if provided
    if let Some(max) = cli.maxsteps {
        resolved_agent.max_steps = Some(max);
    }

    // Apply model selection with priority:
    //   1. --model CLI flag (provider/model format)
    //   2. storage selected_model (saved by TUI /provider or /model command)
    //   3. agent built-in default (e.g. anthropic/claude-sonnet-4)
    //
    // Skip override when the agent has model_pinned=true (custom agents that
    // explicitly fix a model should not be overridden by global selection).
    if let Some(ref model_str) = cli.model {
        if let Some((provider, model)) = model_str.split_once('/') {
            resolved_agent.model = Some(agent::ModelRef {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            });
            resolved_agent.model_pinned = true;
        } else {
            anyhow::bail!(
                "Invalid --model format '{model_str}'. Expected 'provider/model' (e.g. 'copilot/claude-sonnet-4.5')"
            );
        }
    } else if !resolved_agent.model_pinned || resolved_agent.model.is_none() {
        // Fall back to the user's stored provider/model preference
        if let Ok(Some(model_str)) = storage.get_setting("selected_model")
            && let Some((provider, model)) = model_str.split_once('/')
        {
            resolved_agent.model = Some(agent::ModelRef {
                provider_id: provider.to_string(),
                model_id: model.to_string(),
            });
        }
    }

    let max_background_agents = config.experimental.max_background_agents;
    let stream_config = config.stream.clone();

    let config = Arc::new(tokio::sync::RwLock::new(config));

    // Create session manager and processor
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    tracing::debug!("Session manager created");
    let session_processor = Arc::new(SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry: provider_registry.clone(),
        tool_registry: tool_registry.clone(),
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config,
        auto_approve: cli.yes,
    });
    tracing::info!(auto_approve = cli.yes, "Session processor initialized");

    if auto_extract_config.enabled {
        let extraction_engine = Arc::new(ragent_core::memory::ExtractionEngine::with_internal_llm(
            auto_extract_config,
            internal_llm_service.clone(),
        ));
        let _ = session_processor.extraction_engine.set(extraction_engine);
    }

    // Create TaskManager and wire it into the processor (breaks circular dep via OnceLock)
    let task_manager = Arc::new(ragent_core::task::TaskManager::new(
        event_bus.clone(),
        session_processor.clone(),
        max_background_agents,
    ));
    let _ = session_processor.task_manager.set(task_manager);
    tracing::debug!(max_background_agents, "Task manager initialized");

    // Connect MCP servers from config and register their tools into the tool registry.
    let mcp_server_count = config.read().await.mcp.len();
    if mcp_server_count > 0 {
        tracing::info!(mcp_servers = mcp_server_count, "Connecting MCP servers");
        let mcp_configs: Vec<(String, ragent_core::config::McpServerConfig)> = config
            .read()
            .await
            .mcp
            .iter()
            .map(|(id, cfg)| (id.clone(), cfg.clone()))
            .collect();

        let mut mcp_client = ragent_core::mcp::McpClient::new();
        let mut mcp_connected = 0u32;
        for (id, cfg) in mcp_configs {
            if let Err(e) = mcp_client.connect(&id, cfg).await {
                tracing::warn!(server_id = %id, error = %e, "MCP server connection failed at startup");
            } else {
                mcp_connected += 1;
            }
        }
        let shared_client = Arc::new(tokio::sync::RwLock::new(mcp_client));
        session_processor.set_mcp_client(shared_client).await;
        tracing::info!(
            connected = mcp_connected,
            total = mcp_server_count,
            "MCP servers initialized"
        );
    }

    match cli.command {
        None => {
            // Default: run TUI
            if cli.no_tui {
                tracing::info!("Starting interactive mode (plain, no TUI)");
                use tokio::io::AsyncBufReadExt;

                let mut resolved_agent = resolved_agent.clone();
                let config_guard = config.read().await;
                agent::apply_fallback_thinking(
                    &mut resolved_agent,
                    &config_guard,
                    provider_registry.as_ref(),
                );

                tracing::info!("Starting ragent interactive mode (plain)");
                let dir = std::fs::canonicalize(".")?;
                let session = session_manager.create_session(dir)?;
                let reader = tokio::io::BufReader::new(tokio::io::stdin());
                let mut lines = reader.lines();
                let mut stdout = std::io::stdout().lock();
                while let Some(line) = lines.next_line().await? {
                    if line.is_empty() {
                        continue;
                    }
                    match session_processor
                        .process_message(
                            &session.id,
                            &line,
                            &resolved_agent,
                            Arc::new(AtomicBool::new(false)),
                        )
                        .await
                    {
                        Ok(msg) => writeln!(stdout, "{}", msg.text_content())?,
                        Err(e) => tracing::error!(error = %e, "Failed to process message"),
                    }
                }
            } else {
                ragent_tui::run_tui(
                    event_bus,
                    storage,
                    provider_registry,
                    session_processor,
                    resolved_agent.clone(),
                    cli.log,
                    None,
                    tui_log_rx.unwrap_or_else(|| ragent_tui::tracing_layer::tui_log_channel(1).1),
                )
                .await?;
            }
        }
        Some(Commands::Run { prompt }) => {
            tracing::info!("Starting headless run mode");
            let mut resolved_agent = resolved_agent.clone();
            let config_guard = config.read().await;
            agent::apply_fallback_thinking(
                &mut resolved_agent,
                &config_guard,
                provider_registry.as_ref(),
            );
            let dir = std::fs::canonicalize(".")?;
            let session = session_manager.create_session(dir)?;
            match session_processor
                .process_message(
                    &session.id,
                    &prompt,
                    &resolved_agent,
                    Arc::new(AtomicBool::new(false)),
                )
                .await
            {
                Ok(msg) => {
                    writeln!(std::io::stdout(), "{}", msg.text_content())?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Run failed");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Serve { addr }) => {
            tracing::info!(address = %addr, "Starting HTTP server");
            let auth_token = uuid::Uuid::new_v4().to_string();
            let orchestrator_registry = ragent_core::orchestrator::AgentRegistry::new();
            let coordinator = ragent_core::orchestrator::Coordinator::new(orchestrator_registry);

            let state = ragent_server::routes::AppState {
                event_bus,
                config,
                storage,
                session_processor,
                auth_token,
                rate_limiter: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
                coordinator: Some(coordinator),
            };
            ragent_server::start_server(&addr, state).await?;
        }
        Some(Commands::Orchestrate) => {
            tracing::info!("Starting orchestration example");
            run_orchestration_example().await?;
        }
        Some(Commands::Session { command }) => match command {
            SessionCommands::List => {
                let sessions = storage.list_sessions()?;
                if sessions.is_empty() {
                    tracing::info!("No sessions found");
                } else {
                    let mut stdout = std::io::stdout().lock();
                    for s in sessions {
                        writeln!(
                            stdout,
                            "{} | {} | {} | {}",
                            &s.id[..8.min(s.id.len())],
                            s.title,
                            s.directory,
                            s.updated_at
                        )?;
                    }
                }
            }
            SessionCommands::Resume { id } => {
                // Verify session exists before launching TUI
                if storage.get_session(&id)?.is_none() {
                    anyhow::bail!("Session not found: {id}");
                }
                tracing::info!(session_id = %id, "Resuming session");
                ragent_tui::run_tui(
                    event_bus,
                    storage,
                    provider_registry,
                    session_processor,
                    resolved_agent.clone(),
                    cli.log,
                    Some(id),
                    tui_log_rx.unwrap_or_else(|| ragent_tui::tracing_layer::tui_log_channel(1).1),
                )
                .await?;
            }
            SessionCommands::Export { id } => {
                let messages = storage.get_messages(&id)?;
                let json = serde_json::to_string_pretty(&messages)?;
                writeln!(std::io::stdout(), "{json}")?;
            }
            SessionCommands::Import { file } => {
                let content = std::fs::read_to_string(&file)?;
                let messages: Vec<ragent_core::message::Message> = serde_json::from_str(&content)?;

                let dir = std::fs::canonicalize(".")?;
                let session = session_manager.create_session(dir)?;

                let mut imported = 0u64;
                for msg in &messages {
                    // Re-parent each message into the new session with a fresh ID
                    let imported_msg = ragent_core::message::Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session.id.clone(),
                        role: msg.role.clone(),
                        parts: msg.parts.clone(),
                        created_at: msg.created_at,
                        updated_at: msg.updated_at,
                    };
                    storage.create_message(&imported_msg)?;
                    imported += 1;
                }

                writeln!(
                    std::io::stdout(),
                    "Imported {} messages into session {}",
                    imported,
                    &session.id[..8.min(session.id.len())]
                )?;
            }
        },
        Some(Commands::Auth { provider, key }) => {
            storage.set_provider_auth(&provider, &key)?;
            tracing::info!(provider = %provider, "Stored API key");
        }
        Some(Commands::Models {
            provider: filter,
            ollama_url,
        }) => {
            let mut stdout = std::io::stdout().lock();

            if filter.as_deref() == Some("ollama_cloud") {
                let api_key = storage
                    .get_provider_auth("ollama_cloud")
                    .ok()
                    .flatten()
                    .filter(|k| !k.is_empty())
                    .or_else(|| {
                        std::env::var("OLLAMA_API_KEY")
                            .ok()
                            .filter(|k| !k.is_empty())
                    });
                let Some(api_key) = api_key else {
                    writeln!(
                        stdout,
                        "No Ollama Cloud API key found. Run `ragent auth ollama_cloud <key>` \
                         or set OLLAMA_API_KEY."
                    )?;
                    return Ok(());
                };

                match ragent_core::provider::ollama_cloud::list_ollama_cloud_models(
                    &api_key,
                    ollama_url.as_deref(),
                )
                .await
                {
                    Ok(models) if models.is_empty() => {
                        writeln!(stdout, "No models found on Ollama Cloud.")?;
                    }
                    Ok(models) => {
                        writeln!(stdout, "ollama_cloud models:")?;
                        for m in &models {
                            writeln!(stdout, "  ollama_cloud/{:<28} {}", m.id, m.name)?;
                        }
                    }
                    Err(e) => {
                        writeln!(stdout, "Could not connect to Ollama Cloud: {e}")?;
                    }
                }
            } else if filter.as_deref() == Some("ollama") || ollama_url.is_some() {
                match ragent_core::provider::ollama::list_ollama_models(ollama_url.as_deref()).await
                {
                    Ok(models) if models.is_empty() => {
                        writeln!(
                            stdout,
                            "No models found on Ollama server. Pull models with: ollama pull <model>"
                        )?;
                    }
                    Ok(models) => {
                        writeln!(stdout, "ollama models:")?;
                        for m in &models {
                            writeln!(stdout, "  ollama/{:<28} {}", m.id, m.name)?;
                        }
                    }
                    Err(e) => {
                        writeln!(stdout, "Could not connect to Ollama: {e}")?;
                        writeln!(stdout, "Is Ollama running? Start with: ollama serve")?;
                    }
                }
                if filter.as_deref() != Some("ollama") {
                    let providers = provider_registry.list();
                    for p in &providers {
                        if p.id == "ollama" || p.id == "ollama_cloud" {
                            continue;
                        }
                        for m in &p.models {
                            writeln!(stdout, "{}/{}", p.id, m.id)?;
                        }
                    }
                }
            } else {
                let providers = provider_registry.list();
                let providers: Vec<_> = if let Some(ref f) = filter {
                    providers.into_iter().filter(|p| p.id == *f).collect()
                } else {
                    providers
                };

                if providers.is_empty() {
                    writeln!(stdout, "No providers found matching filter")?;
                } else {
                    for p in &providers {
                        for m in &p.models {
                            writeln!(stdout, "{}/{}", p.id, m.id)?;
                        }
                    }
                }
            }
        }
        Some(Commands::Config) => {
            let config = config.read().await;
            println!("{:#?}", *config);
            drop(config);
        }
        Some(Commands::Memory { command }) => {
            let working_dir =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let block_storage = ragent_core::memory::FileBlockStorage::new();
            match command {
                MemoryCommands::Export => {
                    let (export, result) =
                        ragent_core::memory::export_all(&storage, &block_storage, &working_dir)?;
                    let json = serde_json::to_string_pretty(&export)?;
                    writeln!(std::io::stdout(), "{json}")?;
                    eprintln!(
                        "Exported {} memories, {} project blocks, {} global blocks",
                        result.memory_count, result.project_block_count, result.global_block_count,
                    );
                }
                MemoryCommands::Import {
                    path,
                    format,
                    dry_run,
                } => {
                    let path_buf = PathBuf::from(&path);

                    let result = match format.as_str() {
                        "ragent" => {
                            let json_data = std::fs::read_to_string(&path_buf).map_err(|e| {
                                anyhow::anyhow!(
                                    "Failed to read import file: {}: {}",
                                    path_buf.display(),
                                    e
                                )
                            })?;
                            ragent_core::memory::import_ragent(
                                &json_data,
                                &storage,
                                &block_storage,
                                &working_dir,
                                dry_run,
                            )?
                        }
                        "cline" => ragent_core::memory::import_cline(
                            &path_buf,
                            &block_storage,
                            &working_dir,
                            dry_run,
                        )?,
                        "claude-code" => ragent_core::memory::import_claude_code(
                            &path_buf,
                            &block_storage,
                            &working_dir,
                            dry_run,
                        )?,
                        _ => anyhow::bail!(
                            "Unknown import format '{format}'. Supported: ragent, cline, claude-code"
                        ),
                    };

                    if dry_run {
                        writeln!(
                            std::io::stdout(),
                            "[DRY RUN] Would import {} memories, {} project blocks, {} global blocks",
                            result.memory_count,
                            result.project_block_count,
                            result.global_block_count,
                        )?;
                    } else {
                        writeln!(
                            std::io::stdout(),
                            "Imported {} memories, {} project blocks, {} global blocks",
                            result.memory_count,
                            result.project_block_count,
                            result.global_block_count,
                        )?;
                    }
                    if !result.warnings.is_empty() {
                        eprintln!("\nWarnings:");
                        for w in &result.warnings {
                            eprintln!("  - {w}");
                        }
                    }
                }
                MemoryCommands::List => {
                    let block_storage: &dyn BlockStorage =
                        &ragent_core::memory::FileBlockStorage::new();
                    let mut stdout = std::io::stdout().lock();

                    // List structured memory stats.
                    let memory_count = storage.list_memories("", 10_000)?.len();
                    writeln!(stdout, "Structured memories: {memory_count}")?;

                    // List project blocks.
                    let project_labels = block_storage
                        .list(&ragent_core::memory::BlockScope::Project, &working_dir)
                        .unwrap_or_default();
                    writeln!(stdout, "\nProject memory blocks:")?;
                    if project_labels.is_empty() {
                        writeln!(stdout, "  (none)")?;
                    } else {
                        for label in &project_labels {
                            if let Ok(Some(block)) = block_storage.load(
                                label,
                                &ragent_core::memory::BlockScope::Project,
                                &working_dir,
                            ) {
                                writeln!(
                                    stdout,
                                    "  {} ({} bytes, {})",
                                    label,
                                    block.content.len(),
                                    if block.read_only {
                                        "read-only"
                                    } else {
                                        "writable"
                                    }
                                )?;
                            }
                        }
                    }

                    // List global blocks.
                    let global_labels = block_storage
                        .list(&ragent_core::memory::BlockScope::Global, &working_dir)
                        .unwrap_or_default();
                    writeln!(stdout, "\nGlobal memory blocks:")?;
                    if global_labels.is_empty() {
                        writeln!(stdout, "  (none)")?;
                    } else {
                        for label in &global_labels {
                            if let Ok(Some(block)) = block_storage.load(
                                label,
                                &ragent_core::memory::BlockScope::Global,
                                &working_dir,
                            ) {
                                writeln!(
                                    stdout,
                                    "  {} ({} bytes, {})",
                                    label,
                                    block.content.len(),
                                    if block.read_only {
                                        "read-only"
                                    } else {
                                        "writable"
                                    }
                                )?;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
