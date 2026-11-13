//! Projects in the TUI: `/project`, sessions created in a named project,
//! base-model seeding and the pull/merge request count refresh (spec
//! `projects`).
//!
//! A project is a stored record (`ragent_agent::project::Project`). Every
//! session belongs to one; a session may also own a git worktree of the
//! project's repository at a chosen branch. The sidebar reads the stored
//! data; this module is what changes it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ragent_agent::project::worktree::{default_worktree_root, prune_worktrees};
use ragent_agent::project::{Project, ProjectManager, parse_model_ref};

use super::helpers::{from_cmd, short_session_id};
use super::state::App;

/// How often the poll loop looks for projects whose request counts are due.
const REQUEST_SWEEP_INTERVAL: Duration = Duration::from_secs(20);

/// Body of `/project help`.
pub(super) const PROJECT_HELP: &str = "## /project - Projects\n\n\
A project is a local checkout with a base model and, when it came from GitHub or GitLab, \
its remote. Every session belongs to a project.\n\n\
| Subcommand | Description |\n|---|---|\n\
| `/project` or `/project show [name]` | Show the focused session's project, or the named one |\n\
| `/project list` | List projects |\n\
| `/project add <path-or-url>` | Register a directory, or clone a GitHub/GitLab URL into \
this directory and register it |\n\
| `/project rename <name>` | Rename the focused project |\n\
| `/project model <provider>/<model>` | Set the model new sessions in the project start with |\n\
| `/project model current` | Use the focused session's model as the base model |\n\
| `/project model clear` | Remove the base model |\n\
| `/project refresh` | Re-read the remote and refresh pull/merge request counts |\n\
| `/project prune` | Remove clean worktrees of archived sessions |\n\
| `/project remove <name>` | Remove a project with no live sessions from the list \
(files are kept) |\n\
| `/project help` | Show this help |\n\n\
Start a session in a project with `/session new --project <name> [--branch <branch>]`. \
With `--branch` the session gets its own git worktree at that branch. The project's \
checkout must be inside the launch directory or a `dirs.allowed_roots` entry.";

/// The outcome of creating a session with its own worktree in the
/// background: the session and its project, or why it failed.
pub(super) type PendingProjectSession = Result<(ragent_agent::session::Session, Project), String>;

/// Options of `/session new` that name a project or branch.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct NewSessionOptions {
    /// `--project <name>`.
    pub project: Option<String>,
    /// `--branch <branch>`.
    pub branch: Option<String>,
    /// A positional directory.
    pub path: Option<String>,
}

/// Splits a command line into words, honouring single and double quotes so
/// a value may contain spaces (`--project "My App"`).
///
/// # Errors
///
/// Returns a message when a quote is not closed.
fn split_quoted(args: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in args.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            None => {
                current.push(c);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return Err("unclosed quote".to_string());
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

/// Parses the arguments of `/session new`.
///
/// With no `--` option the whole text is the directory, spaces included,
/// as it was before options existed. With options, words are split on
/// whitespace and a value with spaces must be quoted.
///
/// # Errors
///
/// Returns a usage message for a flag without a value, an unknown flag, more
/// than one path, an unclosed quote, or a path combined with `--project`.
pub(super) fn parse_new_session_args(args: &str) -> Result<NewSessionOptions, String> {
    let mut options = NewSessionOptions::default();
    let words = split_quoted(args)?;
    if !words.iter().any(|w| w.starts_with("--")) {
        let path = args.trim();
        if !path.is_empty() {
            options.path = Some(path.to_string());
        }
        return Ok(options);
    }
    let mut words = words.into_iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--project" | "--branch" => {
                let value = words
                    .next()
                    .filter(|v| !v.starts_with("--"))
                    .ok_or_else(|| format!("{word} needs a value"))?;
                if word == "--project" {
                    options.project = Some(value);
                } else {
                    options.branch = Some(value);
                }
            }
            flag if flag.starts_with("--") => {
                return Err(format!(
                    "unknown option `{flag}`; use --project <name> or --branch <branch>"
                ));
            }
            _ if options.path.is_none() => options.path = Some(word),
            _ => {
                return Err(
                    "usage: /session new [path] [--project <name>] [--branch <branch>] \
                     (quote a value that contains spaces)"
                        .to_string(),
                );
            }
        }
    }
    if options.path.is_some() && options.project.is_some() {
        return Err(
            "give a path or --project, not both (quote a value that contains spaces)".to_string(),
        );
    }
    Ok(options)
}

