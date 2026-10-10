//! CLI helper functions extracted from `main.rs` (REMPLAN.md M9/T9.4).
//!
//! Contains `run_orchestration_example` and `handle_research_command`,
//! plus the `ResearchCommands` clap subcommand enum.

use anyhow::Result;

use ragent_agent::{event::EventBus, storage::Storage};

/// A typed process exit requested by a CLI handler.
///
/// ANTIPAT A-04/I-05: the CLI handlers previously called `std::process::exit`
/// directly, which bypassed `main`'s clean-exit crash marker and the runtime
/// `shutdown_timeout`. They now *return* this error; `main` performs the single
/// exit after its cleanup runs. The taxonomy is fixed: usage/validation errors
/// are code 2, runtime failures are code 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliExit {
    /// The process exit code to use.
    pub code: i32,
}

impl CliExit {
    /// A usage / validation error (exit code 2).
    #[must_use]
    pub const fn usage() -> Self {
        Self { code: 2 }
    }

    /// A runtime failure (exit code 1).
    #[must_use]
    pub const fn failure() -> Self {
        Self { code: 1 }
    }
}

impl std::fmt::Display for CliExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cli exit code {}", self.code)
    }
}

impl std::error::Error for CliExit {}

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

#[derive(clap::Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
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
        /// Fetch one or more URLs and use their content as the research
        /// subject in place of (or alongside) an explicit topic. Each page is
        /// captured as a primary source; web search still runs. Repeat the
        /// flag to seed multiple pages.
        #[arg(long, value_name = "URL")]
        from_urls: Vec<String>,
        /// Extract one or more local documents and use their content as the
        /// research subject in place of (or alongside) an explicit topic.
        /// Supported formats: PDF, DOCX, XLSX, PPTX, ODT, ODS, ODP, TXT, and
        /// MD. The extracted content is captured as the primary source; web
        /// search still runs using the derived topic. Repeat the flag to
        /// seed multiple files.
        #[arg(long, value_name = "PATH")]
        from_files: Vec<String>,
        /// Research mode: tiered|supervisor|competitive (competitive implies
        /// --format comparison-table unless an explicit --format is supplied)
        #[arg(long, value_name = "MODE")]
        mode: Option<String>,
        /// Maximum parallel researcher agents in supervisor/competitive modes
        #[arg(long, value_name = "N")]
        max_concurrent_research_units: Option<usize>,
        /// Number of gathering iterations
        #[arg(long)]
        iterations: Option<u32>,
        /// Research depth: shallow|standard|deep
        #[arg(long)]
        depth: Option<String>,
        /// Research tier: light|full|dissertation
        #[arg(long, value_name = "TIER")]
        tier: Option<String>,
        /// Output format: report|executive-summary|comparison-table|source-bibliography
        /// (defaulted to comparison-table when --mode competitive is set)
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
        /// Keep low-relevance web sources instead of filtering them out.
        #[arg(long)]
        use_low_relevance: bool,
        /// Re-enable scholarly search engines (e.g. OpenAlex) during web
        /// gathering. Scholarly engines are excluded by default; `--papers`
        /// opts back in for academic topics.
        #[arg(long)]
        papers: bool,
        /// Allow PDF documents from web search or --from-url to be captured
        /// as sources. By default PDF web sources are skipped.
        #[arg(long)]
        use_pdf: bool,
        /// Force open-access recovery on for this run. Overrides
        /// `research.open_access_recovery` in ragent.json.
        #[arg(long, overrides_with = "no_oa")]
        oa_enable: bool,
        /// Force open-access recovery off for this run, overriding
        /// `research.open_access_recovery` in ragent.json.
        #[arg(long, overrides_with = "oa_enable")]
        no_oa: bool,
        /// Override the maximum number of local scoring/spec-scan tasks that run
        /// in parallel during the local-gathering phase (default 8).
        #[arg(long, value_name = "N")]
        local_concurrently: Option<usize>,
        /// Override the per-page fetch timeout in seconds (default 30).
        #[arg(long, value_name = "N")]
        fetch_timeout_secs: Option<u64>,
        /// Optional wall-clock timeout for the entire web-gathering phase in
        /// seconds (Milestone H-001). When set, the phase is aborted if it
        /// exceeds this duration and a diagnostic is emitted. `--web-time` is
        /// the preferred short alias.
        #[arg(long, value_name = "N", visible_alias = "web-time")]
        web_phase_timeout_secs: Option<u64>,
        /// Optional wall-clock timeout for the entire local-gathering phase in
        /// seconds (Milestone H-001).
        #[arg(long, value_name = "N")]
        local_phase_timeout_secs: Option<u64>,
        /// Maximum retry attempts for a failed sub-query search (Milestone H-002).
        /// Defaults to 2. `0` disables retries.
        #[arg(long, value_name = "N")]
        search_max_retries: Option<u32>,
        /// Base delay in milliseconds for the first search-retry backoff
        /// (Milestone H-002). Defaults to 200 ms.
        #[arg(long, value_name = "N")]
        search_retry_base_delay_ms: Option<u64>,
        /// Hard cap on the total number of web-search calls the run may issue,
        /// shared across all supervisor/competitive researchers and retries.
        /// When the cap is reached the run proceeds with the sources gathered
        /// so far instead of failing. Omit for no cap.
        #[arg(long, value_name = "N")]
        max_search_calls: Option<usize>,
        /// Maximum number of concepts rendered in the research report
        /// (default 5, or `research.max_concepts` from ragent.json). `0` means
        /// unbounded.
        #[arg(long, value_name = "N")]
        max_concepts: Option<usize>,
        /// Maximum number of findings rendered in the research report
        /// (default 20, or `research.max_findings` from ragent.json). `0`
        /// means unbounded.
        #[arg(long, value_name = "N")]
        max_findings: Option<usize>,
        /// Deprecated no-op: clarification is off by default. Use --clarify
        /// to ask a single clarifying question for ambiguous topics.
        #[arg(long, overrides_with = "clarify")]
        no_clarify: bool,
        /// Ask a single clarifying question before web searches when the
        /// topic is ambiguous. Defaults to disabled; --clarify enables it.
        #[arg(long, value_name = "BOOL", num_args = 0..=1, default_missing_value = "true")]
        clarify: Option<bool>,
        /// Defang web source URLs in the generated `RESEARCH.md`: emit them in
        /// the `Sources` bullets and the `References Index` (and `CORPA.md`
        /// `Sources Reference`) as `hxxps://host[.]tld/...` code-span text
        /// instead of clickable links, so automated URL scanners do not flag
        /// or reject the document.
        #[arg(long)]
        url_cloak: bool,
    },
    /// List research items
    List {
        /// Include archived items
        #[arg(long)]
        all: bool,
        /// Output as a JSON array (one object per item)
        #[arg(long)]
        json: bool,
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
    /// Resume an in-progress research item, optionally with a follow-up message
    Continue {
        /// Research name
        name: String,
        /// Optional follow-up message to add to the research plan
        #[arg(value_name = "MESSAGE", trailing_var_arg = true, num_args = 0..)]
        message: Vec<String>,
    },
    /// Replay the invocation recorded in a research item's frontmatter and
    /// overwrite its RESEARCH.md (and associated files) with a fresh run
    Update {
        /// Research name
        name: String,
    },
    /// Extract top 10 concepts from the web-source documents under a research
    /// item and write them to `CONCEPTS.md`.
    Cluster {
        /// Research name
        name: String,
        /// Overwrite an existing `CONCEPTS.md` without prompting.
        #[arg(long)]
        force: bool,
    },
}

