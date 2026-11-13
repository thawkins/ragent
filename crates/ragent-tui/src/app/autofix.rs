//! `/autofix`: binding a session to its pull request (GitHub) or merge
//! request (GitLab), and Auto-fix itself (spec SESSIONMENU M4-M5,
//! FR-14 to FR-22).
//!
//! A binding is stored against the top-level session. `github_create_pr`
//! and `gitlab_create_mr` create it automatically; `/autofix bind` creates it
//! for a request that already exists. Lookups go through
//! `ragent_agent::project::requests`, run off the UI thread, and report back
//! as an `AgentNotice` in the session that asked.
//!
//! With Auto-fix on, the watcher (`ragent_agent::autofix`) publishes
//! `Event::AutofixTrigger`; [`App::handle_autofix_trigger`] sends its prompt
//! to the bound session as a user message, or queues it when that session
//! is busy, opening the session in the background when it is not open.

use std::path::Path;
use std::sync::Arc;

use ragent_agent::project::requests::{BindOutcome, BindTarget, RequestInfo, bind_session};
use ragent_agent::project::{ProjectRemote, RemoteKind};
use ragent_storage::SessionRequest;

use super::App;
use super::helpers::from_cmd;

/// `/autofix help` text.
pub(crate) const AUTOFIX_HELP: &str = "## /autofix - Pull/merge request binding\n\n\
| Subcommand | Description |\n|---|---|\n\
| `/autofix` or `/autofix status` | Show the PR/MR this session is bound to; when none, look for an open one on the session branch |\n\
| `/autofix bind` | Bind the single open PR/MR of the session branch |\n\
| `/autofix bind <number or URL>` | Bind that PR/MR (`12`, `#12`, `!12` or its web URL) |\n\
| `/autofix unbind` | Remove the binding |\n\
| `/autofix on` / `/autofix off` | Turn Auto-fix on or off: wake this session when CI fails, the PR/MR conflicts, or a reviewer comments |\n\
| `/autofix help` | Show this help |\n\n\
A PR or MR the agent opens with `github_create_pr` / `gitlab_create_mr` is bound automatically. \
Auto-fix never merges, never force-pushes and never resolves review threads; after \
`vcs.autofix.max_attempts` wake-ups for one kind on one commit it pauses until `/autofix on`.";

/// Reads a request reference: `12`, `#12`, `!12`, or a web URL ending in
/// `/pull/12` (GitHub) or `/-/merge_requests/12` (GitLab).
///
/// # Arguments
///
/// * `arg` - the reference as typed.
/// * `remote` - the session's project remote; a URL must name its
///   repository.
///
/// # Errors
///
/// Returns a user-facing message when `arg` is not a request reference or
/// a URL points at another repository.
pub fn parse_request_ref(arg: &str, remote: &ProjectRemote) -> Result<u64, String> {
    let arg = arg.trim();
    let noun = remote.kind.noun();
    if arg.contains("://") {
        let marker = match remote.kind {
            RemoteKind::GitHub => "/pull/",
            RemoteKind::GitLab => "/-/merge_requests/",
        };
        let lower = arg.to_ascii_lowercase();
        let (before, after) = lower
            .split_once(marker)
            .ok_or_else(|| format!("`{arg}` is not a {} URL", noun.long))?;
        let repo = remote.path.to_ascii_lowercase();
        if !before.ends_with(&format!("/{repo}")) {
            return Err(format!("`{arg}` is not a {} of {}", noun.long, remote.path));
        }
        let digits = after.split(['/', '?', '#']).next().unwrap_or_default();
        return digits
            .parse()
            .map_err(|_| format!("`{arg}` has no {} number", noun.short));
    }
    arg.trim_start_matches(['#', '!'])
        .parse()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("`{arg}` is not a {} number", noun.short))
}

/// `PR #12` / `MR !5` for a stored binding.
fn label_of(request: &SessionRequest) -> String {
    let noun = RemoteKind::parse(&request.forge).map_or(
        ragent_agent::project::RequestNoun {
            short: "request",
            long: "request",
            sigil: '#',
        },
        RemoteKind::noun,
    );
    format!("{} {}{}", noun.short, noun.sigil, request.number)
}