/// Whether `text` is a remote URL rather than a local path.
fn looks_like_url(text: &str) -> bool {
    text.contains("://") || (text.contains('@') && text.contains(':'))
}

impl App {
    /// A project manager over this app's storage and event bus.
    pub(crate) fn project_manager(&self) -> ProjectManager {
        ProjectManager::new(Arc::clone(&self.storage), Arc::clone(&self.event_bus))
            .with_github_hosts(self.github_hosts())
    }

    /// The configured GitHub Enterprise Server hosts (`vcs.github.hosts`).
    pub(crate) fn github_hosts(&self) -> Vec<ragent_config::GitHubHost> {
        self.session_processor
            .load_config_cached()
            .vcs
            .github
            .hosts
            .clone()
    }

    /// [`App::project_manager`] for the consolidated test target; not part
    /// of the crate's public API surface.
    #[doc(hidden)]
    pub fn project_manager_for_test(&self) -> ProjectManager {
        self.project_manager()
    }

    /// [`App::focused_project`] for the consolidated test target; not part
    /// of the crate's public API surface.
    ///
    /// # Errors
    ///
    /// Returns a message when storage fails.
    #[doc(hidden)]
    pub fn focused_project_for_test(&self) -> Result<Project, String> {
        self.focused_project()
    }

    /// The clone step of `/project add <url>` for the consolidated test
    /// target; not part of the crate's public API surface.
    ///
    /// # Errors
    ///
    /// Returns the scrubbed clone or registration error.
    #[doc(hidden)]
    pub fn clone_project_for_test(
        &self,
        url: &str,
        clean_url: &str,
        parent: &Path,
        name: &str,
    ) -> anyhow::Result<Project> {
        clone_and_register(&self.project_manager(), url, clean_url, parent, name)
    }

    /// The directory session worktrees are created under.
    pub(crate) fn session_worktree_root(&self) -> PathBuf {
        self.worktree_root
            .clone()
            .or_else(default_worktree_root)
            .unwrap_or_else(|| self.cwd_path.join(".ragent").join("worktrees"))
    }

    /// The project of the focused session.
    ///
    /// # Errors
    ///
    /// Returns a message when storage fails.
    pub(crate) fn focused_project(&self) -> Result<Project, String> {
        let directory = if self.session.directory.as_os_str().is_empty() {
            &self.cwd_path
        } else {
            &self.session.directory
        };
        self.project_manager()
            .resolve_for_dir(directory)
            .map_err(|e| e.to_string())
    }

    /// Looks up a listed project by name, id or id prefix.
    fn named_project(&self, key: &str) -> Result<Project, String> {
        self.project_manager()
            .find(key)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no project named `{key}` (/project list shows them)"))
    }

    /// Starts the focused session on its project's base model, when the
    /// project has one (spec `projects`, T-007). Called once, when a session
    /// is created; `/model` afterwards changes only that session.
    pub(crate) fn seed_project_base_model(&mut self, project: &Project) {
        let Some(label) = project.base_model_label() else {
            return;
        };
        if self.session.selected_model.as_deref() == Some(label.as_str()) {
            return;
        }
        self.session.selected_model = Some(label);
        // The window is looked up from the model list when it is needed.
        self.session.selected_model_ctx_window = None;
        self.session.cached_provider_model_label = None;
        self.status_bar_cache = None;
    }

