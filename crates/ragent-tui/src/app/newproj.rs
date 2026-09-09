//! `/new` slash-command implementation: project scaffolding in the current
//! working directory (spec `newproj`, FR-001 command availability, FR-012
//! help surface, FR-014 foreground execution with streamed progress).
//!
//! The heavy lifting lives in `ragent-tools-extended::project_scaffold`:
//! flag parsing/validation (T-001), recipes (T-002), layout planning (T-003),
//! stack overlays (T-004), workspace artifacts (T-005), the empty-directory
//! guard (T-006), the no-silent-overwrite emitter (T-007), local git init
//! (T-008), the GitHub (T-009) and GitLab (T-010) remote-init flows, and the
//! FR-011 summary report. This module wires them into the TUI command
//! surface: parse -> guard -> spawn worker (emit -> git init -> remote init
//! -> summary) while streaming progress lines into the message window.
//!
//! T-013/FR-014 concurrency model: the command stays foreground (no
//! sub-agent, no LLM turn, `is_processing` untouched); only the blocking
//! file-emission / git / remote work moves to a worker `std::thread` so the
//! UI keeps painting. The worker appends progress lines to a shared buffer
//! and deposits the final outcome in `newproj_result`; the UI thread drains
//! both via `poll_newproj_result` each frame, updating the progress message
//! in place (mirroring `refresh_research_progress_message`) and printing the
//! FR-011 summary as the usual `From: /new` message when the run finishes.

use ragent_tools_extended::project_scaffold::{
    HostingTarget, RemoteStatus, ScaffoldRequest, enforce_empty_directory_guard, init_and_commit,
    init_github_remote, init_gitlab_remote, parse_flags, plan_and_emit, recipe_for,
};

use crate::app::state::{App, LogLevel};

/// Header line of the streamed scaffold progress message (T-013). Must stay
/// in sync with the first line pushed when the worker starts so the poll
/// path can find and replace the message in place.
const PROGRESS_HEADER: &str = "Scaffolding";

/// Build the FR-012/FR-018 usage/help message.
///
/// The body (purpose, per-argument docs, registry-derived accepted values,
/// worked examples) comes from the shared engine renderer
/// (`render_detailed_help`, NFR-001); the usage lines and example
/// invocations are the slash-command surface spellings.
fn new_usage_message() -> String {
    use ragent_tools_extended::project_scaffold::render_detailed_help;

    render_detailed_help(
        "## /new - Scaffold a new project\n\
         \n\
         \x20 /new --language <lang> --type <type> [--stack <name>] [--github | --gitlab]\n\
         \x20 /new help",
        "\x20 /new --language rust --type cmdline",
        "\x20 /new --language python --type library --gitlab",
    )
}

/// One scaffold progress line: the label shown while the step runs plus the
/// completion note recorded for the streamed progress panel (T-013).
enum ProgressLine {
    /// A step is starting (shown immediately, before it completes).
    Start(String),
    /// A step finished; `ok` picks the check-mark prefix.
    Done(String, bool),
}

impl ProgressLine {
    fn render(&self) -> String {
        match self {
            Self::Start(label) => format!("[ .. ] {label}"),
            Self::Done(label, ok) => {
                if *ok {
                    format!("[ ok ] {label}")
                } else {
                    format!("[fail] {label}")
                }
            }
        }
    }
}

/// Append a progress line to the shared worker buffer (T-013).
///
/// Lock-poisoned siblings are tolerated: a progress note is best-effort and
/// must never take the scaffold down.
fn push_progress(buffer: &std::sync::Mutex<Vec<String>>, line: ProgressLine) {
    if let Ok(mut lines) = buffer.lock() {
        lines.push(line.render());
    }
}