/// `PR #12` / `MR !5` for a looked-up request.
fn label_for(kind: RemoteKind, info: &RequestInfo) -> String {
    let noun = kind.noun();
    format!("{} {}{}", noun.short, noun.sigil, info.number)
}

/// The notice text for a finished `/autofix bind`.
fn bind_message(kind: RemoteKind, branch: Option<&str>, outcome: &BindOutcome) -> String {
    let noun = kind.noun();
    match outcome {
        BindOutcome::Bound(info) => format!(
            "Bound to {} \"{}\" ({}) {}",
            label_for(kind, info),
            info.title,
            info.state.as_str(),
            info.web_url
        ),
        BindOutcome::NoneOpen => format!(
            "No open {} has `{}` as its source branch.",
            noun.long,
            branch.unwrap_or_default()
        ),
        BindOutcome::Several(open) => {
            let list = open
                .iter()
                .map(|info| format!("{} \"{}\"", label_for(kind, info), info.title))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Several open {}s have `{}` as source: {list}. Use /autofix bind <number>.",
                noun.long,
                branch.unwrap_or_default()
            )
        }
    }
}

impl App {
    /// The request the top-level session of `id` is bound to, if any.
    pub(crate) fn session_request_of(&self, id: &str) -> Option<SessionRequest> {
        let found = self
            .storage
            .root_session_id(id)
            .and_then(|root| self.storage.session_request(&root));
        match found {
            Ok(request) => request,
            Err(e) => {
                tracing::warn!(error = %e, session_id = id, "could not read session binding");
                None
            }
        }
    }

    /// The focused session's id.
    fn autofix_session_id(&self) -> Result<String, String> {
        self.session
            .session_id
            .clone()
            .ok_or_else(|| "no session is open".to_string())
    }

    /// The branch session `id` works on: its checkout's branch, else the
    /// branch checked out in its directory.
    fn session_branch(&self, id: &str) -> Option<String> {
        if let Some(branch) = self
            .storage
            .session_checkout(id)
            .ok()
            .flatten()
            .and_then(|c| c.branch)
        {
            return Some(branch);
        }
        let dir = if self.session.directory.as_os_str().is_empty() {
            self.cwd_path.as_path()
        } else {
            self.session.directory.as_path()
        };
        current_branch(dir)
    }

    /// The focused project's remote and an API client for it.
    fn autofix_source(
        &self,
    ) -> Result<
        (
            ProjectRemote,
            ragent_agent::project::requests::RequestSource,
        ),
        String,
    > {
        let project = self.focused_project()?;
        let remote = project.remote.ok_or_else(|| {
            format!(
                "project **{}** has no GitHub or GitLab remote",
                project.name
            )
        })?;
        let source = self
            .request_counts
            .source_for(&remote)
            .ok_or_else(|| format!("no {} credentials for {}", remote.kind, remote.host))?;
        Ok((remote, source))
    }

    /// Runs a bind lookup off the UI thread and reports the result as a
    /// notice in session `notice_session`.
    fn spawn_bind(&self, notice_session: String, target: BindTarget) -> Result<(), String> {
        let (remote, source) = self.autofix_source()?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let storage = Arc::clone(&self.storage);
        let events = Arc::clone(&self.event_bus);
        let autofix_default = self
            .session_processor
            .load_config_cached()
            .vcs
            .autofix
            .default_on();
        runtime.spawn(async move {
            let branch = match &target {
                BindTarget::Branch(branch) => Some(branch.clone()),
                BindTarget::Number(_) => None,
            };
            let message = match bind_session(
                &storage,
                &notice_session,
                &source,
                &remote,
                target,
                autofix_default,
            )
            .await
            {
                Ok(outcome) => bind_message(remote.kind, branch.as_deref(), &outcome),
                Err(e) => format!("/autofix bind failed: {e}"),
            };
            events.publish(ragent_agent::event::Event::AgentNotice {
                session_id: notice_session,
                message,
            });
        });
        Ok(())
    }