/// Dispatch `ragent research ...` sub-commands to the `ragent-research`
/// crate. Emits a `ragent-research:` JSON line for each event so the
/// output is machine-parseable even when the session produces many
/// sources (T-035).
pub async fn handle_research_command(
    command: ResearchCommands,
    active_model: Option<ragent_agent::agent::ModelRef>,
    storage: Option<std::sync::Arc<Storage>>,
) -> Result<()> {
    use ragent_research::cli::ResearchCliCommand;
    use ragent_research::{ResearchManager, SessionEvent, SessionObserver};
    use std::sync::Arc;

    // Wire the session through a streaming JSON observer so the CLI consumer
    // (e.g. `jq -R '.payload'`) can pipe the output. Shared by both the
    // `create` and `continue` subcommands.
    struct CliObserver;
    impl SessionObserver for CliObserver {
        fn on_event(&self, event: SessionEvent) {
            println!("{}", ragent_research::render_session_event_json(&event));
        }
    }

    let working_dir = std::env::current_dir()?;
    let research_root = working_dir.join("research");
    let manager = ResearchManager::new(&research_root);

    // Use the caller's persistent storage when available so the research
    // pipeline can reach credentials stored with `ragent auth <provider> <key>`.
    // When running research as a bare CLI invocation (no main-session storage
    // was created), open the persistent database directly; fall back to
    // in-memory storage when the database cannot be opened.
    let research_storage = match storage {
        Some(s) => s,
        None => {
            let db_path = ragent_config::user_dirs::global_db_path().unwrap_or_else(|| {
                std::path::PathBuf::from(".")
                    .join("ragent")
                    .join("ragent.db")
            });
            match Storage::open(&db_path) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        db_path = %db_path.display(),
                        "ragent-research: failed to open persistent storage; \
                         stored provider credentials unavailable"
                    );
                    Arc::new(Storage::open_in_memory()?)
                }
            }
        }
    };

    let cli_cmd = match command {
        ResearchCommands::Create {
            name,
            topic,
            from_urls,
            from_files,
            mode,
            max_concurrent_research_units,
            iterations,
            depth,
            tier,
            format,
            sources_dir,
            template,
            fetch_concurrently,
            use_local,
            use_specs,
            use_low_relevance,
            papers,
            use_pdf,
            oa_enable,
            no_oa,
            local_concurrently,
            fetch_timeout_secs,
            web_phase_timeout_secs,
            local_phase_timeout_secs,
            search_max_retries,
            search_retry_base_delay_ms,
            max_search_calls,
            max_concepts,
            max_findings,
            no_clarify: _,
            clarify,
            url_cloak,
        } => {
            let topic = topic.join(" ");
            // Clarification defaults to off; --clarify opts in.
            let clarify = clarify.unwrap_or(false);
            if topic.is_empty() && from_urls.is_empty() && from_files.is_empty() {
                eprintln!(
                    "ragent-research: usage: ragent research create <name> <topic...> [--from-url <URL>] [--from-file <PATH>]"
                );
                return Err(CliExit::usage().into());
            }
            ResearchCliCommand::Create {
                name,
                topic,
                from_urls,
                from_files,
                iterations,
                depth,
                tier,
                mode,
                summarization_model: None,
                research_model: None,
                compression_model: None,
                final_report_model: None,
                max_concurrent_research_units,
                clarify: Some(clarify),
                format,
                sources_dir,
                template,
                fetch_concurrency: fetch_concurrently,
                use_local,
                use_specs,
                use_low_relevance,
                papers,
                use_pdf,
                oa_recovery: match (oa_enable, no_oa) {
                    (true, _) => Some(true),
                    (_, true) => Some(false),
                    _ => None,
                },
                local_concurrency: local_concurrently,
                fetch_timeout_secs,
                web_phase_timeout_secs,
                local_phase_timeout_secs,
                search_max_retries,
                search_retry_base_delay_ms,
                max_search_calls,
                max_web_results: None,
                max_local_sources: None,
                max_synthesis_sources: None,
                max_concepts,
                max_findings,
                brief: None,
                evaluate: false,
                url_cloak,
            }
        }
        ResearchCommands::List { all, json } => ResearchCliCommand::List { all, json },
        ResearchCommands::Open { name } => ResearchCliCommand::Open { name },
        ResearchCommands::Search { query } => ResearchCliCommand::Search {
            query: query.join(" "),
            json: false,
        },
        ResearchCommands::Show { name } => ResearchCliCommand::Show { name, json: false },
        ResearchCommands::Delete { name, yes } => ResearchCliCommand::Delete { name, yes },
        ResearchCommands::Archive { name } => ResearchCliCommand::Archive { name },
        ResearchCommands::Continue { name, message } => ResearchCliCommand::Continue {
            name,
            message: if message.is_empty() {
                None
            } else {
                Some(message.join(" "))
            },
        },
        ResearchCommands::Update { name } => ResearchCliCommand::Update { name },
        ResearchCommands::Cluster { name, force } => ResearchCliCommand::Cluster { name, force },
    };
    match cli_cmd {
        ResearchCliCommand::Help => {
            println!("{}", ResearchCliCommand::build_help_message());
        }
        ResearchCliCommand::List { all, json } => {
            // `list` scans the research root directly, so a refresh here also
            // repairs a stale INDEX.md after items are moved or removed from
            // disk outside of the manager (e.g. manual directory moves).
            manager.refresh_index().await?;
            let items = manager.list(all).await?;
            let rows: Vec<(String, String, String, String, String, String)> = items
                .into_iter()
                .map(|i| {
                    (
                        i.name.to_string(),
                        i.title,
                        i.topic,
                        i.status.as_str().to_string(),
                        i.created_at.to_rfc3339(),
                        i.modified_at.to_rfc3339(),
                    )
                })
                .collect();
            if json {
                println!("{}", ragent_research::render_list_output_json(&rows));
            } else {
                print!("{}", ragent_research::render_list_output(&rows));
            }
        }
        ResearchCliCommand::Open { name } => {
            let item = manager.show(&name).await?;
            let path = ragent_research::ResearchIo::research_md_path(manager.root(), &item.name);
            println!("{}", path.display());
        }
        ResearchCliCommand::Search { query, json } => {
            let hits = manager.search(&query, 25).await?;
            let json_rows: Vec<(String, String, String, String)> = hits
                .into_iter()
                .map(|h| (h.name, h.title, h.snippet, h.path.display().to_string()))
                .collect();
            if json {
                println!("{}", ragent_research::render_search_output_json(&json_rows));
            } else {
                let rows: Vec<(String, String, String)> = json_rows
                    .into_iter()
                    .map(|(name, title, snippet, _path)| (name, title, snippet))
                    .collect();
                print!("{}", ragent_research::render_search_output(&rows));
            }
        }
        ResearchCliCommand::Show { name, .. } => {
            let item = manager.show(&name).await?;
            let sources: Vec<(String, String, String, String, Option<String>)> = item
                .sources
                .iter()
                .map(|s| {
                    (
                        s.type_str().to_string(),
                        s.path_or_url().to_string(),
                        s.title().to_string(),
                        "web".to_string(),
                        s.oa_recovery_note(),
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
            from_urls,
            from_files,
            iterations,
            depth,
            tier,
            mode,
            research_model,
            compression_model,
            final_report_model,
            max_concurrent_research_units,
            clarify,
            format,
            sources_dir,
            template,
            fetch_concurrency,
            use_local,
            use_specs,
            use_low_relevance,
            papers,
            use_pdf,
            local_concurrency,
            fetch_timeout_secs,
            web_phase_timeout_secs,
            local_phase_timeout_secs,
            search_max_retries,
            search_retry_base_delay_ms,
            max_web_results,
            max_search_calls,
            max_local_sources,
            max_synthesis_sources,
            brief,
            evaluate,
            url_cloak,
            ..
        } => {
            // Derive a human-readable item title that summarises the topic
            // (rather than truncating to its first word). Falls back to the
            // URL when only `--from-url` was supplied, then the first file
            // path when only `--from-file` was supplied, then to "Research".
            let title = ragent_research::derive_title_files(
                &topic,
                from_urls.first().map(String::as_str),
                &from_files,
            );
            let config_arc = ragent_config::Config::load().ok().map(Arc::new);
            let req = ragent_research::ResearchRunRequest {
                name: name.clone(),
                topic,
                title: Some(title.clone()),
                from_urls,
                from_files,
                sources_dir,
                template,
                depth,
                tier,
                mode,
                clarify,
                iterations,
                output_format: format,
                use_local,
                use_specs,
                use_low_relevance,
                papers,
                use_pdf,
                fetch_concurrency,
                local_concurrency,
                fetch_timeout_secs,
                web_phase_timeout_secs,
                local_phase_timeout_secs,
                search_max_retries,
                search_retry_base_delay_ms,
                max_web_results,
                max_search_calls,
                max_local_sources,
                max_synthesis_sources,
                brief,
                research_model,
                compression_model,
                final_report_model,
                max_concurrent_research_units,
                evaluate: Some(evaluate),
                url_cloak,
                // Record the verbatim command line for frontmatter replay.
                invocation: {
                    let argv: Vec<String> = std::env::args().collect();
                    Some(argv.join(" "))
                },
                ..Default::default()
            };
            let config = ragent_research::build_session_config(&req, config_arc.as_deref());
            // Build a full research session backed by the default tool
            // registry so the CLI can capture web sources when a search API
            // key is available, as well as local in-project sources.
            let tool_registry = Arc::new(ragent_agent::tool::create_default_registry());
            let event_bus = Arc::new(EventBus::new(256));
            let storage = research_storage.clone();
            let session = ragent_agent::research_adapter::build_research_session(
                &tool_registry,
                manager.clone(),
                name.clone(),
                working_dir.clone(),
                event_bus,
                Some(storage),
                config_arc.clone(),
                Some(Arc::new(ragent_agent::provider::create_default_registry())),
                active_model,
                Some(name.as_str()),
            );
            match session
                .run(&name, &title, &config, Arc::new(CliObserver))
                .await
            {
                Ok(outcome) => {
                    let mut summary = format!(
                        "ragent-research: created research/{} ({} sources",
                        outcome.research_name,
                        outcome.sources.len()
                    );
                    if outcome.pdf_count > 0 {
                        summary.push_str(&format!(
                            ", {} PDF{}",
                            outcome.pdf_count,
                            if outcome.pdf_count == 1 { "" } else { "s" }
                        ));
                    }
                    if outcome.youtube_count > 0 {
                        summary.push_str(&format!(
                            ", {} YouTube video{}",
                            outcome.youtube_count,
                            if outcome.youtube_count == 1 { "" } else { "s" }
                        ));
                    }
                    summary.push(')');
                    summary.push_str(&ragent_research::provider_calls_suffix(
                        &outcome.provider_tool_calls,
                    ));
                    println!("{summary}");
                }
                Err(ragent_research::ResearchError::NeedsClarification { question }) => {
                    eprintln!("ragent-research: {}", question);
                    eprint!("Answer: ");
                    let mut answer = String::new();
                    std::io::stdin().read_line(&mut answer)?;
                    let answer = answer.trim();
                    if answer.is_empty() {
                        eprintln!("ragent-research: clarification cancelled");
                        return Err(CliExit::failure().into());
                    }
                    let mut req = req;
                    req.topic = format!("{} (clarification: {})", req.topic, answer);
                    let config = ragent_research::build_session_config(&req, config_arc.as_deref());
                    match session
                        .run(&name, &title, &config, Arc::new(CliObserver))
                        .await
                    {
                        Ok(outcome) => {
                            let mut summary = format!(
                                "ragent-research: created research/{} ({} sources",
                                outcome.research_name,
                                outcome.sources.len()
                            );
                            if outcome.pdf_count > 0 {
                                summary.push_str(&format!(
                                    ", {} PDF{}",
                                    outcome.pdf_count,
                                    if outcome.pdf_count == 1 { "" } else { "s" }
                                ));
                            }
                            if outcome.youtube_count > 0 {
                                summary.push_str(&format!(
                                    ", {} YouTube video{}",
                                    outcome.youtube_count,
                                    if outcome.youtube_count == 1 { "" } else { "s" }
                                ));
                            }
                            summary.push(')');
                            summary.push_str(&ragent_research::provider_calls_suffix(
                                &outcome.provider_tool_calls,
                            ));
                            println!("{summary}");
                        }
                        Err(e) => {
                            eprintln!("ragent-research: {e}");
                            return Err(CliExit::failure().into());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("ragent-research: {e}");
                    return Err(CliExit::failure().into());
                }
            }
        }
        ResearchCliCommand::Continue { name, message } => {
            // Resume an in-progress research item. The manager adds the
            // follow-up message to the plan and marks the item InProgress;
            // then we re-run the session to gather and synthesize.
            match manager.continue_item(&name, message.as_deref()).await {
                Ok(state) => {
                    let topic = state.plan.topic.clone();
                    let title = name.clone();
                    eprintln!(
                        "ragent-research: resuming research/{} - topic: {}",
                        name, topic
                    );
                    // Build and run the research session.
                    let req = ragent_research::ResearchRunRequest {
                        name: name.clone(),
                        topic,
                        from_urls: Vec::new(),
                        from_files: Vec::new(),
                        title: Some(title.clone()),
                        ..Default::default()
                    };
                    let config = ragent_research::build_session_config(&req, None);
                    let tool_registry = Arc::new(ragent_agent::tool::create_default_registry());
                    let event_bus = Arc::new(EventBus::new(256));
                    let storage = research_storage.clone();
                    let session = ragent_agent::research_adapter::build_research_session(
                        &tool_registry,
                        manager.clone(),
                        name.clone(),
                        working_dir.clone(),
                        event_bus,
                        Some(storage),
                        None,
                        Some(Arc::new(ragent_agent::provider::create_default_registry())),
                        active_model.clone(),
                        Some(name.as_str()),
                    );
                    match session
                        .run(&name, &title, &config, Arc::new(CliObserver))
                        .await
                    {
                        Ok(outcome) => {
                            println!(
                                "ragent-research: continued research/{} ({} sources{})",
                                outcome.research_name,
                                outcome.sources.len(),
                                ragent_research::provider_calls_suffix(
                                    &outcome.provider_tool_calls
                                )
                            );
                        }
                        Err(e) => {
                            eprintln!("ragent-research: {e}");
                            return Err(CliExit::failure().into());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("ragent-research: {e}");
                    return Err(CliExit::failure().into());
                }
            }
        }
        ResearchCliCommand::Update { name } => {
            // Replay the invocation recorded in the item's frontmatter and
            // overwrite RESEARCH.md (and the associated supporting files) with
            // a fresh run.
            let item = match manager.show(&name).await {
                Ok(item) => item,
                Err(e) => {
                    eprintln!("ragent-research: {e}");
                    return Err(CliExit::failure().into());
                }
            };
            let Some(recorded) = item.invocation.clone() else {
                eprintln!(
                    "ragent-research: research/{name} has no invocation recorded in its \
                     frontmatter; only runs created with an invocation-aware front-end \
                     can be replayed"
                );
                return Err(CliExit::failure().into());
            };
            let mut req = match ragent_research::ResearchRunRequest::from_invocation(&recorded) {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("ragent-research: cannot replay research/{name}: {e}");
                    return Err(CliExit::failure().into());
                }
            };
            // The item name comes from the frontmatter, which is authoritative.
            req.name = name.clone();
            req.title = Some(item.title.clone());
            eprintln!("ragent-research: updating research/{name} - replaying: {recorded}");
            let config_arc = ragent_config::Config::load().ok().map(Arc::new);
            let config = ragent_research::build_session_config(&req, config_arc.as_deref());
            let tool_registry = Arc::new(ragent_agent::tool::create_default_registry());
            let event_bus = Arc::new(EventBus::new(256));
            let storage = research_storage.clone();
            let session = ragent_agent::research_adapter::build_research_session(
                &tool_registry,
                manager.clone(),
                name.clone(),
                working_dir.clone(),
                event_bus,
                Some(storage),
                config_arc.clone(),
                Some(Arc::new(ragent_agent::provider::create_default_registry())),
                active_model,
                Some(name.as_str()),
            );
            match session
                .run(&name, &item.title, &config, Arc::new(CliObserver))
                .await
            {
                Ok(outcome) => {
                    println!(
                        "ragent-research: updated research/{} ({} sources{})",
                        outcome.research_name,
                        outcome.sources.len(),
                        ragent_research::provider_calls_suffix(&outcome.provider_tool_calls)
                    );
                }
                Err(ragent_research::ResearchError::NeedsClarification { question }) => {
                    eprintln!("ragent-research: {}", question);
                    eprint!("Answer: ");
                    let mut answer = String::new();
                    std::io::stdin().read_line(&mut answer)?;
                    let answer = answer.trim();
                    if answer.is_empty() {
                        eprintln!("ragent-research: clarification cancelled");
                        return Err(CliExit::failure().into());
                    }
                    req.topic = format!("{} (clarification: {})", req.topic, answer);
                    let config = ragent_research::build_session_config(&req, config_arc.as_deref());
                    match session
                        .run(&name, &item.title, &config, Arc::new(CliObserver))
                        .await
                    {
                        Ok(outcome) => {
                            println!(
                                "ragent-research: updated research/{} ({} sources{})",
                                outcome.research_name,
                                outcome.sources.len(),
                                ragent_research::provider_calls_suffix(
                                    &outcome.provider_tool_calls
                                )
                            );
                        }
                        Err(e) => {
                            eprintln!("ragent-research: {e}");
                            return Err(CliExit::failure().into());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("ragent-research: {e}");
                    return Err(CliExit::failure().into());
                }
            }
        }
        ResearchCliCommand::Cluster { name, force } => {
            // Mirror the validation performed by the TUI slash-command so the
            // CLI path fails fast with the same clear diagnostics (FR-009,
            // FR-010, FR-011, FR-012).
            use ragent_research::{ResearchIo, ResearchName};
            let valid_name = match ResearchName::try_new(&name) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("ragent-research: invalid research name `{name}`: {e}");
                    return Err(CliExit::usage().into());
                }
            };
            let item_dir = ResearchIo::item_dir(manager.root(), &valid_name);
            let sources_dir = ResearchIo::sources_dir(manager.root(), &valid_name);
            if !item_dir.exists() {
                eprintln!("ragent-research: research folder `research/{name}` does not exist.");
                return Err(CliExit::failure().into());
            }
            if !sources_dir.exists() || !sources_dir.is_dir() {
                eprintln!("ragent-research: `research/{name}/sources/` folder not found.");
                return Err(CliExit::failure().into());
            }
            let is_empty = match std::fs::read_dir(&sources_dir) {
                Ok(entries) => entries.count() == 0,
                Err(_) => true,
            };
            if is_empty {
                eprintln!("ragent-research: `research/{name}/sources/` is empty.");
                return Err(CliExit::failure().into());
            }
            let concepts_path = ResearchIo::concepts_md_path(manager.root(), &valid_name);
            if concepts_path.exists() && !force {
                eprintln!(
                    "ragent-research: `research/{name}/CONCEPTS.md` already exists. \
                     Re-run with --force to overwrite it."
                );
                return Err(CliExit::failure().into());
            } // T-003: read the captured source documents and enforce the active
            // model's context-window budget.
            let registry = ragent_agent::provider::create_default_registry();
            let context_window = ragent_research::resolve_context_window_tokens(
                active_model.as_ref().map(|m| m.provider_id.as_str()),
                active_model.as_ref().map(|m| m.model_id.as_str()),
                Some(&registry),
            )
            .unwrap_or(ragent_research::DEFAULT_CONTEXT_WINDOW_TOKENS);
            let payload = match ragent_research::build_cluster_payload(
                manager.root(),
                &valid_name,
                Some(context_window),
            )
            .await
            {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("ragent-research: failed to read sources for `{name}`: {e}");
                    return Err(CliExit::failure().into());
                }
            };
            println!("ragent-research: cluster request accepted for `{name}` (force={force}).");
            println!(
                "ragent-research: read {} source file(s) (payload: {} / {} bytes, \
                   context window: {context_window} tokens, truncated: {}).",
                payload.files.len(),
                payload.total_bytes,
                payload.max_bytes,
                payload.truncated,
            );

            // T-005: dispatch the fixed concept-extraction prompt to the active
            // LLM and stream/await the response.
            let model_ref = match active_model {
                Some(m) => m,
                None => {
                    eprintln!(
                        "ragent-research: no active model selected. Use --model or configure a default."
                    );
                    return Err(CliExit::failure().into());
                }
            };
            let prompt = ragent_research::build_concept_extraction_prompt(&payload);
            let response = match ragent_agent::send_one_shot(
                Arc::new(registry),
                None,
                model_ref,
                None,
                prompt,
                Some(4_096),
            )
            .await
            {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("ragent-research: LLM call failed for `{name}`: {e}");
                    return Err(CliExit::failure().into());
                }
            };
            println!("ragent-research: concept extraction completed for `{name}`.");
            let concepts_path =
                match ragent_research::write_concepts_md(manager.root(), &valid_name, &response)
                    .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("ragent-research: failed to write CONCEPTS.md for `{name}`: {e}");
                        return Err(CliExit::failure().into());
                    }
                };
            println!(
                "ragent-research: wrote `{path}` ({size} bytes).",
                path = concepts_path.display(),
                size = response.len()
            );
        }
        ResearchCliCommand::Config => {
            println!("ragent-research: effective defaults are loaded from ragent.json.");
        }
        ResearchCliCommand::Resume { name } => {
            println!("ragent-research: resume '{name}' is not implemented in the CLI.");
        }
        ResearchCliCommand::Export { name, .. } => {
            println!("ragent-research: export '{name}' is not implemented in the CLI.");
        }
        ResearchCliCommand::Import { path, .. } => {
            println!("ragent-research: import '{path}' is not implemented in the CLI.");
        }
        ResearchCliCommand::Invalid(arg) => {
            eprintln!("ragent-research: {arg}. Try `ragent research help`.");
            return Err(CliExit::usage().into());
        }
        ResearchCliCommand::Unknown(sub) => {
            eprintln!("ragent-research: unknown subcommand '{sub}'. Try `ragent research help`.");
            return Err(CliExit::usage().into());
        }
    }
    Ok(())
}

/// Flag set for the `/new` scaffold surface (spec `newproj` T-012, FR-013).
///
/// Shared between the flattened `ragent new ...` invocation and the
/// `ragent new scaffold ...` subcommand spelling; the values are validated by
/// the same `project_scaffold::parse_flags` engine the TUI uses, so accepted
/// values and error messages cannot drift between the modes.
#[derive(clap::Args, Debug)]
pub struct ScaffoldArgs {
    /// Computer language to scaffold (required; e.g. rust, python, go, ts)
    #[arg(long, value_name = "LANG")]
    pub language: Option<String>,
    /// Type of application to scaffold (required; e.g. library, cmdline, tui, gui)
    #[arg(long = "type", value_name = "TYPE")]
    pub app_type: Option<String>,
    /// Optional framework stack to layer on the base layout (e.g. axum)
    #[arg(long, value_name = "STACK")]
    pub stack: Option<String>,
    /// Create a GitHub repository and push the initial commit
    #[arg(long, conflicts_with = "gitlab")]
    pub github: bool,
    /// Create a GitLab repository and push the initial commit
    #[arg(long)]
    pub gitlab: bool,
    /// Positional `help` word: `ragent new help` prints the scaffold usage
    /// page (slash parity, FR-012) instead of scaffolding
    #[arg(value_name = "HELP_WORD", num_args = 0..=1, hide = true)]
    pub help_word: Option<String>,
}

impl ScaffoldArgs {
    /// Convert the clap flag set into the token list `/new` would see after
    /// the verb (the engine parser's single source of truth).
    pub fn to_tokens(&self) -> Vec<String> {
        let mut argv: Vec<String> = Vec::new();
        if self.help_word.is_some() {
            argv.push("help".to_owned());
        }
        if let Some(lang) = &self.language {
            argv.push("--language".to_owned());
            argv.push(lang.clone());
        }
        if let Some(app_type) = &self.app_type {
            argv.push("--type".to_owned());
            argv.push(app_type.clone());
        }
        if let Some(stack) = &self.stack {
            argv.push("--stack".to_owned());
            argv.push(stack.clone());
        }
        if self.github {
            argv.push("--github".to_owned());
        }
        if self.gitlab {
            argv.push("--gitlab".to_owned());
        }
        argv
    }
}

/// Sub-commands for the `new` namespace: the redundant subcommand spelling
/// kept so `ragent new scaffold ...` also parses (the flattened form
/// `ragent new --language ...` is the documented surface).
#[derive(clap::Subcommand, Debug)]
pub enum NewCommands {
    /// Scaffold a new project in the current directory
    Scaffold(#[command(flatten)] ScaffoldArgs),
}

/// CLI parity handler for `/new` (spec `newproj` T-012, FR-013).
///
/// Reuses the same `ragent-tools-extended::project_scaffold` engine as the
/// TUI `/new` command: FR-002 empty-directory guard, FR-003 validation, the
/// FR-016 no-silent-overwrite emission, the FR-008 git + remote half, and the
/// FR-011 summary - printed to stdout instead of the TUI message window.
///
/// # Errors
///
/// Returns an error when the current directory cannot be read; flag
/// validation, guard, and emission failures are printed as diagnostics with
/// process exits instead (matching the `ragent-research` CLI precedent).
pub fn handle_new_command(command: NewCommands) -> Result<()> {
    let NewCommands::Scaffold(scaffold) = command;
    let argv = scaffold.to_tokens();
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    handle_new_tokens(&argv)
}

/// Execute an already-tokenised `/new` argument list.
///
/// Shared tail of [`handle_new_command`] and [`handle_new_prompt`]:
/// validation, guard, emission, git, remote, and the FR-011 summary.
fn handle_new_tokens(tokens: &[&str]) -> Result<()> {
    use ragent_tools_extended::project_scaffold::parse_flags;

    let parsed = match parse_flags(tokens) {
        Ok(request) => request,
        Err(err) => {
            eprintln!("ragent new: {err}");
            eprintln!();
            eprint!("{}", new_usage_message());
            return Err(CliExit::usage().into());
        }
    };
    if parsed.is_help() {
        print!("{}", new_usage_message());
        return Ok(());
    }
    run_new_scaffold(&parsed)
}

/// Run the scaffold pipeline for a validated [`ScaffoldRequest`].
///
/// Mirrors the TUI `newproj::run_scaffold` flow: FR-002 guard, the shared
/// [`plan_and_emit`](ragent_tools_extended::project_scaffold::plan_and_emit)
/// engine pipeline (FR-004/FR-005/FR-006/FR-007/FR-016/FR-019), the FR-008
/// local git + remote half, and the FR-011 summary on stdout.
fn run_new_scaffold(
    request: &ragent_tools_extended::project_scaffold::ScaffoldRequest,
) -> Result<()> {
    use ragent_tools_extended::project_scaffold::{
        HostingTarget, RemoteStatus, enforce_empty_directory_guard, init_and_commit,
        init_github_remote_with_token, init_gitlab_remote, load_github_token, plan_and_emit,
        recipe_for,
    };

    let cwd = std::env::current_dir().map_err(|e| anyhow::anyhow!("cannot read cwd: {e}"))?;

    // FR-002: refuse non-empty targets before any filesystem writes.
    if let Err(err) = enforce_empty_directory_guard(&cwd) {
        eprintln!("ragent new: {err}");
        return Err(CliExit::failure().into());
    }

    let slug = cwd
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "new-project".to_owned());

    let Some(recipe) = recipe_for(request.language()) else {
        eprintln!("ragent new: internal: no scaffold recipe for the selected language");
        return Err(CliExit::usage().into());
    };

    // FR-004 + FR-005 + FR-006 + FR-007 + FR-016 + FR-019 planning and
    // emission through the shared engine pipeline.
    let generated_at_utc = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let (mut summary, stack_note) =
        match plan_and_emit(&cwd, request, &slug, recipe, &generated_at_utc) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("ragent new: {err}");
                return Err(CliExit::failure().into());
            }
        };

    // FR-008 local half: git init + initial commit, then the remote half
    // when a hosting flag was supplied. Failures are contained (FR-010):
    // the summary reports the failed step while the local scaffold stays
    // intact.
    let git_outcome = init_and_commit(&cwd, "Initial scaffold");
    let remote_status = match request.hosting() {
        None => RemoteStatus::None,
        Some(HostingTarget::GitHub) => {
            // Use the remote-init entry point directly (rather than the
            // `reqwest::blocking` wrapper in `remote.rs`) so the `/new` CLI
            // scaffold can run under the main tokio runtime: the blocking
            // client spawns its own runtime, which panics when the runtime is
            // dropped from an async context. `reqwest::blocking` inside a
            // `spawn_blocking` worker has no ambient runtime, so it is safe.
            let base_url = "https://api.github.com";
            let token = load_github_token();
            let root = cwd.clone();
            let repo_name = slug.clone();
            match tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    tokio::task::spawn_blocking(move || {
                        init_github_remote_with_token(
                            base_url,
                            token.as_deref(),
                            &root,
                            &repo_name,
                            true,
                        )
                    })
                    .await
                })
            }) {
                Ok(Ok(report)) => RemoteStatus::Created { url: report.url },
                Ok(Err(failure)) => RemoteStatus::Failed {
                    step: failure.step.as_str().to_owned(),
                    message: failure.message,
                },
                Err(join_err) => RemoteStatus::Failed {
                    step: "github repo create".to_owned(),
                    message: format!("GitHub remote-init worker failed: {join_err}"),
                },
            }
        }
        Some(HostingTarget::GitLab) => match init_gitlab_remote(&cwd, &slug, true) {
            Ok(report) => RemoteStatus::Created { url: report.url },
            Err(failure) => RemoteStatus::Failed {
                step: failure.step.as_str().to_owned(),
                message: failure.message,
            },
        },
    };
    summary.git = Some(git_outcome);
    summary.remote = remote_status;

    print!("ragent new: {}", summary.render());
    print!("{stack_note}");
    Ok(())
}