    /// Starts a new session in `project` and focuses it; the previous
    /// session keeps running in the background.
    ///
    /// Without a branch the session is created and focused at once. With a
    /// branch, creating the git worktree may fetch from the remote, so it
    /// runs on a blocking task and the session is focused when it finishes
    /// (see [`App::poll_pending_project_sessions`]); the UI stays responsive
    /// meanwhile and a failure is reported on the status line.
    ///
    /// # Arguments
    ///
    /// * `project` - the owning project.
    /// * `branch` - when given, the session gets its own git worktree at
    ///   this branch; otherwise it works in the project worktree.
    ///
    /// # Errors
    ///
    /// Returns a message when the project's checkout is outside the
    /// permitted roots, when the session cannot be created or, for a
    /// branch, when the work cannot be started; the previous session stays
    /// focused.
    pub fn open_new_session_in_project(
        &mut self,
        project: &Project,
        branch: Option<&str>,
    ) -> Result<(), String> {
        // The same containment rule as `/session new <path>` (SEC-062): the
        // project's checkout must be inside the launch directory or a
        // `dirs.allowed_roots` entry. Checking the checkout, not the path
        // the user typed, also covers an allowed subdirectory whose
        // repository root lies outside the permitted roots. A session's own
        // worktree is created by ragent under its data directory and needs
        // no check.
        self.allowed_session_directory(&project.worktree.display().to_string())?;
        let manager = self.project_manager();
        let sessions = Arc::clone(&self.session_processor.session_manager);
        let root = self.session_worktree_root();
        let Some(branch) = branch else {
            let created = manager
                .create_session(&sessions, &project.id, None, &root)
                .map_err(|e| e.to_string())?;
            self.focus_created_session(&created, project);
            return Ok(());
        };
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let pending = Arc::clone(&self.pending_project_sessions);
        let project = project.clone();
        let branch = branch.to_string();
        self.status = format!("session: creating a worktree for {branch}...");
        runtime.spawn_blocking(move || {
            let result = manager
                .create_session(&sessions, &project.id, Some(&branch), &root)
                .map(|session| (session, project))
                .map_err(|e| e.to_string());
            pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(result);
        });
        Ok(())
    }