    /// `/autofix bind [number|url]`.
    fn autofix_bind(&self, arg: &str) -> Result<String, String> {
        let id = self.autofix_session_id()?;
        let project = self.focused_project()?;
        let remote = project.remote.ok_or_else(|| {
            format!(
                "project **{}** has no GitHub or GitLab remote",
                project.name
            )
        })?;
        let noun = remote.kind.noun();
        if arg.is_empty() {
            let branch = self
                .session_branch(&id)
                .ok_or("this session is not on a branch; use /autofix bind <number>")?;
            self.spawn_bind(id, BindTarget::Branch(branch.clone()))?;
            return Ok(format!(
                "Looking for the open {} of `{branch}`...",
                noun.long
            ));
        }
        let number = parse_request_ref(arg, &remote)?;
        self.spawn_bind(id, BindTarget::Number(number))?;
        Ok(format!(
            "Looking up {} {}{number}...",
            noun.short, noun.sigil
        ))
    }

    /// `/autofix unbind`.
    fn autofix_unbind(&self) -> Result<String, String> {
        let id = self.autofix_session_id()?;
        let root = self
            .storage
            .root_session_id(&id)
            .map_err(|e| e.to_string())?;
        let Some(request) = self.session_request_of(&id) else {
            return Ok("This session is not bound to a pull or merge request.".to_string());
        };
        self.storage
            .clear_session_request(&root)
            .map_err(|e| e.to_string())?;
        Ok(format!("Unbound from {}.", label_of(&request)))
    }

    /// `/autofix status`.
    fn autofix_status(&self) -> Result<String, String> {
        let id = self.autofix_session_id()?;
        if let Some(request) = self.session_request_of(&id) {
            let state = self
                .storage
                .session_autofix(&request.session_id)
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            return Ok(format!(
                "Bound to {} \"{}\" ({} on {}, last seen {})\n\n{}\n\n{}",
                label_of(&request),
                request.title,
                request.repo,
                request.forge,
                request.state,
                request.web_url,
                autofix_lines(&self.storage, &request, &state)
            ));
        }
        // Not bound: offer the branch's open request, if there is one.
        let Some(branch) = self.session_branch(&id) else {
            return Ok("Not bound. This session is not on a branch.".to_string());
        };
        match self.spawn_bind_offer(id, branch.clone()) {
            Ok(()) => Ok(format!(
                "Not bound. Looking for an open request on `{branch}`..."
            )),
            Err(reason) => Ok(format!("Not bound ({reason}).")),
        }
    }