/// Dispatch a `ragent run "/new ..."` prompt (spec `newproj` T-012, FR-013).
///
/// `args` is everything after the `/new` verb (leading whitespace already
/// trimmed; may be empty for bare `/new`). Parsing and execution are
/// delegated to [`handle_new_tokens`] so both CLI surfaces behave
/// identically.
///
/// # Errors
///
/// See [`handle_new_command`].
pub fn handle_new_prompt(args: &str) -> Result<()> {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    handle_new_tokens(&tokens)
}

/// Build the FR-012/FR-018 usage/help message for the CLI surface.
///
/// The body (purpose, per-argument docs, registry-derived accepted values,
/// worked examples) comes from the shared engine renderer
/// (`render_detailed_help`, NFR-001); the usage lines and example
/// invocations are the `ragent new` binary surface spellings.
fn new_usage_message() -> String {
    use ragent_tools_extended::project_scaffold::render_detailed_help;

    render_detailed_help(
        "\x20 ragent new --language <lang> --type <type> [--stack <name>] [--github | --gitlab]\n\
         \x20 ragent run \"/new <same flags>\"\n\
         \n\
         Equivalent slash command: /new <same flags>",
        "\x20 ragent new --language rust --type cmdline",
        "\x20 ragent new --language python --type library --gitlab",
    )
}

