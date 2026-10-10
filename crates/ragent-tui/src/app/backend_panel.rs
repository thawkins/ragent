//! `/backend` TUI surface: the active execution backend and the switcher
//! (spec `openhands` T-005; FR-008).
//!
//! FR-008 requires the TUI to surface the active execution backend and the
//! health of every registered backend, and to offer a command to switch the
//! active one. The registry itself is owned by `ragent-agent`
//! ([`ragent_agent::backend::BackendRegistry`]); this module is its read model
//! for the TUI: it builds a fixed list of rows (id, kind, health, connection)
//! when the panel opens, renders them, and persists a switch by writing the
//! `execution_backend` key into the loaded config file.
//!
//! The panel never executes a tool and never re-probes health: a `local` row is
//! always `ok`, a `docker`/`podman` row is `ok` when its runtime resolves on
//! `PATH` (FR-026), and a `remote` row is `ok` when a URL and key are configured
//! (FR-030). Selecting an `unavailable` row is refused so the switcher cannot
//! leave the session on a backend that cannot run a tool.

use std::path::{Path, PathBuf};

use ragent_agent::backend::BackendRegistry;
use ragent_config::ExecutionBackendKind;

use crate::app::state::{App, LogLevel};

/// One row of the `/backend` panel - a registered backend and its health.
#[derive(Debug, Clone)]
pub struct BackendPanelRow {
    /// The stable registry id.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The backend kind label (`local`, `docker`, `podman`, `remote`).
    pub kind: &'static str,
    /// Whether this row is the active backend.
    pub active: bool,
    /// The health label (`ok`, `unavailable`, `unknown`).
    pub health: &'static str,
    /// Whether the health is `ok`, for the selectable/dim decision without a
    /// string comparison.
    pub health_ok: bool,
    /// A human-readable detail for a non-`ok` health, if any.
    pub detail: Option<String>,
    /// The secret-free connection summary (FR-035).
    pub connection: String,
}

impl BackendPanelRow {
    /// Whether this row can be selected: its health is `ok`.
    #[must_use]
    pub fn is_selectable(&self) -> bool {
        self.health_ok
    }
}

/// The `/backend` switcher panel state, present while it is open (FR-008).
///
/// The rows are a snapshot taken when the panel opened, so a frame render and a
/// keypress see a fixed list without re-reading the config or probing `PATH`.
#[derive(Debug, Clone)]
pub struct BackendPanelState {
    /// Every registered backend, in registry order (`local` first).
    pub rows: Vec<BackendPanelRow>,
    /// Block-cursor position within [`Self::rows`].
    pub cursor: usize,
}

impl BackendPanelState {
    /// Build the panel from the configured backends, marking the row whose kind
    /// matches `active_backend` as active and parking the cursor on it.
    #[must_use]
    pub fn build(config: &ragent_config::Config, cwd: &Path, active_backend: &str) -> Self {
        let registry = BackendRegistry::from_config(config, cwd);
        let active_kind = ExecutionBackendKind::from_label(active_backend);
        let rows: Vec<BackendPanelRow> = registry
            .entries()
            .iter()
            .map(|entry| BackendPanelRow {
                id: entry.id().to_string(),
                name: entry.name().to_string(),
                kind: entry.kind().as_str(),
                active: active_kind == Some(entry.kind()),
                health: entry.health().as_str(),
                health_ok: entry.health().is_ok(),
                detail: entry.detail().map(str::to_string),
                connection: entry.connection().summary(),
            })
            .collect();
        let cursor = rows.iter().position(|row| row.active).unwrap_or(0);
        Self { rows, cursor }
    }

    /// The number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there are no rows (never in practice: `local` is always present).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The highlighted row, or `None` when the panel has no rows.
    #[must_use]
    pub fn selected(&self) -> Option<&BackendPanelRow> {
        self.rows.get(self.cursor)
    }

    /// Move the highlight up one row (a no-op at the first row).
    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the highlight down one row (a no-op at the last row).
    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.rows.len() {
            self.cursor += 1;
        }
    }
}

impl App {
    /// Open the `/backend` switcher panel, snapshotting the registry (FR-008).
    pub fn open_backend_panel(&mut self) {
        let config = ragent_agent::Config::load().unwrap_or_default();
        self.backend_panel = Some(BackendPanelState::build(
            &config,
            &self.cwd_path,
            &self.active_backend,
        ));
        self.needs_redraw = true;
    }

    /// Dismiss the `/backend` panel (`Esc`).
    pub fn close_backend_panel(&mut self) {
        self.backend_panel = None;
        self.needs_redraw = true;
    }

    /// Move the panel highlight up (`Up`).
    pub fn backend_panel_move_up(&mut self) {
        if let Some(panel) = self.backend_panel.as_mut() {
            panel.move_up();
            self.needs_redraw = true;
        }
    }

    /// Move the panel highlight down (`Down`).
    pub fn backend_panel_move_down(&mut self) {
        if let Some(panel) = self.backend_panel.as_mut() {
            panel.move_down();
            self.needs_redraw = true;
        }
    }

    /// Switch the active backend to the highlighted row (`Enter`), leaving the
    /// panel open so the new active row is visible.
    pub fn backend_panel_activate_selected(&mut self) {
        let Some(row) = self
            .backend_panel
            .as_ref()
            .and_then(BackendPanelState::selected)
        else {
            return;
        };
        let id = row.id.clone();
        let kind = row.kind;
        let health = row.health;
        let detail = row.detail.clone();
        self.activate_backend_row(&id, kind, health, detail.as_deref());
        // Re-snapshot the panel so the active marker follows a successful switch
        // (a refused switch leaves the previous active row correct).
        let active = self.active_backend.clone();
        let config = self.session_processor.load_config_cached();
        self.backend_panel = Some(BackendPanelState::build(&config, &self.cwd_path, &active));
    }

