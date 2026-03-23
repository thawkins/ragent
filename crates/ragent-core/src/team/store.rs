//! Team store: discovery, creation, loading, and saving of team configs.
//!
//! Teams are stored in:
//! - `~/.ragent/teams/{name}/` — user-global (lower priority)
//! - `[PROJECT]/.ragent/teams/{name}/` — project-local (higher priority)

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;

use crate::team::config::{TeamConfig, TeamMember};
use crate::team::mailbox::Mailbox;
use crate::team::task::{Task, TaskList, TaskStore};

// ── Directory discovery ───────────────────────────────────────────────────────

/// Return the user-global teams base directory: `~/.ragent/teams/`.
pub fn global_teams_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent").join("teams"))
}

/// Walk up from `working_dir` to find the nearest project `.ragent/` directory,
/// returning `[PROJECT]/.ragent/teams/`.
pub fn find_project_teams_dir(working_dir: &Path) -> Option<PathBuf> {
    let mut current = working_dir;
    loop {
        let candidate = current.join(".ragent");
        if candidate.is_dir() {
            return Some(candidate.join("teams"));
        }
        match current.parent() {
            Some(p) => current = p,
            None => return None,
        }
    }
}

/// Locate the on-disk directory for a named team.
///
/// Searches project-local first (higher priority), then user-global.
/// Returns `None` if the team does not exist in either location.
pub fn find_team_dir(working_dir: &Path, name: &str) -> Option<PathBuf> {
    // Project-local wins.
    if let Some(proj_teams) = find_project_teams_dir(working_dir) {
        let candidate = proj_teams.join(name);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    // Fall back to global.
    if let Some(global_teams) = global_teams_dir() {
        let candidate = global_teams.join(name);
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

// ── TeamStore ─────────────────────────────────────────────────────────────────

/// Manages the on-disk representation of one team.
pub struct TeamStore {
    /// Absolute path to the team directory (e.g. `~/.ragent/teams/my-team/`).
    pub dir: PathBuf,
    /// Loaded team configuration.
    pub config: TeamConfig,
}

impl TeamStore {
    // ── Lifecycle ──────────────────────────────────────────────────────────

    /// Create a brand-new team directory and write the initial `config.json`.
    ///
    /// If `project_local` is `true`, the team is created under
    /// `[PROJECT]/.ragent/teams/`; otherwise under `~/.ragent/teams/`.
    pub fn create(
        name: &str,
        lead_session_id: &str,
        working_dir: &Path,
        project_local: bool,
    ) -> Result<Self> {
        let base = if project_local {
            find_project_teams_dir(working_dir)
                .ok_or_else(|| anyhow!("no .ragent/ directory found in or above {}", working_dir.display()))?
        } else {
            global_teams_dir()
                .ok_or_else(|| anyhow!("cannot determine home directory"))?
        };

        let team_dir = base.join(name);
        if team_dir.exists() {
            return Err(anyhow!("team '{}' already exists at {}", name, team_dir.display()));
        }

        fs::create_dir_all(&team_dir)
            .with_context(|| format!("create team directory {}", team_dir.display()))?;
        fs::create_dir_all(team_dir.join("mailbox"))
            .with_context(|| "create mailbox subdirectory")?;

        let config = TeamConfig::new(name, lead_session_id);
        let store = Self { dir: team_dir, config };
        store.save()?;
        Ok(store)
    }

    /// Initialize an existing team directory that does not yet contain `config.json`.
    ///
    /// This is used to recover from partially-created team directories.
    pub fn initialize_existing_without_config(
        name: &str,
        lead_session_id: &str,
        working_dir: &Path,
    ) -> Result<Self> {
        let team_dir = find_team_dir(working_dir, name)
            .ok_or_else(|| anyhow!("team '{name}' not found"))?;
        let config_path = team_dir.join("config.json");
        if config_path.exists() {
            return Err(anyhow!(
                "team '{}' already has config at {}",
                name,
                config_path.display()
            ));
        }

        fs::create_dir_all(team_dir.join("mailbox"))
            .with_context(|| "create mailbox subdirectory")?;

        let config = TeamConfig::new(name, lead_session_id);
        let store = Self { dir: team_dir, config };
        store.save()?;
        Ok(store)
    }

    /// Load an existing team from `team_dir`.
    pub fn load(team_dir: &Path) -> Result<Self> {
        let config_path = team_dir.join("config.json");
        let raw = fs::read_to_string(&config_path)
            .with_context(|| format!("read {}", config_path.display()))?;
        let config: TeamConfig = serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", config_path.display()))?;
        Ok(Self {
            dir: team_dir.to_path_buf(),
            config,
        })
    }

    /// Load a team by name from the standard discovery directories.
    pub fn load_by_name(name: &str, working_dir: &Path) -> Result<Self> {
        let team_dir = find_team_dir(working_dir, name)
            .ok_or_else(|| anyhow!("team '{name}' not found"))?;
        Self::load(&team_dir)
    }

    /// Persist the current config to `config.json`.
    pub fn save(&self) -> Result<()> {
        let config_path = self.dir.join("config.json");
        let tmp_path = self.dir.join("config.json.tmp");
        let json = serde_json::to_string_pretty(&self.config)?;
        fs::write(&tmp_path, json)
            .with_context(|| format!("write {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &config_path)
            .with_context(|| format!("rename {} -> {}", tmp_path.display(), config_path.display()))?;
        Ok(())
    }

    // ── Discovery ─────────────────────────────────────────────────────────

    /// List all teams visible from `working_dir`.
    ///
    /// Returns `(name, dir, is_project_local)` tuples, deduplicating by name
    /// (project-local wins over global).
    pub fn list_teams(working_dir: &Path) -> Vec<(String, PathBuf, bool)> {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut results: Vec<(String, PathBuf, bool)> = Vec::new();

        // Project-local first (higher priority).
        if let Some(proj_dir) = find_project_teams_dir(working_dir) {
            if let Ok(entries) = fs::read_dir(&proj_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join("config.json").exists() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        seen.insert(name.clone());
                        results.push((name, path, true));
                    }
                }
            }
        }

        // Global (lower priority; skip if already seen).
        if let Some(global_dir) = global_teams_dir() {
            if let Ok(entries) = fs::read_dir(&global_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join("config.json").exists() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if !seen.contains(&name) {
                            results.push((name, path, false));
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| a.0.cmp(&b.0));
        results
    }

    // ── Convenience helpers ────────────────────────────────────────────────

    /// Return the `TaskStore` for this team.
    pub fn task_store(&self) -> Result<TaskStore> {
        TaskStore::open(&self.dir)
    }

    /// Return the `Mailbox` for a given agent ID.
    pub fn mailbox(&self, agent_id: &str) -> Result<Mailbox> {
        Mailbox::open(&self.dir, agent_id)
    }

    /// Add a member to the config and persist.
    pub fn add_member(&mut self, member: TeamMember) -> Result<()> {
        self.config.members.push(member);
        self.save()
    }

    /// Add a task to the task store and create an empty `TaskList` file if needed.
    pub fn add_task(&self, task: Task) -> Result<()> {
        let task_path = self.dir.join("tasks.json");
        if !task_path.exists() {
            let initial = TaskList::new(&self.config.name);
            fs::write(&task_path, serde_json::to_string_pretty(&initial)?)
                .with_context(|| format!("initialise {}", task_path.display()))?;
        }
        self.task_store()?.add_task(task)
    }

    /// Generate the next available task ID in the form `task-NNN`.
    pub fn next_task_id(&self) -> Result<String> {
        let store = self.task_store()?;
        let list = store.read()?;
        let max = list
            .tasks
            .iter()
            .filter_map(|t| {
                t.id.strip_prefix("task-")
                    .and_then(|n| n.parse::<u32>().ok())
            })
            .max()
            .unwrap_or(0);
        Ok(format!("task-{:03}", max + 1))
    }

    /// Generate the next available agent ID in the form `tm-NNN`.
    pub fn next_agent_id(&self) -> String {
        let max = self
            .config
            .members
            .iter()
            .filter_map(|m| {
                m.agent_id
                    .strip_prefix("tm-")
                    .and_then(|n| n.parse::<u32>().ok())
            })
            .max()
            .unwrap_or(0);
        format!("tm-{:03}", max + 1)
    }

    /// Stamp the config with a UTC creation time (used internally by `create`).
    pub fn timestamp(&mut self) {
        self.config.created_at = Utc::now();
    }
}