// ---------------------------------------------------------------------------
// `ragent spec govcreate` CLI parity (spec `govdoc` T-015, FR-020)
// ---------------------------------------------------------------------------

/// Typed clap argument set for `ragent spec govcreate ...`.
///
/// [`GovCreateArgs::to_token_string`] rebuilds the exact token list the TUI
/// `/spec govcreate` slash command would receive, so both surfaces share the
/// single [`ragent_specs::SpecCommand::parse`] parser and cannot drift
/// (FR-020, same pattern as [`ScaffoldArgs::to_tokens`]).
#[derive(clap::Args, Debug)]
pub struct GovCreateArgs {
    /// Spec identifier (validated with the existing spec-ID rules)
    #[arg(value_name = "SPECID")]
    pub spec_id: Option<String>,
    /// Architecture source: an http(s) URL or a local file/folder path
    #[arg(value_name = "CONTENT_REF")]
    pub content_ref: Option<String>,
    /// Directory in which the project is scaffolded (created if missing)
    #[arg(value_name = "TARGET_FOLDER")]
    pub target_folder: Option<String>,
    /// Computer language to scaffold (required; e.g. rust, python, go, ts)
    #[arg(long, value_name = "LANG")]
    pub language: Option<String>,
    /// Type of application to scaffold (required; e.g. library, cmdline, tui, gui)
    #[arg(long = "type", value_name = "TYPE")]
    pub app_type: Option<String>,
    /// Optional framework stack to layer on the base layout (e.g. axum)
    #[arg(long, value_name = "STACK")]
    pub stack: Option<String>,
    /// Create a GitHub repository and push the initial commit
    #[arg(long, conflicts_with = "gitlab")]
    pub github: bool,
    /// Create a GitLab repository and push the initial commit
    #[arg(long)]
    pub gitlab: bool,
    /// Overwrite an existing spec at `<target-folder>/specs/<specid>/`
    #[arg(long)]
    pub force: bool,
}