    /// Ensure the active backend for this session matches `kind`, without an
    /// interactive panel. `/backend <kind|id>` and the startup sync both use it.
    ///
    /// A selector that names a backend kind with no registered descriptor
    /// (e.g. `remote` on a config that only declares `local`) is still a real
    /// switch: it persists the bare label so the next turn resolves that kind.
    /// An unregistered id is refused with the registry listing.
    ///
    /// Returns `true` when a switch was attempted (persisted or already active).
    pub fn switch_backend_by_selector(&mut self, selector: &str) -> bool {
        let config = self.session_processor.load_config_cached();
        let panel = BackendPanelState::build(&config, &self.cwd_path, &self.active_backend);
        let needle = selector.trim().to_ascii_lowercase();
        if let Some(row) = panel
            .rows
            .iter()
            .find(|row| row.id.to_ascii_lowercase() == needle || row.kind == needle)
        {
            let id = row.id.clone();
            let kind = row.kind;
            let health = row.health;
            let detail = row.detail.clone();
            self.activate_backend_row(&id, kind, health, detail.as_deref());
            return true;
        }
        // No registered entry matched. A bare kind label is still a valid
        // selector: probe a config that selects it so the registry synthesises
        // the row, then apply the same health gate as a panel selection. This
        // keeps `/backend remote` (no URL configured) a refusal rather than a
        // switch to a backend that would fail the next turn (FR-031).
        if let Some(kind) = ragent_config::ExecutionBackendKind::from_label(&needle) {
            let mut probe = (*config).clone();
            probe.execution_backend = Some(ragent_config::ExecutionBackend::Kind(
                kind.as_str().to_string(),
            ));
            let probe_panel = BackendPanelState::build(&probe, &self.cwd_path, kind.as_str());
            if let Some(row) = probe_panel
                .rows
                .iter()
                .find(|row| row.kind == kind.as_str())
            {
                let id = row.id.clone();
                let kind = row.kind;
                let health = row.health;
                let detail = row.detail.clone();
                self.activate_backend_row(&id, kind, health, detail.as_deref());
                return true;
            }
        }
        self.append_assistant_text(&format!(
            "From: /backend\n[warn] No registered backend matches `{selector}`. \
             Use `/backend` to list them."
        ));
        self.status = "backend: unknown".to_string();
        false
    }

    /// Apply a backend selection: refuse an `unavailable` row, no-op when
    /// already active, otherwise persist the switch to the config file.
    fn activate_backend_row(&mut self, id: &str, kind: &str, health: &str, detail: Option<&str>) {
        if health != "ok" {
            let reason = detail.unwrap_or("not reachable");
            self.append_assistant_text(&format!(
                "From: /backend\n[warn] Backend `{id}` is unavailable ({reason}); \
                 the active backend is unchanged."
            ));
            self.status = "backend: unavailable".to_string();
            return;
        }
        if kind == self.active_backend {
            self.append_assistant_text(&format!(
                "From: /backend\n[i]  `{id}` is already the active execution backend."
            ));
            self.status = "backend: active".to_string();
            return;
        }
        match self.persist_active_backend(kind) {
            Ok(()) => {
                self.append_assistant_text(&format!(
                    "From: /backend\n[ok] Active execution backend is now **{kind}** \
                     (`{id}`). The change applies to the next turn."
                ));
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("backend: switched active backend to {kind} ({id})"),
                );
                self.status = format!("backend: {kind}");
            }
            Err(e) => {
                self.append_assistant_text(&format!(
                    "From: /backend\n[warn] Failed to persist the backend switch: {e}"
                ));
                self.push_log_no_agent(LogLevel::Error, format!("backend: save failed: {e}"));
                self.status = "backend: error".to_string();
            }
        }
    }

    /// Persist `execution_backend: "<kind>"` into the loaded config file and set
    /// the in-memory active backend.
    fn persist_active_backend(&mut self, kind: &str) -> Result<(), String> {
        let path = self
            .backend_config_path()
            .ok_or_else(|| "no config destination found".to_string())?;
        crate::app::state::atomic_config_update(&path, |json| {
            let obj = json
                .as_object_mut()
                .ok_or_else(|| "config root is not an object".to_string())?;
            obj.insert(
                "execution_backend".to_string(),
                serde_json::Value::String(kind.to_string()),
            );
            Ok(())
        })?;
        self.active_backend = kind.to_string();
        // The next turn resolves the backend from the freshly-saved file.
        self.session_processor.invalidate_config_cache();
        Ok(())
    }

    /// The config file a backend switch is written to: the project-local
    /// `.ragent/ragent.json` when one was loaded, else the global config.
    fn backend_config_path(&self) -> Option<PathBuf> {
        let is_project = |path: &PathBuf| {
            path.file_name().is_some_and(|f| f == "ragent.json")
                && path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .is_some_and(|f| f == ".ragent")
        };
        if self.config_paths.iter().any(is_project) {
            return Some(
                std::env::current_dir()
                    .unwrap_or_default()
                    .join(".ragent/ragent.json"),
            );
        }
        self.config_paths
            .first()
            .cloned()
            .or_else(ragent_config::Config::global_config_path)
    }
}