impl App {
    /// Handle the `/new` slash command (FR-001, FR-012, FR-014).
    ///
    /// Splits into three paths:
    /// - `help` / bare `/new` — print the usage page (FR-012), zero writes.
    /// - invalid flags — print the FR-003 validation error plus usage.
    /// - valid request — run the scaffold in the foreground (FR-014): the
    ///   FR-002 guard runs inline, then the emit / git / remote pipeline is
    ///   handed to a worker thread while streamed progress lines land in the
    ///   message window; `poll_newproj_result` prints the FR-011 summary.
    pub(crate) fn handle_new_command(&mut self, args: &str) {
        let argv: Vec<&str> = args.split_whitespace().collect();
        match parse_flags(&argv) {
            Ok(request) if request.is_help() => {
                self.append_assistant_text(&format!("From: /new\n\n{}", new_usage_message()));
                self.status = "new: help".to_string();
            }
            Ok(request) => self.run_scaffold(&request),
            Err(err) => {
                self.append_assistant_text(&format!(
                    "From: /new\n\n[err] **{err}**\n\n{}",
                    new_usage_message()
                ));
                self.status = "new: usage".to_string();
            }
        }
    }

    /// Start a validated [`ScaffoldRequest`] scaffold in the current
    /// directory (T-013/FR-014).
    ///
    /// The FR-002 guard and request planning run inline (fast, and they
    /// decide whether any work happens at all); the blocking file-emission,
    /// git, and remote steps then run on a worker thread while the UI keeps
    /// painting. Progress lines stream into the message window via
    /// `poll_newproj_result`, and the FR-011 summary lands as the usual
    /// `From: /new` message when the worker finishes. No sub-agent and no
    /// LLM turn is involved.
    fn run_scaffold(&mut self, request: &ScaffoldRequest) {
        let cwd = crate::app::helpers::current_working_dir();

        // FR-002: refuse non-empty targets before any filesystem writes.
        if let Err(err) = enforce_empty_directory_guard(&cwd) {
            self.append_assistant_text(&format!("From: /new\n\n[err] **{err}**"));
            self.status = "new: directory not empty".to_string();
            return;
        }

        let slug = cwd
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "new-project".to_owned());

        let Some(recipe) = recipe_for(request.language()) else {
            self.append_assistant_text(
                "From: /new\n\n[err] **internal: no scaffold recipe for the selected language**",
            );
            self.status = "new: internal error".to_string();
            return;
        };

        // The FR-002 guard passed: nothing exists to collide with, so this
        // run owns the directory. Replace any stray progress state from a
        // previous run before spawning the worker.
        if let Ok(mut lines) = self.newproj_progress.lock() {
            lines.clear();
        }
        self.newproj_progress_text = None;
        self.newproj_progress_slug = None;
        self.newproj_running = true;

        // Seed the progress panel: the first streamed line is visible as
        // soon as the worker's first step starts. The header is plain text
        // (no markdown) so the in-place refresh can match the message's
        // first line byte-for-byte.
        let header_line = format!("{PROGRESS_HEADER} '{slug}'...");
        let progress = self.newproj_progress.clone();
        push_progress(
            &progress,
            ProgressLine::Start("guard passed, emitting files".to_owned()),
        );

        // FR-014: the user sees the run start immediately in the message
        // window; the in-place-updated panel streams each step as the
        // worker reaches it. The panel message's first line is exactly the
        // header so `refresh_newproj_progress_message` finds it.
        self.newproj_progress_slug = Some(header_line.clone());
        self.newproj_progress_text = Some(header_line.clone());
        self.refresh_newproj_progress_message();
        self.status = format!("[wait] new: scaffolding {slug}...");
        self.push_log_no_agent(
            LogLevel::Info,
            format!("new: scaffolding {slug} at {}", cwd.display()),
        );