    /// Looks for open requests of `branch` without binding, and reports how
    /// to bind one.
    fn spawn_bind_offer(&self, notice_session: String, branch: String) -> Result<(), String> {
        let (remote, source) = self.autofix_source()?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let events = Arc::clone(&self.event_bus);
        runtime.spawn(async move {
            let noun = remote.kind.noun();
            let message = match ragent_agent::project::requests::open_requests_for_branch(
                &source, &remote, &branch,
            )
            .await
            {
                Ok(open) if open.is_empty() => {
                    format!("No open {} has `{branch}` as its source branch.", noun.long)
                }
                Ok(open) => open
                    .iter()
                    .map(|info| {
                        format!(
                            "Open {} \"{}\" on `{branch}`: /autofix bind {}",
                            label_for(remote.kind, info),
                            info.title,
                            info.number
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                Err(e) => format!("/autofix status lookup failed: {e}"),
            };
            events.publish(ragent_agent::event::Event::AgentNotice {
                session_id: notice_session,
                message,
            });
        });
        Ok(())
    }

    /// Dispatches `/autofix <sub> [args]`.
    pub(crate) fn handle_autofix_command(&mut self, args: &str) {
        let (sub, rest) = args
            .split_once(char::is_whitespace)
            .map_or((args, ""), |(s, r)| (s, r.trim()));
        let result = match sub {
            "" | "status" => self.autofix_status(),
            "bind" => self.autofix_bind(rest),
            "unbind" => self.autofix_unbind(),
            "on" | "off" => self
                .autofix_session_id()
                .and_then(|id| self.set_session_autofix(&id, sub == "on")),
            other => Err(format!(
                "unknown subcommand `{other}` (/autofix help lists them)"
            )),
        };
        let shown = if sub.is_empty() { "status" } else { sub };
        match result {
            Ok(text) => {
                self.append_assistant_text(&from_cmd(&format!("autofix {shown}"), &text));
                self.status = format!("autofix: {shown}");
            }
            Err(message) => self.status = format!("autofix: {message}"),
        }
        self.sidebar.dirty = true;
    }
}

impl App {
    /// Starts the Auto-fix watcher with the global `vcs.autofix` settings
    /// (spec SESSIONMENU D5). Called once at TUI startup; a second call does
    /// nothing.
    pub fn start_autofix_watcher(&mut self) {
        if self.autofix_wake.is_some() || tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let config = self
            .session_processor
            .load_config_cached()
            .vcs
            .autofix
            .clone();
        let counts = Arc::clone(&self.request_counts);
        let watcher = ragent_agent::autofix::AutofixWatcher::new(
            Arc::clone(&self.storage),
            Arc::clone(&self.event_bus),
            Arc::new(move |remote| counts.source_for(remote)),
            &config,
        );
        self.autofix_wake = Some(watcher.wake_handle());
        // Runs for the life of the process.
        drop(watcher.spawn());
    }

    /// Turns Auto-fix on or off for the request session `id` is bound to.
    ///
    /// # Returns
    ///
    /// A one-line confirmation.
    ///
    /// # Errors
    ///
    /// Returns a message when the session is not bound or the write fails.
    pub(crate) fn set_session_autofix(&mut self, id: &str, on: bool) -> Result<String, String> {
        let request = self
            .session_request_of(id)
            .ok_or("this session is not bound to a pull or merge request (/autofix bind)")?;
        self.storage
            .set_session_autofix(&request.session_id, on)
            .map_err(|e| e.to_string())?;
        self.sidebar.dirty = true;
        self.autofix_status.remove(&request.session_id);
        if !on {
            return Ok(format!("Auto-fix off for {}.", label_of(&request)));
        }
        if let Some(wake) = &self.autofix_wake {
            wake.notify_one();
        }
        let note = if request.state == "open" {
            String::new()
        } else {
            format!(" It is {}, so there is nothing to watch.", request.state)
        };
        Ok(format!("Auto-fix on for {}.{note}", label_of(&request)))
    }

    /// Flips Auto-fix for the request session `id` is bound to (menu
    /// `Auto-fix: on/off`).
    ///
    /// # Errors
    ///
    /// Returns a message when the session is not bound or the write fails.
    pub(crate) fn toggle_session_autofix(&mut self, id: &str) -> Result<String, String> {
        let root = self
            .storage
            .root_session_id(id)
            .map_err(|e| e.to_string())?;
        let on = self
            .storage
            .session_autofix(&root)
            .map_err(|e| e.to_string())?
            .is_some_and(|state| state.enabled);
        self.set_session_autofix(id, !on)
    }

    /// Acts on one `Event::AutofixTrigger` (FR-18, FR-20, FR-22).
    ///
    /// A trigger with a prompt is sent to session `session_id` as a user
    /// message, or queued when that session is busy; a session that is not
    /// open is opened in the background first. A trigger without a prompt
    /// (`paused`, `closed`) is shown as a notice. A background session is
    /// marked unread and the `on_notification` hook runs with kind
    /// `autofix`.
    ///
    /// # Arguments
    ///
    /// * `session_id` - the bound session.
    /// * `kind` - `ci`, `conflict`, `review`, `paused` or `closed`.
    /// * `summary` - the status line.
    /// * `prompt` - the message to send; empty for `paused` and `closed`.
    pub fn handle_autofix_trigger(
        &mut self,
        session_id: &str,
        kind: &str,
        summary: &str,
        prompt: &str,
    ) {
        self.autofix_status
            .insert(session_id.to_string(), summary.to_string());
        self.sidebar.dirty = true;
        let focused = self.session.session_id.as_deref() == Some(session_id);
        if !focused && self.background.position(session_id).is_none() {
            if let Err(e) = self.open_in_background(session_id) {
                tracing::warn!(error = %e, session_id, "autofix: could not open session");
                return;
            }
        }
        let status = self.status.clone();
        self.with_origin_session(Some(session_id), |app| {
            if prompt.is_empty() {
                app.session.force_new_message = true;
                app.append_assistant_text(&format!("Auto-fix: {summary}"));
                app.session.force_new_message = true;
            } else if app.session.is_processing
                || app.session.compact_in_progress
                || app.session.auto_compact_in_progress
                || app.session.pending_send_after_compact.is_some()
            {
                app.session.input_queue.push_back(super::QueuedInput {
                    text: prompt.to_string(),
                    image_paths: Vec::new(),
                });
            } else {
                // Keep whatever the user is typing in that session.
                let draft = std::mem::take(&mut app.session.input);
                let cursor = app.session.input_cursor;
                app.dispatch_user_message(prompt.to_string(), Vec::new());
                app.session.input = draft;
                app.session.input_cursor = cursor;
            }
            if !focused {
                app.session.unread = true;
                if let Err(e) = app.storage.set_session_unread(session_id, true) {
                    tracing::warn!(error = %e, session_id, "could not mark session unread");
                }
                let directory = app.session.directory.clone();
                let config = app.session_processor.load_config_for(&directory);
                ragent_agent::hooks::fire_notification_hooks(
                    &config, &directory, session_id, "autofix", summary,
                );
            }
        });
        self.status = if focused { summary.to_string() } else { status };
        tracing::info!(session_id, kind, summary, "autofix trigger");
        self.needs_redraw = true;
    }

    /// Opens stored session `session_id` in the background, leaving the
    /// focused session focused.
    fn open_in_background(&mut self, session_id: &str) -> Result<(), String> {
        let focused = self.session.session_id.clone();
        self.switch_to_session(session_id)?;
        match focused {
            Some(id) => self.switch_to_session(&id),
            None => Ok(()),
        }
    }
}

/// The Auto-fix part of `/autofix status`: on/off, paused, last poll, last
/// trigger, and attempts per kind at the last-seen head commit.
fn autofix_lines(
    storage: &ragent_storage::Storage,
    request: &SessionRequest,
    state: &ragent_storage::AutofixState,
) -> String {
    let mode = match (state.enabled, state.paused) {
        (false, _) => "off (/autofix on to turn it on)",
        (true, true) => "paused, waiting on you (/autofix on to resume)",
        (true, false) => "on",
    };
    let mut lines = vec![format!("Auto-fix: {mode}")];
    if let Some(at) = &state.last_polled_at {
        lines.push(format!("Last poll: {at}"));
    }
    if let Some(trigger) = &state.last_trigger {
        lines.push(format!("Last trigger: {trigger}"));
    }
    if let Some(sha) = &state.head_sha {
        let short: String = sha.chars().take(10).collect();
        let counts: Vec<String> = ["ci", "conflict", "review"]
            .iter()
            .map(|kind| {
                let n = storage
                    .autofix_attempts(&request.session_id, sha, kind)
                    .unwrap_or(0);
                format!("{kind} {n}")
            })
            .collect();
        lines.push(format!("Attempts at {short}: {}", counts.join(", ")));
    }
    lines.join("\n")
}

/// The branch checked out in `dir`, or `None` outside a work tree or on a
/// detached head.
fn current_branch(dir: &Path) -> Option<String> {
    let output =
        ragent_tools_vcs::git::run_git_output(&["rev-parse", "--abbrev-ref", "HEAD"], dir).ok()?;
    if !output.success {
        return None;
    }
    let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!branch.is_empty() && branch != "HEAD").then_some(branch)
}
