//! Per-project services (spec MULTISESSION, milestone M8).
//!
//! Each open project (the git work tree of a session's directory) has its own
//! code index and file watcher, specs, custom agents and plugin commands. The
//! focused project's services live in the existing `App` fields
//! (`code_index`, `code_index_watch_session`, `spec_manager`,
//! `custom_agent_defs`, ...), so every command keeps reading them as before;
//! the other open projects' code indexes are parked in
//! `App::project_services`. Each such project also gets its own MCP client
//! (phase B): the user-global and plugin servers, started in the project
//! root, with its own tool registry, so its sessions see its MCP tools and
//! not the launch project's. Switching to a session in another project parks
//! the old project's index, restores or opens the new one, and reloads the
//! project-scoped lists. A project's services are released when no open
//! session uses it; the launch project's are kept for the whole run.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::state::{App, LogLevel};

/// A parked project's code index and its file watcher.
#[derive(Default)]
pub struct ProjectServices {
    /// The project's code index, when indexing is enabled there.
    pub code_index: Option<Arc<ragent_codeindex::CodeIndex>>,
    /// The watcher keeping the index current; dropping it stops the watcher.
    pub watch_session: Option<ragent_codeindex::WatchSession>,
    /// Whether indexing is enabled in the project's config.
    pub code_index_enabled: bool,
}

impl std::fmt::Debug for ProjectServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectServices")
            .field("code_index", &self.code_index.is_some())
            .field("watching", &self.watch_session.is_some())
            .field("code_index_enabled", &self.code_index_enabled)
            .finish()
    }
}

impl ProjectServices {
    /// Stops the project's reindex and watcher.
    fn stop(self) {
        if let Some(index) = &self.code_index {
            index.request_stop();
        }
        drop(self.watch_session);
    }
}

/// The project root of a directory (see `ragent_storage::project_root`).
pub(super) fn root_of(directory: &Path) -> PathBuf {
    PathBuf::from(ragent_storage::project_root(
        &directory.display().to_string(),
    ))
}

/// The launch project's root (the project of the process cwd).
pub(super) fn launch_root() -> PathBuf {
    root_of(&std::env::current_dir().unwrap_or_default())
}

impl App {
    /// Moves the project-scoped services from the project of `old_directory`
    /// to the project of the current `cwd_path` (spec MULTISESSION M8).
    ///
    /// # Arguments
    ///
    /// * `old_directory` - the working directory before the focus change.
    pub(crate) fn switch_project_services(&mut self, old_directory: &Path) {
        let old_root = root_of(old_directory);
        let new_root = root_of(&self.cwd_path);
        if old_root == new_root {
            return;
        }
        let parked = ProjectServices {
            code_index: self.code_index.take(),
            watch_session: self.code_index_watch_session.take(),
            code_index_enabled: self.code_index_enabled,
        };
        self.project_services.insert(old_root, parked);
        let services = match self.project_services.remove(&new_root) {
            Some(services) => services,
            None => self.open_project_services(&new_root),
        };
        self.code_index = services.code_index;
        self.code_index_watch_session = services.watch_session;
        self.code_index_enabled = services.code_index_enabled;
        self.code_index_stats_cache = None;
        self.code_index_busy = false;
        self.spec_manager = Some(Arc::new(ragent_specs::SpecManager::new(
            self.cwd_path.join("specs"),
        )));
        self.plugin_commands = crate::app::state::plugin_commands_for_surface(&self.cwd_path);
        for diagnostic in self.reload_custom_agents() {
            self.push_log_no_agent(LogLevel::Warn, format!("[agents] {diagnostic}"));
        }
    }