        // T-013/FR-014: hand the blocking emit/git/remote pipeline to a
        // worker thread; the request is cloned across (the engine types own
        // their data), progress streams through the shared buffer, and the
        // outcome is deposited in `newproj_result`. When the thread cannot
        // spawn, the pipeline runs inline so the scaffold still completes.
        let hosting = request.hosting();
        let result_slot = self.newproj_result.clone();
        let worker_cwd = cwd.clone();
        let worker_slug = slug.clone();
        let worker_request = request.clone();
        let progress_for_worker = self.newproj_progress.clone();
        let spawn_result = std::thread::Builder::new()
            .name("newproj-scaffold".to_owned())
            .spawn(move || {
                push_progress(
                    &progress_for_worker,
                    ProgressLine::Done("guard passed, emitting files".to_owned(), true),
                );

                let outcome = run_scaffold_steps(
                    &worker_cwd,
                    &worker_slug,
                    &worker_request,
                    recipe,
                    hosting,
                    &progress_for_worker,
                );
                if let Ok(mut slot) = result_slot.lock() {
                    *slot = Some(outcome);
                }
            });
        match spawn_result {
            Ok(handle) => {
                // The worker owns the run now; it deposits the outcome in
                // `newproj_result` for `poll_newproj_result` to drain.
                drop(handle);
            }
            Err(err) => {
                self.push_log_no_agent(LogLevel::Warn, format!("new: worker spawn failed: {err}"));
                let outcome = run_scaffold_steps(
                    &cwd,
                    &slug,
                    request,
                    recipe,
                    hosting,
                    &self.newproj_progress,
                );
                if let Ok(mut slot) = self.newproj_result.lock() {
                    *slot = Some(outcome);
                }
            }
        }
    }

    /// Poll the `/new` scaffold worker: stream drained progress lines into
    /// the in-place-updated progress message and apply the final outcome
    /// (T-013/FR-014).
    ///
    /// While the run is active, every drained line re-renders the progress
    /// panel in place (one message per run, mirroring the research-progress
    /// panel pattern). When the worker deposits its outcome, the panel is
    /// collapsed into the final FR-011 summary message and the status line
    /// reports the terminal state.
    pub fn poll_newproj_result(&mut self) {
        // Stream any fresh worker progress lines into the panel.
        let drained: Vec<String> = match self.newproj_progress.lock() {
            Ok(mut lines) => std::mem::take(&mut *lines),
            Err(_) => Vec::new(),
        };
        if !drained.is_empty() {
            let mut text = self
                .newproj_progress_text
                .take()
                .unwrap_or_else(|| self.newproj_progress_slug.clone().unwrap_or_default());
            for line in drained {
                text.push('\n');
                text.push_str(&line);
            }
            self.newproj_progress_text = Some(text);
            self.needs_redraw = true;
        }
        if self.newproj_progress_text.is_some() {
            self.refresh_newproj_progress_message();
        }

        // Apply the final outcome once the worker has deposited it.
        let outcome = {
            let mut guard = self
                .newproj_result
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.take()
        };
        let Some(outcome) = outcome else {
            return;
        };
        self.newproj_running = false;
        self.needs_redraw = true;
        match outcome {
            Ok((summary, stack_note)) => {
                let mut text = format!("From: /new\n\n{summary}");
                text.push_str(&stack_note);
                // Start a fresh message: the progress panel message must
                // not absorb the summary as a trailing text part.
                self.force_new_message = true;
                self.append_assistant_text(&text);
                self.status = "new: scaffolded".to_string();
                self.push_log_no_agent(LogLevel::Info, "new: scaffold complete".to_owned());
            }
            Err(err) => {
                self.force_new_message = true;
                self.append_assistant_text(&format!("From: /new\n\n[err] **{err}**"));
                self.status = "new: emission failed".to_string();
                self.push_log_no_agent(LogLevel::Warn, "new: scaffold failed".to_owned());
            }
        }
        // Arm the status auto-expiry timer so the terminal status
        // transitions back to "ready" after the grace period.
        self.arm_status_expiry();
    }

    /// Replace the active scaffold progress message in place with the
    /// current rendered panel text (T-013).
    ///
    /// Mirrors `refresh_research_progress_message`: the message is located
    /// by its first line (`Scaffolding 'slug'...`) so repeated polls update
    /// the same message instead of stacking one message per step.
    fn refresh_newproj_progress_message(&mut self) {
        let Some(rendered) = self.newproj_progress_text.clone() else {
            return;
        };
        let Some(header_line) = self.newproj_progress_slug.clone() else {
            return;
        };
        for msg in self.messages.iter_mut() {
            if msg.role != ragent_agent::message::Role::Assistant {
                continue;
            }
            if let Some(ragent_agent::message::MessagePart::Text { text }) = msg.parts.first_mut()
                && text.lines().next() == Some(header_line.as_str())
            {
                *text = rendered;
                msg.touch();
                return;
            }
        }
        // No panel message yet: create one (first drained batch). The raw
        // header is stored verbatim so the first-line lookup keeps matching.
        if let Some(ref sid) = self.session_id {
            self.force_new_message = false;
            self.messages.push(ragent_agent::message::Message::new(
                sid.clone(),
                ragent_agent::message::Role::Assistant,
                vec![ragent_agent::message::MessagePart::Text { text: rendered }],
            ));
            self.trim_messages_if_needed();
        }
    }
}