impl GovCreateArgs {
    /// Rebuild the token list `/spec govcreate` would see after the verb.
    ///
    /// The shared TUI tokenizer treats an unquoted token as one
    /// whitespace-free token; clap positionals arrive pre-split, so each
    /// value is emitted verbatim.
    pub fn to_token_string(&self) -> String {
        let mut tokens: Vec<String> = Vec::new();
        if let Some(spec_id) = &self.spec_id {
            tokens.push(spec_id.clone());
        }
        if let Some(content_ref) = &self.content_ref {
            tokens.push(content_ref.clone());
        }
        if let Some(target_folder) = &self.target_folder {
            tokens.push(target_folder.clone());
        }
        if let Some(lang) = &self.language {
            tokens.push("--language".to_owned());
            tokens.push(lang.clone());
        }
        if let Some(app_type) = &self.app_type {
            tokens.push("--type".to_owned());
            tokens.push(app_type.clone());
        }
        if let Some(stack) = &self.stack {
            tokens.push("--stack".to_owned());
            tokens.push(stack.clone());
        }
        if self.github {
            tokens.push("--github".to_owned());
        }
        if self.gitlab {
            tokens.push("--gitlab".to_owned());
        }
        if self.force {
            tokens.push("--force".to_owned());
        }
        tokens.join(" ")
    }
}

