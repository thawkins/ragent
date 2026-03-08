//! ragent CLI binary.
//!
//! Entry point for the ragent terminal AI coding agent. Parses CLI arguments,
//! loads configuration, initialises storage and the event bus, and dispatches
//! to the requested sub-command (TUI, headless run, HTTP server, session
//! management, auth, or config display).

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

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
    #[arg(long, global = true, default_value = "build")]
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

    /// Path to config file
    #[arg(long, global = true)]
    config: Option<String>,
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
    Models,
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
    let event_bus = Arc::new(EventBus::default());

    // Create registries
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(
        config.permission.clone(),
    )));

    // Resolve the active agent
    let agent_name = &cli.agent;
    let resolved_agent = agent::resolve_agent(agent_name, &config)?;

    let config = Arc::new(tokio::sync::RwLock::new(config));

    // Create session manager and processor
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry: provider_registry.clone(),
        tool_registry: tool_registry.clone(),
        permission_checker,
        event_bus: event_bus.clone(),
    });

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
                        .process_message(&session.id, &line, &resolved_agent)
                        .await
                    {
                        Ok(msg) => writeln!(stdout, "{}", msg.text_content())?,
                        Err(e) => tracing::error!(error = %e, "Failed to process message"),
                    }
                }
            } else {
                ragent_tui::run_tui(event_bus).await?;
            }
        }
        Some(Commands::Run { prompt }) => {
            let dir = std::fs::canonicalize(".")?;
            let session = session_manager.create_session(dir)?;
            match session_processor
                .process_message(&session.id, &prompt, &resolved_agent)
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
                tracing::info!(session_id = %id, "Resuming session");
                // TODO: implement resume with TUI
            }
            SessionCommands::Export { id } => {
                let messages = storage.get_messages(&id)?;
                let json = serde_json::to_string_pretty(&messages)?;
                writeln!(std::io::stdout(), "{json}")?;
            }
            SessionCommands::Import { file } => {
                let content = std::fs::read_to_string(&file)?;
                let _messages: Vec<ragent_core::message::Message> = serde_json::from_str(&content)?;
                tracing::info!(file = %file, "Imported session");
                // TODO: store imported messages
            }
        },
        Some(Commands::Auth { provider, key }) => {
            storage.set_provider_auth(&provider, &key)?;
            tracing::info!(provider = %provider, "Stored API key");
        }
        Some(Commands::Models) => {
            let providers = provider_registry.list();
            if providers.is_empty() {
                tracing::info!("No providers configured");
            } else {
                let mut stdout = std::io::stdout().lock();
                for p in &providers {
                    for m in &p.models {
                        writeln!(stdout, "{}/{}", p.id, m.id)?;
                    }
                }
            }
        }
        Some(Commands::Config) => {
            let config = config.read().await;
            let json = serde_json::to_string_pretty(&*config)?;
            writeln!(std::io::stdout(), "{json}")?;
        }
    }

    Ok(())
}
