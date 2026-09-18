//! Canonical global-state directory helpers for ragent.
//!
//! ragent consolidates ALL user-global state under `~/.config/ragent/`
//! (the XDG config directory). Legacy locations are:
//!
//! - `~/.ragent/`            — legacy home-dir root
//! - `~/.local/share/ragent/` — legacy XDG data root
//!
//! This module exposes `Config::global_state_dir()` as the single source of
//! truth for the global root, plus one helper per state subdirectory. Use
//! these instead of hand-writing `home.join(".ragent").join("...")` or
//! `dirs::data_dir().join("ragent").join("...")` so the path is consistent
//! and the legacy migration target never drifts.
//!
//! ## Subdirectories
//!
//! | Helper                        | Canonical path                              | Legacy path |
//! |-------------------------------|---------------------------------------------|-------------|
//! | [`global_memory_dir`]         | `~/.config/ragent/memory/`                  | `~/.ragent/memory/`           |
//! | [`global_agents_dir`]         | `~/.config/ragent/agents/`                  | `~/.ragent/agents/`           |
//! | [`global_skills_dir`]         | `~/.config/ragent/skills/`                  | `~/.ragent/skills/`           |
//! | [`global_teams_dir`]          | `~/.config/ragent/teams/`                   | `~/.ragent/teams/`            |
//! | [`global_blueprints_dir`]     | `~/.config/ragent/blueprints/`              | `~/.ragent/blueprints/`       |
//! | [`global_templates_dir`]      | `~/.config/ragent/templates/`               | `~/.ragent/templates/`        |
//! | [`global_agent_memory_dir`]   | `~/.config/ragent/agent-memory/`            | `~/.ragent/agent-memory/`     |
//! | [`global_models_dir`]         | `~/.config/ragent/models/`                  | `~/.local/share/ragent/models/` |
//! | [`global_loop_state_dir`]     | `~/.config/ragent/loop-state/`              | `~/.local/share/ragent/loop-state/` |
//! | [`global_inbox_dir`]          | `~/.config/ragent/log/inbox/`               | `~/.local/share/ragent/log/inbox/`  |
//! | [`global_history_path`]       | `~/.config/ragent/input_history.txt`        | `~/.local/share/ragent/input_history.txt` |
//! | [`global_db_path`]            | `~/.config/ragent/ragent.db`                | `~/.local/share/ragent/ragent.db`   |
//! | [`global_gmail_db_path`]      | `~/.config/ragent/gmail.db`                 | `~/.local/share/ragent/gmail.db`    |
//! | [`global_github_token_path`]  | `~/.config/ragent/github_token`             | `~/.ragent/github_token`            |
//! | [`global_gitlab_token_path`]  | `~/.config/ragent/gitlab_token`             | `~/.ragent/gitlab_token`            |
//! | [`global_gitlab_config_path`] | `~/.config/ragent/gitlab_config.json`       | `~/.ragent/gitlab_config.json`      |

use std::path::PathBuf;

use crate::config::Config;

/// Return the canonical global-state root: `~/.config/ragent/` (Linux),
/// `%APPDATA%/ragent` (Windows), `~/Library/Application Support/` (macOS).
///
/// Returns `None` when the platform config directory cannot be determined
/// (e.g. `XDG_CONFIG_HOME` unset on a headless Linux box).
///
/// This is the single source of truth for the consolidated state root.
/// Everything under `~/.ragent/` or `~/.local/share/ragent/` that is not
/// project-local should resolve through this.
///
/// Note: on Windows and macOS this collides with `dirs::data_dir()` (they
/// point at the same backing dir); on Linux it intentionally folds the data/
/// split into `config/` per ragent's consolidation policy.
#[must_use]
pub fn global_state_dir() -> Option<PathBuf> {
    Config::global_state_dir()
}

/// Legacy home-dir root. Kept only for the legacy-migration scan; new code
/// must not read or write here.
///
/// Returns `None` when the home directory cannot be determined.
#[doc(hidden)]
#[must_use]
pub fn legacy_home_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".ragent"))
}

/// Legacy XDG-data root. Kept only for the legacy-migration scan; new code
/// must not read or write here.
///
/// Returns `None` when the platform data directory cannot be determined.
#[doc(hidden)]
#[must_use]
pub fn legacy_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("ragent"))
}

/// Global memory directory: `~/.config/ragent/memory/`.
#[must_use]
pub fn global_memory_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("memory"))
}

/// Global custom-agents directory: `~/.config/ragent/agents/`.
///
/// Priority sits between project-local (lowest) and OpenSkills global
/// (highest), matching the existing multi-tier agent discovery chain.
#[must_use]
pub fn global_agents_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("agents"))
}

/// Global skills directory: `~/.config/ragent/skills/`.
#[must_use]
pub fn global_skills_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("skills"))
}

/// Global teams directory: `~/.config/ragent/teams/`.
#[must_use]
pub fn global_teams_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("teams"))
}

/// Global team-blueprints directory: `~/.config/ragent/blueprints/`.
#[must_use]
pub fn global_blueprints_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("blueprints"))
}

/// Global templates directory: `~/.config/ragent/templates/`.
#[must_use]
pub fn global_templates_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("templates"))
}

/// Global per-agent memory directory root: `~/.config/ragent/agent-memory/`.
#[must_use]
pub fn global_agent_memory_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("agent-memory"))
}

/// Global embedding / LLM model directory: `~/.config/ragent/models/`.
#[must_use]
pub fn global_models_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("models"))
}

/// Global loop-state directory: `~/.config/ragent/loop-state/`.
#[must_use]
pub fn global_loop_state_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("loop-state"))
}

/// Global inbox directory: `~/.config/ragent/log/inbox/`.
#[must_use]
pub fn global_inbox_dir() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("log").join("inbox"))
}

/// Global input history file: `~/.config/ragent/input_history.txt`.
#[must_use]
pub fn global_history_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("input_history.txt"))
}

/// Global structured-memory database: `~/.config/ragent/ragent.db`.
#[must_use]
pub fn global_db_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("ragent.db"))
}

/// Global Gmail OAuth token database: `~/.config/ragent/gmail.db`.
#[must_use]
pub fn global_gmail_db_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("gmail.db"))
}

/// Global GitHub personal access token file: `~/.config/ragent/github_token`.
#[must_use]
pub fn global_github_token_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("github_token"))
}

/// Global GitLab personal access token file: `~/.config/ragent/gitlab_token`.
#[must_use]
pub fn global_gitlab_token_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("gitlab_token"))
}

/// Global GitLab legacy config JSON: `~/.config/ragent/gitlab_config.json`.
#[must_use]
pub fn global_gitlab_config_path() -> Option<PathBuf> {
    global_state_dir().map(|d| d.join("gitlab_config.json"))
}

/// Global embedding model directory for `all-MiniLM-L6-v2`.
#[must_use]
pub fn global_embedding_model_dir() -> Option<PathBuf> {
    global_models_dir().map(|d| d.join("all-MiniLM-L6-v2"))
}

/// Return the AGENTS.md global instruction file directory.
///
/// This is the directory that `collect_agents_md_content_with_discovery`
/// resolves as "the global directory" for `AGENTS.md` / `CLAUDE.md` /
/// `INSTRUCTIONS.md` files when the project root has no instruction file.
/// The resolved path is used for discovery only — actual loading is
/// per-file inside the walker.
#[must_use]
pub fn global_instructions_dir() -> Option<PathBuf> {
    global_state_dir()
}
