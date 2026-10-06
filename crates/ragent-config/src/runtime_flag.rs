//! Shared runtime boolean flag helper.
//!
//! Several toggle modules (`activity_log`, `edit_log`, `yolo`) all follow the
//! same shape: a process-wide atomic flag synchronised with a corresponding
//! field in `ragent.json` so the toggle survives restarts. This helper removes
//! the duplicated `AtomicBool` + `persist`/`sync`/`toggle` boilerplate.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context as _;

/// A process-wide boolean runtime flag.
///
/// Wraps an [`AtomicBool`] and provides the persist/sync/toggle helpers shared
/// by the config-backed toggles.
pub struct RuntimeFlag {
    name: &'static str,
    flag: AtomicBool,
}

impl RuntimeFlag {
    /// Create a new flag with the given initial state.
    pub const fn new(name: &'static str, initial: bool) -> Self {
        Self {
            name,
            flag: AtomicBool::new(initial),
        }
    }

    /// Returns the current state.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// Enable or disable the flag globally.
    pub fn set_enabled(&self, enabled: bool) {
        self.flag.store(enabled, Ordering::Relaxed);
    }

    /// Persist the requested state to the config file and update the runtime
    /// flag.
    ///
    /// The current config is reloaded, the named field is updated, and the
    /// result is written back to the same source file that was loaded (project
    /// config preferred over global config). Any error during persistence is
    /// returned so callers can decide how to report it.
    ///
    /// If the config file cannot be loaded (e.g. it is corrupt), the error is
    /// propagated rather than silently overwriting the file with defaults.
    pub fn persist(&self, enabled: bool) -> anyhow::Result<()> {
        let mut config = crate::config::Config::load()
            .with_context(|| format!("failed to load config before persisting {}", self.name))?;
        self.apply_to_config(&mut config, enabled);
        config.save_to_source()?;
        self.set_enabled(enabled);
        Ok(())
    }

    /// Persist the requested state to the **user-global** config file and update
    /// the runtime flag.
    ///
    /// Unlike [`RuntimeFlag::persist`], which writes back to whichever source
    /// file was loaded (project preferred over global), this always targets the
    /// global config (`~/.config/ragent/ragent.json`). It exists for
    /// privileged, user-only toggles such as YOLO mode: the project-local
    /// config is untrusted repository content whose `yolo` value is stripped by
    /// [`Config::merge_project`](crate::config::Config::merge_project)
    /// (SECTASKS T-011), so a project-file write would never round-trip on the
    /// next load.
    ///
    /// The named field is set directly in the on-disk global file rather than
    /// re-serialising the merged [`Config`](crate::config::Config): writing the
    /// merged value would fold a project overlay (and any key this build does
    /// not model) into the user's global file. See
    /// [`Config::set_global_bool_key`](crate::config::Config::set_global_bool_key).
    ///
    /// If the config file cannot be read, the error is propagated rather than
    /// silently overwriting the file with defaults.
    pub fn persist_to_global(&self, enabled: bool) -> anyhow::Result<()> {
        let key = self
            .global_config_key()
            .ok_or_else(|| anyhow::anyhow!("runtime flag '{}' has no config key", self.name))?;
        crate::config::Config::set_global_bool_key(key, enabled)
            .with_context(|| format!("failed to persist {}", self.name))?;
        self.set_enabled(enabled);
        Ok(())
    }

    /// Toggle the flag and persist the new state to the **user-global** config
    /// file (see [`RuntimeFlag::persist_to_global`]); returns the new state.
    pub fn toggle_persist_global(&self) -> anyhow::Result<bool> {
        let previous = self.is_enabled();
        let new_state = !previous;
        self.persist_to_global(new_state)?;
        tracing::info!(
            flag = self.name,
            previous,
            new_state,
            "runtime flag toggled and persisted to the user-global config"
        );
        Ok(new_state)
    }

    /// Load the current config and update the runtime flag from its value,
    /// falling back to `default` on load failure.
    pub fn sync_from_config(&self, default: bool) {
        let enabled = crate::config::Config::load()
            .map(|c| self.read_from_config(&c))
            .unwrap_or(default);
        self.set_enabled(enabled);
    }

    /// Toggle the flag, persist the new state, and return it.
    ///
    /// This is the recommended path for UI toggles because `Config::load()`
    /// does not sync the runtime flag automatically. Using plain
    /// [`RuntimeFlag::set_enabled`] followed by a later `sync_from_config`
    /// would change the flag only until the next explicit sync or persistence
    /// call.
    pub fn toggle_persist(&self) -> anyhow::Result<bool> {
        let new_state = !self.is_enabled();
        self.persist(new_state)?;
        Ok(new_state)
    }

    fn read_from_config(&self, config: &crate::config::Config) -> bool {
        match self.name {
            "activity_log" => config.activity_log,
            "edit_log" => config.edit_log,
            "gcf" => config.gcf.enabled,
            "yolo" => config.yolo,
            other => {
                debug_assert!(false, "unhandled runtime flag name: {other}");
                false
            }
        }
    }

    fn apply_to_config(&self, config: &mut crate::config::Config, enabled: bool) {
        match self.name {
            "activity_log" => config.activity_log = enabled,
            "edit_log" => config.edit_log = enabled,
            "gcf" => config.gcf.enabled = enabled,
            "yolo" => config.yolo = enabled,
            other => {
                debug_assert!(false, "unhandled runtime flag name: {other}");
            }
        }
    }

    /// The *top-level* `ragent.json` key this flag persists to, or `None` for a
    /// flag whose value lives in a nested section (e.g. `gcf.enabled`).
    ///
    /// Used by [`RuntimeFlag::persist_to_global`] to edit the raw global file in
    /// place instead of re-serialising the merged config.
    fn global_config_key(&self) -> Option<&'static str> {
        match self.name {
            "activity_log" => Some("activity_log"),
            "edit_log" => Some("edit_log"),
            "yolo" => Some("yolo"),
            // `gcf` is nested under `gcf.enabled`; it is persisted through the
            // `Config::save` path in the `/gcf` handler, not this helper.
            _ => None,
        }
    }
}