/// Sub-commands for the `spec` namespace (FR-020 CLI parity).
#[derive(clap::Subcommand, Debug)]
pub enum SpecCommands {
    /// Create a project from a system architecture document (URL or local
    /// file/folder): acquire the content, extract the architecture structure,
    /// author the spec, and scaffold the project
    #[command(name = "govcreate")]
    GovCreate(#[command(flatten)] GovCreateArgs),
}

/// Production [`GovCreateStages`] wiring for the CLI parity surface.
///
/// Identical stage behaviour to the TUI dispatch (`TuiGovCreateStages` in the
/// TUI crate): acquisition delegates to the shared `archdoc` helpers, the
/// extraction and authoring stages drive the configured model through
/// [`ragent_agent::send_one_shot`], and the spec write delegates to
/// [`ragent_specs::SpecCommand::write_govcreate_spec`]. Auth resolution comes
/// from `send_one_shot` (environment variables fall back to stored
/// credentials), unlike the TUI's db-only reader.
struct CliGovCreateStages {
    registry: std::sync::Arc<ragent_agent::provider::ProviderRegistry>,
    storage: std::sync::Arc<ragent_agent::storage::Storage>,
    model_ref: ragent_agent::agent::ModelRef,
}

impl CliGovCreateStages {
    /// One-shot completion against the resolved provider/model.
    async fn complete(&self, system: &str, user: &str) -> Result<String> {
        ragent_agent::send_one_shot(
            std::sync::Arc::clone(&self.registry),
            Some(std::sync::Arc::clone(&self.storage)),
            self.model_ref.clone(),
            Some(system.to_owned()),
            user.to_owned(),
            None,
        )
        .await
    }
}

impl ragent_tools_extended::archdoc::GovCreateStages for CliGovCreateStages {
    async fn acquire(
        &self,
        reference: &ragent_tools_extended::archdoc::ContentRef,
    ) -> Result<
        ragent_tools_extended::archdoc::GatheredCorpus,
        ragent_tools_extended::archdoc::GovCreateRunError,
    > {
        use ragent_tools_extended::archdoc::{
            AcquisitionBudget, ContentRef, GovCreateRunError, LocalAcquisitionBudget,
            acquire_local, acquire_url,
        };
        match reference {
            ContentRef::Url(url) => acquire_url(url, &AcquisitionBudget::default())
                .await
                .map_err(|e| GovCreateRunError::Acquire(e.to_string())),
            ContentRef::Local(path) => {
                // The walk + per-file extraction is blocking I/O; offload so
                // the CLI runtime's worker is not stalled.
                let path = path.clone();
                let budget = LocalAcquisitionBudget::default();
                tokio::task::spawn_blocking(move || acquire_local(&path, &budget))
                    .await
                    .map_err(|e| GovCreateRunError::Acquire(e.to_string()))
            }
        }
    }

