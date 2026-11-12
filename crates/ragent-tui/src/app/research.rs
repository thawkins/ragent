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

/// Publish the terminal failure report for research run `name`.
///
/// The TUI's progress handler treats a finalize-phase error as the end of the
/// run: it marks the run's tracker finished and replaces the `[wait]` status
/// with a `[warn]` one. Without this, a failed run (including a cancelled
/// clarification) leaves the status bar on `[wait]` indefinitely, because
/// `[wait]` is never auto-expired.
fn publish_run_failed(event_bus: &EventBus, session_id: &str, name: &str, error: &str) {
    use crate::research_progress::{SessionPhase, encode_cluster_progress_event};
    event_bus.publish(Event::AgentNotice {
        session_id: session_id.to_string(),
        message: encode_cluster_progress_event(
            name,
            name,
            SessionPhase::Finalize,
            "error",
            &format!("{name} failed: {error}"),
        ),
    });
}

/// Maximum number of hits `/research search` returns, matching
/// `ragent research search`.
const SEARCH_RESULT_LIMIT: usize = 25;

/// Wrap `body` in a fenced code block tagged `lang` (empty for none).
fn code_block(lang: &str, body: &str) -> String {
    format!("```{lang}\n{}\n```", body.trim_end())
}

/// Holds a `/research` run's name in [`App::research_live_runs`] for as long
/// as the run's task lives, and frees it on drop - on success, failure or
/// panic alike.
struct LiveRunGuard {
    runs: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    name: String,
}

impl Drop for LiveRunGuard {
    fn drop(&mut self) {
        crate::app::session_ops::recover_poisoned(self.runs.lock(), "research_live_runs")
            .remove(&self.name);
    }
}

/// Everything a spawned `/research` run needs from the [`App`], captured
/// before the spawn so the task never borrows `self`.
///
/// Shared by `create`, `update` and `continue` so the session wiring lives in
/// one place.
#[derive(Clone)]
struct ResearchRunContext {
    tool_registry: Arc<ragent_agent::tool::ToolRegistry>,
    manager: ragent_research::ResearchManager,
    session_id: String,
    cwd: std::path::PathBuf,
    event_bus: Arc<EventBus>,
    storage: Arc<Storage>,
    config: Option<Arc<ragent_agent::Config>>,
    provider_registry: Arc<ProviderRegistry>,
    active_model: Option<ModelRef>,
}

impl ResearchRunContext {
    /// Build the research session for run `name` with `config`.
    fn build_session(
        &self,
        name: &str,
        config: &ragent_research::SessionConfig,
    ) -> ragent_research::ResearchSession {
        crate::research_adapter::build_research_session(
            &self.tool_registry,
            self.manager.clone(),
            self.session_id.clone(),
            self.cwd.clone(),
            self.event_bus.clone(),
            Some(self.storage.clone()),
            self.config.clone(),
            Some(self.provider_registry.clone()),
            self.active_model.clone(),
            Some(name),
            config.web.max_queries,
        )
    }

    /// The progress observer for run `name` researching `topic`.
    fn observer(&self, name: &str, topic: &str) -> Arc<dyn ragent_research::SessionObserver> {
        Arc::new(TuiResearchObserver {
            app_event_bus: self.event_bus.clone(),
            session_id: self.session_id.clone(),
            name: name.to_string(),
            topic: topic.to_string(),
        })
    }

