//! Projects: a local worktree with a base model and an optional remote
//! (spec `projects`).
//!
//! Every session belongs to a project. A project owns the local checkout
//! (`worktree`), the provider/model new sessions start with, and, when the
//! checkout came from GitHub or GitLab, a description of that remote. A
//! session may additionally own a git worktree of the project's repository
//! at a chosen branch (see [`worktree`]), and the sidebar shows how many
//! pull/merge requests exist for that branch (see [`requests`]).
//!
//! Depends on `ragent-storage` for persistence and `ragent-tools-vcs` for
//! git and the GitHub/GitLab API clients.

pub mod remote;
pub mod report;
pub mod requests;
pub mod worktree;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use ragent_storage::{ProjectRow, Storage};
use serde::{Deserialize, Serialize};

pub use remote::{
    ProjectRemote, RemoteKind, RequestNoun, detect_remote, detect_remote_with_hosts,
    github_host_entry, parse_remote_url, parse_remote_url_with_hosts, split_instance_url,
};

use crate::agent::ModelRef;
use crate::event::{Event, EventBus};

/// A project record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Unique project identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Canonical root of the local checkout.
    pub worktree: PathBuf,
    /// Provider/model new sessions in this project start with, if set.
    pub base_model: Option<ModelRef>,
    /// The remote the worktree was checked out from, if recognised.
    pub remote: Option<ProjectRemote>,
    /// RFC 3339 creation time.
    pub created_at: String,
    /// RFC 3339 last-update time.
    pub updated_at: String,
    /// RFC 3339 time the project was removed from the list, if it was.
    pub archived_at: Option<String>,
}

/// Longest project description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 80;

impl From<ProjectRow> for Project {
    fn from(row: ProjectRow) -> Self {
        let base_model = match (row.provider_id, row.model_id) {
            (Some(provider_id), Some(model_id)) => Some(ModelRef {
                provider_id,
                model_id,
            }),
            _ => None,
        };
        Self {
            id: row.id,
            name: row.name,
            worktree: PathBuf::from(row.worktree),
            base_model,
            remote: row.remote.as_ref().and_then(ProjectRemote::from_row),
            created_at: row.created_at,
            updated_at: row.updated_at,
            archived_at: row.archived_at,
        }
    }
}

impl Project {
    /// The base model as the `provider/model` text the model selector uses.
    #[must_use]
    pub fn base_model_label(&self) -> Option<String> {
        self.base_model
            .as_ref()
            .map(|m| format!("{}/{}", m.provider_id, m.model_id))
    }
}

/// Parses `provider/model` into a [`ModelRef`]. The model part may itself
/// contain `/` (e.g. `openrouter/meta/llama`).
///
/// # Errors
///
/// Returns an error when either part is empty or there is no `/`.
pub fn parse_model_ref(text: &str) -> Result<ModelRef> {
    let (provider_id, model_id) = text
        .trim()
        .split_once('/')
        .filter(|(p, m)| !p.is_empty() && !m.is_empty())
        .ok_or_else(|| anyhow::anyhow!("expected <provider>/<model>, got '{}'", text.trim()))?;
    Ok(ModelRef {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
    })
}

/// Creates, resolves and edits project records, publishing
/// [`Event::ProjectUpdated`] on every change.
#[derive(Clone)]
pub struct ProjectManager {
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
    github_hosts: Vec<ragent_config::GitHubHost>,
}

impl std::fmt::Debug for ProjectManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectManager").finish_non_exhaustive()
    }
}

impl ProjectManager {
    /// Creates a manager over the given storage and event bus.
    #[must_use]
    pub const fn new(storage: Arc<Storage>, event_bus: Arc<EventBus>) -> Self {
        Self {
            storage,
            event_bus,
            github_hosts: Vec::new(),
        }
    }

    /// Recognises remotes on these GitHub Enterprise Server hosts
    /// (`vcs.github.hosts`) as GitHub.
    #[must_use]
    pub fn with_github_hosts(mut self, hosts: Vec<ragent_config::GitHubHost>) -> Self {
        self.github_hosts = hosts;
        self
    }

