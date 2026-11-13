//! Multi-session core: switching, background routing and `/session`
//! (spec MULTISESSION, milestones M3, M4 and M6).
//!
//! The focused session is always `App::session`; every other open session is
//! parked in `App::background`. Event handlers and session-scoped polls were
//! written against `self.session`, so work for a background session reuses
//! them unchanged: the background view is swapped into `self.session`, the
//! handler runs, and the views are swapped back. The swap is two pointer-sized
//! moves of the view's fields, and the global status line is restored after
//! it so background work never overwrites what the user is reading.
//!
//! Global UI (dialogs, panels, the status bar) only ever reads `self.session`,
//! so a permission request from a background session queues on that session
//! and cannot open a dialog over the focused one (FR-06).
//!
//! Each session works in its own directory (M6): focusing a session makes its
//! directory the TUI's `cwd_path`, which every project-scoped command reads.

use std::path::{Path, PathBuf};

use ragent_agent::event::Event;
use ragent_agent::message::Role;

use super::helpers::short_session_id;
use super::session_view::{Attention, SessionView};
use super::state::App;

/// Settings key holding the open session ids, focused first, then the
/// background sessions most recently focused first (FR-13).
const OPEN_SESSIONS_KEY: &str = "tui.sessions.open";

/// Most sessions reopened at startup; each one holds its transcript in
/// memory (NFR-02).
const MAX_RESTORED_SESSIONS: usize = 8;

/// Body of `/session help`.
pub(super) const SESSION_HELP: &str = "## /session - Multiple sessions\n\n\
| Subcommand | Description |\n|---|---|\n\
| `/session` or `/session list` | List open sessions and what each needs |\n\
| `/session new [path]` | Start a new session in `path` (default: this project); \
the current one keeps running |\n\
| `/session new --project <name>` | Start a new session in a named project |\n\
| `/session new --branch <branch>` | Start a new session in its own git worktree at \
`branch` (combine with `--project` or a path) |\n\
| `/session switch <id-or-title>` | Focus a session by id, id prefix/suffix or title prefix |\n\
| `/session close` | Close the focused session (it stays in storage) |\n\
| `/session list --all` | List stored sessions grouped by project |\n\
| `/session list --archived` | List archived sessions |\n\
| `/session rename <title>` | Rename the focused session |\n\
| `/session pin` | Pin or unpin the focused session |\n\
| `/session archive` | Archive and close the focused session |\n\
| `/session unarchive <id-or-title>` | Restore an archived session to the list |\n\
| `/session unread [id-or-title]` | Mark a session unread, or read again |\n\
| `/session fork [id-or-title]` | Copy a session's transcript into a new session |\n\
| `/session delete <id-or-title>` | Permanently delete a session (asks first) |\n\
| `/session section` | List sidebar sections |\n\
| `/session section <name>` / `--none` | Move the focused session to a section, or back to its project |\n\
| `/session section new <name>` | Create a section and move the focused session to it |\n\
| `/session section rename <name>` | Rename the focused session's section |\n\
| `/session section delete [name]` | Delete a section (default: the focused session's); its sessions go back to their projects |\n\
| `/session help` | Show this help |\n\n\
A `path` must be inside the launch directory or a `dirs.allowed_roots` entry.";

impl App {
    /// Index of the background session that owns events for `session_id`.
    ///
    /// A background session owns its own events, the events of sub-agents it
    /// spawned (`TaskEntry::parent_session_id`) and those of teammates in the
    /// team it leads.
    ///
    /// # Arguments
    ///
    /// * `session_id` - session id carried by an event.
    ///
    /// # Returns
    ///
    /// The owning background index, or `None` when the event belongs to the
    /// focused session or to no open session.
    pub(crate) fn background_owner(&self, session_id: &str) -> Option<usize> {
        if let Some(index) = self.background.position(session_id) {
            return Some(index);
        }
        let parent = self
            .active_tasks
            .iter()
            .find(|task| task.child_session_id == session_id)
            .map(|task| task.parent_session_id.as_str());
        if let Some(index) = parent.and_then(|p| self.background.position(p)) {
            return Some(index);
        }
        self.background.iter().position(|view| {
            view.team_members
                .iter()
                .any(|member| member.session_id.as_deref() == Some(session_id))
        })
    }

    /// Swaps the focused view with the background view at `index`.
    ///
    /// Calling it twice with the same index restores the original layout.
    fn swap_with_background(&mut self, index: usize) {
        if let Some(view) = self.background.view_mut(index) {
            std::mem::swap(&mut self.session, view);
        }
    }

    /// Runs `f` with the session `origin` in `self.session`, swapping a
    /// background session in and back out, so output from a long-running
    /// job lands in the session that started it (`/bench run`, `/new`,
    /// `/spec govcreate`). The status line is left as `f` sets it: job
    /// progress stays visible whichever session is focused. `None`, the
    /// focused session, or a session that is no longer open runs `f` on the
    /// focused session.
    pub(crate) fn with_origin_session<R>(
        &mut self,
        origin: Option<&str>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let index = origin
            .filter(|id| self.session.session_id.as_deref() != Some(*id))
            .and_then(|id| self.background.position(id));
        match index {
            None => f(self),
            Some(index) => {
                self.swap_with_background(index);
                let result = f(self);
                self.swap_with_background(index);
                result
            }
        }
    }

    /// Routes one bus event to the session that owns it.
    ///
    /// Events for a background session are applied to that session's view;
    /// everything else goes to the focused-session handler as before.
    ///
    /// # Arguments
    ///
    /// * `event` - the event received from the bus.
    pub async fn handle_event(&mut self, event: Event) {
        // Auto-fix wake-ups go to the bound session, opening it if needed.
        if let Event::AutofixTrigger {
            session_id,
            kind,
            summary,
            prompt,
        } = &event
        {
            self.handle_autofix_trigger(session_id, kind, summary, prompt);
            return;
        }
        // Session activity changes the sidebar's titles and ordering (NFR-03).
        if matches!(
            event,
            Event::SessionCreated { .. }
                | Event::SessionUpdated { .. }
                | Event::MessageEnd { .. }
                | Event::ProjectUpdated { .. }
        ) {
            self.sidebar.dirty = true;
        }
        // A finished turn may have opened a pull/merge request.
        if let Event::MessageEnd { session_id, .. } = &event {
            if self.session.session_id.as_deref() == Some(session_id.as_str()) {
                self.nudge_request_counts();
            }
        }
        // The end of a real reply (not the AGENTS.md `init` exchange) is when
        // an untitled session gets its title (FR-07).
        let finished_reply = match &event {
            Event::MessageEnd {
                session_id,
                message_id,
                ..
            } if message_id != "init" => Some(session_id.clone()),
            _ => None,
        };
        let owner = event
            .session_id()
            .and_then(|sid| self.background_owner(sid));
        match owner {
            Some(index) => self.handle_background_event(index, event).await,
            None => self.handle_focused_event(event).await,
        }
        if let Some(session_id) = finished_reply {
            self.auto_title(&session_id);
        }
    }