    /// Publish a `From: /research <verb>` result block to the message window.
    fn publish_text(&self, text: String) {
        self.event_bus.publish(Event::TextDelta {
            session_id: self.session_id.clone(),
            text,
        });
    }
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
        self.append_assistant_text(
            &ragent_research::cli::ResearchCliCommand::build_help_message_for("/research"),
        );
        self.status = "research: help".to_string();
    }

    pub(crate) fn handle_research_command(&mut self, args: &str) {
        use ragent_research::cli::ResearchCliCommand;
        use ragent_research::{
            OutputFormat, ResearchIo, ResearchManager, ResearchMode, ResearchName,
        };

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
        let session_id = self.session.session_id.clone().unwrap_or_default();

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
                // Reject an unusable name before announcing the run or seeding
                // its progress tracker.
                if let Err(e) = ResearchName::try_new(&name) {
                    self.status = "research: create invalid name".to_string();
                    self.append_assistant_text(&format!(
                        "From: /research create\n\n**Error:** invalid research name `{name}`: {e}"
                    ));
                    return;
                }
                // Use the `[wait]` prefix so the status is treated as
                // async-in-progress and NOT auto-expired to "ready" by
                // [`App::arm_status_expiry`] while the background research
                // session is still running. The live progress events update
                // this status per-phase (see the AgentNotice handler), and
                // the completion notice sets a terminal status.
                let Some(live) = self.claim_research_run("create", &name) else {
                    return;
                };
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
                self.begin_research_progress(crate::research_progress::ResearchProgress::new(
                    &name, &topic,
                ));
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
                    // Omitted -> None, so `research.evaluate.enabled` applies.
                    evaluate: evaluate.then_some(true),
                    url_cloak,
                    max_age,
                    invocation: Some(invocation.clone()),
                };
                let ctx = self.research_run_context(&manager, &session_id);
                let config = ragent_research::build_session_config(&req, ctx.config.as_deref());
                let session = ctx.build_session(&name, &config);
                let observer = ctx.observer(&name, &topic);
                let name_for_spawn = name.clone();
                let topic_for_assistant = topic.clone();
                let from_urls_for_msg = config.input.from_urls.clone();
                let from_files_for_msg = config.input.from_files.clone();
                let mode_override = config.engine.mode;
                tokio::spawn(async move {
                    let _live = live;
                    let name = name_for_spawn;
                    let outcome = run_with_clarification(
                        &session,
                        &name,
                        &title,
                        &config,
                        observer,
                        &ctx.event_bus,
                        &ctx.session_id,
                    )
                    .await;
                    match outcome {
                        Ok(o) => ctx.publish_text(format!(
                            "From: /research create\n\n\
                             [ok] **Created `research/{name}`** with {} sources.{}\n\n\
                             Tip: run `/research open {name}` to view the report.",
                            o.sources.len(),
                            format_provider_calls(&o.provider_tool_calls),
                            name = o.research_name,
                        )),
                        Err(e) => {
                            publish_run_failed(&ctx.event_bus, &ctx.session_id, &name, &e);
                            ctx.publish_text(format!("From: /research create\n\n**Error:** {e}"));
                        }
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
                        "Topic: (none given)".to_string()
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
            ResearchCliCommand::List { all, json } => {
                self.status = format!(
                    "research: listing items{}...",
                    if all { " (including archived)" } else { "" }
                );
                let mgr = manager.clone();
                self.spawn_research_query(&session_id, "list", async move {
                    // Refresh first, as `ragent research list` does, so items
                    // moved or removed outside the manager are reflected.
                    mgr.refresh_index().await.map_err(|e| e.to_string())?;
                    let items = mgr.list(all).await.map_err(|e| e.to_string())?;
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
                    Ok(Some(if json {
                        code_block("json", &ragent_research::render_list_output_json(&rows))
                    } else {
                        code_block("", &ragent_research::render_list_output(&rows))
                    }))
                });
            }
            ResearchCliCommand::Open { name } => {
                self.status = format!("research: opening '{name}'...");
                let mgr = manager.clone();
                let event_bus = self.event_bus.clone();
                self.spawn_research_query(&session_id, "open", async move {
                    let item = mgr.show(&name).await.map_err(|e| e.to_string())?;
                    // The report may be dated (`RESEARCH-YYYY-MM-DD.md`) or the
                    // legacy undated `RESEARCH.md`; resolve the one that exists.
                    let path = ragent_research::ResearchIo::resolve_research_md(
                        mgr.root(),
                        &item.name,
                        item.created_at,
                    )
                    .await
                    .map_err(|e| format!("failed to locate the research report: {e}"))?;
                    let content = ragent_research::ResearchIo::read_file(&path)
                        .await
                        .map_err(|e| format!("failed to read the research report: {e}"))?;
                    let (_frontmatter, body) =
                        ragent_research::ResearchIo::split_frontmatter(&content);
                    event_bus.publish(Event::OpenResearchView {
                        name: item.name.to_string(),
                        path,
                        markdown: body,
                    });
                    Ok(None)
                });
            }
            ResearchCliCommand::Search { query, json } => {
                self.status = format!("research: searching for '{query}'...");
                let mgr = manager.clone();
                self.spawn_research_query(&session_id, "search", async move {
                    let hits = mgr
                        .search(&query, SEARCH_RESULT_LIMIT)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(Some(if json {
                        let rows: Vec<(String, String, String, String)> = hits
                            .into_iter()
                            .map(|h| (h.name, h.title, h.snippet, h.path.display().to_string()))
                            .collect();
                        code_block("json", &ragent_research::render_search_output_json(&rows))
                    } else {
                        let rows: Vec<(String, String, String)> = hits
                            .into_iter()
                            .map(|h| (h.name, h.title, h.snippet))
                            .collect();
                        code_block("", &ragent_research::render_search_output(&rows))
                    }))
                });
            }
            ResearchCliCommand::Show { name, json } => {
                self.status = format!("research: showing '{name}'...");
                let mgr = manager.clone();
                self.spawn_research_query(&session_id, "show", async move {
                    let item = mgr.show(&name).await.map_err(|e| e.to_string())?;
                    let sources = ragent_research::show_source_rows(&item.sources);
                    let render = if json {
                        ragent_research::render_show_output_json
                    } else {
                        ragent_research::render_show_output
                    };
                    let body = render(
                        item.name.as_ref(),
                        &item.title,
                        &item.topic,
                        item.status.as_str(),
                        &item.created_at.to_rfc3339(),
                        &item.modified_at.to_rfc3339(),
                        &sources,
                    );
                    Ok(Some(code_block(if json { "json" } else { "" }, &body)))
                });
            }
            ResearchCliCommand::Delete { name, yes } => {
                if !yes {
                    // The TUI has no inline y/N prompt, so the flag is the
                    // confirmation.
                    self.append_assistant_text(&format!(
                        "From: /research delete\n\nNot deleted. Deleting `research/{name}` \
                         cannot be undone; confirm with `/research delete {name} --yes`."
                    ));
                    return;
                }
                self.status = format!("research: deleting '{name}'...");
                let mgr = manager.clone();
                self.spawn_research_query(&session_id, "delete", async move {
                    mgr.delete(&name).await.map_err(|e| e.to_string())?;
                    Ok(Some(format!("[ok] Deleted research/{name}.")))
                });
            }
            ResearchCliCommand::Archive { name } => {
                self.status = format!("research: archiving '{name}'...");
                let mgr = manager.clone();
                self.spawn_research_query(&session_id, "archive", async move {
                    mgr.archive(&name).await.map_err(|e| e.to_string())?;
                    Ok(Some(format!("[ok] Archived research/{name}.")))
                });
            }
            ResearchCliCommand::Continue { name, message } => {
                // Add the follow-up to the item's plan (marking it in
                // progress), then re-run the session on the plan's topic -
                // the same flow as `ragent research continue`.
                let Some(live) = self.claim_research_run("continue", &name) else {
                    return;
                };
                self.status = format!("[wait] research: continue {name}...");
                self.push_log_no_agent(LogLevel::Info, format!("research: continue '{name}'"));
                self.begin_research_progress(crate::research_progress::ResearchProgress::new(
                    &name, "continue",
                ));
                self.append_assistant_text(&format!(
                    "From: /research continue\n\n[note] **Resuming `{name}`**{}...",
                    message
                        .as_ref()
                        .map(|m| format!(" with follow-up: {m}"))
                        .unwrap_or_default()
                ));
                let ctx = self.research_run_context(&manager, &session_id);
                tokio::spawn(async move {
                    let _live = live;
                    let run_result: Result<_, String> = async {
                        let state = ctx
                            .manager
                            .continue_item(&name, message.as_deref())
                            .await
                            .map_err(|e| e.to_string())?;
                        let topic = state.plan.topic.clone();
                        let title = match ctx.manager.show(&name).await {
                            Ok(item) => item.title,
                            Err(_) => name.clone(),
                        };
                        let req = ragent_research::ResearchRunRequest {
                            name: name.clone(),
                            topic: topic.clone(),
                            title: Some(title.clone()),
                            ..Default::default()
                        };
                        let config =
                            ragent_research::build_session_config(&req, ctx.config.as_deref());
                        let session = ctx.build_session(&name, &config);
                        run_with_clarification(
                            &session,
                            &name,
                            &title,
                            &config,
                            ctx.observer(&name, &topic),
                            &ctx.event_bus,
                            &ctx.session_id,
                        )
                        .await
                    }
                    .await;
                    match run_result {
                        Ok(o) => ctx.publish_text(format!(
                            "From: /research continue\n\n\
                             [ok] **Continued `research/{name}`** with {} sources.{}\n\n\
                             Tip: run `/research open {name}` to view the report.",
                            o.sources.len(),
                            format_provider_calls(&o.provider_tool_calls),
                            name = o.research_name,
                        )),
                        Err(e) => {
                            publish_run_failed(&ctx.event_bus, &ctx.session_id, &name, &e);
                            ctx.publish_text(format!("From: /research continue\n\n**Error:** {e}"));
                        }
                    }
                });
            }
            ResearchCliCommand::Update { name } => {
                // Replay the invocation recorded in the item's frontmatter and
                // overwrite RESEARCH.md (and its supporting files) with a
                // fresh run. The lookup is async, so all item work happens in
                // the spawned task - mirroring the `create` spawn pattern.
                let Some(live) = self.claim_research_run("update", &name) else {
                    return;
                };
                self.status = format!("[wait] research: update {name}...");
                self.push_log_no_agent(LogLevel::Info, format!("research: update '{name}'"));
                self.begin_research_progress(crate::research_progress::ResearchProgress::new(
                    &name,
                    "update (replay recorded invocation)",
                ));
                let ctx = self.research_run_context(&manager, &session_id);
                tokio::spawn(async move {
                    let _live = live;
                    let mut replayed: Option<String> = None;
                    let run_result: Result<_, String> = async {
                        let item = ctx
                            .manager
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
                            ragent_research::build_session_config(&req, ctx.config.as_deref());
                        let session = ctx.build_session(&name, &config);
                        run_with_clarification(
                            &session,
                            &name,
                            &item.title,
                            &config,
                            ctx.observer(&name, &req.topic),
                            &ctx.event_bus,
                            &ctx.session_id,
                        )
                        .await
                    }
                    .await;
                    // Echo the replayed command line so the message window
                    // shows what was executed.
                    let echo = match replayed.as_deref() {
                        Some(cmd) => format!("```\n{cmd}\n```\n\n"),
                        None => String::new(),
                    };
                    match run_result {
                        Ok(o) => ctx.publish_text(format!(
                            "From: /research update\n\n\
                             {echo}\
                             [ok] **Updated `research/{name}`** with {} sources.{}\n\n\
                             Tip: run `/research open {name}` to view the refreshed report.",
                            o.sources.len(),
                            format_provider_calls(&o.provider_tool_calls),
                            name = o.research_name,
                        )),
                        Err(e) => {
                            publish_run_failed(&ctx.event_bus, &ctx.session_id, &name, &e);
                            ctx.publish_text(format!(
                                "From: /research update\n\n{echo}**Error:** {e}"
                            ));
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
                        // Only the first entry is read: emptiness is all that
                        // matters, and a large sources folder must not stall
                        // the UI thread while it is counted.
                        let is_empty = match std::fs::read_dir(&sources_dir) {
                            Ok(mut entries) => entries.next().is_none(),
                            Err(e) => {
                                self.status = format!("research: cluster '{name}' unreadable");
                                self.append_assistant_text(&format!(
                                    "From: /research cluster\n\n**Error:** cannot read \
                                     `research/{name}/sources/`: {e}"
                                ));
                                return;
                            }
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
                            .session
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
                        let model_label = self.session.selected_model.clone().unwrap_or_default();

                        let Some(live) = self.claim_research_run("cluster", &name) else {
                            return;
                        };
                        self.begin_research_progress(
                            crate::research_progress::ResearchProgress::new(&name, &name),
                        );
                        self.status =
                            format!("[wait] research: cluster '{name}' reading sources...");
                        self.append_assistant_text(&format!(
                            "From: /research cluster\n\n[note] **Clustering `{name}`**{}... \
                             Watch the progress log below.",
                            if force {
                                " (overwriting CONCEPTS.md)"
                            } else {
                                ""
                            }
                        ));

                        let provider_registry = self.provider_registry.clone();
                        let storage = Some(self.storage.clone());
                        let event_bus = self.event_bus.clone();
                        let session_id = session_id.clone();

                        tokio::spawn(async move {
                            let _live = live;
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
                                // Success already published its result block
                                // and final progress step.
                                Ok(_) => {}
                                Err(e) => {
                                    publish_run_failed(&event_bus, &session_id, &name, &e);
                                    event_bus.publish(Event::TextDelta {
                                        session_id,
                                        text: format!(
                                            "From: /research cluster\n\n**Error:** `{name}`: {e}"
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
                let research = self.load_research_config().unwrap_or_default().research;
                let body = serde_json::to_string_pretty(&research)
                    .unwrap_or_else(|e| format!("failed to render the research settings: {e}"));
                self.append_assistant_text(&format!(
                    "From: /research config\n\nEffective `research` settings \
                     (`ragent.json` over built-in defaults; per-run flags override these):\n\n{}",
                    code_block("json", &body)
                ));
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

    /// Claim `name` for a new `/research <verb>` run.
    ///
    /// Returns the guard the run's task must hold, or `None` - after telling
    /// the user - when a run for `name` is already live in this session.
    fn claim_research_run(&mut self, verb: &str, name: &str) -> Option<LiveRunGuard> {
        let claimed = crate::app::session_ops::recover_poisoned(
            self.research_live_runs.lock(),
            "research_live_runs",
        )
        .insert(name.to_string());
        if !claimed {
            self.status = format!("research: '{name}' is already running");
            self.append_assistant_text(&format!(
                "From: /research {verb}\n\n**Error:** a research run for `{name}` is already \
                 in progress in this session. Wait for it to finish, then run the command again."
            ));
            return None;
        }
        Some(LiveRunGuard {
            runs: self.research_live_runs.clone(),
            name: name.to_string(),
        })
    }

    /// Run `work` in the background and publish its result as one
    /// `From: /research <verb>` block: the returned body on success (nothing
    /// for `Ok(None)`), or an `**Error:**` line on failure.
    ///
    /// Every short `/research` query (list, show, search, open, delete,
    /// archive) reports through this, so they all present results and errors
    /// the same way.
    fn spawn_research_query<F>(&self, session_id: &str, verb: &'static str, work: F)
    where
        F: std::future::Future<Output = Result<Option<String>, String>> + Send + 'static,
    {
        let event_bus = self.event_bus.clone();
        let session_id = session_id.to_string();
        tokio::spawn(async move {
            let text = match work.await {
                Ok(Some(body)) => format!("From: /research {verb}\n\n{body}"),
                Ok(None) => return,
                Err(e) => format!("From: /research {verb}\n\n**Error:** {e}"),
            };
            event_bus.publish(Event::TextDelta { session_id, text });
        });
    }

    /// Load `ragent.json` for a research run, reporting a load failure.
    ///
    /// A broken config is not fatal - the run falls back to the built-in
    /// defaults - but the user is told, so research settings that silently
    /// stopped applying do not go unnoticed.
    fn load_research_config(&mut self) -> Option<ragent_agent::Config> {
        match ragent_agent::Config::load_for(&self.cwd_path) {
            Ok(config) => Some(config),
            Err(error) => {
                self.push_log_no_agent(
                    LogLevel::Warn,
                    format!("research: failed to load ragent.json: {error}"),
                );
                self.append_assistant_text(&format!(
                    "From: /research\n\n[warn] Could not load `ragent.json` ({error}); \
                     using built-in research defaults."
                ));
                None
            }
        }
    }

    /// Capture the state a spawned research run needs.
    fn research_run_context(
        &mut self,
        manager: &ragent_research::ResearchManager,
        session_id: &str,
    ) -> ResearchRunContext {
        ResearchRunContext {
            tool_registry: self.session_processor.tool_registry.clone(),
            manager: manager.clone(),
            session_id: session_id.to_string(),
            cwd: self.cwd_path.clone(),
            event_bus: self.event_bus.clone(),
            storage: self.storage.clone(),
            config: self.load_research_config().map(Arc::new),
            provider_registry: self.provider_registry.clone(),
            active_model: self.session.agent_info.model.clone(),
        }
    }

    /// The model the deck enrichment stage may consult, if one is selected.
    ///
    /// It reuses the session's provider/model wiring exactly as the research
    /// session's own analysis engine does, so a `/provider` choice applies to
    /// the deck without a second configuration surface. `None` means "no model
    /// selected" and is not an error: the deck then falls back to the
    /// deterministic enrichment.
    fn deck_model(&mut self) -> Option<Arc<dyn ragent_research::deck_enrich::DeckModel>> {
        let (provider_id, model_id) = self.session.selected_model.as_deref()?.split_once('/')?;
        let registry = self.provider_registry.clone();
        let provider_id = provider_id.to_string();
        let model_id = model_id.to_string();
        let storage = self.storage.clone();
        let config = self.load_research_config();
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

    /// Drive a `/research deck <name>` run (spec `diffdeck`, FR-001).
    ///
    /// The whole seven-step orchestration lives in [`ResearchManager::deck`], so
    /// this arm only resolves the session, spawns the future on the tokio
    /// runtime, and reports the outcome: one [`Event::TextDelta`] carrying the
    /// written path and the deck's own per-status counts on success (FR-010),
    /// and on a refusal one `TextDelta` plus the terminal failure report that
    /// ends the run's progress tracker and `[wait]` status.
    ///
    /// A refusal is never silent: every branch of [`ResearchError`] - a missing
    /// item (FR-018), an unusable report pair (FR-011/FR-012/FR-020), an
    /// unparseable report (FR-019), a bad `--out` or an existing deck without
    /// `--force` (FR-016/FR-017) - surfaces as an `[err]` line naming the reason.
    pub(crate) fn handle_research_deck(
        &mut self,
        manager: &ragent_research::ResearchManager,
        name: &str,
        options: ragent_research::deck::DeckRunOptions,
        model: Option<Arc<dyn ragent_research::deck_enrich::DeckModel>>,
    ) {
        let Some(session_id) = self.session.session_id.clone() else {
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
        let Some(live) = self.claim_research_run("deck", &name) else {
            return;
        };
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
        let mut seeded =
            crate::research_progress::ResearchProgress::new(name.clone(), name.clone());
        seeded.deck = Some(tracker);
        self.begin_research_progress(seeded);

        tokio::spawn(async move {
            let _live = live;
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
                        session_id,
                        text: format_deck_success_line(
                            &name,
                            &outcome.path,
                            &outcome.pair,
                            &outcome,
                        ),
                    });
                }
                Err(e) => {
                    publish_run_failed(&event_bus, &session_id, &name, &e.to_string());
                    event_bus.publish(Event::TextDelta {
                        session_id,
                        text: format!("From: /research deck\n\n**Error:** {e}"),
                    });
                }
            }
        });
    }
}
