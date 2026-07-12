//! CLI helper functions extracted from `main.rs` (REMPLAN.md M9/T9.4).
//!
//! Contains `run_orchestration_example` and `handle_research_command`,
//! plus the `ResearchCommands` clap subcommand enum.

use anyhow::Result;

use ragent_agent::{Config, event::EventBus, storage::Storage};

/// small CLI demo for orchestration
///
/// # Errors
///
/// Returns an error if job execution fails.
pub async fn run_orchestration_example() -> anyhow::Result<()> {
    tracing::info!("Running orchestration example");
    let registry = ragent_agent::orchestrator::AgentRegistry::new();

    use futures::future::FutureExt;
    use ragent_agent::orchestrator::{Coordinator, JobDescriptor, Responder};
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

#[derive(clap::Subcommand)]
pub enum ResearchCommands {
    /// Run a gathering session and create a research item.
    Create {
        /// Research name (URL-safe identifier)
        #[arg(value_name = "NAME")]
        name: String,
        /// Topic description (everything after the name). Optional when
        /// `--from-url` is supplied; in that case the fetched page content
        /// becomes the research subject.
        #[arg(value_name = "TOPIC", trailing_var_arg = true, num_args = 0.., required = false)]
        topic: Vec<String>,
        /// Fetch the URL and use its content as the research subject in
        /// place of an explicit topic. The page is captured as the primary
        /// source; web search still runs.
        #[arg(long, value_name = "URL")]
        from_url: Option<String>,
        /// Number of gathering iterations
        #[arg(long)]
        iterations: Option<u32>,
        /// Research depth: shallow|standard|deep
        #[arg(long)]
        depth: Option<String>,
        /// Output format: report|executive-summary|comparison-table|source-bibliography
        #[arg(long)]
        format: Option<String>,
        /// Optional extra sources directory (FR-019)
        #[arg(long)]
        sources_dir: Option<String>,
        /// Optional template name (FR-020)
        #[arg(long)]
        template: Option<String>,
        /// Override the maximum number of candidate pages fetched in
        /// parallel during the web-gathering phase (default 10).
        #[arg(long, value_name = "N")]
        fetch_concurrently: Option<usize>,
        /// Include the local-file scanning phase
        #[arg(long)]
        use_local: bool,
        /// Include the prior-spec cross-reference phase
        #[arg(long)]
        use_specs: bool,
    },
    /// List research items
    List {
        /// Include archived items
        #[arg(long)]
        all: bool,
    },
    /// Print the absolute path of a research item's RESEARCH.md
    Open {
        /// Research name
        name: String,
    },
    /// Full-text search across all RESEARCH.md files
    Search {
        /// Search query
        #[arg(value_name = "QUERY", trailing_var_arg = true)]
        query: Vec<String>,
    },
    /// Show metadata for a single research item
    Show {
        /// Research name
        name: String,
    },
    /// Delete a research item
    Delete {
        /// Research name
        name: String,
        /// Skip the confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Archive a research item
    Archive {
        /// Research name
        name: String,
    },
}

/// Dispatch `ragent research …` sub-commands to the `ragent-research`
/// crate. Emits a `ragent-research:` JSON line for each event so the
/// output is machine-parseable even when the session produces many
/// sources (T-035).
pub async fn handle_research_command(command: ResearchCommands) -> Result<()> {
    use ragent_research::cli::ResearchCliCommand;
    use ragent_research::{
        Depth, OutputFormat, ResearchManager, SessionConfig, SessionEvent, SessionObserver,
    };
    use std::sync::Arc;
    let working_dir = std::env::current_dir()?;
    let research_root = working_dir.join("research");
    let manager = ResearchManager::new(&research_root);

    let cli_cmd = match command {
        ResearchCommands::Create {
            name,
            topic,
            from_url,
            iterations,
            depth,
            format,
            sources_dir,
            template,
            fetch_concurrently,
            use_local,
            use_specs,
        } => {
            let topic = topic.join(" ");
            if topic.is_empty() && from_url.is_none() {
                eprintln!(
                    "ragent-research: usage: ragent research create <name> <topic...> [--from-url <URL>]"
                );
                std::process::exit(2);
            }
            ResearchCliCommand::Create {
                name,
                topic,
                from_url,
                iterations,
                depth,
                format,
                sources_dir,
                template,
                fetch_concurrency: fetch_concurrently,
                use_local,
                use_specs,
            }
        }
        ResearchCommands::List { all } => ResearchCliCommand::List { all },
        ResearchCommands::Open { name } => ResearchCliCommand::Open { name },
        ResearchCommands::Search { query } => ResearchCliCommand::Search {
            query: query.join(" "),
        },
        ResearchCommands::Show { name } => ResearchCliCommand::Show { name },
        ResearchCommands::Delete { name, yes } => ResearchCliCommand::Delete { name, yes },
        ResearchCommands::Archive { name } => ResearchCliCommand::Archive { name },
    };
    match cli_cmd {
        ResearchCliCommand::Help => {
            println!("{}", ResearchCliCommand::build_help_message());
        }
        ResearchCliCommand::List { all } => {
            let items = manager.list(all).await?;
            let rows: Vec<(String, String, String, String, String)> = items
                .into_iter()
                .map(|i| {
                    (
                        i.name.to_string(),
                        i.title,
                        i.status.as_str().to_string(),
                        i.created_at.to_rfc3339(),
                        i.modified_at.to_rfc3339(),
                    )
                })
                .collect();
            print!("{}", ragent_research::render_list_output(&rows));
        }
        ResearchCliCommand::Open { name } => {
            let item = manager.show(&name).await?;
            let path = ragent_research::ResearchIo::research_md_path(manager.root(), &item.name);
            println!("{}", path.display());
        }
        ResearchCliCommand::Search { query } => {
            let hits = manager.search(&query, 25).await?;
            let rows: Vec<(String, String, String)> = hits
                .into_iter()
                .map(|h| (h.name, h.title, h.snippet))
                .collect();
            print!("{}", ragent_research::render_search_output(&rows));
        }
        ResearchCliCommand::Show { name } => {
            let item = manager.show(&name).await?;
            let sources: Vec<(String, String, String, String)> = item
                .sources
                .iter()
                .map(|s| {
                    (
                        s.type_str().to_string(),
                        s.path_or_url().to_string(),
                        s.title().to_string(),
                        s.captured_at().to_rfc3339(),
                    )
                })
                .collect();
            print!(
                "{}",
                ragent_research::render_show_output(
                    item.name.as_ref(),
                    &item.title,
                    &item.topic,
                    item.status.as_str(),
                    &item.created_at.to_rfc3339(),
                    &item.modified_at.to_rfc3339(),
                    &sources,
                )
            );
        }
        ResearchCliCommand::Delete { name, yes } => {
            if !yes {
                eprint!("Are you sure you want to delete research/{name}? [y/N] ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                    println!("ragent-research: cancelled");
                    return Ok(());
                }
            }
            manager.delete(&name).await?;
            println!("ragent-research: deleted research/{name}");
        }
        ResearchCliCommand::Archive { name } => {
            manager.archive(&name).await?;
            println!("ragent-research: archived research/{name}");
        }
        ResearchCliCommand::Create {
            name,
            topic,
            from_url,
            iterations,
            depth,
            format,
            sources_dir,
            template,
            fetch_concurrency,
            use_local,
            use_specs,
        } => {
            // Wire the session through a streaming JSON observer so the
            // CLI consumer (e.g. `jq -R '.payload'`) can pipe the output.
            struct CliObserver;
            impl SessionObserver for CliObserver {
                fn on_event(&self, event: SessionEvent) {
                    println!("{}", ragent_research::render_session_event_json(&event));
                }
            }
            // Derive a human-readable item title that summarises the topic
            // (rather than truncating to its first word). Falls back to the
            // URL when only `--from-url` was supplied, then to "Research".
            let title = ragent_research::derive_title(&topic, from_url.as_deref());
            let config = SessionConfig {
                topic: topic.clone(),
                from_url,
                sources_dir: sources_dir.map(std::path::PathBuf::from),
                template,
                disable_local: !use_local,
                disable_specs: !use_specs,
                fetch_concurrency: fetch_concurrency
                    .unwrap_or(ragent_research::DEFAULT_FETCH_CONCURRENCY),
                depth: depth.as_deref().and_then(Depth::parse),
                iterations,
                output_format: format
                    .as_deref()
                    .map(|s| OutputFormat::parse(s).unwrap_or(OutputFormat::Report))
                    .unwrap_or(OutputFormat::Report),
                ..SessionConfig::default()
            };

            // Build a full research session backed by the default tool
            // registry so the CLI can capture web sources when a search API
            // key is available, as well as local in-project sources.
            let tool_registry = Arc::new(ragent_agent::tool::create_default_registry());
            let event_bus = Arc::new(EventBus::new(256));
            let storage = Arc::new(Storage::open_in_memory()?);
            let config_arc = Config::load().ok().map(Arc::new);
            let session = ragent_agent::research_adapter::build_research_session(
                &tool_registry,
                manager.clone(),
                name.clone(),
                working_dir.clone(),
                event_bus,
                Some(storage),
                config_arc,
                Some(Arc::new(ragent_agent::provider::create_default_registry())),
                None,
            );
            match session
                .run(&name, &title, &config, Arc::new(CliObserver))
                .await
            {
                Ok(outcome) => {
                    println!(
                        "ragent-research: created research/{} ({} sources)",
                        outcome.research_name,
                        outcome.sources.len()
                    );
                }
                Err(e) => {
                    eprintln!("ragent-research: {e}");
                    std::process::exit(1);
                }
            }
        }
        ResearchCliCommand::Continue { name, message } => {
            eprintln!("ragent-research: continue is not yet supported from the CLI; use the TUI.");
            let _ = name;
            let _ = message;
            std::process::exit(2);
        }
        ResearchCliCommand::Unknown(sub) => {
            eprintln!("ragent-research: unknown subcommand '{sub}'. Try `ragent research help`.");
            std::process::exit(2);
        }
    }
    Ok(())
}