    /// Titles an untitled open session from its first prompt (FR-07).
    ///
    /// A title the user set (`/session rename`, F2) is never replaced: only
    /// a session whose stored title is empty is titled.
    fn auto_title(&mut self, session_id: &str) {
        let view = if self.session.session_id.as_deref() == Some(session_id) {
            Some(&self.session)
        } else {
            self.background
                .iter()
                .find(|v| v.session_id.as_deref() == Some(session_id))
        };
        let Some(title) = view.and_then(|v| {
            v.messages
                .iter()
                .find(|m| m.role == Role::User)
                .and_then(|m| title_from_prompt(&m.text_content()))
        }) else {
            return;
        };
        match self.storage.get_session(session_id) {
            Ok(Some(row)) if row.title.is_empty() => {
                match self.storage.update_session(session_id, &title) {
                    Ok(()) => self.sidebar.dirty = true,
                    Err(e) => tracing::warn!(error = %e, "failed to save session title"),
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "failed to read session for titling"),
        }
    }

    /// Applies `event` to the background session at `index`.
    async fn handle_background_event(&mut self, index: usize, event: Event) {
        let status = self.status.clone();
        let failed = matches!(event, Event::AgentError { .. });
        self.swap_with_background(index);
        let was_processing = self.session.is_processing;
        let was_waiting = self.session.attention() == Attention::Waiting;
        self.handle_focused_event(event).await;
        let finished = was_processing && !self.session.is_processing;
        if finished {
            self.session.unread = true;
            if let Some(id) = self.session.session_id.as_deref() {
                // Persisted so the mark survives a restart (SESSIONMENU FR-08).
                if let Err(e) = self.storage.set_session_unread(id, true) {
                    tracing::warn!(error = %e, session_id = %id, "could not mark session unread");
                }
            }
        }
        if failed {
            self.session.errored = true;
        }
        // FR-06: tell the user's notification hook when a background session
        // starts waiting on them, fails, or finishes.
        let kind = if !was_waiting && self.session.attention() == Attention::Waiting {
            Some("waiting")
        } else if failed {
            Some("failed")
        } else if finished {
            Some("finished")
        } else {
            None
        };
        if let Some(kind) = kind {
            self.notify_attention(kind);
        }
        self.swap_with_background(index);
        self.status = status;
    }

    /// Fires `on_notification` hooks for the session in `self.session`
    /// (FR-06). Called while a background session is swapped in.
    ///
    /// # Arguments
    ///
    /// * `kind` - `waiting`, `finished` or `failed`.
    fn notify_attention(&self, kind: &str) {
        let Some(session_id) = self.session.session_id.as_deref() else {
            return;
        };
        let title = self.session_title(session_id);
        let name = if title.is_empty() {
            format!("session {}", short_session_id(session_id))
        } else {
            title
        };
        let message = match kind {
            "waiting" => format!("{name} needs you"),
            "failed" => format!("{name} failed"),
            _ => format!("{name} finished"),
        };
        let directory = &self.session.directory;
        let config = self.session_processor.load_config_for(directory);
        ragent_agent::hooks::fire_notification_hooks(
            &config, directory, session_id, kind, &message,
        );
    }

    /// Runs the session-scoped housekeeping polls for every background
    /// session, so their compaction, rollback, swarm, question-timeout and
    /// autopilot results land while they are not focused.
    pub async fn poll_background_sessions(&mut self) {
        for index in 0..self.background.len() {
            let status = self.status.clone();
            self.swap_with_background(index);
            self.poll_rollback_result();
            self.poll_compaction_result();
            self.poll_pending_swarm().await;
            self.poll_question_expiry();
            self.poll_swarm_unblock();
            self.poll_swarm_completion().await;
            self.poll_autopilot_continue();
            self.swap_with_background(index);
            self.status = status;
        }
    }

    /// A fresh view that inherits the focused session's agent, model and
    /// directory.
    pub(super) fn fresh_session_view(&self) -> SessionView {
        let current = &self.session;
        let mut view = SessionView::new(
            current.agent_name.clone(),
            current.agent_info.clone(),
            current.current_agent_index,
        );
        view.selected_model = current.selected_model.clone();
        view.selected_model_ctx_window = current.selected_model_ctx_window;
        view.selected_thinking_level = current.selected_thinking_level;
        view.provider_health = std::sync::Arc::clone(&current.provider_health);
        view.active_backend = current.active_backend.clone();
        view.directory = current.directory.clone();
        view
    }

    /// Parks `previous` in the background when it is a real session.
    ///
    /// A view without a session id has never been created in storage and
    /// holds nothing worth keeping, so it is dropped.
    pub(super) fn park_session(&mut self, previous: SessionView) {
        if previous.session_id.is_some() {
            self.background.insert_front(previous);
        }
    }

    /// Resets global UI that describes the previously focused session and
    /// moves the TUI into the focused session's directory.
    pub(super) fn after_focus_change(&mut self) {
        self.session.unread = false;
        self.session.errored = false;
        if let Some(id) = self.session.session_id.as_deref() {
            if let Err(e) = self.storage.set_session_unread(id, false) {
                tracing::warn!(error = %e, session_id = %id, "could not mark session read");
            }
        }
        self.sidebar.dirty = true;
        self.apply_session_directory();
        self.text_selection = None;
        self.scrollbar_drag = None;
        self.context_menu = None;
        self.slash_menu = None;
        self.file_menu = None;
        self.status_bar_cache = None;
        self.needs_redraw = true;
        let short = self
            .session
            .session_id
            .as_deref()
            .map(short_session_id)
            .unwrap_or_default();
        self.status = format!("session {short}");
        self.persist_open_sessions();
        self.release_unused_projects();
        self.nudge_request_counts();
    }

    /// Makes the focused session's directory the TUI's working directory
    /// (M6). Project-scoped commands read `cwd_path`, so they follow the
    /// session; the caches that describe the previous project are dropped.
    fn apply_session_directory(&mut self) {
        if self.session.directory.as_os_str().is_empty() {
            self.session.directory = self.cwd_path.clone();
            return;
        }
        if self.session.directory == self.cwd_path {
            return;
        }
        let old_directory = std::mem::replace(&mut self.cwd_path, self.session.directory.clone());
        self.cwd = Self::collapse_home_path(&self.cwd_path);
        self.git_branch = Self::detect_git_branch(&self.cwd_path);
        self.memory_cache_dirty = true;
        self.skill_registry_cache = None;
        self.cached_skill_registry = None;
        self.skill_dirs_cache.clear();
        self.switch_project_services(&old_directory);
    }

    /// Records the open sessions so the next launch can reopen them (FR-13).
    ///
    /// The list is written focused session first, then background sessions
    /// most recently focused first. A storage failure is logged and otherwise
    /// ignored: losing the list only means the next launch starts fresh.
    pub fn persist_open_sessions(&self) {
        let ids: Vec<&str> = std::iter::once(&self.session)
            .chain(self.background.iter())
            .filter_map(|v| v.session_id.as_deref())
            .collect();
        let json = match serde_json::to_string(&ids) {
            Ok(json) => json,
            Err(e) => {
                tracing::warn!(error = %e, "failed to encode open session list");
                return;
            }
        };
        if let Err(e) = self.storage.set_setting(OPEN_SESSIONS_KEY, &json) {
            tracing::warn!(error = %e, "failed to save open session list");
        }
    }

    /// Reopens the sessions recorded by [`App::persist_open_sessions`]
    /// (FR-13).
    ///
    /// Sessions that were deleted or archived are skipped, and at most
    /// [`MAX_RESTORED_SESSIONS`] are reopened. The previously focused session
    /// ends up focused and the rest keep their most-recently-focused order in
    /// the background.
    ///
    /// # Returns
    ///
    /// The number of sessions reopened; `0` means the caller should start a
    /// fresh session.
    pub fn restore_open_sessions(&mut self) -> usize {
        let json = match self.storage.get_setting(OPEN_SESSIONS_KEY) {
            Ok(Some(json)) => json,
            Ok(None) => return 0,
            Err(e) => {
                tracing::warn!(error = %e, "failed to read open session list");
                return 0;
            }
        };
        let ids: Vec<String> = match serde_json::from_str(&json) {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!(error = %e, "ignoring malformed open session list");
                return 0;
            }
        };
        let reopenable: Vec<String> = ids
            .into_iter()
            .filter(|id| match self.storage.get_session(id) {
                Ok(Some(row)) => row.archived_at.is_none(),
                Ok(None) => false,
                Err(e) => {
                    tracing::warn!(error = %e, session_id = %id, "skipping unreadable session");
                    false
                }
            })
            .take(MAX_RESTORED_SESSIONS)
            .collect();
        // Focusing in reverse leaves the first id focused and parks the rest
        // in their saved order (each switch parks the previous one in front).
        let mut reopened = 0;
        for id in reopenable.iter().rev() {
            match self.switch_to_session(id) {
                Ok(()) => reopened += 1,
                Err(e) => tracing::warn!(error = %e, session_id = %id, "failed to reopen session"),
            }
        }
        reopened
    }