    /// Focuses the sessions whose worktrees finished being created in the
    /// background, and reports the ones that failed. Called from the poll
    /// loop.
    pub fn poll_pending_project_sessions(&mut self) {
        let finished: Vec<PendingProjectSession> = std::mem::take(
            &mut *self
                .pending_project_sessions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for result in finished {
            match result {
                Ok((created, project)) => self.focus_created_session(&created, &project),
                Err(message) => self.status = format!("session: {message}"),
            }
            self.sidebar.dirty = true;
            self.needs_redraw = true;
        }
    }

    /// Makes a session that already exists in storage the focused one,
    /// parking the previous session.
    fn focus_created_session(
        &mut self,
        created: &ragent_agent::session::Session,
        project: &Project,
    ) {
        let mut fresh = self.fresh_session_view();
        fresh.directory = created.directory.clone();
        fresh.session_id = Some(created.id.clone());
        let previous = std::mem::replace(&mut self.session, fresh);
        self.clear_input_queue();
        self.sid_to_display_name.insert(
            short_session_id(&created.id),
            self.session.agent_name.clone(),
        );
        self.seed_project_base_model(project);
        self.park_session(previous);
        self.after_focus_change();
        // Every newly created session prints the startup block.
        self.announce_session_startup();
        self.request_counts.nudge(&project.id);
    }

    /// `/session new [path] [--project <name>] [--branch <branch>]`.
    pub(super) fn session_new_command(&mut self, args: &str) -> Result<(), String> {
        let options = parse_new_session_args(args)?;
        if options.project.is_none() && options.branch.is_none() {
            return match options.path {
                Some(path) => {
                    let dir = self.allowed_session_directory(&path)?;
                    self.open_new_session_in(dir)
                }
                None => self.open_new_session(),
            };
        }
        let project = match (&options.project, &options.path) {
            (Some(name), _) => self.named_project(name)?,
            (None, Some(path)) => {
                let dir = self.allowed_session_directory(path)?;
                self.project_manager()
                    .resolve_for_dir(&dir)
                    .map_err(|e| e.to_string())?
            }
            (None, None) => self.focused_project()?,
        };
        self.open_new_session_in_project(&project, options.branch.as_deref())
    }

    /// Looks for projects whose pull/merge request counts are due and
    /// refreshes them in the background. Called from the poll loop; does
    /// nothing while the sidebar is hidden or within the sweep interval.
    pub fn poll_request_counts(&mut self) {
        if !self.sidebar.visible {
            return;
        }
        if self
            .last_request_sweep
            .is_some_and(|at| at.elapsed() < REQUEST_SWEEP_INTERVAL)
        {
            return;
        }
        self.last_request_sweep = Some(Instant::now());
        self.spawn_remote_check();
        self.spawn_request_refresh(None);
    }

    /// Detects, off the UI thread, the remote of every project never
    /// checked, so its forge tag shows without a session being opened in
    /// it. Each check publishes `ProjectUpdated`, which reloads the sidebar;
    /// a checked project is skipped on later sweeps.
    fn spawn_remote_check(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let manager = self.project_manager();
        runtime.spawn_blocking(move || {
            if let Err(e) = manager.check_unchecked_remotes() {
                tracing::debug!(error = %e, "project remote check failed");
            }
        });
    }

    /// Refreshes request counts off the UI thread: for `only`, or for every
    /// project with a remote that is due. The service publishes
    /// `ProjectUpdated` when a stored count changes, which reloads the
    /// sidebar.
    fn spawn_request_refresh(&self, only: Option<Project>) {
        let projects = match only {
            Some(project) => vec![project],
            None => match self.project_manager().list() {
                Ok(projects) => projects,
                Err(e) => {
                    tracing::debug!(error = %e, "could not list projects");
                    return;
                }
            },
        };
        let due: Vec<Project> = projects
            .into_iter()
            .filter(|p| p.remote.is_some() && self.request_counts.is_due(&p.id))
            .collect();
        if due.is_empty() {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let service = Arc::clone(&self.request_counts);
        runtime.spawn(async move {
            for project in due {
                service.refresh(&project).await;
            }
        });
    }

    /// Hints that the focused session's project may have new requests (its
    /// turn ended, or it was just focused).
    pub(crate) fn nudge_request_counts(&mut self) {
        if let Ok(project) = self.focused_project() {
            self.request_counts.nudge(&project.id);
            self.last_request_sweep = None;
        }
    }

    fn project_summary(&self, project: &Project) -> String {
        let remote = project.remote.as_ref().map_or_else(
            || "none (local project)".to_string(),
            |r| {
                let branch = r
                    .default_branch
                    .as_deref()
                    .map(|b| format!(", default branch `{b}`"))
                    .unwrap_or_default();
                format!("{} `{}` on {}{branch}", r.kind, r.path, r.host)
            },
        );
        let sessions = self
            .storage
            .list_project_sessions(&project.id, false)
            .map_or(0, |rows| rows.len());
        format!(
            "## Project {}\n\n| | |\n|---|---|\n\
             | Worktree | `{}` |\n| Remote | {remote} |\n| Base model | {} |\n\
             | Sessions | {sessions} |\n| Id | `{}` |",
            project.name,
            project.worktree.display(),
            project
                .base_model_label()
                .unwrap_or_else(|| "none (uses the selected model)".to_string()),
            project.id
        )
    }

    fn project_list_text(&self) -> Result<String, String> {
        let projects = self.project_manager().list().map_err(|e| e.to_string())?;
        if projects.is_empty() {
            return Ok("No projects yet.".to_string());
        }
        let mut text = String::from(
            "| Project | Worktree | Remote | Base model | Sessions |\n|---|---|---|---|---|\n",
        );
        for project in &projects {
            let sessions = self
                .storage
                .list_project_sessions(&project.id, false)
                .map_or(0, |rows| rows.len());
            text.push_str(&format!(
                "| {} | `{}` | {} | {} | {sessions} |\n",
                project.name,
                project.worktree.display(),
                project
                    .remote
                    .as_ref()
                    .map_or_else(String::new, |r| format!("{} {}", r.kind, r.path)),
                project.base_model_label().unwrap_or_default(),
            ));
        }
        Ok(text)
    }

    /// `/project add <path-or-url>`.
    fn project_add(&mut self, target: &str) -> Result<String, String> {
        if !looks_like_url(target) {
            let dir = self.allowed_session_directory(target)?;
            let project = self
                .project_manager()
                .resolve_for_dir(&dir)
                .map_err(|e| e.to_string())?;
            return Ok(format!(
                "Project **{}** registered at `{}`.",
                project.name,
                project.worktree.display()
            ));
        }
        // A URL: only GitHub and GitLab are supported, and the clone lands in
        // the focused directory under the repository's name.
        let gitlab_host = ragent_tools_vcs::gitlab::auth::load_config(self.storage.as_ref())
            .map(|c| c.instance_url);
        let remote = ragent_agent::project::parse_remote_url_with_hosts(
            target,
            "origin",
            gitlab_host.as_deref(),
            &self.github_hosts(),
        )
        .ok_or_else(|| format!("`{target}` is not a GitHub or GitLab repository URL"))?;
        let name = remote.path.rsplit('/').next().unwrap_or("repo").to_string();
        let dest = self.cwd_path.join(&name);
        if dest.exists() {
            return Err(format!("`{}` already exists", dest.display()));
        }
        let manager = self.project_manager();
        let parent = self.cwd_path.clone();
        let url = target.to_string();
        let clean_url = remote.url.clone();
        let events = Arc::clone(&self.event_bus);
        let session_id = self.session.session_id.clone().unwrap_or_default();
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        // Cloning can take a while; keep it off the UI thread and report the
        // result as a notice in this session.
        runtime.spawn_blocking(move || {
            let message = match clone_and_register(&manager, &url, &clean_url, &parent, &name) {
                Ok(project) => format!(
                    "Project {} cloned to {}",
                    project.name,
                    project.worktree.display()
                ),
                Err(e) => format!("/project add failed: {e}"),
            };
            events.publish(ragent_agent::event::Event::AgentNotice {
                session_id,
                message,
            });
        });
        Ok(format!(
            "Cloning {} `{}` into `{}`...",
            remote.kind,
            remote.path,
            dest.display()
        ))
    }

    /// `/project model <provider>/<model> | current | clear`.
    fn project_model(&mut self, arg: &str) -> Result<String, String> {
        let project = self.focused_project()?;
        let manager = self.project_manager();
        match arg {
            "" => Ok(format!(
                "Base model of **{}**: {}",
                project.name,
                project
                    .base_model_label()
                    .unwrap_or_else(|| "none".to_string())
            )),
            "clear" | "none" => {
                manager
                    .set_base_model(&project.id, None)
                    .map_err(|e| e.to_string())?;
                Ok(format!("Base model of **{}** cleared.", project.name))
            }
            text => {
                let label = if text == "current" {
                    self.session
                        .selected_model
                        .clone()
                        .ok_or("this session has no model selected")?
                } else {
                    text.to_string()
                };
                let model = parse_model_ref(&label).map_err(|e| e.to_string())?;
                manager
                    .set_base_model(&project.id, Some(&model))
                    .map_err(|e| e.to_string())?;
                Ok(format!(
                    "New sessions in **{}** start on `{label}`.",
                    project.name
                ))
            }
        }
    }

    /// `/project prune`.
    fn project_prune(&self) -> Result<String, String> {
        let project = self.focused_project()?;
        let report = prune_worktrees(&self.storage, &project).map_err(|e| e.to_string())?;
        let mut text = format!("Removed {} worktree(s).", report.removed.len());
        for path in &report.removed {
            text.push_str(&format!("\n- removed `{}`", path.display()));
        }
        for skipped in &report.skipped {
            text.push_str(&format!(
                "\n- kept `{}`: {}",
                skipped.path.display(),
                skipped.reason
            ));
        }
        Ok(text)
    }

    /// Handles `/project <args>` (help is handled by the slash dispatcher).
    ///
    /// # Arguments
    ///
    /// * `args` - text after `/project`.
    pub(crate) fn handle_project_command(&mut self, args: &str) {
        let (sub, rest) = args
            .split_once(char::is_whitespace)
            .map_or((args, ""), |(s, r)| (s, r.trim()));
        let result: Result<String, String> = match sub {
            "" | "show" => if rest.is_empty() {
                self.focused_project()
            } else {
                self.named_project(rest)
            }
            .map(|p| self.project_summary(&p)),
            "list" => self.project_list_text(),
            "add" if rest.is_empty() => Err("usage: /project add <path-or-url>".to_string()),
            "add" => self.project_add(rest),
            "rename" if rest.is_empty() => Err("usage: /project rename <name>".to_string()),
            "rename" => {
                // `--project <id> <name>` renames a listed project (the
                // sidebar's project menu); otherwise the focused one.
                let target = match rest.strip_prefix("--project ") {
                    Some(tail) => tail
                        .trim_start()
                        .split_once(char::is_whitespace)
                        .map(|(id, name)| (Some(id), name.trim()))
                        .ok_or_else(|| {
                            "usage: /project rename [--project <id>] <name>".to_string()
                        }),
                    None => Ok((None, rest)),
                };
                target.and_then(|(id, name)| {
                    let project = match id {
                        Some(id) => self.named_project(id)?,
                        None => self.focused_project()?,
                    };
                    self.project_manager()
                        .rename(&project.id, name)
                        .map_err(|e| e.to_string())?;
                    Ok(format!("Project renamed to **{}**.", name.trim()))
                })
            }
            "model" => self.project_model(rest),
            "refresh" => self.focused_project().and_then(|p| {
                let refreshed = self
                    .project_manager()
                    .refresh_remote(&p.id)
                    .map_err(|e| e.to_string())?;
                self.request_counts.force_due(&refreshed.id);
                self.spawn_request_refresh(Some(refreshed.clone()));
                Ok(self.project_summary(&refreshed))
            }),
            "prune" => self.project_prune(),
            "remove" if rest.is_empty() => Err("usage: /project remove <name>".to_string()),
            "remove" => self.named_project(rest).and_then(|p| {
                self.project_manager()
                    .remove(&p.id)
                    .map_err(|e| e.to_string())?;
                Ok(format!(
                    "Project **{}** removed from the list; its files are untouched.",
                    p.name
                ))
            }),
            other => Err(format!(
                "unknown subcommand `{other}` (/project help lists them)"
            )),
        };
        match result {
            Ok(text) => {
                self.append_assistant_text(&from_cmd(&format!("project {sub}"), &text));
                self.status = format!("project: {}", if sub.is_empty() { "show" } else { sub });
            }
            Err(message) => self.status = format!("project: {message}"),
        }
        self.sidebar.dirty = true;
    }
}

/// Clones `url` into `parent/name` and registers the result as a project.
///
/// `url` is used for the clone as typed, so a credential embedded in it
/// keeps working. It must never reach the transcript: the git runner quotes
/// its arguments in a timeout error and git may echo the URL on stderr, so
/// every error leaves here with `url` replaced by `clean_url` (the same URL
/// without credentials) and any remaining secret-shaped text redacted.
fn clone_and_register(
    manager: &ProjectManager,
    url: &str,
    clean_url: &str,
    parent: &Path,
    name: &str,
) -> anyhow::Result<Project> {
    let scrub = |text: &str| ragent_types::sanitize::redact_secrets(&text.replace(url, clean_url));
    ragent_tools_vcs::git::reject_option_like(url, "url")
        .map_err(|e| anyhow::anyhow!("{}", scrub(&e.to_string())))?;
    let output = ragent_tools_vcs::git::run_git_output(&["clone", "--", url, name], parent)
        .map_err(|e| anyhow::anyhow!("{}", scrub(&format!("{e:#}"))))?;
    anyhow::ensure!(
        output.success,
        "git clone failed: {}",
        scrub(String::from_utf8_lossy(&output.stderr).trim())
    );
    manager.resolve_for_dir(&parent.join(name))
}
