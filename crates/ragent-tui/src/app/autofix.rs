//! `/autofix`: binding a session to its pull request (GitHub) or merge
//! request (GitLab) (spec SESSIONMENU M4, FR-14).
//!
//! A binding is stored against the top-level session. `github_create_pr`
//! and `gitlab_create_mr` create it automatically; `/autofix bind` creates it
//! for a request that already exists. Lookups go through
//! `ragent_agent::project::requests`, run off the UI thread, and report back
//! as an `AgentNotice` in the session that asked.

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
| `/autofix help` | Show this help |\n\n\
A PR or MR the agent opens with `github_create_pr` / `gitlab_create_mr` is bound automatically.";

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
        let root = self.storage.root_session_id(id).ok()?;
        self.storage.session_request(&root).ok().flatten()
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
        runtime.spawn(async move {
            let branch = match &target {
                BindTarget::Branch(branch) => Some(branch.clone()),
                BindTarget::Number(_) => None,
            };
            let message =
                match bind_session(&storage, &notice_session, &source, &remote, target).await {
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
            return Ok(format!(
                "Bound to {} \"{}\" ({}, last seen {})\n\n{}",
                label_of(&request),
                request.title,
                request.repo,
                request.state,
                request.web_url
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