    /// Drops the shared step bookkeeping of one session (MULTISESSION D2).
    ///
    /// # Arguments
    ///
    /// * `session_id` - full id of the session whose entries are removed.
    pub(crate) fn forget_session_step_state(&mut self, session_id: &str) {
        let short = short_session_id(session_id);
        self.tool_step_map.retain(|_, (sid, _, _)| *sid != short);
        self.last_step_per_session.remove(session_id);
        self.substep_counter_per_session.remove(session_id);
        self.sid_to_display_name.remove(&short);
    }

    /// Prints the session startup block into the focused session's
    /// transcript: the ragent banner, version and links, the session id, and
    /// the config files the session's directory resolves to. Then announces
    /// the project instruction files (AGENTS.md and the like) for the
    /// session; that discovery walks the project tree, so it runs on a
    /// blocking thread and its notice arrives over the event bus. No model
    /// call is made - the files already reach the model through the system
    /// prompt (#38).
    ///
    /// Called for every newly created session: at startup, `/session new`,
    /// `Ctrl+N`, the sidebar's `+ New session`, and the replacement created
    /// when the last session is closed.
    pub fn announce_session_startup(&mut self) {
        let Some(session_id) = self.session.session_id.clone() else {
            return;
        };
        let banner = crate::logo::LOGO.join("\n");
        self.append_assistant_text(&banner);
        self.append_assistant_text(&format!("\n  Version {}", env!("CARGO_PKG_VERSION")));
        self.append_assistant_text("\n  Created by Tim Hawkins <tim.thawkins@gmail.com>");
        self.append_assistant_text("\n  https://github.com/thawkins/ragent");
        self.append_assistant_text("\n");
        self.append_assistant_text(&format!(
            "\n[ok] Session created: `{}`",
            super::helpers::short_id(&session_id)
        ));
        // The config this session runs under (MULTISESSION M6): its own
        // project file, or the launch configuration it inherits.
        let config_paths = match ragent_agent::Config::load_for(&self.cwd_path) {
            Ok(config) => config.config_paths,
            Err(e) => {
                tracing::warn!(error = %e, "could not load session config for the startup block");
                self.config_paths.clone()
            }
        };
        if config_paths.is_empty() {
            self.append_assistant_text("\n[i] No config file found; using defaults");
        } else {
            let home = std::env::var_os("HOME").map(|h| h.to_string_lossy().into_owned());
            let mut paths_text = config_paths
                .iter()
                .map(|p| {
                    let s = p.display().to_string();
                    match home.as_deref().and_then(|h| s.strip_prefix(h)) {
                        Some(rest) => format!("~{rest}"),
                        None => s,
                    }
                })
                .collect::<Vec<_>>()
                .join("`\n  ");
            if config_paths.len() > 1 {
                paths_text = format!("\n  {paths_text}");
            }
            self.append_assistant_text(&format!("\n[ok] Loaded config file: `{paths_text}`"));
        }
        let processor = std::sync::Arc::clone(&self.session_processor);
        drop(tokio::task::spawn_blocking(move || {
            if let Err(e) = processor.announce_instruction_files(&session_id) {
                tracing::warn!(error = %e, "Instruction-file announcement failed");
            }
        }));
    }

