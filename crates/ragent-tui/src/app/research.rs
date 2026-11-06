//! Research/skill command handling for the TUI.
//!
//! Extracted from `app.rs` in REMPLAN.md M5 / T5.3.

use ragent_agent::event::Event;
use ragent_agent::storage::Storage;
use ragent_agent::{agent::ModelRef, event::EventBus, provider::ProviderRegistry};
use std::sync::Arc;

use crate::research_adapter::TuiResearchObserver;

use crate::app::state::{App, LogLevel};

/// Resolve a provider API key for the deck's enrichment model.
///
/// Environment variables win (through the single resolver in
/// [`ragent_llm::provider::env_key`]), then the encrypted credential store. A
/// `None` result leaves the client keyless, which is valid for a local provider
/// such as Ollama and fatal only for a provider that then rejects the request -
/// in which case the deck falls back to its deterministic enrichment.
fn resolve_provider_api_key(provider_id: &str, storage: &Storage) -> Option<String> {
    if let Some(key) = ragent_llm::provider::env_key::provider_env_key(provider_id) {
        return Some(key);
    }
    match storage.get_provider_auth(provider_id) {
        Ok(key) => key,
        Err(error) => {
            tracing::debug!(
                provider = provider_id,
                %error,
                "credential store lookup failed; deck enrichment runs keyless"
            );
            None
        }
    }
}

/// Resolve a provider-specific base URL for the deck's enrichment model.
///
/// Only the providers whose endpoint is configurable by the user resolve to a
/// value here (Copilot, a generic OpenAI-compatible endpoint, Azure AI Foundry);
/// every other provider builds its client from its own default endpoint, so
/// `None` is the correct answer for it.
fn resolve_provider_base_url(
    provider_id: &str,
    storage: &Storage,
    config: Option<&ragent_agent::Config>,
) -> Option<String> {
    let setting_key = match provider_id {
        "copilot" => "copilot_api_base",
        "generic_openai" => "generic_openai_api_base",
        "azure_foundry" => "azure_foundry_api_base",
        _ => return None,
    };
    storage
        .get_setting(setting_key)
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            config
                .and_then(|c| c.provider.get(provider_id))
                .and_then(|p| p.api.as_ref())
                .and_then(|api| api.base_url.clone())
        })
        .or_else(|| ragent_llm::provider::env_key::provider_env_base_url(provider_id))
}

/// Marker the TUI question dialog returns when the user dismisses the prompt
/// with Esc (see the free-text question key handling in `src/input.rs`).
const QUESTION_DISMISSED_MARKER: &str = "[User dismissed question]";

/// Ask the user a single clarifying question through the TUI question dialog
/// and wait for the answer.
///
/// Mirrors the `AskUserQuestion` tool: publishes [`Event::QuestionRequested`] on the
/// app event bus and awaits the matching [`Event::QuestionAnswered`] reply.
/// Returns `None` when the event bus closes before the user answers.
async fn ask_user_clarification(
    event_bus: &Arc<EventBus>,
    session_id: &str,
    question: &str,
) -> Option<String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    // Subscribe before publishing so the reply cannot be missed.
    let mut rx = event_bus.subscribe();
    event_bus.publish(Event::QuestionRequested {
        session_id: session_id.to_string(),
        request_id: request_id.clone(),
        question: question.to_string(),
        options: Vec::new(),
        // Free-text clarification: no options, so no default to fall back to.
        default_index: None,
        // A clarification answer is ordinary prose, not a credential.
        secret: false,
    });
    loop {
        match rx.recv().await {
            Ok(Event::QuestionAnswered {
                session_id: ref answered_session,
                request_id: ref answered_request,
                response: ref answer,
                ..
            }) if answered_session == session_id && answered_request == &request_id => {
                return Some(answer.clone());
            }
            Ok(_) => {
                // Unrelated event; keep waiting.
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                // Dropped events; keep waiting.
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
        }
    }
}

/// Returns `true` when the user supplied a usable clarification answer.
fn clarification_answered(answer: &str) -> bool {
    !answer.trim().is_empty() && answer.trim() != QUESTION_DISMISSED_MARKER
}

/// Format a `provider_tool_calls` list as a human-readable suffix like
/// `", 12 search request(s) (web_search: 8, mf_search: 4)"`.
///
/// Delegates to [`ragent_research::provider_calls_suffix`] so the CLI and TUI
/// surfaces render the summary identically (see `ANTIPAT.md` M3.10).
fn format_provider_calls(calls: &[(String, usize)]) -> String {
    ragent_research::provider_calls_suffix(calls)
}