    fn changed(&self, project_id: &str) {
        self.event_bus.publish(Event::ProjectUpdated {
            project_id: project_id.to_string(),
        });
    }

    /// The configured self-hosted GitLab host, if any.
    fn gitlab_host(&self) -> Option<String> {
        ragent_tools_vcs::gitlab::auth::load_config(self.storage.as_ref()).map(|c| c.instance_url)
    }

    /// Returns the project a session working in `dir` belongs to, creating
    /// the record on first use and detecting its remote once.
    ///
    /// The project is the one rooted at the git work tree containing `dir`
    /// (or `dir` itself outside a work tree). When `dir` is a session-owned
    /// worktree, it is that session's project.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub fn resolve_for_dir(&self, dir: &Path) -> Result<Project> {
        let row = self
            .storage
            .project_for_directory(&dir.display().to_string())?;
        if row.remote_checked {
            return Ok(row.into());
        }
        self.refresh_remote(&row.id)
    }

    /// Re-reads the project's remote from its checkout and stores the result
    /// (or the absence of a recognised remote).
    ///
    /// # Errors
    ///
    /// Returns an error if the project does not exist or storage fails.
    pub fn refresh_remote(&self, project_id: &str) -> Result<Project> {
        let row = self
            .storage
            .get_project(project_id)?
            .ok_or_else(|| anyhow::anyhow!("project not found: {project_id}"))?;
        let detected = detect_remote_with_hosts(
            Path::new(&row.worktree),
            self.gitlab_host().as_deref(),
            &self.github_hosts,
        );
        if detected.is_none() {
            tracing::debug!(project = %row.name, "no GitHub or GitLab remote detected");
        }
        let remote_row = detected.as_ref().map(ProjectRemote::to_row);
        let unchanged = row.remote_checked && row.remote == remote_row;
        self.storage
            .set_project_remote(project_id, remote_row.as_ref())?;
        if !unchanged {
            self.changed(project_id);
        }
        self.get(project_id)?
            .ok_or_else(|| anyhow::anyhow!("project not found: {project_id}"))
    }

    /// Detects the remote of every listed project whose remote has never
    /// been checked and whose checkout still exists. Projects recorded
    /// without a session starting in them (the projects migration, `/project
    /// add` of a folder) otherwise show no forge until one is opened.
    ///
    /// # Returns
    ///
    /// The number of projects checked.
    ///
    /// # Errors
    ///
    /// Returns an error if the project list cannot be read; a failure on one
    /// project is logged and the rest are still checked.
    pub fn check_unchecked_remotes(&self) -> Result<usize> {
        let unchecked: Vec<String> = self
            .storage
            .list_projects()?
            .into_iter()
            .filter(|row| !row.remote_checked && Path::new(&row.worktree).exists())
            .map(|row| row.id)
            .collect();
        for id in &unchecked {
            if let Err(e) = self.refresh_remote(id) {
                tracing::debug!(project = %id, error = %e, "remote check failed");
            }
        }
        Ok(unchecked.len())
    }

    /// Fetches a project by id.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub fn get(&self, project_id: &str) -> Result<Option<Project>> {
        Ok(self.storage.get_project(project_id)?.map(Into::into))
    }

    /// Finds a listed project by id, name, or unique id prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails or `key` is ambiguous.
    pub fn find(&self, key: &str) -> Result<Option<Project>> {
        Ok(self.storage.find_project(key)?.map(Into::into))
    }

    /// Lists the projects, most recently updated first.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub fn list(&self) -> Result<Vec<Project>> {
        Ok(self
            .storage
            .list_projects()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Renames a project.
    ///
    /// # Errors
    ///
    /// Returns an error if `name` is blank or storage fails.
    pub fn rename(&self, project_id: &str, name: &str) -> Result<()> {
        self.storage.rename_project(project_id, name)?;
        self.changed(project_id);
        Ok(())
    }

    /// Sets or clears the base model new sessions in the project start with.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub fn set_base_model(&self, project_id: &str, model: Option<&ModelRef>) -> Result<()> {
        self.storage.set_project_base_model(
            project_id,
            model.map(|m| (m.provider_id.as_str(), m.model_id.as_str())),
        )?;
        self.changed(project_id);
        Ok(())
    }

    /// Sets or clears the description shown after the project's name. The
    /// text is trimmed; blank clears it.
    ///
    /// # Errors
    ///
    /// Returns an error if the description is longer than
    /// [`MAX_DESCRIPTION_CHARS`] characters or storage fails.
    pub fn set_description(&self, project_id: &str, description: Option<&str>) -> Result<()> {
        let description = description.map(str::trim).filter(|d| !d.is_empty());
        if let Some(text) = description {
            anyhow::ensure!(
                text.chars().count() <= MAX_DESCRIPTION_CHARS,
                "a project description is at most {MAX_DESCRIPTION_CHARS} characters"
            );
        }
        self.storage
            .set_project_description(project_id, description)?;
        self.changed(project_id);
        Ok(())
    }

    /// Sets or clears (`None`: the global one) the execution backend that
    /// sessions created in the project from now on use. Existing sessions
    /// keep the backend they were created with.
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    pub fn set_backend(
        &self,
        project_id: &str,
        kind: Option<ragent_config::ExecutionBackendKind>,
    ) -> Result<()> {
        self.storage.set_project_backend(
            project_id,
            kind.map(ragent_config::ExecutionBackendKind::as_str),
        )?;
        self.changed(project_id);
        Ok(())
    }

    /// Creates an interactive session in `project_id`.
    ///
    /// With no `branch` the session works in the project worktree. With a
    /// branch it gets its own git worktree at that branch, created under
    /// `worktree_root` (see [`worktree::create_session_worktree`]).
    ///
    /// # Arguments
    ///
    /// * `sessions` - the session manager that records the session.
    /// * `project_id` - the owning project.
    /// * `branch` - the branch for a session-owned worktree, if wanted.
    /// * `worktree_root` - directory session worktrees are created under.
    ///
    /// # Errors
    ///
    /// Returns an error if the project does not exist, the worktree cannot
    /// be created, or storage fails.
    pub fn create_session(
        &self,
        sessions: &crate::session::SessionManager,
        project_id: &str,
        branch: Option<&str>,
        worktree_root: &Path,
    ) -> Result<crate::session::Session> {
        let project = self
            .get(project_id)?
            .ok_or_else(|| anyhow::anyhow!("project not found: {project_id}"))?;
        let Some(branch) = branch else {
            return sessions.create_session_in_project(
                project_id,
                project.worktree.clone(),
                &ragent_storage::SessionCheckout::default(),
            );
        };
        let path = worktree::create_session_worktree(&project, branch, worktree_root)?;
        let checkout = ragent_storage::SessionCheckout {
            branch: Some(branch.to_string()),
            worktree: Some(path.display().to_string()),
            pr_count: None,
        };
        // The worktree exists only for this session: when the session cannot
        // be recorded, remove it again rather than leave an orphan on disk.
        sessions
            .create_session_in_project(project_id, path.clone(), &checkout)
            .map_err(|e| {
                if let Err(cleanup) = worktree::remove_session_worktree(&project, &path) {
                    tracing::warn!(
                        "could not remove the worktree {} of a session that was not \
                         recorded: {cleanup:#}",
                        path.display()
                    );
                }
                e.context(format!(
                    "the session for branch '{branch}' could not be recorded"
                ))
            })
    }

    /// Removes a project from the list; its files and archived sessions are
    /// kept.
    ///
    /// # Errors
    ///
    /// Returns an error if the project still has live sessions or storage
    /// fails.
    pub fn remove(&self, project_id: &str) -> Result<()> {
        self.storage.archive_project(project_id)?;
        self.changed(project_id);
        Ok(())
    }
}