    /// Checks a `/session new <path>` directory (FR-01, M6).
    ///
    /// The path (with `~` expanded) must be an existing directory inside the
    /// directory ragent was launched in or inside a `dirs.allowed_roots`
    /// entry of the launch configuration, the same containment rule the HTTP
    /// server applies to session directories (SEC-062).
    ///
    /// # Arguments
    ///
    /// * `path` - the directory the user typed.
    ///
    /// # Errors
    ///
    /// Returns a message when the path does not exist, is not a directory, or
    /// is outside every permitted root.
    pub(crate) fn allowed_session_directory(&self, path: &str) -> Result<PathBuf, String> {
        let expanded = Self::expand_home_path(Path::new(path));
        let canonical = expanded
            .canonicalize()
            .map_err(|e| format!("`{path}`: {e}"))?;
        if !canonical.is_dir() {
            return Err(format!("`{path}` is not a directory"));
        }
        let canonical_root = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let mut roots: Vec<PathBuf> = std::env::current_dir()
            .map(|cwd| vec![canonical_root(&cwd)])
            .unwrap_or_default();
        let config = self.session_processor.load_config_cached();
        roots.extend(
            config
                .dirs
                .allowed_roots
                .iter()
                .map(|r| canonical_root(&Self::expand_home_path(Path::new(r)))),
        );
        if roots.iter().any(|root| canonical.starts_with(root)) {
            Ok(canonical)
        } else {
            Err(format!(
                "`{path}` is outside the launch directory and every `dirs.allowed_roots` entry"
            ))
        }
    }

    /// Starts a new session in the focused session's directory and focuses
    /// it; the previous session keeps running in the background (FR-01).
    ///
    /// # Errors
    ///
    /// Returns a message when the session cannot be created in storage; the
    /// previous session stays focused.
    pub fn open_new_session(&mut self) -> Result<(), String> {
        let directory = self.session.directory.clone();
        self.open_new_session_in(directory)
    }

    /// Starts a new session working in `directory` and focuses it; the
    /// previous session keeps running in the background (FR-01, M6).
    ///
    /// # Arguments
    ///
    /// * `directory` - the new session's working directory (already checked
    ///   by [`App::allowed_session_directory`] when it came from the user).
    ///
    /// # Errors
    ///
    /// Returns a message when the session cannot be created in storage; the
    /// previous session stays focused.
    pub fn open_new_session_in(&mut self, directory: PathBuf) -> Result<(), String> {
        let mut fresh = self.fresh_session_view();
        fresh.directory = directory;
        let previous = std::mem::replace(&mut self.session, fresh);
        if !self.ensure_session() {
            self.session = previous;
            return Err("could not create a new session".to_string());
        }
        self.park_session(previous);
        self.after_focus_change();
        self.announce_session_startup();
        Ok(())
    }

    /// Focuses the session with id `session_id` (FR-12).
    ///
    /// An open background session is swapped in as is; a stored session is
    /// loaded from storage into a new view. The previously focused session
    /// keeps running in the background.
    ///
    /// # Arguments
    ///
    /// * `session_id` - full session id.
    ///
    /// # Errors
    ///
    /// Returns a message when the session does not exist or cannot be loaded.
    pub fn switch_to_session(&mut self, session_id: &str) -> Result<(), String> {
        if self.session.session_id.as_deref() == Some(session_id) {
            return Ok(());
        }
        if let Some(index) = self.background.position(session_id) {
            let Some(view) = self.background.remove(index) else {
                return Err(format!("session not found: {session_id}"));
            };
            let previous = std::mem::replace(&mut self.session, view);
            self.park_session(previous);
            self.after_focus_change();
            return Ok(());
        }
        self.storage
            .get_session(session_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("session not found: {session_id}"))?;
        let fresh = self.fresh_session_view();
        let previous = std::mem::replace(&mut self.session, fresh);
        let previous_cwd = (self.cwd_path.clone(), self.cwd.clone());
        let loaded = self.load_session(session_id);
        // `load_session` moves `cwd_path` itself; put it back so the focus
        // change below moves it and the project's services together.
        (self.cwd_path, self.cwd) = previous_cwd;
        if let Err(e) = loaded {
            self.session = previous;
            return Err(e.to_string());
        }
        self.park_session(previous);
        self.after_focus_change();
        Ok(())
    }

    /// Closes the focused session and focuses the most recently used
    /// background session, or a new session when none is open. The closed
    /// session stays in storage.
    ///
    /// # Errors
    ///
    /// Returns a message while the focused session is running a turn, or
    /// when a replacement session cannot be created.
    pub fn close_focused_session(&mut self) -> Result<(), String> {
        if self.session.is_processing {
            return Err("this session is running; press Esc to stop it first".to_string());
        }
        let closing = self.session.session_id.clone();
        let mut created_replacement = false;
        match self.background.remove(0) {
            Some(next) => self.session = next,
            None => {
                let fresh = self.fresh_session_view();
                let previous = std::mem::replace(&mut self.session, fresh);
                if !self.ensure_session() {
                    self.session = previous;
                    return Err("could not create a replacement session".to_string());
                }
                created_replacement = true;
            }
        }
        if let Some(id) = closing {
            self.forget_session_step_state(&id);
        }
        self.after_focus_change();
        if created_replacement {
            self.announce_session_startup();
        }
        Ok(())
    }

    /// Resolves `/session switch` input to one session id.
    ///
    /// Matches, in order: an exact id; then open and stored top-level
    /// sessions whose id starts or ends with `query` or whose title starts
    /// with it (case-insensitive).
    fn resolve_session_query(&self, query: &str) -> Result<String, String> {
        let open_ids = std::iter::once(&self.session)
            .chain(self.background.iter())
            .filter_map(|v| v.session_id.clone());
        let mut candidates: Vec<(String, String)> = open_ids
            .map(|id| {
                let title = self.session_title(&id);
                (id, title)
            })
            .collect();
        if let Ok(stored) = self.storage.list_sessions() {
            for row in stored {
                if row.parent_id.is_none() && !candidates.iter().any(|(id, _)| *id == row.id) {
                    candidates.push((row.id, row.title));
                }
            }
        }
        match_session_query(query, &candidates)
    }