/// Run a research session, routing a `NeedsClarification` result through the
/// TUI question dialog and re-running once with the clarified topic.
///
/// Shared by the `/research create` and `/research update` spawns so the
/// clarification retry policy lives in one place.
async fn run_with_clarification(
    session: &ragent_research::ResearchSession,
    name: &str,
    title: &str,
    config: &ragent_research::SessionConfig,
    observer: Arc<dyn ragent_research::SessionObserver>,
    event_bus: &Arc<EventBus>,
    session_id: &str,
) -> Result<ragent_research::RunOutcome, String> {
    match session.run(name, title, config, observer.clone()).await {
        Ok(outcome) => Ok(outcome),
        Err(ragent_research::ResearchError::NeedsClarification { question }) => {
            let answer = ask_user_clarification(event_bus, session_id, &question).await;
            let Some(answer) = answer.filter(|a| clarification_answered(a)) else {
                return Err("clarification cancelled".to_string());
            };
            let mut clarified = config.clone();
            clarified.input.topic = format!("{} (clarification: {answer})", clarified.input.topic);
            session
                .run(name, title, &clarified, observer)
                .await
                .map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

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
        "reading sources...".to_string(),
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
        format!("sending concept-extraction prompt to {model_label}..."),
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
        "writing CONCEPTS.md...".to_string(),
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

/// Render the `[ok]` line a successful `/research deck` run publishes.
///
/// The per-status counts come from the written deck's own diff (FR-010), so this
/// line can never disagree with the file on disk. FR-013's no-change case says
/// so explicitly instead of reporting a deck full of zeroes.
#[must_use]
fn format_deck_success_line(
    name: &str,
    path: &std::path::Path,
    pair: &ragent_research::deck::ReportPair,
    outcome: &ragent_research::deck::DeckOutcome,
) -> String {
    if outcome.no_changes {
        return format!(
            "From: /research deck\n\n[ok] Wrote `{path}` for `{name}` \
             ({} to {}): no changes detected between the two reports.{pptx}",
            pair.older_label(),
            pair.newer_label(),
            path = path.display(),
            pptx = format_deck_pptx_line(outcome),
        );
    }
    format!(
        "From: /research deck\n\n[ok] Wrote `{path}` for `{name}` ({} to {}): \
         NEW {}, ADDED {}, UPDATE {}, OLD {} - {} change(s) across {} element slide(s).{pptx}",
        pair.older_label(),
        pair.newer_label(),
        outcome.new_count,
        outcome.added_count,
        outcome.update_count,
        outcome.old_count,
        outcome.change_count(),
        outcome.document.elements.len(),
        path = path.display(),
        pptx = format_deck_pptx_line(outcome),
    )
}

/// The FR-030 second-artifact line appended to a deck success message.
///
/// A written presentation is reported by path; an absent capability adds
/// nothing, so a session with no PowerPoint server reads exactly as it did
/// before the export existed. A failure is reported because the user asked for a
/// deck and should know one of its two artifacts is missing, but it is phrased as
/// a warning: the markdown deck is on disk and complete.
#[must_use]
fn format_deck_pptx_line(outcome: &ragent_research::deck::DeckOutcome) -> String {
    use ragent_research::deck_pptx::DeckPptxOutcome;
    match &outcome.pptx {
        DeckPptxOutcome::Unavailable => String::new(),
        DeckPptxOutcome::Written { path, slides, .. } => {
            format!(
                "\n\n[ok] Also wrote `{}` ({slides} slides).",
                path.display()
            )
        }
        DeckPptxOutcome::Failed { reason } => {
            format!(
                "\n\n[warn] PowerPoint export skipped: {reason} (the markdown deck is unaffected)."
            )
        }
    }
}

impl App {
    /// Render `/research` help and update the status message. Used by both the
    /// Help early-return and the defensive fallback in the dispatch match.
    fn show_research_help(&mut self) {
        self.append_assistant_text(&ragent_research::cli::ResearchCliCommand::build_help_message());
        self.status = "research: help".to_string();
    }

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
            self.show_research_help();
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
        // Session id: ensure_session() guarantees this is Some.
        let session_id = self.session_id.clone().unwrap_or_default();
        let config_arc = ragent_agent::Config::load().ok().map(Arc::new);
        let observer = Arc::new(TuiResearchObserver {
            app_event_bus: self.event_bus.clone(),
            session_id: session_id.clone(),
            name: String::new(),
            topic: String::new(),
        });

        match cmd {
            // FUNC-043: `Help` is handled above; a no-op here avoids the
            // `unreachable!()` panic if the early-return ever changes. Reuse
            // the shared helper so help rendering lives in exactly one place.
            ResearchCliCommand::Help => {
                self.show_research_help();
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
                papers,
                use_pdf,
                oa_recovery,
                fetch_timeout_secs,
                local_concurrency,
                web_phase_timeout_secs,
                local_phase_timeout_secs,
                search_max_retries,
                search_retry_base_delay_ms,
                max_web_results,
                max_search_calls,
                max_queries,
                max_local_sources,
                max_synthesis_sources,
                max_concepts,
                max_findings,
                brief,
                evaluate,
                url_cloak,
                max_age,
            } => {
                // Use the `[wait]` prefix so the status is treated as
                // async-in-progress and NOT auto-expired to "ready" by
                // [`App::arm_status_expiry`] while the background research
                // session is still running. The live progress events update
                // this status per-phase (see the AgentNotice handler), and
                // the completion notice sets a terminal status.
                self.status = format!("[wait] research: {name}...");
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
                    papers,
                    use_pdf,
                    open_access_recovery: oa_recovery,
                    fetch_concurrency,
                    local_concurrency,
                    fetch_timeout_secs,
                    web_phase_timeout_secs,
                    local_phase_timeout_secs,
                    search_max_retries,
                    search_retry_base_delay_ms,
                    max_web_results,
                    max_search_calls,
                    max_queries,
                    max_local_sources,
                    max_synthesis_sources,
                    max_concepts,
                    max_findings,
                    summarization_model,
                    clarify,
                    brief,
                    research_model,
                    compression_model,
                    final_report_model,
                    max_concurrent_research_units,
                    evaluate: Some(evaluate),
                    url_cloak,
                    max_age,
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
                    config.web.max_queries,
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
                    let outcome = run_with_clarification(
                        &session,
                        &name_for_spawn,
                        &title,
                        &config,
                        observer_clone,
                        &event_bus_for_spawn,
                        &session_id_for_spawn,
                    )
                    .await;
                    match outcome {
                        Ok(o) => {
                            let provider_calls = format_provider_calls(&o.provider_tool_calls);
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
                // Echo the full command line first so the message window
                // records exactly what was run (every argument verbatim)
                // immediately above the run's output.
                let rendered = format!(
                    "From: /research create\n\
                     ```\n\
                     {invocation}\n\
                     ```\n\n\
                     [note] **Gathering sources for `{name}`...**\n\n\
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
                    "research: listing items{}...",
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
                self.status = format!("research: opening '{name}'...");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                let app_event_bus = self.event_bus.clone();
                let session_id = self.session_id.clone().unwrap_or_default();
                tokio::spawn(async move {
                    match mgr.show(&name).await {
                        Ok(item) => {
                            let root = mgr.root().to_path_buf();
                            // The report may be dated (`RESEARCH-YYYY-MM-DD.md`)
                            // or the legacy undated `RESEARCH.md`; resolve the
                            // one that actually exists.
                            let path = match ragent_research::ResearchIo::resolve_research_md(
                                &root,
                                &item.name,
                                item.created_at,
                            )
                            .await
                            {
                                Ok(path) => path,
                                Err(e) => {
                                    event_bus.publish(Event::TextDelta {
                                        session_id,
                                        text: format!(
                                            "From: /research open\n\n**Error:** failed to locate the research report: {e}"
                                        ),
                                    });
                                    return;
                                }
                            };
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
                                            "From: /research open\n\n**Error:** failed to read the research report: {e}"
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
                self.status = format!("research: searching for '{query}'...");
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
                self.status = format!("research: showing '{name}'...");
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
                self.status = format!("research: deleting '{name}'...");
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
                self.status = format!("research: archiving '{name}'...");
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
                // the spawned task - mirroring the `create` spawn pattern.
                self.status = format!("[wait] research: update {name}...");
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
                    let mut replayed: Option<String> = None;
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
                        replayed = Some(recorded.clone());
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
                            config.web.max_queries,
                        );
                        event_bus.publish(Event::AgentNotice {
                            session_id: session_id.clone(),
                            message: format!(
                                "research: updating `{name}` - replaying `{recorded}`"
                            ),
                        });
                        run_with_clarification(
                            &session,
                            &name,
                            &item.title,
                            &config,
                            observer_for_spawn,
                            &event_bus,
                            &session_id,
                        )
                        .await
                    }
                    .await;
                    match run_result {
                        Ok(o) => {
                            let provider_calls = format_provider_calls(&o.provider_tool_calls);
                            // Echo the replayed command line so the message
                            // window shows what was executed, not just the
                            // recorded-invocation notice.
                            let echo = match replayed.as_deref() {
                                Some(cmd) => format!("```\n{cmd}\n```\n\n"),
                                None => String::new(),
                            };
                            event_bus.publish(Event::TextDelta {
                                session_id: session_id.clone(),
                                text: format!(
                                    "From: /research update\n\n\
                                     {echo}\
                                     [ok] **Updated `research/{}`** with {} sources.{}\n\n\
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

                        self.status =
                            format!("[wait] research: cluster '{name}' reading sources...");
                        self.append_assistant_text(&format!(
                            "From: /research cluster\n\n[ok] Request accepted for `{name}` \
                               (force={force}). Reading sources and preparing payload; \
                               progress updates will appear below."
                        ));

                        let provider_registry = self.provider_registry.clone();
                        let storage = Some(self.storage.clone());
                        let event_bus = self.event_bus.clone();
                        // Fail fast rather than publishing events with an empty
                        // session id, which would not route to this session.
                        let Some(session_id) = self.session_id.clone() else {
                            self.status = "research: no active session".to_string();
                            return;
                        };

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
            ResearchCliCommand::Deck {
                name,
                from,
                to,
                elements,
                include_old,
                out,
                force,
            } => {
                // FR-023, FR-024: the finding paragraphs and the
                // implication-to-finding ranking are written by the session's
                // own provider/model when one is selected. With no model the
                // manager's deterministic enrichment still produces both, so
                // the run is never refused for want of a provider.
                let model = self.deck_model();
                self.handle_research_deck(
                    &manager,
                    &name,
                    ragent_research::deck::DeckRunOptions {
                        from,
                        to,
                        elements,
                        include_old,
                        out,
                        force,
                    },
                    model,
                )
            }
            ResearchCliCommand::Import { path, .. } => {
                self.append_assistant_text(&format!("From: /research import\n\nImporting `{path}` is not yet implemented in the TUI."));
            }
            ResearchCliCommand::Invalid(arg) => {
                self.append_assistant_text(&format!(
                    "From: /research\n\n**Error:** {arg}. Try `/research help`."
                ));
            }
            ResearchCliCommand::Unknown(sub) => {
                self.append_assistant_text(&format!(
                    "From: /research\n\n**Error:** unknown subcommand `{sub}`. Try `/research help`."
                ));
            }
        }
    }

    /// Drive a `/research deck <name>` run (spec `diffdeck`, FR-001).
    ///
    /// The whole seven-step orchestration lives in [`ResearchManager::deck`], so
    /// this arm only resolves the session, spawns the future on the tokio
    /// runtime, and reports the outcome: one [`Event::TextDelta`] carrying the
    /// written path and the deck's own per-status counts on success (FR-010),
    /// and one `TextDelta` plus an [`Event::AgentNotice`] on a refusal.
    ///
    /// A refusal is never silent: every branch of [`ResearchError`] - a missing
    /// item (FR-018), an unusable report pair (FR-011/FR-012/FR-020), an
    /// unparseable report (FR-019), a bad `--out` or an existing deck without
    /// `--force` (FR-016/FR-017) - surfaces as an `[err]` line naming the reason.
    /// The model the deck enrichment stage may consult, if one is selected.
    ///
    /// It reuses the session's provider/model wiring exactly as the research
    /// session's own analysis engine does, so a `/provider` choice applies to
    /// the deck without a second configuration surface. `None` means "no model
    /// selected" and is not an error: the deck then falls back to the
    /// deterministic enrichment.
    fn deck_model(&self) -> Option<Arc<dyn ragent_research::deck_enrich::DeckModel>> {
        let (provider_id, model_id) = self.selected_model.as_deref()?.split_once('/')?;
        let registry = self.provider_registry.clone();
        let provider_id = provider_id.to_string();
        let model_id = model_id.to_string();
        let storage = self.storage.clone();
        let config = ragent_agent::Config::load().ok();
        let api_key = resolve_provider_api_key(&provider_id, &storage);
        let base_url = resolve_provider_base_url(&provider_id, &storage, config.as_ref());
        Some(
            Arc::new(ragent_research::deck_enrich::LlmDeckModel::new(Arc::new(
                ragent_research::analysis::LlmAnalysisEngine::new(registry, provider_id, model_id)
                    .with_api_key(api_key)
                    .with_base_url(base_url),
            ))) as Arc<dyn ragent_research::deck_enrich::DeckModel>,
        )
    }

    pub(crate) fn handle_research_deck(
        &mut self,
        manager: &ragent_research::ResearchManager,
        name: &str,
        options: ragent_research::deck::DeckRunOptions,
        model: Option<Arc<dyn ragent_research::deck_enrich::DeckModel>>,
    ) {
        let Some(session_id) = self.session_id.clone() else {
            self.status = "research: no active session".to_string();
            return;
        };
        let manager = manager.clone();
        let name = name.to_string();
        let event_bus = self.event_bus.clone();
        // FR-030: a connected MCP server that can write presentation files is
        // offered the deck as a second artifact. The sink is built from the
        // session's own shared `McpClient`, so it reuses a server the user has
        // already connected and spawns nothing. `None` when no client is wired
        // or no server qualifies, which the manager records as `Unavailable`.
        let deck_sink: Option<Arc<dyn ragent_research::deck_pptx::DeckSlideSink>> =
            self.session_processor.mcp_client.get().map(|client| {
                Arc::new(ragent_agent::mcp::deck_sink::McpDeckSink::new(
                    client.clone(),
                )) as Arc<dyn ragent_research::deck_pptx::DeckSlideSink>
            });
        // The run is short on the deterministic path and long whenever it does
        // real work - one model call per finding in the enrichment stage, one
        // server round trip per slide in the export - so it reports each stage
        // (FR-031). The adapter publishes sentinel notices that the TUI folds into
        // one in-place `Research Progress` message for this run, the same
        // presentation `/research create` uses, so the window shows a run that is
        // working rather than one that went quiet.
        self.status = format!("[wait] research: deck '{name}'...");
        let progress = crate::deck_progress::DeckProgressAdapter::new(
            self.event_bus.clone(),
            session_id.clone(),
            name.clone(),
        );
        // Seed the run's tracker and render it immediately, so the message window
        // acknowledges the command before the first stage report arrives rather
        // than staying silent until the lookup completes.
        let mut tracker = crate::deck_progress::DeckTracker::new();
        tracker.apply(&ragent_research::deck_progress::DeckEvent::started(
            ragent_research::deck_progress::DeckStage::Select,
            format!("starting a deck run for '{name}'"),
        ));
        match self.research_progress.iter_mut().find(|p| p.name == name) {
            Some(existing) => existing.deck = Some(tracker),
            None => {
                let mut seeded =
                    crate::research_progress::ResearchProgress::new(name.clone(), name.clone());
                seeded.deck = Some(tracker);
                self.research_progress.push(seeded);
            }
        }
        self.refresh_research_progress_message(&name);

        tokio::spawn(async move {
            // The model is borrowed for the run rather than cloned into it:
            // the trait object is behind an `Arc`, so the call keeps it alive
            // across the await without duplicating provider state.
            let model_ref = model.as_deref();
            let sink_ref = deck_sink.as_deref();
            match manager
                .deck_with_progress(&name, &options, model_ref, sink_ref, Some(&progress))
                .await
            {
                Ok(outcome) => {
                    // Close the run's stage list in the message window before the
                    // outcome lands, so the tracker never shows an export stage
                    // still in flight beside a finished deck.
                    ragent_research::deck_progress::DeckProgress::report(
                        &progress,
                        &ragent_research::deck_progress::DeckEvent::done(
                            ragent_research::deck_progress::DeckStage::Export,
                            "deck run complete",
                        ),
                    );
                    event_bus.publish(Event::TextDelta {
                        session_id: session_id.clone(),
                        text: format_deck_success_line(
                            &name,
                            &outcome.path,
                            &outcome.pair,
                            &outcome,
                        ),
                    });
                    event_bus.publish(Event::AgentNotice {
                        session_id,
                        message: if outcome.no_changes {
                            format!("research: deck '{name}' written (no changes)")
                        } else {
                            format!(
                                "research: deck '{name}' written ({} change(s))",
                                outcome.change_count()
                            )
                        },
                    });
                }
                Err(e) => {
                    event_bus.publish(Event::TextDelta {
                        session_id: session_id.clone(),
                        text: format!("From: /research deck\n\n**Error:** {e}"),
                    });
                    event_bus.publish(Event::AgentNotice {
                        session_id,
                        message: format!("research: deck '{name}' failed: {e}"),
                    });
                }
            }
        });
    }
}
