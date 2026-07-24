//! Team store: discovery, creation, loading, and saving of team configs.
//!
//! Teams are stored in:
//! - `~/.ragent/teams/{name}/` — user-global (lower priority)
//! - `[PROJECT]/.ragent/teams/{name}/` — project-local (higher priority)
//!
//! `config.json` writes are serialised using a companion `config.json.lock`
//! file and an atomic rename so concurrent saves cannot clobber each other
//! (Milestone 1).

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use fs2::FileExt as _;

use crate::team::config::{TeamConfig, TeamMember};
use crate::team::mailbox::Mailbox;
use crate::team::task::{Task, TaskList, TaskStore};

// ── Directory discovery ───────────────────────────────────────────────────────

/// Return the user-global teams base directory: `~/.ragent/teams/`.
#[must_use]
pub fn global_teams_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent").join("teams"))
}

/// Walk up from `working_dir` to find the nearest project `.ragent/` directory,
/// returning `[PROJECT]/.ragent/teams/`.
#[must_use]
pub fn find_project_teams_dir(working_dir: &Path) -> Option<PathBuf> {
    let mut current = working_dir;
    loop {
        let candidate = current.join(".ragent");
        if candidate.is_dir() {
            return Some(candidate.join("teams"));
        }
        current = current.parent()?;
    }
}

/// Locate the on-disk directory for a named team, caching the result on the
/// supplied [`ToolContext`] (PERF-019) so repeated tool calls within the
/// same agent step skip the directory walk.
///
/// `find_team_dir` walks up the directory tree calling `candidate.is_dir()`
/// (a `stat()` syscall per level) on every parent. This is called from
/// nearly every team tool's `execute()` method, often multiple times per
/// call. PERF-019 caches the resolved `PathBuf` on the
/// [`ToolContext::cached_team_dir`] field so the walk runs at most once per
/// `process_user_message` turn.
#[must_use]
pub fn find_team_dir_cached(ctx: &crate::tool::ToolContext, name: &str) -> Option<PathBuf> {
    {
        let guard = ctx.cached_team_dir.lock().ok()?;
        if let Some((ref cached_name, ref dir)) = *guard {
            if cached_name.as_str() == name {
                return Some(dir.clone());
            }
        }
    }
    let dir = find_team_dir(&ctx.working_dir, name)?;
    if let Ok(mut guard) = ctx.cached_team_dir.lock() {
        *guard = Some((name.to_string(), dir.clone()));
    }
    Some(dir)
}