    /// Resolves `/session unarchive` input against the archived sessions.
    fn resolve_archived_query(&self, query: &str) -> Result<String, String> {
        let candidates: Vec<(String, String)> = self
            .storage
            .list_archived_sessions()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|row| row.parent_id.is_none())
            .map(|row| (row.id, row.title))
            .collect();
        match_session_query(query, &candidates)
    }

    /// Stored title of a session, or an empty string.
    fn session_title(&self, session_id: &str) -> String {
        self.storage
            .get_session(session_id)
            .ok()
            .flatten()
            .map(|row| row.title)
            .unwrap_or_default()
    }

    /// Markdown table of the open sessions for `/session list`.
    fn session_list_text(&self) -> String {
        let mut text = String::from("| | Session | Title | State |\n|---|---|---|---|\n");
        let rows = std::iter::once((true, &self.session))
            .chain(self.background.iter().map(|v| (false, v)));
        for (focused, view) in rows {
            let id = view.session_id.as_deref().unwrap_or("");
            let state = match view.attention() {
                Attention::Waiting => "needs you",
                Attention::Running => "running",
                Attention::Error => "failed (unread)",
                Attention::Unread => "finished (unread)",
                Attention::Idle => "idle",
            };
            text.push_str(&format!(
                "| {} | `{}` | {} | {} |\n",
                if focused { ">" } else { "" },
                short_session_id(id),
                self.session_title(id),
                state
            ));
        }
        text
    }

    /// The focused session's id, or an error when it has none yet.
    fn focused_session_id(&self) -> Result<String, String> {
        self.session
            .session_id
            .clone()
            .ok_or_else(|| "no session is focused".to_string())
    }

    /// `/session rename <title>`: sets the focused session's title (FR-07).
    fn rename_focused_session(&mut self, title: &str) -> Result<(), String> {
        let id = self.focused_session_id()?;
        self.storage
            .update_session(&id, title)
            .map_err(|e| e.to_string())?;
        self.status = format!("session renamed: {title}");
        Ok(())
    }

    /// `/session pin`: pins the focused session, or unpins it (FR-08).
    fn toggle_focused_pin(&mut self) -> Result<(), String> {
        let id = self.focused_session_id()?;
        self.toggle_session_pin(&id)
    }

    /// Pins session `id`, or unpins it when it is pinned (FR-08).
    ///
    /// # Errors
    ///
    /// Returns a message when storage cannot be read or updated.
    pub(crate) fn toggle_session_pin(&mut self, id: &str) -> Result<(), String> {
        let id = id.to_string();
        let pinned = self
            .storage
            .pinned_session_ids()
            .map_err(|e| e.to_string())?
            .contains(&id);
        self.storage
            .set_session_pinned(&id, !pinned)
            .map_err(|e| e.to_string())?;
        self.status = if pinned {
            "session unpinned"
        } else {
            "session pinned"
        }
        .to_string();
        self.sidebar.dirty = true;
        Ok(())
    }

    /// Archives session `id` from the sidebar (FR-08). The focused session
    /// is closed as `/session archive` does; an open background session is
    /// dropped from the background. A running session is refused.
    ///
    /// # Errors
    ///
    /// Returns a message when the session is running or storage fails.
    pub(crate) fn archive_session_by_id(&mut self, id: &str) -> Result<(), String> {
        if self.session.session_id.as_deref() == Some(id) {
            return self.archive_focused_session();
        }
        if let Some(index) = self.background.position(id) {
            if self
                .background
                .iter()
                .nth(index)
                .is_some_and(|v| v.is_processing)
            {
                return Err("that session is running; stop it first".to_string());
            }
            self.background.remove(index);
        }
        self.storage
            .archive_session(id)
            .map_err(|e| e.to_string())?;
        self.forget_session_step_state(id);
        self.persist_open_sessions();
        self.release_unused_projects();
        self.sidebar.dirty = true;
        self.status = format!("archived session {}", short_session_id(id));
        Ok(())
    }

    /// Archives every session in `ids` (the project and selection menus).
    /// Every session is tried; a running one, or one with a running
    /// sub-agent or teammate, is skipped.
    ///
    /// # Errors
    ///
    /// Returns a message naming how many could not be archived, and the
    /// first reason.
    pub(crate) fn archive_sessions_by_id(&mut self, ids: &[String]) -> Result<(), String> {
        self.for_each_session(ids, "archived", |app, id| {
            // A running sub-agent or teammate would keep writing into an
            // archived tree, as it would into a deleted one.
            let tree = app
                .storage
                .session_tree_ids(id)
                .map_err(|e| e.to_string())?;
            app.check_tree_idle(id, &tree)?;
            app.archive_session_by_id(id)
        })
    }

    /// Runs `action` on every session in `ids` (the project and selection
    /// menus), trying each one, and puts `<verb> N session(s)` on the status
    /// line.
    ///
    /// # Errors
    ///
    /// Returns a message naming how many failed, and the first reason.
    pub(crate) fn for_each_session(
        &mut self,
        ids: &[String],
        verb: &str,
        mut action: impl FnMut(&mut Self, &str) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut failed: Vec<String> = Vec::new();
        for id in ids {
            if let Err(e) = action(self, id) {
                failed.push(e);
            }
        }
        let done = ids.len() - failed.len();
        self.status = format!("{verb} {done} session(s)");
        match failed.first() {
            None => Ok(()),
            Some(first) => Err(format!("{verb} {done}, {} failed: {first}", failed.len())),
        }
    }

    /// Directory of session `id`: its open view's, else the stored one.
    pub(crate) fn session_directory(&self, id: &str) -> Option<PathBuf> {
        std::iter::once(&self.session)
            .chain(self.background.iter())
            .find(|v| v.session_id.as_deref() == Some(id))
            .map(|v| v.directory.clone())
            .or_else(|| {
                self.storage
                    .get_session(id)
                    .ok()
                    .flatten()
                    .map(|row| PathBuf::from(row.directory))
            })
    }

    /// Opens session `id`'s directory in an editor, the file manager or a
    /// terminal (SESSIONMENU FR-06). The program is started detached.
    ///
    /// # Errors
    ///
    /// Returns a message when the session is unknown, this machine has no
    /// such program, or it cannot be started.
    pub(crate) fn open_session_in(
        &mut self,
        id: &str,
        target: crate::open_in::OpenTarget,
    ) -> Result<(), String> {
        let dir = self
            .session_directory(id)
            .ok_or_else(|| format!("session not found: {id}"))?;
        self.open_directory_in(&dir, target)
    }

    /// Opens directory `dir` in `target` (editor, file manager, terminal).
    ///
    /// # Errors
    ///
    /// Returns a message when no such program is found or it cannot start.
    pub(crate) fn open_directory_in(
        &mut self,
        dir: &std::path::Path,
        target: crate::open_in::OpenTarget,
    ) -> Result<(), String> {
        let env = crate::open_in::LaunchEnv::current();
        let spec = crate::open_in::launch_spec(target, &dir, &env)
            .ok_or_else(|| format!("no {} found", target.label().to_lowercase()))?;
        match self.sidebar.launch_log.as_mut() {
            Some(log) => log.push(spec),
            None => crate::open_in::spawn(&spec)?,
        }
        self.status = format!(
            "opened {} in {}",
            dir.display(),
            target.label().to_lowercase()
        );
        Ok(())
    }

    /// Opens the pull/merge request session `id` is bound to in the browser
    /// (SESSIONMENU T4.4).
    ///
    /// # Errors
    ///
    /// Returns a message when the session is not bound or its stored URL is
    /// not a web URL.
    pub(crate) fn open_session_request(&mut self, id: &str) -> Result<(), String> {
        let request = self
            .session_request_of(id)
            .ok_or("this session is not bound to a pull or merge request")?;
        self.open_url(&request.web_url)
    }

    /// Opens web URL `url` in the default browser (the `PR/MRs` list).
    ///
    /// # Errors
    ///
    /// Returns a message when `url` is not an `http(s)` URL or the browser
    /// cannot start.
    pub(crate) fn open_url(&mut self, url: &str) -> Result<(), String> {
        let env = crate::open_in::LaunchEnv::current();
        let spec = crate::open_in::url_spec(url, &env)
            .ok_or_else(|| format!("`{url}` is not a web URL"))?;
        match self.sidebar.launch_log.as_mut() {
            Some(log) => log.push(spec),
            None => crate::open_in::spawn(&spec)?,
        }
        self.status = format!("opened {url}");
        Ok(())
    }

    /// Moves session `id` into the section `section_id`, or back to its
    /// project with `None` (SESSIONMENU FR-11).
    ///
    /// # Errors
    ///
    /// Returns a message when the section does not exist or storage fails.
    pub(crate) fn move_session_to_section(
        &mut self,
        id: &str,
        section_id: Option<&str>,
    ) -> Result<(), String> {
        self.storage
            .set_session_section(id, section_id)
            .map_err(|e| e.to_string())?;
        self.sidebar.dirty = true;
        self.status = match section_id {
            None => "session moved back to its project".to_string(),
            Some(_) => "session moved to section".to_string(),
        };
        Ok(())
    }

    /// The section named `name` (ignoring case).
    fn section_named(&self, name: &str) -> Result<ragent_storage::SidebarSection, String> {
        self.storage
            .list_sections()
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| format!("no section named `{}`", name.trim()))
    }

    /// The focused session's section id, or an error when it has none.
    fn focused_section_id(&self) -> Result<String, String> {
        let id = self.focused_session_id()?;
        self.storage
            .session_sections()
            .map_err(|e| e.to_string())?
            .remove(&id)
            .ok_or_else(|| "this session is not in a section".to_string())
    }

    /// `/session section ...` (SESSIONMENU FR-11).
    fn handle_section_command(&mut self, args: &str) -> Result<(), String> {
        let (sub, rest) = args
            .split_once(char::is_whitespace)
            .map_or((args, ""), |(s, r)| (s, r.trim()));
        match sub {
            "" => {
                let sections = self.storage.list_sections().map_err(|e| e.to_string())?;
                let mut text = String::from("| Section |\n|---|\n");
                for s in &sections {
                    text.push_str(&format!("| {} |\n", s.name));
                }
                if sections.is_empty() {
                    text = "No sections. `/session section new <name>` creates one.".into();
                }
                self.append_assistant_text(&super::helpers::from_cmd("session section", &text));
                self.status = format!("sections: {}", sections.len());
                Ok(())
            }
            "--none" => {
                let id = self.focused_session_id()?;
                self.move_session_to_section(&id, None)
            }
            "new" if rest.is_empty() => Err("usage: /session section new <name>".to_string()),
            "new" => {
                let id = self.focused_session_id()?;
                let section = self
                    .storage
                    .create_section(rest)
                    .map_err(|e| e.to_string())?;
                self.move_session_to_section(&id, Some(&section.id))?;
                self.status = format!("section `{}` created", section.name);
                Ok(())
            }
            "rename" if rest.is_empty() => Err("usage: /session section rename <name>".to_string()),
            "rename" => {
                let section = self.focused_section_id()?;
                self.storage
                    .rename_section(&section, rest)
                    .map_err(|e| e.to_string())?;
                self.sidebar.dirty = true;
                self.status = format!("section renamed to `{rest}`");
                Ok(())
            }
            "delete" => {
                let section = if rest.is_empty() {
                    self.focused_section_id()?
                } else {
                    self.section_named(rest)?.id
                };
                self.storage
                    .delete_section(&section)
                    .map_err(|e| e.to_string())?;
                self.sidebar.dirty = true;
                self.status = "section deleted; its sessions are back in their projects".into();
                Ok(())
            }
            _ => {
                let section = self.section_named(args)?;
                let id = self.focused_session_id()?;
                self.move_session_to_section(&id, Some(&section.id))
            }
        }
    }

    /// `query` resolved to a session id, or the focused session when empty.
    fn session_or_focused(&self, query: &str) -> Result<String, String> {
        if query.is_empty() {
            self.focused_session_id()
        } else {
            self.resolve_session_query(query)
        }
    }

    /// Whether session `id` is open and running a turn.
    fn session_is_running(&self, id: &str) -> bool {
        std::iter::once(&self.session)
            .chain(self.background.iter())
            .any(|v| v.session_id.as_deref() == Some(id) && v.is_processing)
    }

    /// Ids of the sessions that carry the unread mark: stored, or set on
    /// their open view. Unlike the sidebar's attention marker this is not
    /// masked by a waiting, running or failed state.
    ///
    /// # Errors
    ///
    /// Returns a message when storage cannot be read.
    pub(crate) fn unread_marks(&self) -> Result<std::collections::HashSet<String>, String> {
        let mut marks = self
            .storage
            .unread_session_ids()
            .map_err(|e| e.to_string())?;
        marks.extend(
            std::iter::once(&self.session)
                .chain(self.background.iter())
                .filter(|v| v.unread)
                .filter_map(|v| v.session_id.clone()),
        );
        Ok(marks)
    }

    /// Whether session `id` carries the unread mark (see
    /// [`App::unread_marks`]).
    ///
    /// # Errors
    ///
    /// Returns a message when storage cannot be read.
    pub(crate) fn session_unread(&self, id: &str) -> Result<bool, String> {
        Ok(self.unread_marks()?.contains(id))
    }

    /// Marks session `id` unread, or read when it is unread (SESSIONMENU
    /// FR-08). The mark is stored, and copied to the session's open view.
    ///
    /// # Errors
    ///
    /// Returns a message when storage cannot be read or updated.
    pub(crate) fn toggle_session_unread(&mut self, id: &str) -> Result<(), String> {
        let unread = !self.session_unread(id)?;
        self.storage
            .set_session_unread(id, unread)
            .map_err(|e| e.to_string())?;
        if self.session.session_id.as_deref() == Some(id) {
            self.session.unread = unread;
        } else if let Some(index) = self.background.position(id)
            && let Some(view) = self.background.view_mut(index)
        {
            view.unread = unread;
        }
        self.status = if unread {
            "session marked unread"
        } else {
            "session marked read"
        }
        .to_string();
        self.sidebar.dirty = true;
        Ok(())
    }

    /// Copies session `id`'s transcript into a new session in the same
    /// directory and focuses it (SESSIONMENU FR-10). A running session is
    /// refused: its last turn is not finished.
    ///
    /// # Errors
    ///
    /// Returns a message when the session is running or storage fails.
    pub(crate) fn fork_session_by_id(&mut self, id: &str) -> Result<(), String> {
        if self.session_is_running(id) {
            return Err("that session is running; stop it first".to_string());
        }
        let title = match self.session_title(id) {
            t if t.is_empty() => format!("{} (fork)", short_session_id(id)),
            t => format!("{t} (fork)"),
        };
        let fork = self
            .session_processor
            .session_manager
            .fork_session(id, &title)
            .map_err(|e| e.to_string())?;
        self.switch_to_session(&fork)?;
        self.persist_open_sessions();
        self.status = format!("forked into {}", short_session_id(&fork));
        Ok(())
    }

    /// Opens the delete confirmation for session `id` (SESSIONMENU FR-13).
    /// A running session is refused before asking.
    ///
    /// # Errors
    ///
    /// Returns a message when the session is running or does not exist.
    pub(crate) fn request_delete_session(&mut self, id: &str) -> Result<(), String> {
        let tree = self
            .storage
            .session_tree_ids(id)
            .map_err(|e| e.to_string())?;
        self.check_tree_idle(id, &tree)?;
        if tree.is_empty() {
            return Err(format!("session not found: {id}"));
        }
        let title = match self.session_title(id) {
            t if t.is_empty() => format!("untitled {}", short_session_id(id)),
            t => t,
        };
        self.open_row_action(crate::app::RowAction::DeleteSession {
            session_id: id.to_string(),
            title,
            children: tree.len() - 1,
        });
        Ok(())
    }

    /// Opens one delete confirmation for every session in `ids` (the
    /// project and selection menus); `label` names where they come from.
    /// A running session, or one with a running sub-agent, is refused
    /// before asking.
    ///
    /// # Errors
    ///
    /// Returns a message when `ids` is empty or a session is running.
    pub(crate) fn request_delete_sessions(
        &mut self,
        ids: Vec<String>,
        label: &str,
    ) -> Result<(), String> {
        if ids.is_empty() {
            return Err("no sessions to delete".to_string());
        }
        for id in &ids {
            let tree = self
                .storage
                .session_tree_ids(id)
                .map_err(|e| e.to_string())?;
            self.check_tree_idle(id, &tree)?;
            if tree.is_empty() {
                return Err(format!("session not found: {id}"));
            }
        }
        self.open_row_action(crate::app::RowAction::DeleteSessions {
            session_ids: ids,
            label: label.to_string(),
        });
        Ok(())
    }

    /// Deletes every session in `ids` and their sub-agent sessions, after
    /// the confirmation. Every session is tried.
    ///
    /// # Errors
    ///
    /// Returns a message naming how many could not be deleted, and the
    /// first reason.
    pub(crate) fn delete_sessions_by_id(&mut self, ids: &[String]) -> Result<(), String> {
        self.for_each_session(ids, "deleted", Self::delete_session_by_id)
    }

    /// Refuses deleting session `id` while it, or any session in its `tree`
    /// (sub-agent, teammate, skill sessions), is running: deleting removes
    /// the whole tree, and a running child would keep writing into it.
    fn check_tree_idle(&self, id: &str, tree: &[String]) -> Result<(), String> {
        if self.session_is_running(id) {
            return Err("that session is running; stop it first".to_string());
        }
        let running_task = |sid: &str| {
            self.active_tasks.iter().any(|task| {
                !task.status.is_terminal()
                    && (task.child_session_id == sid || task.parent_session_id == sid)
            })
        };
        let busy = running_task(id)
            || tree
                .iter()
                .skip(1)
                .any(|child| self.session_is_running(child) || running_task(child));
        if busy {
            return Err(
                "a sub-agent or teammate of that session is still running; stop it first"
                    .to_string(),
            );
        }
        Ok(())
    }

    /// Permanently deletes session `id` and its sub-agent sessions
    /// (SESSIONMENU FR-13). An open session is closed first; a running one
    /// is refused. Callers ask the user before calling this.
    ///
    /// # Errors
    ///
    /// Returns a message when the session is running, cannot be closed, or
    /// storage fails.
    pub(crate) fn delete_session_by_id(&mut self, id: &str) -> Result<(), String> {
        let tree = self
            .storage
            .session_tree_ids(id)
            .map_err(|e| e.to_string())?;
        self.check_tree_idle(id, &tree)?;
        if self.session.session_id.as_deref() == Some(id) {
            self.close_focused_session()?;
        } else if let Some(index) = self.background.position(id) {
            self.background.remove(index);
        }
        let deleted = self
            .session_processor
            .session_manager
            .delete_session(id)
            .map_err(|e| e.to_string())?;
        for gone in &deleted {
            self.forget_session_step_state(gone);
        }
        self.persist_open_sessions();
        self.release_unused_projects();
        self.sidebar.dirty = true;
        self.status = format!("deleted session {}", short_session_id(id));
        Ok(())
    }

    /// `/session archive`: archives the focused session and closes it
    /// (FR-08). A running session must be stopped first.
    fn archive_focused_session(&mut self) -> Result<(), String> {
        if self.session.is_processing {
            return Err("this session is running; press Esc to stop it first".to_string());
        }
        let id = self.focused_session_id()?;
        self.storage
            .archive_session(&id)
            .map_err(|e| e.to_string())?;
        self.close_focused_session()?;
        self.status = format!("archived session {}", short_session_id(&id));
        Ok(())
    }

    /// `/session unarchive <query>`: lists an archived session again.
    fn unarchive_session_query(&mut self, query: &str) -> Result<(), String> {
        let id = self.resolve_archived_query(query)?;
        self.storage
            .unarchive_session(&id)
            .map_err(|e| e.to_string())?;
        self.status = format!(
            "unarchived session {}; /session switch to open it",
            short_session_id(&id)
        );
        Ok(())
    }

    /// `/session list --all`: stored sessions grouped by project, pinned
    /// sessions marked (FR-02).
    fn show_grouped_sessions(&mut self) -> Result<(), String> {
        let groups = self
            .storage
            .list_sessions_grouped()
            .map_err(|e| e.to_string())?;
        let pinned = self
            .storage
            .pinned_session_ids()
            .map_err(|e| e.to_string())?;
        let mut text = String::new();
        for group in &groups {
            text.push_str(&format!(
                "### {} - `{}`\n\n| | Session | Title | Updated |\n|---|---|---|---|\n",
                group.project.name, group.project.worktree
            ));
            for row in &group.sessions {
                text.push_str(&format!(
                    "| {} | `{}` | {} | {} |\n",
                    if pinned.contains(&row.id) { "pin" } else { "" },
                    short_session_id(&row.id),
                    row.title,
                    row.updated_at
                ));
            }
            text.push('\n');
        }
        if groups.is_empty() {
            text.push_str("No stored sessions.");
        }
        self.append_assistant_text(&super::helpers::from_cmd("session list --all", &text));
        self.status = format!("sessions: {} projects", groups.len());
        Ok(())
    }

    /// `/session list --archived`: archived top-level sessions.
    fn show_archived_sessions(&mut self) -> Result<(), String> {
        let rows: Vec<_> = self
            .storage
            .list_archived_sessions()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|row| row.parent_id.is_none())
            .collect();
        let mut text = String::from("| Session | Title | Archived |\n|---|---|---|\n");
        for row in &rows {
            text.push_str(&format!(
                "| `{}` | {} | {} |\n",
                short_session_id(&row.id),
                row.title,
                row.archived_at.as_deref().unwrap_or("")
            ));
        }
        self.append_assistant_text(&super::helpers::from_cmd("session list --archived", &text));
        self.status = format!("sessions: {} archived", rows.len());
        Ok(())
    }

    /// Handles `/session <args>` (help is handled by the slash dispatcher).
    ///
    /// # Arguments
    ///
    /// * `args` - text after `/session`.
    pub(crate) fn handle_session_command(&mut self, args: &str) {
        let (sub, rest) = args
            .split_once(char::is_whitespace)
            .map_or((args, ""), |(s, r)| (s, r.trim()));
        let result = match sub {
            "" | "list" if rest.is_empty() => {
                let list = self.session_list_text();
                self.append_assistant_text(&super::helpers::from_cmd("session list", &list));
                self.status = format!("sessions: {} open", self.background.len() + 1);
                return;
            }
            "list" if rest == "--all" => self.show_grouped_sessions(),
            "list" if rest == "--archived" => self.show_archived_sessions(),
            "list" => Err(format!(
                "unknown list option `{rest}`; use --all or --archived"
            )),
            "rename" if rest.is_empty() => Err("usage: /session rename <title>".to_string()),
            "rename" => self.rename_focused_session(rest),
            "pin" => self.toggle_focused_pin(),
            "archive" => self.archive_focused_session(),
            "unarchive" if rest.is_empty() => {
                Err("usage: /session unarchive <id-or-title>".to_string())
            }
            "unarchive" => self.unarchive_session_query(rest),
            "new" => self.session_new_command(rest),
            "switch" if rest.is_empty() => Err("usage: /session switch <id-or-title>".to_string()),
            "switch" => self
                .resolve_session_query(rest)
                .and_then(|id| self.switch_to_session(&id)),
            "close" => self.close_focused_session(),
            "unread" => self
                .session_or_focused(rest)
                .and_then(|id| self.toggle_session_unread(&id)),
            "fork" => self
                .session_or_focused(rest)
                .and_then(|id| self.fork_session_by_id(&id)),
            "section" => self.handle_section_command(rest),
            "delete" if rest.is_empty() => Err("usage: /session delete <id-or-title>".to_string()),
            "delete" => self
                .resolve_session_query(rest)
                .or_else(|_| self.resolve_archived_query(rest))
                .and_then(|id| self.request_delete_session(&id)),
            other => Err(format!(
                "unknown subcommand `{other}` (/session help lists them)"
            )),
        };
        if let Err(message) = result {
            self.status = format!("session: {message}");
        }
        self.sidebar.dirty = true;
    }
}

