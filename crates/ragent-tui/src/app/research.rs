//! Research/skill command handling for the TUI.
//!
//! Extracted from `app.rs` in REMPLAN.md M5 / T5.3.

use ragent_agent::event::Event;
use ragent_agent::storage::Storage;
use ragent_agent::{agent::ModelRef, event::EventBus, provider::ProviderRegistry};
use std::sync::Arc;

use crate::research_adapter::TuiResearchObserver;

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{App, LogLevel};

// Helpers

// Re-export status types from theme

/// Run concept-extraction for a `/research cluster` command end-to-end.
///
/// All progress and result events are published directly to `event_bus`; the
/// caller only has to spawn this future. Returns the written CONCEPTS.md path
/// on success, or an error string suitable for a final status line.
async fn run_cluster_extraction(
    root: &std::path::Path,
    valid_name: &ragent_research::ResearchName,
    name: &str,
    model_ref: ModelRef,
    model_label: &str,
    context_window: usize,
    provider_registry: Arc<ProviderRegistry>,
    storage: Option<Arc<Storage>>,
    event_bus: Arc<EventBus>,
    session_id: String,
) -> Result<std::path::PathBuf, String> {
    use crate::research_progress::{SessionPhase, encode_cluster_progress_event};

    let session_id_for_notice = session_id.clone();
    let publish_progress = |phase: SessionPhase, status: &str, detail: String| {
        event_bus.publish(Event::AgentNotice {
            session_id: session_id_for_notice.clone(),
            message: encode_cluster_progress_event(name, name, phase, status, &detail),
        });
    };

    publish_progress(
        SessionPhase::Setup,
        "started",
        "reading sources…".to_string(),
    );

    let payload = ragent_research::build_cluster_payload(root, valid_name, Some(context_window))
        .await
        .map_err(|e| format!("failed to read sources: {e}"))?;

    let file_list = payload.files.join(", ");
    publish_progress(
        SessionPhase::Setup,
        "done",
        format!(
            "read {} source file(s): {file_list}. payload: {} / {} bytes (truncated: {})",
            payload.files.len(),
            payload.total_bytes,
            payload.max_bytes,
            payload.truncated,
        ),
    );
    publish_progress(
        SessionPhase::Synthesize,
        "started",
        format!("sending concept-extraction prompt to {model_label}…"),
    );

    let prompt = ragent_research::build_concept_extraction_prompt(&payload);
    let text = ragent_agent::send_one_shot(
        provider_registry,
        storage,
        model_ref,
        None,
        prompt,
        Some(4_096),
    )
    .await
    .map_err(|e| format!("LLM call failed: {e}"))?;

    publish_progress(
        SessionPhase::Synthesize,
        "done",
        format!("LLM returned {} bytes", text.len()),
    );
    publish_progress(
        SessionPhase::Finalize,
        "started",
        "writing CONCEPTS.md…".to_string(),
    );

    let path = ragent_research::write_concepts_md(root, valid_name, &text)
        .await
        .map_err(|e| format!("failed to write CONCEPTS.md: {e}"))?;

    publish_progress(
        SessionPhase::Finalize,
        "done",
        format!("wrote CONCEPTS.md ({} bytes)", text.len()),
    );
    event_bus.publish(Event::TextDelta {
        session_id,
        text: format!(
            "From: /research cluster\n\n[ok] Concept extraction finished for `{name}`. \
             Wrote `{path}` ({} bytes).\n\n{text}",
            text.len(),
            path = path.display()
        ),
    });

    Ok(path)
}