    async fn extract(
        &self,
        corpus: &ragent_tools_extended::archdoc::GatheredCorpus,
    ) -> Result<
        ragent_tools_extended::archdoc::ArchitectureStructure,
        ragent_tools_extended::archdoc::GovCreateRunError,
    > {
        use ragent_tools_extended::archdoc::{
            GovCreateRunError, build_arch_extraction_prompt, fallback_structure,
            parse_architecture_response,
        };
        let prompt = build_arch_extraction_prompt(corpus);
        let response = self
            .complete(
                "You are a software architect reading architecture documentation.",
                &prompt,
            )
            .await
            .map_err(|e| GovCreateRunError::Extract(e.to_string()))?;
        // FR-007 fallback: an unparseable model output degrades to the
        // deterministic mechanical structure - extraction never aborts a
        // non-empty corpus.
        Ok(parse_architecture_response(&response).unwrap_or_else(|| fallback_structure(corpus)))
    }

    async fn author(
        &self,
        structure: &ragent_tools_extended::archdoc::ArchitectureStructure,
        run: &ragent_tools_extended::archdoc::GovCreateRun,
    ) -> Result<
        ragent_tools_extended::archdoc::AuthoredSpec,
        ragent_tools_extended::archdoc::GovCreateRunError,
    > {
        use ragent_tools_extended::archdoc::{AuthoredSpec, GovCreateRunError};

        let prompt = ragent_specs::SpecCommand::build_govcreate_prompt(
            &run.spec_id,
            structure,
            &run.content_ref,
            run.target_folder.to_string_lossy().as_ref(),
            &run.scaffold,
        );
        let body = self
            .complete("You are an expert specification writer.", &prompt)
            .await
            .map_err(|e| GovCreateRunError::Author(e.to_string()))?;
        // The splitter is shared with the TUI slash surface so both produce
        // the same sections from the same response.
        let (spec_md, plan_md, testplan_md) =
            ragent_tools_extended::archdoc::split_authored_sections(&body);
        Ok(AuthoredSpec {
            spec_md,
            plan_md,
            testplan_md,
        })
    }