/// Picks the one session that `query` names among `candidates`.
///
/// An exact id wins; otherwise a candidate matches when its id starts or ends
/// with `query` or its title starts with it (case-insensitive).
///
/// # Arguments
///
/// * `query` - user input: an id, id prefix or suffix, or title prefix.
/// * `candidates` - `(id, title)` pairs to search.
///
/// # Errors
///
/// Returns a message when nothing or more than one session matches.
fn match_session_query(query: &str, candidates: &[(String, String)]) -> Result<String, String> {
    if let Some((id, _)) = candidates.iter().find(|(id, _)| id == query) {
        return Ok(id.clone());
    }
    let lower = query.to_lowercase();
    let matches: Vec<&(String, String)> = candidates
        .iter()
        .filter(|(id, title)| {
            id.starts_with(query)
                || id.ends_with(query)
                || (!title.is_empty() && title.to_lowercase().starts_with(&lower))
        })
        .collect();
    match matches.as_slice() {
        [] => Err(format!("no session matches `{query}`")),
        [(id, _)] => Ok(id.clone()),
        many => Err(format!(
            "`{query}` matches {} sessions: {}",
            many.len(),
            many.iter()
                .map(|(id, _)| short_session_id(id))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Derives a session title from its first prompt (FR-07).
///
/// Takes the first non-empty line, collapses runs of whitespace, and cuts it
/// at a word boundary so the title is at most 60 characters (a cut is marked
/// with `...`).
///
/// # Arguments
///
/// * `prompt` - the session's first user message.
///
/// # Returns
///
/// The title, or `None` when the prompt has no text.
pub fn title_from_prompt(prompt: &str) -> Option<String> {
    const MAX: usize = 60;
    let line = prompt.lines().map(str::trim).find(|l| !l.is_empty())?;
    let words: Vec<&str> = line.split_whitespace().collect();
    let full = words.join(" ");
    if full.chars().count() <= MAX {
        return Some(full);
    }
    let mut title = String::new();
    for word in words {
        let extra = usize::from(!title.is_empty()) + word.chars().count();
        if title.chars().count() + extra > MAX - 3 {
            break;
        }
        if !title.is_empty() {
            title.push(' ');
        }
        title.push_str(word);
    }
    if title.is_empty() {
        // One word longer than the limit: cut inside it.
        title = line.chars().take(MAX - 3).collect();
    }
    title.push_str("...");
    Some(title)
}
