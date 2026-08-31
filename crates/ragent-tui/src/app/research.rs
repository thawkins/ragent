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
        use ragent_research::{OutputFormat, ResearchIo, ResearchManager, ResearchName};
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
                from_files,
                iterations,
                depth,
                tier,
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
                    ..Default::default()
                };
                let config = ragent_research::build_session_config(&req, config_arc.as_deref());
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
                let from_urls_for_msg = config.input.from_urls.clone();
                let from_files_for_msg = config.input.from_files.clone();
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
                            let sources: Vec<(String, String, String, String, Option<String>)> =
                                item.sources
                                    .iter()
                                    .map(|s| {
                                        (
                                            s.type_str().to_string(),
                                            s.path_or_url().to_string(),
                                            s.title().to_string(),
                                            s.captured_at().to_rfc3339(),
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
            ResearchCliCommand::Continue { name, message } => {
                self.append_assistant_text(&format!(
                    "From: /research continue\n\nResuming `{name}`{}",
                    message
                        .as_ref()
                        .map(|m| format!(" with follow-up: {m}"))
                        .unwrap_or_default()
                ));
            }
            ResearchCliCommand::Cluster { name, force } => match ResearchName::try_new(&name) {
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
            ResearchCliCommand::Unknown(sub) => {
                self.append_assistant_text(&format!(
                    "From: /research\n\n**Error:** unknown subcommand `{sub}`. Try `/research help`."
                ));
            }
        }
    }
}
