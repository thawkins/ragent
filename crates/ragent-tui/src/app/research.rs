//! Research/skill command handling for the TUI.
//!
//! Extracted from `app.rs` in REMPLAN.md M5 / T5.3.

use ragent_agent::event::Event;

use crate::research_adapter::TuiResearchObserver;

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{App, LogLevel};

// Helpers

// Re-export status types from theme

impl App {
    pub(crate) fn handle_research_command(&mut self, args: &str) {
        use ragent_research::cli::ResearchCliCommand;
        use ragent_research::{Depth, OutputFormat, ResearchManager, SessionConfig};
        use std::sync::Arc;

        let cmd = ResearchCliCommand::parse(args);
        if matches!(cmd, ResearchCliCommand::Help) {
            self.append_assistant_text(ResearchCliCommand::build_help_message());
            self.status = "research: help".to_string();
            return;
        }
        let manager = ResearchManager::new("research");
        if tokio::runtime::Handle::try_current().is_err() {
            self.append_assistant_text(
                "From: /research\n\n**Error:** async runtime not available in this context.",
            );
            return;
        }

        if !self.ensure_session() {
            return;
        }
        let config_arc = ragent_agent::Config::load().ok().map(Arc::new);
        let observer = Arc::new(TuiResearchObserver {
            app_event_bus: self.event_bus.clone(),
            session_id: self.session_id.clone().unwrap_or_default(),
            name: String::new(),
            topic: String::new(),
        });

        match cmd {
            ResearchCliCommand::Help => unreachable!(),
            ResearchCliCommand::Create {
                name,
                topic,
                from_urls,
                from_file,
                iterations,
                depth,
                format,
                sources_dir,
                template,
                fetch_concurrency,
                use_local,
                use_specs,
                use_low_relevance,
                fetch_timeout_secs,
                local_concurrency,
                web_phase_timeout_secs,
                local_phase_timeout_secs,
                search_max_retries,
                search_retry_base_delay_ms,
                search_circuit_breaker_threshold,
            } => {
                // Use the `⏳` prefix so the status is treated as
                // async-in-progress and NOT auto-expired to "ready" by
                // [`App::arm_status_expiry`] while the background research
                // session is still running. The live progress events update
                // this status per-phase (see the AgentNotice handler), and
                // the completion notice sets a terminal status.
                self.status = format!("⏳ research: {name}…");
                self.push_log_no_agent(
                    LogLevel::Info,
                    if !from_urls.is_empty() {
                        format!(
                            "research: create '{name}' from URL(s): {}",
                            from_urls.join(", ")
                        )
                    } else {
                        format!("research: create '{name}' for topic: {topic}")
                    },
                );
                // Seed the live progress tracker so the message window shows
                // a log list of each phase as it runs. Each `/research
                // create` gets its own tracker so older runs stay visible.
                self.research_progress
                    .push(crate::research_progress::ResearchProgress::new(
                        &name, &topic,
                    ));
                self.refresh_research_progress_message(&name);
                // Use the shared `derive_title` so the TUI produces the same
                // full-topic title as the CLI and HTTP server (not just the
                // first word). The session derives the real topic from the
                // fetched page when `--from-url` is used without a topic, or
                // from the extracted document when `--from-file` is used.
                let title = ragent_research::derive_title_full(
                    &topic,
                    from_urls.first().map(String::as_str),
                    from_file.as_deref(),
                );
                let config = SessionConfig {
                    topic: topic.clone(),
                    from_urls,
                    from_file: from_file.map(std::path::PathBuf::from),
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
                    use_low_relevance,
                    fetch_timeout_secs: fetch_timeout_secs.unwrap_or(30),
                    local_concurrency: local_concurrency
                        .unwrap_or(ragent_research::DEFAULT_LOCAL_CONCURRENCY),
                    web_phase_timeout_secs,
                    local_phase_timeout_secs,
                    search_max_retries: search_max_retries
                        .unwrap_or(ragent_research::DEFAULT_SEARCH_MAX_RETRIES),
                    search_retry_base_delay_ms: search_retry_base_delay_ms
                        .unwrap_or(ragent_research::DEFAULT_SEARCH_RETRY_BASE_DELAY_MS),
                    search_circuit_breaker_threshold: search_circuit_breaker_threshold
                        .unwrap_or(ragent_research::DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD),
                    ..SessionConfig::default()
                };
                let session = crate::research_adapter::build_research_session(
                    &self.session_processor.tool_registry,
                    manager.clone(),
                    self.session_id.clone().unwrap_or_default(),
                    std::env::current_dir().unwrap_or_else(|_| self.cwd.clone().into()),
                    self.event_bus.clone(),
                    Some(self.storage.clone()),
                    config_arc,
                    Some(self.provider_registry.clone()),
                    self.agent_info.model.clone(),
                    Some(name.as_str()),
                );
                let observer_clone = Arc::new(TuiResearchObserver {
                    app_event_bus: observer.app_event_bus.clone(),
                    session_id: observer.session_id.clone(),
                    name: name.clone(),
                    topic: topic.clone(),
                });
                let name_for_spawn = name.clone();
                let topic_for_assistant = topic.clone();
                let from_urls_for_msg = config.from_urls.clone();
                let event_bus_for_spawn = self.event_bus.clone();
                let session_id_for_spawn = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    let outcome = session
                        .run(&name_for_spawn, &title, &config, observer_clone)
                        .await;
                    match outcome {
                        Ok(o) => event_bus_for_spawn.publish(Event::AgentNotice {
                            session_id: session_id_for_spawn.clone(),
                            message: format!(
                                "research: created research/{} with {} sources",
                                o.research_name,
                                o.sources.len()
                            ),
                        }),
                        Err(e) => event_bus_for_spawn.publish(Event::AgentNotice {
                            session_id: session_id_for_spawn.clone(),
                            message: format!("research: error: {e}"),
                        }),
                    }
                });
                let resolved_format = format
                    .as_deref()
                    .and_then(OutputFormat::parse)
                    .unwrap_or(OutputFormat::Report)
                    .as_str();
                let subject_line = if topic_for_assistant.is_empty() {
                    if from_urls_for_msg.is_empty() {
                        "URL: ".to_string()
                    } else {
                        format!("URL(s): {}", from_urls_for_msg.join(", "))
                    }
                } else {
                    format!("Topic: {topic_for_assistant}")
                };
                let rendered = format!(
                    "From: /research create\n\
                     📝 **Gathering sources for `{name}`…**\n\n\
                     {subject_line}\n\
                     Format: `{resolved_format}`\n\n\
                     Watch the progress log below for each phase (setup, web, local, specs, synthesize, assemble, finalize).\n\
                     Tip: run `/research list` once finished, or `/research open {name}` to view the result."
                );
                self.append_assistant_text(&rendered);
            }
            ResearchCliCommand::List { all } => {
                self.status = format!(
                    "research: listing items{}…",
                    if all { " (including archived)" } else { "" }
                );
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    match mgr.list(all).await {
                        Ok(items) => {
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
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!(
                                    "From: /research list\n\n```\n{}\n```",
                                    ragent_research::render_list_output(&rows)
                                ),
                            });
                        }
                        Err(e) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!("From: /research list\n\n**Error:** {e}"),
                            });
                        }
                    }
                });
            }
            ResearchCliCommand::Open { name } => {
                self.status = format!("research: opening '{name}'…");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let app_event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    match mgr.show(&name).await {
                        Ok(item) => {
                            let root = mgr.root().to_path_buf();
                            let path =
                                ragent_research::ResearchIo::research_md_path(&root, &item.name);
                            match ragent_research::ResearchIo::read_file(&path).await {
                                Ok(content) => {
                                    let (_frontmatter, body) =
                                        ragent_research::ResearchIo::split_frontmatter(&content);
                                    app_event_bus.publish(
                                        ragent_agent::event::Event::OpenResearchView {
                                            name: item.name.to_string(),
                                            path,
                                            markdown: body,
                                        },
                                    );
                                }
                                Err(e) => {
                                    event_bus.publish(Event::TextDelta {
                                        session_id,
                                        text: format!(
                                            "From: /research open\n\n**Error:** failed to read RESEARCH.md: {e}"
                                        ),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!("From: /research open\n\n**Error:** {e}"),
                            });
                        }
                    }
                });
            }
            ResearchCliCommand::Search { query } => {
                self.status = format!("research: searching for '{query}'…");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    match mgr.search(&query, 25).await {
                        Ok(hits) => {
                            let rows: Vec<(String, String, String)> = hits
                                .into_iter()
                                .map(|h| (h.name, h.title, h.snippet))
                                .collect();
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!(
                                    "From: /research search\n\n```\n{}\n```",
                                    ragent_research::render_search_output(&rows)
                                ),
                            });
                        }
                        Err(e) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!("From: /research search\n\n**Error:** {e}"),
                            });
                        }
                    }
                });
            }
            ResearchCliCommand::Show { name } => {
                self.status = format!("research: showing '{name}'…");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    match mgr.show(&name).await {
                        Ok(item) => {
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
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!(
                                    "From: /research show\n\n```\n{}\n```",
                                    ragent_research::render_show_output(
                                        item.name.as_ref(),
                                        &item.title,
                                        &item.topic,
                                        item.status.as_str(),
                                        &item.created_at.to_rfc3339(),
                                        &item.modified_at.to_rfc3339(),
                                        &sources,
                                    )
                                ),
                            });
                        }
                        Err(e) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!("From: /research show\n\n**Error:** {e}"),
                            });
                        }
                    }
                });
            }
            ResearchCliCommand::Delete { name, yes } => {
                if !yes {
                    self.append_assistant_text(&format!(
                        "From: /research delete\n\nRefusing to delete research/{name} without confirmation. Re-run with `--yes` to skip this prompt."
                    ));
                    return;
                }
                self.status = format!("research: deleting '{name}'…");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    let session_id_for_notice = session_id.clone();
                    match mgr.delete(&name).await {
                        Ok(()) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!(
                                    "From: /research delete\n\n✅ Deleted research/{name}."
                                ),
                            });
                            event_bus.publish(Event::AgentNotice {
                                session_id: session_id_for_notice,
                                message: format!("research: deleted research/{name}"),
                            });
                        }
                        Err(e) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!("From: /research delete\n\n**Error:** {e}"),
                            });
                        }
                    }
                });
            }
            ResearchCliCommand::Archive { name } => {
                self.status = format!("research: archiving '{name}'…");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    match mgr.archive(&name).await {
                        Ok(()) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!(
                                    "From: /research archive\n\n✅ Archived research/{name}."
                                ),
                            });
                        }
                        Err(e) => {
                            event_bus.publish(Event::TextDelta {
                                session_id,
                                text: format!("From: /research archive\n\n**Error:** {e}"),
                            });
                        }
                    }
                });
            }
            ResearchCliCommand::Continue { name, message } => {
                self.append_assistant_text(&format!(
                    "From: /research continue\n\nResuming `{name}`{}",
                    message
                        .as_ref()
                        .map(|m| format!(" with follow-up: {m}"))
                        .unwrap_or_default()
                ));
            }
            ResearchCliCommand::Unknown(sub) => {
                self.append_assistant_text(&format!(
                    "From: /research\n\n**Error:** unknown subcommand `{sub}`. Try `/research help`."
                ));
            }
        }
    }
}
