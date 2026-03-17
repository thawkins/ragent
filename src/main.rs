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
use tracing_subscriber::EnvFilter;

use ragent_core::{
    agent,
    config::Config,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};

/// small CLI demo for orchestration
async fn run_orchestration_example() -> anyhow::Result<()> {
    tracing::info!("Running orchestration example");
    let registry = ragent_core::orchestrator::AgentRegistry::new();

    use ragent_core::orchestrator::{Responder, Coordinator, JobDescriptor};
    use std::sync::Arc;
    use futures::future::FutureExt;
    use tokio::time::sleep;
    use tokio::time::Duration;

    let responder_a: Responder = Arc::new(|payload: String| async move { format!("demo-a: {}", payload) }.boxed());
    let responder_b: Responder = Arc::new(|payload: String| async move { sleep(Duration::from_millis(30)).await; format!("demo-b: {}", payload) }.boxed());

    registry.register("demo-a", vec!["demo".to_string()], Some(responder_a)).await;
    registry.register("demo-b", vec!["demo".to_string()], Some(responder_b)).await;

    let coord = Coordinator::new(registry.clone());
    let desc = JobDescriptor { id: "demo-job".to_string(), required_capabilities: vec!["demo".to_string()], payload: "payload".to_string() };

    let res = coord.start_job_sync(desc).await?;
    println!("Orchestration sync result:\n{}", res);

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
    #[arg(long, global = true)]
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

    // Initialize tracing
    let filter = EnvFilter::try_new(&cli.log_level).unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Load config
    let config = if let Some(ref path) = cli.config {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content)?
    } else {
        Config::load()?
    };

    // Initialize storage
    let db_path = data_dir().join("ragent.db");
    let storage = Arc::new(Storage::open(&db_path)?);

    // Create event bus
    let event_bus = Arc::new(EventBus::new(2048));

    // Create registries
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(
        config.permission.clone(),
    )));

    // Resolve the active agent
    let agent_name = &cli.agent;
    let mut resolved_agent = agent::resolve_agent(agent_name, &config)?;

    // Apply CLI --maxsteps override if provided
    if let Some(max) = cli.maxsteps {
        resolved_agent.max_steps = Some(max);
    }

    let max_background_agents = config.experimental.max_background_agents;

    let config = Arc::new(tokio::sync::RwLock::new(config));

    // Create session manager and processor
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry: provider_registry.clone(),
        tool_registry: tool_registry.clone(),
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
            lsp_manager: std::sync::OnceLock::new(),
    });

    // Create TaskManager and wire it into the processor (breaks circular dep via OnceLock)
    let task_manager = Arc::new(ragent_core::task::TaskManager::new(
        event_bus.clone(),
        session_processor.clone(),
        max_background_agents,
    ));
    let _ = session_processor.task_manager.set(task_manager);

    match cli.command {
        None => {
            // Default: run TUI
            if cli.no_tui {
                tracing::info!("Starting ragent interactive mode (plain)");
                let dir = std::fs::canonicalize(".")?;
                let session = session_manager.create_session(dir)?;
                let reader = tokio::io::BufReader::new(tokio::io::stdin());
                use tokio::io::AsyncBufReadExt;
                let mut lines = reader.lines();
                let mut stdout = std::io::stdout().lock();
                while let Some(line) = lines.next_line().await? {
                    if line.is_empty() {
                        continue;
                    }
                    match session_processor
                        .process_message(&session.id, &line, &resolved_agent, Arc::new(AtomicBool::new(false)))
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
                )
                .await?;
            }
        }
        Some(Commands::Run { prompt }) => {
            let dir = std::fs::canonicalize(".")?;
            let session = session_manager.create_session(dir)?;
            match session_processor
                .process_message(&session.id, &prompt, &resolved_agent, Arc::new(AtomicBool::new(false)))
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
            let auth_token = uuid::Uuid::new_v4().to_string();
            let state = ragent_server::routes::AppState {
                event_bus,
                config,
                storage,
                session_processor,
                auth_token,
                rate_limiter: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            };
            ragent_server::start_server(&addr, state).await?;
        }
        Some(Commands::Orchestrate) => {
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
                    anyhow::bail!("Session not found: {}", id);
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
        Some(Commands::Models { provider: filter, ollama_url }) => {
            let mut stdout = std::io::stdout().lock();

            if filter.as_deref() == Some("ollama") || ollama_url.is_some() {
                match ragent_core::provider::ollama::list_ollama_models(ollama_url.as_deref()).await {
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
                if filter.as_deref() == Some("ollama") {
                    // Only showing Ollama, skip other providers
                } else {
                    let providers = provider_registry.list();
                    for p in &providers {
                        if p.id == "ollama" {
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
        }
    }

    Ok(())
}