impl App {
    pub(crate) fn handle_research_command(&mut self, args: &str) {
        use ragent_research::cli::ResearchCliCommand;
        use ragent_research::{
            OutputFormat, ResearchIo, ResearchManager, ResearchMode, ResearchName,
        };
        use std::sync::Arc;

        let cmd = ResearchCliCommand::parse(args);
        // Record the verbatim slash command for frontmatter replay.
        let invocation = format!("/research {args}");
        if matches!(cmd, ResearchCliCommand::Help) {
            self.append_assistant_text(&ResearchCliCommand::build_help_message());
            self.status = "research: help".to_string();
            return;
        }
        // `self.cwd` is a `~`-collapsed DISPLAY string (see `App::new`), so it
        // must never be used to build filesystem paths: joining "research"
        // onto "~/Projects/ragent" yields a relative path that tokio fs
        // resolves against the process cwd, creating a literal `~/` directory
        // tree inside the project. `cwd_path` carries the real absolute path.
        let manager = ResearchManager::new(self.cwd_path.join("research"));
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
                from_files,
                iterations,
                depth,
                tier,
                mode,
                summarization_model,
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
                no_papers,
                use_pdf,
                fetch_timeout_secs,
                local_concurrency,
                web_phase_timeout_secs,
                local_phase_timeout_secs,
                search_max_retries,
                search_retry_base_delay_ms,
                search_circuit_breaker_threshold,
                max_web_results,
                max_search_calls,
                max_local_sources,
                max_synthesis_sources,
                brief,
                evaluate,
            } => {
                // Use the `[wait]` prefix so the status is treated as
                // async-in-progress and NOT auto-expired to "ready" by
                // [`App::arm_status_expiry`] while the background research
                // session is still running. The live progress events update
                // this status per-phase (see the AgentNotice handler), and
                // the completion notice sets a terminal status.
                self.status = format!("[wait] research: {name}…");
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
                let title = ragent_research::derive_title_files(
                    &topic,
                    from_urls.first().map(String::as_str),
                    &from_files,
                );
                let req = ragent_research::ResearchRunRequest {
                    name: name.clone(),
                    topic: topic.clone(),
                    title: Some(title.clone()),
                    from_urls: from_urls.clone(),
                    from_files: from_files.clone(),
                    sources_dir,
                    template,
                    depth,
                    tier,
                    mode,
                    iterations,
                    output_format: format.clone(),
                    use_local,
                    use_specs,
                    use_low_relevance,
                    no_scholarly: no_papers,
                    use_pdf,
                    fetch_concurrency,
                    local_concurrency,
                    fetch_timeout_secs,
                    web_phase_timeout_secs,
                    local_phase_timeout_secs,
                    search_max_retries,
                    search_retry_base_delay_ms,
                    search_circuit_breaker_threshold,
                    max_web_results,
                    max_search_calls,
                    max_local_sources,
                    max_synthesis_sources,
                    summarization_model,
                    clarify,
                    brief,
                    research_model,
                    compression_model,
                    final_report_model,
                    max_concurrent_research_units,
                    evaluate: Some(evaluate),
                    invocation: Some(invocation.clone()),
                };
                let config = ragent_research::build_session_config(&req, config_arc.as_deref());
                let session = crate::research_adapter::build_research_session(
                    &self.session_processor.tool_registry,
                    manager.clone(),
                    self.session_id.clone().unwrap_or_default(),
                    self.cwd_path.clone(),
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
                let from_urls_for_msg = config.input.from_urls.clone();
                let from_files_for_msg = config.input.from_files.clone();
                let mode_override = config.engine.mode;
                let event_bus_for_spawn = self.event_bus.clone();
                let session_id_for_spawn = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    let outcome = session
                        .run(&name_for_spawn, &title, &config, observer_clone)
                        .await;
                    match outcome {
                        Ok(o) => {
                            let provider_calls = if o.provider_tool_calls.is_empty() {
                                String::new()
                            } else {
                                let per_tool = o
                                    .provider_tool_calls
                                    .iter()
                                    .map(|(tool, count)| format!("{tool}: {count}"))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let total: usize =
                                    o.provider_tool_calls.iter().map(|(_, count)| count).sum();
                                format!(", {} search request(s) ({})", total, per_tool)
                            };
                            event_bus_for_spawn.publish(Event::AgentNotice {
                                session_id: session_id_for_spawn.clone(),
                                message: format!(
                                    "research: created research/{} with {} sources{}",
                                    o.research_name,
                                    o.sources.len(),
                                    provider_calls
                                ),
                            })
                        }
                        Err(e) => event_bus_for_spawn.publish(Event::AgentNotice {
                            session_id: session_id_for_spawn.clone(),
                            message: format!("research: error: {e}"),
                        }),
                    }
                });
                let resolved_format = format
                    .as_deref()
                    .and_then(OutputFormat::parse)
                    .unwrap_or(if mode_override == ResearchMode::Competitive {
                        OutputFormat::ComparisonTable
                    } else {
                        OutputFormat::Report
                    })
                    .as_str();
                let resolved_mode = mode_override.as_str();
                let subject_line = if topic_for_assistant.is_empty() {
                    if !from_urls_for_msg.is_empty() {
                        format!("URL(s): {}", from_urls_for_msg.join(", "))
                    } else if !from_files_for_msg.is_empty() {
                        let paths: Vec<String> = from_files_for_msg
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect();
                        format!("File(s): {}", paths.join(", "))
                    } else {
                        "URL: ".to_string()
                    }
                } else {
                    format!("Topic: {topic_for_assistant}")
                };
                let rendered = format!(
                    "From: /research create\n\
                     📝 **Gathering sources for `{name}`…**\n\n\
                     {subject_line}\n\
                     Mode: `{resolved_mode}`\n\
                     Format: `{resolved_format}`\n\n\
                     Watch the progress log below for each phase (setup, web, local, specs, synthesize, assemble, finalize).\n\
                     Tip: run `/research list` once finished, or `/research open {name}` to view the result."
                );
                self.append_assistant_text(&rendered);
            }
            ResearchCliCommand::List { all, .. } => {
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
            ResearchCliCommand::Search { query, .. } => {
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
            ResearchCliCommand::Show { name, .. } => {
                self.status = format!("research: showing '{name}'…");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    match mgr.show(&name).await {
                        Ok(item) => {
                            let sources: Vec<(String, String, String, String, Option<String>)> =
                                item.sources
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
            ResearchCliCommand::Delete { name, yes, .. } => {
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
                                    "From: /research delete\n\n[ok] Deleted research/{name}."
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
            ResearchCliCommand::Archive { name, .. } => {
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
                                    "From: /research archive\n\n[ok] Archived research/{name}."
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
            ResearchCliCommand::Continue { name, message, .. } => {
                self.append_assistant_text(&format!(
                    "From: /research continue\n\nResuming `{name}`{}",
                    message
                        .as_ref()
                        .map(|m| format!(" with follow-up: {m}"))
                        .unwrap_or_default()
                ));
            }
            ResearchCliCommand::Update { name } => {
                // Replay the invocation recorded in the item's frontmatter and
                // overwrite RESEARCH.md (and its supporting files) with a
                // fresh run. The lookup is async, so all item work happens in
                // the spawned task — mirroring the `create` spawn pattern.
                self.status = format!("[wait] research: update {name}…");
                self.push_log_no_agent(LogLevel::Info, format!("research: update '{name}'"));
                self.research_progress
                    .push(crate::research_progress::ResearchProgress::new(
                        &name,
                        "update (replay recorded invocation)",
                    ));
                self.refresh_research_progress_message(&name);
                let manager_for_spawn = manager.clone();
                let config_arc = ragent_agent::Config::load().ok().map(Arc::new);
                let session_id = self.session_id.clone().unwrap_or_default();
                let event_bus = self.event_bus.clone();
                let storage = self.storage.clone();
                let provider_registry = self.provider_registry.clone();
                let active_model = self.agent_info.model.clone();
                let cwd = self.cwd_path.clone();
                let tool_registry = self.session_processor.tool_registry.clone();
                let observer_for_spawn = Arc::new(TuiResearchObserver {
                    app_event_bus: self.event_bus.clone(),
                    session_id: self.session_id.clone().unwrap_or_default(),
                    name: name.clone(),
                    topic: String::new(),
                });
                tokio::spawn(async move {
                    let run_result: Result<_, String> = async {
                        let item = manager_for_spawn
                            .show(&name)
                            .await
                            .map_err(|e| format!("research item `{name}` not found: {e}"))?;
                        let recorded = item.invocation.clone().ok_or_else(|| {
                            format!(
                                "research/{name} has no invocation recorded in its \
                                 frontmatter; only runs created with an invocation-aware \
                                 front-end can be replayed"
                            )
                        })?;
                        let mut req =
                            ragent_research::ResearchRunRequest::from_invocation(&recorded)
                                .map_err(|e| format!("cannot replay research/{name}: {e}"))?;
                        // The item name and title from the frontmatter are
                        // authoritative for a replay.
                        req.name = name.clone();
                        req.title = Some(item.title.clone());
                        let config =
                            ragent_research::build_session_config(&req, config_arc.as_deref());
                        let session = crate::research_adapter::build_research_session(
                            &tool_registry,
                            manager_for_spawn.clone(),
                            session_id.clone(),
                            cwd,
                            event_bus.clone(),
                            Some(storage),
                            config_arc,
                            Some(provider_registry),
                            active_model,
                            Some(name.as_str()),
                        );
                        event_bus.publish(Event::AgentNotice {
                            session_id: session_id.clone(),
                            message: format!(
                                "research: updating `{name}` — replaying `{recorded}`"
                            ),
                        });
                        session
                            .run(&name, &item.title, &config, observer_for_spawn.clone())
                            .await
                            .map_err(|e| e.to_string())
                    }
                    .await;
                    match run_result {
                        Ok(o) => {
                            let provider_calls = if o.provider_tool_calls.is_empty() {
                                String::new()
                            } else {
                                let per_tool = o
                                    .provider_tool_calls
                                    .iter()
                                    .map(|(tool, count)| format!("{tool}: {count}"))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let total: usize =
                                    o.provider_tool_calls.iter().map(|(_, count)| count).sum();
                                format!(" {total} search request(s) ({per_tool})")
                            };
                            event_bus.publish(Event::TextDelta {
                                session_id: session_id.clone(),
                                text: format!(
                                    "From: /research update\n\n\
                                     ✅ **Updated `research/{}`** with {} sources.{}\n\n\
                                     Tip: run `/research open {}` to view the refreshed report.",
                                    o.research_name,
                                    o.sources.len(),
                                    provider_calls,
                                    o.research_name
                                ),
                            });
                            event_bus.publish(Event::AgentNotice {
                                session_id: session_id.clone(),
                                message: format!(
                                    "research: updated research/{} with {} sources",
                                    o.research_name,
                                    o.sources.len()
                                ),
                            });
                        }
                        Err(e) => {
                            event_bus.publish(Event::TextDelta {
                                session_id: session_id.clone(),
                                text: format!("From: /research update\n\n**Error:** {e}"),
                            });
                            event_bus.publish(Event::AgentNotice {
                                session_id: session_id.clone(),
                                message: format!("research: error: {e}"),
                            });
                        }
                    }
                });
            }
            ResearchCliCommand::Cluster { name, force, .. } => match ResearchName::try_new(&name) {
                Ok(valid_name) => {
                    let root = manager.root().to_path_buf();
                    let item_dir = ResearchIo::item_dir(&root, &valid_name);
                    let sources_dir = ResearchIo::sources_dir(&root, &valid_name);
                    if !item_dir.exists() {
                        self.status = format!("research: cluster '{name}' folder missing");
                        self.append_assistant_text(&format!(
                                "From: /research cluster\n\n**Error:** research folder `research/{name}` does not exist."
                            ));
                    } else if !sources_dir.exists() || !sources_dir.is_dir() {
                        self.status = format!("research: cluster '{name}' no sources");
                        self.append_assistant_text(&format!(
                                "From: /research cluster\n\n**Error:** `research/{name}/sources/` folder not found."
                            ));
                    } else {
                        let is_empty = match std::fs::read_dir(&sources_dir) {
                            Ok(entries) => entries.count() == 0,
                            Err(_) => true,
                        };
                        if is_empty {
                            self.status = format!("research: cluster '{name}' empty sources");
                            self.append_assistant_text(&format!(
                                    "From: /research cluster\n\n**Error:** `research/{name}/sources/` is empty."
                                ));
                            return;
                        }
                        let concepts_path = ResearchIo::concepts_md_path(&root, &valid_name);
                        if concepts_path.exists() && !force {
                            self.status = format!("research: cluster '{name}' already clustered");
                            self.append_assistant_text(&format!(
                                    "From: /research cluster\n\n**Error:** `research/{name}/CONCEPTS.md` already exists. \
                                     Re-run with `--force` to overwrite it."
                                ));
                            return;
                        }
                        let model_ref = self
                            .selected_model
                            .as_deref()
                            .and_then(|s| s.split_once('/'))
                            .map(|(p, m)| ragent_agent::agent::ModelRef {
                                provider_id: p.to_string(),
                                model_id: m.to_string(),
                            });
                        let Some(model_ref) = model_ref else {
                            self.status = format!("research: cluster '{name}' no model selected");
                            self.append_assistant_text(&format!(
                                "From: /research cluster\n\n**Error:** no model selected. \
                                       Choose a provider/model with /provider or /model first."
                            ));
                            return;
                        };
                        let context_window = self
                            .selected_model_context_window()
                            .unwrap_or(ragent_research::DEFAULT_CONTEXT_WINDOW_TOKENS);
                        let model_label = self.selected_model.clone().unwrap_or_default();

                        self.status = format!("[wait] research: cluster '{name}' reading sources…");
                        self.append_assistant_text(&format!(
                            "From: /research cluster\n\n[ok] Request accepted for `{name}` \
                               (force={force}). Reading sources and preparing payload; \
                               progress updates will appear below."
                        ));

                        let provider_registry = self.provider_registry.clone();
                        let storage = Some(self.storage.clone());
                        let event_bus = self.event_bus.clone();
                        let session_id = self
                            .session_id
                            .clone()
                            .unwrap_or_else(|| "cluster".to_string());

                        tokio::spawn(async move {
                            match run_cluster_extraction(
                                &root,
                                &valid_name,
                                &name,
                                model_ref,
                                &model_label,
                                context_window,
                                provider_registry,
                                storage,
                                event_bus.clone(),
                                session_id.clone(),
                            )
                            .await
                            {
                                Ok(_) => {
                                    event_bus.publish(Event::AgentNotice {
                                        session_id,
                                        message: format!(
                                            "research: cluster '{name}' extraction complete"
                                        ),
                                    });
                                }
                                Err(e) => {
                                    event_bus.publish(Event::AgentNotice {
                                        session_id: session_id.clone(),
                                        message: format!("research: cluster '{name}' failed: {e}"),
                                    });
                                    event_bus.publish(Event::TextDelta {
                                        session_id,
                                        text: format!(
                                            "From: /research cluster\n\n**Error:** {e} for `{name}`"
                                        ),
                                    });
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    self.status = "research: cluster invalid name".to_string();
                    self.append_assistant_text(&format!(
                        "From: /research cluster\n\n**Error:** invalid research name `{name}`: {e}"
                    ));
                }
            },
            ResearchCliCommand::Config => {
                self.append_assistant_text("From: /research config\n\nEffective research defaults come from `ragent.json` and crate-level constants.");
            }
            ResearchCliCommand::Resume { name, .. } => {
                self.append_assistant_text(&format!(
                    "From: /research resume\n\nResuming `{name}` is not yet implemented in the TUI."
                ));
            }
            ResearchCliCommand::Export { name, .. } => {
                self.append_assistant_text(&format!("From: /research export\n\nExporting `{name}` is not yet implemented in the TUI."));
            }
            ResearchCliCommand::Import { path, .. } => {
                self.append_assistant_text(&format!("From: /research import\n\nImporting `{path}` is not yet implemented in the TUI."));
            }
            ResearchCliCommand::Unknown(sub) => {
                self.append_assistant_text(&format!(
                    "From: /research\n\n**Error:** unknown subcommand `{sub}`. Try `/research help`."
                ));
            }
        }
    }
}