/// Locate the on-disk directory for a named team.
///
/// Searches project-local first (higher priority), then user-global.
/// Returns `None` if the team does not exist in either location.
///
/// For the hot path inside tool `execute()` methods prefer
/// [`find_team_dir_cached`] which caches the resolved path on the
/// [`ToolContext`](crate::tool::ToolContext) (PERF-019).
#[must_use]
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
            find_project_teams_dir(working_dir).ok_or_else(|| {
                anyhow!(
                    "no .ragent/ directory found in or above {}",
                    working_dir.display()
                )
            })?
        } else {
            global_teams_dir().ok_or_else(|| anyhow!("cannot determine home directory"))?
        };

        let team_dir = base.join(name);
        if team_dir.exists() {
            return Err(anyhow!(
                "team '{}' already exists at {}",
                name,
                team_dir.display()
            ));
        }

        fs::create_dir_all(&team_dir)
            .with_context(|| format!("create team directory {}", team_dir.display()))?;
        fs::create_dir_all(team_dir.join("mailbox"))
            .with_context(|| "create mailbox subdirectory")?;

        let config = TeamConfig::new(name, lead_session_id);
        let store = Self {
            dir: team_dir,
            config,
        };
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
        let team_dir =
            find_team_dir(working_dir, name).ok_or_else(|| anyhow!("team '{name}' not found"))?;
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
        let store = Self {
            dir: team_dir,
            config,
        };
        store.save()?;
        Ok(store)
    }

    /// Return the companion lock-file path for `config.json`.
    fn lock_path(config_path: &Path) -> PathBuf {
        let mut name = config_path.as_os_str().to_os_string();
        name.push(".lock");
        PathBuf::from(name)
    }

    /// Acquire an advisory `flock` on the companion lock file for `config.json`.
    fn acquire_lock(config_path: &Path, exclusive: bool) -> Result<std::fs::File> {
        let lock = Self::lock_path(config_path);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock)
            .with_context(|| format!("open lock file {}", lock.display()))?;
        if exclusive {
            file.lock_exclusive()
                .with_context(|| format!("acquire exclusive lock on {}", lock.display()))?;
        } else {
            file.lock_shared()
                .with_context(|| format!("acquire shared lock on {}", lock.display()))?;
        }
        Ok(file)
    }

    /// Load an existing team from `team_dir` (acquires a shared lock on the
    /// companion lock file).
    ///
    /// M5-T2: migrates the loaded config to the current schema version before
    /// returning it.
    pub fn load(team_dir: &Path) -> Result<Self> {
        let config_path = team_dir.join("config.json");
        let lock = Self::acquire_lock(&config_path, false)?;
        let raw = fs::read_to_string(&config_path)
            .with_context(|| format!("read {}", config_path.display()))?;
        drop(lock);
        let mut config: TeamConfig = serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", config_path.display()))?;
        config.migrate();
        Ok(Self {
            dir: team_dir.to_path_buf(),
            config,
        })
    }

    /// Load a team by name from the standard discovery directories.
    pub fn load_by_name(name: &str, working_dir: &Path) -> Result<Self> {
        let team_dir =
            find_team_dir(working_dir, name).ok_or_else(|| anyhow!("team '{name}' not found"))?;
        Self::load(&team_dir)
    }

    /// Persist the current config to `config.json` (acquires an exclusive lock
    /// on the companion lock file and uses an atomic rename).
    ///
    /// Stamps `schema_version` and `updated_at` on every write (M5-T2/T5).
    pub fn save(&self) -> Result<()> {
        let config_path = self.dir.join("config.json");
        let lock = Self::acquire_lock(&config_path, true)?;

        // M5-T2/T5: stamp schema_version and updated_at on every write.
        let mut config = self.config.clone();
        if config.schema_version == 0 {
            config.schema_version = crate::team::config::TEAM_CONFIG_SCHEMA_VERSION;
        }
        config.updated_at = Some(chrono::Utc::now());

        let json = serde_json::to_string_pretty(&config)?;
        let file_name = config_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config.json".to_string());
        let tmp_path = self
            .dir
            .join(format!(".{file_name}.{}", uuid::Uuid::new_v4()));
        fs::write(&tmp_path, json).with_context(|| format!("write {}", tmp_path.display()))?;
        let result: Result<()> = (|| {
            let temp = OpenOptions::new().read(true).open(&tmp_path)?;
            temp.sync_all()
                .with_context(|| format!("sync temp file {}", tmp_path.display()))?;
            fs::rename(&tmp_path, &config_path).with_context(|| {
                format!("rename {} -> {}", tmp_path.display(), config_path.display())
            })?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        drop(lock);
        result.with_context(|| {
            format!(
                "atomic write {} -> {}",
                tmp_path.display(),
                config_path.display()
            )
        })
    }

    // ── Discovery ─────────────────────────────────────────────────────────

    /// List all teams visible from `working_dir`.
    ///
    /// Returns `(name, dir, is_project_local)` tuples, deduplicating by name
    /// (project-local wins over global).
    #[must_use]
    pub fn list_teams(working_dir: &Path) -> Vec<(String, PathBuf, bool)> {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut results: Vec<(String, PathBuf, bool)> = Vec::new();

        // Project-local first (higher priority).
        if let Some(proj_dir) = find_project_teams_dir(working_dir)
            && let Ok(entries) = fs::read_dir(&proj_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("config.json").exists() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    seen.insert(name.clone());
                    results.push((name, path, true));
                }
            }
        }

        // Global (lower priority; skip if already seen).
        if let Some(global_dir) = global_teams_dir()
            && let Ok(entries) = fs::read_dir(&global_dir)
        {
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
    #[must_use]
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

    // ── PERF-016: async `spawn_blocking` wrappers ──────────────────────────────
    //
    // `TeamStore::load` / `save` perform synchronous `fs::read_to_string` /
    // `fs::write` + `serde_json` (de)serialisation under a `flock`. On the
    // async path these stall the tokio worker thread. The helpers below move
    // the I/O onto a blocking-pool thread so the async executor stays free
    // during concurrent teammate activity. `TeamStore` is cheap to
    // reconstruct (`load` is a single file read), so we pass the `team_dir`
    // into the closure and reopen there.

    /// PERF-016: `spawn_blocking` wrapper around [`TeamStore::load`].
    pub async fn load_blocking(team_dir: PathBuf) -> Result<Self> {
        tokio::task::spawn_blocking(move || Self::load(&team_dir)).await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TeamStore::load_by_name`].
    pub async fn load_by_name_blocking(name: String, working_dir: PathBuf) -> Result<Self> {
        tokio::task::spawn_blocking(move || Self::load_by_name(&name, &working_dir)).await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TeamStore::save`].
    ///
    /// Consumes `self` (matching `save`'s `&self` receiver via a clone) so
    /// the blocking closure can take ownership without borrowing across an
    /// await boundary.
    pub async fn save_blocking(self) -> Result<()> {
        let dir = self.dir.clone();
        let config = self.config.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self { dir, config };
            store.save()
        })
        .await?
    }
}