    /// Opens a project's code index when its config enables indexing and
    /// ragent has been used there before (it has a `.ragent` directory), so
    /// focusing a session never creates index files in an unrelated project.
    fn open_project_services(&mut self, root: &Path) -> ProjectServices {
        let config = self.session_processor.load_config_for(root);
        self.open_project_mcp(root, &config);
        let enabled = config.code_index.enabled;
        if !enabled || !root.join(".ragent").is_dir() {
            return ProjectServices {
                code_index_enabled: enabled,
                ..ProjectServices::default()
            };
        }
        let index_config = ragent_codeindex::types::CodeIndexConfig {
            enabled: true,
            project_root: root.to_path_buf(),
            index_dir: root.join(".ragent/codeindex"),
            scan_config: ragent_codeindex::types::ScanConfig::default(),
        };
        let index = match ragent_codeindex::CodeIndex::open(&index_config) {
            Ok(index) => Arc::new(index),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    root = %root.display(),
                    "failed to open project code index"
                );
                return ProjectServices {
                    code_index_enabled: enabled,
                    ..ProjectServices::default()
                };
            }
        };
        // The watcher runs the initial reindex on its own thread.
        let watch_session = match ragent_codeindex::start_watching(
            Arc::clone(&index),
            ragent_codeindex::worker::WorkerConfig::default(),
        ) {
            Ok(session) => Some(session),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    root = %root.display(),
                    "project code index watcher failed"
                );
                None
            }
        };
        self.session_processor
            .register_project_code_index(root.to_path_buf(), Arc::clone(&index));
        ProjectServices {
            code_index: Some(index),
            watch_session,
            code_index_enabled: enabled,
        }
    }

    /// Starts a project's own MCP servers in the background
    /// (spec MULTISESSION M8): the user-global servers plus those of the
    /// project's enabled plugins, with stdio servers started in the project
    /// root. A project's own `.ragent/ragent.json` cannot add servers - the
    /// project-local config guard drops them (SEC-001) - so a repository
    /// still cannot start a process just by being opened.
    fn open_project_mcp(&self, root: &Path, config: &ragent_config::Config) {
        let servers = ragent_agent::plugin::plugin_mcp_servers(root, &config.mcp);
        if servers.is_empty() {
            return;
        }
        let processor = Arc::clone(&self.session_processor);
        let root = root.to_path_buf();
        tokio::spawn(async move {
            let total = servers.len();
            let connected = processor.open_project_mcp(root.clone(), servers).await;
            tracing::info!(
                project = %root.display(),
                connected,
                total,
                "project MCP servers started"
            );
        });
    }

    /// Releases the parked services of every project that no open session
    /// uses any more (spec MULTISESSION M8). The launch project's services
    /// are kept.
    pub(crate) fn release_unused_projects(&mut self) {
        let in_use: std::collections::HashSet<PathBuf> = std::iter::once(&self.session)
            .chain(self.background.iter())
            .map(|v| root_of(&v.directory))
            .chain(std::iter::once(root_of(&self.cwd_path)))
            .collect();
        let launch = launch_root();
        let unused: Vec<PathBuf> = self
            .project_services
            .keys()
            .filter(|root| **root != launch && !in_use.contains(*root))
            .cloned()
            .collect();
        for root in unused {
            if let Some(services) = self.project_services.remove(&root) {
                services.stop();
            }
            self.session_processor.release_project_code_index(&root);
            let processor = Arc::clone(&self.session_processor);
            tokio::spawn(async move {
                processor.release_project_mcp(&root).await;
            });
        }
    }

    /// Stops every parked project's code index (TUI shutdown).
    pub fn stop_project_services(&mut self) {
        for (_, services) in self.project_services.drain() {
            services.stop();
        }
    }

    /// Reloads custom agent definitions for `cwd_path` and rebuilds the
    /// cycleable agent list, keeping the active agent selected when it still
    /// exists.
    ///
    /// # Returns
    ///
    /// Load diagnostics (parse errors, name collisions) for the caller to
    /// report.
    pub(crate) fn reload_custom_agents(&mut self) -> Vec<String> {
        let builtin_agents = ragent_agent::agent::create_builtin_agents();
        let builtin_names: std::collections::HashSet<String> =
            builtin_agents.iter().map(|a| a.name.clone()).collect();
        let (new_defs, mut diags) = ragent_agent::agent::custom::load_custom_agents(&self.cwd_path);
        // Rebuild the cycleable list: built-ins (non-hidden) + custom.
        let mut new_cycleable: Vec<_> = builtin_agents.into_iter().filter(|a| !a.hidden).collect();
        for def in &new_defs {
            let mut info = def.agent_info.as_ref().clone();
            if builtin_names.contains(&info.name) {
                let new_name = format!("custom:{}", info.name);
                diags.push(format!(
                    "custom agent '{}' collides with a built-in - loaded as '{}'",
                    info.name, new_name
                ));
                info.name = new_name;
            }
            if !info.hidden {
                new_cycleable.push(info);
            }
        }
        self.custom_agent_defs = new_defs;
        self.custom_agent_diagnostics = diags.clone();
        let current_name = self.session.agent_name.clone();
        self.session.current_agent_index = new_cycleable
            .iter()
            .position(|a| a.name == current_name)
            .unwrap_or(0);
        self.cycleable_agents = new_cycleable;
        diags
    }
}