    async fn write_spec(
        &self,
        run: &ragent_tools_extended::archdoc::GovCreateRun,
        authored: &ragent_tools_extended::archdoc::AuthoredSpec,
    ) -> Result<(), ragent_tools_extended::archdoc::GovCreateRunError> {
        ragent_specs::SpecCommand::write_govcreate_spec(
            &run.target_folder,
            &run.invoking_root,
            &run.spec_id,
            &authored.spec_md,
            &authored.plan_md,
            &authored.testplan_md,
            run.force,
        )
        .await
        .map(|_| ())
        .map_err(|e| ragent_tools_extended::archdoc::GovCreateRunError::Write(e.to_string()))
    }
}

/// Execute one tokenised `spec govcreate` invocation (FR-020).
///
/// `tokens` is the exact argument tail the TUI slash command receives after
/// the verb (produced from typed CLI flags by [`GovCreateArgs::to_token_string`]);
/// parsing is delegated to [`ragent_specs::SpecCommand::parse`] so the slash
/// and CLI surfaces share one parser and one usage text. Progress renders to
/// stdout as the runner's FR-015 events stream in; the terminal report is the
/// same NFR-005 block the TUI renders. Exit codes: 0 success, 1
/// guard/acquisition/stage failure (FR-013/FR-014), 2 usage error (FR-003).
///
/// # Errors
///
/// Returns an error only for infrastructure failures outside the runner's
/// contained-failure contract (e.g. reading the current directory).
pub async fn handle_govcreate_command(
    tokens: &str,
    model_ref: ragent_agent::agent::ModelRef,
    provider_registry: std::sync::Arc<ragent_agent::provider::ProviderRegistry>,
    storage: std::sync::Arc<ragent_agent::storage::Storage>,
    invoking_root: std::path::PathBuf,
) -> Result<()> {
    use ragent_specs::SpecCommand;
    use ragent_tools_extended::archdoc::{
        CancellationToken, GovCreateRun, run_govcreate_with_progress,
    };

    match SpecCommand::parse(&format!("govcreate {tokens}")) {
        SpecCommand::GovCreate {
            spec_id,
            content_ref,
            target_folder,
            scaffold,
            force,
        } => {
            let run = GovCreateRun {
                spec_id,
                content_ref,
                target_folder: std::path::PathBuf::from(&target_folder),
                scaffold,
                force,
                invoking_root,
            };
            let stages = CliGovCreateStages {
                registry: provider_registry,
                storage,
                model_ref,
            };
            let cancel = CancellationToken::none();
            let mut sink = |event: ragent_tools_extended::archdoc::GovCreateProgress| {
                println!("{}", event.render());
            };
            let report = run_govcreate_with_progress(&run, &stages, &cancel, &mut sink).await;
            println!("From: /spec govcreate\n\n{}", report.render());
            if !report.succeeded() {
                return Err(CliExit::failure().into());
            }
            Ok(())
        }
        SpecCommand::GovCreateUsage(reason) => {
            eprintln!("ragent spec govcreate: {reason}");
            eprintln!();
            eprint!(
                "{}",
                govcreate_usage_message().replace("/spec govcreate", "ragent spec govcreate")
            );
            Err(CliExit::usage().into())
        }
        _ => {
            // `SpecCommand::Unknown("govcreate")`: missing positionals (FR-003).
            eprint!(
                "{}",
                govcreate_usage_message().replace("/spec govcreate", "ragent spec govcreate")
            );
            Err(CliExit::usage().into())
        }
    }
}

/// Handle `ragent automation <args...>` (spec `openhands` T-016; FR-018).
///
/// The CLI parity surface of the `/automation` slash family:
///
/// - `ragent automation list` - list configured automations with trigger,
///   backend, and next-due time (reads the resolved config synchronously).
/// - `ragent automation runs <id>` - print an automation's run history.
/// - `ragent automation run <id>` - enqueue a manual run (best-effort; the
///   spawned run executes on a background task which the process does not
///   outlive, so use the server/TUI surface for a long-lived run).
/// - `ragent automation help` (or a bare `ragent automation`) - usage.
///
/// # Errors
///
/// Returns a usage error for a malformed invocation, or a storage error.
pub fn handle_automation_command(args: &[String], storage: std::sync::Arc<Storage>) -> Result<()> {
    let joined = args.join(" ");
    let text = joined.trim();
    let mut parts = text.split_whitespace();
    let sub = parts.next().unwrap_or("").to_lowercase();
    let rest: Vec<&str> = parts.collect();

    match sub.as_str() {
        "help" | "--help" | "-h" => {
            println!("{}", automation_usage());
            Ok(())
        }
        "runs" => {
            let Some(id) = rest.first() else {
                eprintln!("ragent automation runs: missing automation id");
                return Err(CliExit::usage().into());
            };
            let runs = storage.list_automation_runs(id, 50)?;
            if runs.is_empty() {
                println!("No run history for `{id}`.");
                return Ok(());
            }
            println!("Run history for `{id}` ({}):", runs.len());
            println!();
            println!(
                "{:<38} {:<9} {:<10} {:<8} STARTED",
                "RUN", "TRIGGER", "OUTCOME", "BACKEND"
            );
            for run in &runs {
                println!(
                    "{:<38} {:<9} {:<10} {:<8} {}",
                    run.id,
                    run.trigger.as_str(),
                    run.outcome.map_or("running", |o| o.as_str()),
                    run.backend,
                    run.started_at.to_rfc3339(),
                );
            }
            Ok(())
        }
        "run" => {
            let Some(id) = rest.first() else {
                eprintln!("ragent automation run: missing automation id");
                return Err(CliExit::usage().into());
            };
            let config = ragent_config::Config::load().unwrap_or_default();
            let automation = config.automation_config();
            if !automation.is_enabled() {
                eprintln!("ragent automation run: the automation service is disabled");
                return Err(CliExit::failure().into());
            }
            let Some(def) = automation.get(id).filter(|d| d.enabled) else {
                eprintln!("ragent automation run: no enabled automation `{id}`");
                return Err(CliExit::failure().into());
            };
            // Insert the run record in the running state. The agent turn is
            // driven by the long-lived TUI or server scheduler; a one-shot CLI
            // process has no session processor, so the run is recorded here as a
            // manual enqueue and the caller is pointed at the live surface.
            let run = ragent_types::AutomationRun::start(
                uuid::Uuid::new_v4().to_string(),
                def.id.clone(),
                ragent_types::AutomationTrigger::Manual,
                def.backend_label().to_string(),
            );
            storage.insert_automation_run(&run)?;
            let _ = storage.finish_automation_run(&run.id, ragent_types::RunOutcome::Error, None);
            println!(
                "Recorded manual run `{}` for `{id}` on backend `{}`. Run the live `ragent \
                 serve` or TUI session to execute it.",
                run.id,
                def.backend_label()
            );
            Ok(())
        }
        "" | "list" => {
            let config = ragent_config::Config::load().unwrap_or_default();
            let automation = config.automation_config();
            if automation.automations.is_empty() {
                println!("No automations configured under the `automation` block in ragent.json.");
                return Ok(());
            }
            let now = chrono::Utc::now();
            println!(
                "Configured automations ({}), service {}:",
                automation.automations.len(),
                if automation.is_enabled() { "on" } else { "off" }
            );
            println!();
            println!(
                "{:<20} {:<9} {:<16} {:<8} NEXT DUE",
                "ID", "TRIGGER", "SCHEDULE", "BACKEND"
            );
            for def in &automation.automations {
                let next_due = def
                    .trigger
                    .schedule()
                    .and_then(|expr| ragent_agent::automation::compute_next_due(expr, now))
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "{:<20} {:<9} {:<16} {:<8} {}",
                    def.id,
                    def.trigger.label(),
                    def.trigger.schedule().unwrap_or("-"),
                    def.backend_label(),
                    next_due,
                );
            }
            Ok(())
        }
        other => {
            eprintln!("ragent automation: unknown subcommand `{other}`");
            eprintln!();
            eprint!("{}", automation_usage());
            Err(CliExit::usage().into())
        }
    }
}

/// The usage block for `ragent automation`, mirroring the `/automation help`
/// wording so the two surfaces stay in step.
fn automation_usage() -> String {
    let mut out = String::from(
        "ragent automation - automation service\n\n\
         Usage:\n\
         \x20 ragent automation list                List configured automations\n\
         \x20 ragent automation runs <id>           Show an automation's run history\n\
         \x20 ragent automation run <id>            Enqueue a manual run\n\
         \x20 ragent automation help                Show this help\n\n\
         Automations are declared under the `automation` block in ragent.json.\n",
    );
    out.push('\n');
    out
}

/// The shared govcreate usage block, minus the TUI `From:` header line so the
/// CLI surface prints plain usage text (NFR-005 keeps the body identical).
fn govcreate_usage_message() -> String {
    let body = ragent_specs::SpecCommand::build_govcreate_help_message()
        .lines()
        .skip(2)
        .collect::<Vec<_>>()
        .join("\n");
    format!("{body}\n")
}

#[cfg(test)]
#[path = "../tests/inline/cli_tests.rs"]
mod tests;
