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

    /// The field name this flag is persisted under.
    pub fn name(&self) -> &'static str {
        self.name
    }

    fn read_from_config(&self, config: &crate::config::Config) -> bool {
        match self.name {
            "activity_log" => config.activity_log,
            "edit_log" => config.edit_log,
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
            "yolo" => config.yolo = enabled,
            other => {
                debug_assert!(false, "unhandled runtime flag name: {other}");
            }
        }
    }
}