/// Execute the blocking scaffold pipeline: planning + emission, local git,
/// remote-init, and the FR-011 summary assembly (T-007/T-008/T-009/T-010
/// flow) over the shared engine pipeline ([`plan_and_emit`]).
///
/// Runs on the worker thread (or inline when the worker cannot spawn).
/// Progress notes stream through `progress`; failures are contained
/// (FR-010): the summary still reports whatever completed.
fn run_scaffold_steps(
    cwd: &std::path::Path,
    slug: &str,
    request: &ScaffoldRequest,
    recipe: &'static ragent_tools_extended::project_scaffold::LanguageRecipe,
    hosting: Option<HostingTarget>,
    progress: &std::sync::Mutex<Vec<String>>,
) -> Result<(String, String), String> {
    push_progress(
        progress,
        ProgressLine::Start("writing project files".to_owned()),
    );
    let generated_at_utc = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let partial = match plan_and_emit(cwd, request, slug, recipe, &generated_at_utc) {
        Ok(partial) => {
            push_progress(
                progress,
                ProgressLine::Done(
                    format!(
                        "writing project files ({} created)",
                        partial.0.emit.created.len()
                    ),
                    true,
                ),
            );
            partial
        }
        Err(err) => {
            push_progress(
                progress,
                ProgressLine::Done("writing project files".to_owned(), false),
            );
            return Err(err.to_string());
        }
    };
    let (mut summary, stack_note) = partial;

    // FR-008 local half: git init + initial commit, then the remote
    // half (T-009 GitHub / T-010 GitLab) when a hosting flag was
    // supplied. Failures are contained (FR-010): the summary
    // reports the failed step while the local scaffold stays intact.
    push_progress(
        progress,
        ProgressLine::Start("initialising git repository".to_owned()),
    );
    let git_outcome = init_and_commit(cwd, "Initial scaffold");
    let git_ok = git_outcome.is_ok();
    push_progress(
        progress,
        ProgressLine::Done("initialising git repository".to_owned(), git_ok),
    );

    let remote_status = match hosting {
        None => {
            push_progress(
                progress,
                ProgressLine::Done("no hosting flag, no remote".to_owned(), true),
            );
            RemoteStatus::None
        }
        Some(target) => {
            let label = match target {
                HostingTarget::GitHub => "github",
                HostingTarget::GitLab => "gitlab",
            };
            push_progress(
                progress,
                ProgressLine::Start(format!("creating {label} repository and pushing")),
            );
            let status = match target {
                HostingTarget::GitHub => match init_github_remote(cwd, slug, true) {
                    Ok(report) => RemoteStatus::Created { url: report.url },
                    Err(failure) => RemoteStatus::Failed {
                        step: failure.step.as_str().to_owned(),
                        message: failure.message,
                    },
                },
                HostingTarget::GitLab => match init_gitlab_remote(cwd, slug, true) {
                    Ok(report) => RemoteStatus::Created { url: report.url },
                    Err(failure) => RemoteStatus::Failed {
                        step: failure.step.as_str().to_owned(),
                        message: failure.message,
                    },
                },
            };
            let ok = matches!(status, RemoteStatus::Created { .. });
            push_progress(
                progress,
                ProgressLine::Done(format!("creating {label} repository and pushing"), ok),
            );
            status
        }
    };

    summary.git = Some(git_outcome);
    summary.remote = remote_status;
    push_progress(
        progress,
        ProgressLine::Done("scaffold complete".to_owned(), true),
    );
    Ok((summary.render(), stack_note))
}
