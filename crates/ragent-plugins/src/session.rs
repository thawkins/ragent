//! Session-start integration: discover, load enabled plugins, register their
//! tools and commands, record telemetry, and deregister on shutdown (spec
//! `plugins` T-016; FR-008, FR-022).
//!
//! [`PluginSession`] is the single entry point a session bootstrap calls once
//! the tool registry and command surface exist:
//!
//! 1. [`PluginSession::start`] discovers plugins (manifest parse only — FR-023),
//!    loads every **enabled** plugin through [`PluginManager::load_all_enabled`]
//!    (FR-008: descriptor validation, host-API version check, sandbox checkout,
//!    host-API install, entry execution), then registers each loaded plugin's
//!    tools and commands through caller-supplied closures. Registration is
//!    collision-checked and all-or-nothing per plugin (FR-024); a collision
//!    marks the plugin `errored` with cause `name-collision` and contributes
//!    nothing.
//! 2. [`PluginSession::shutdown`] unloads every loaded plugin cleanly (dropping
//!    its sandbox context) and returns the contributed names so the session can
//!    deregister them from the tool registry and the command surface.
//!
//! ## Why the surface is a trait
//!
//! This crate must not depend on `ragent-agent` (the tool registry) or
//! `ragent-tui` (the command surface). The session therefore passes closures
//! that mutate its own registries. Collision sets are queried live from the
//! surface through [`PluginSurface::existing_tool_names`] /
//! [`PluginSurface::existing_command_names`] so a second plugin cannot take a
//! name a first plugin already registered.
//!
//! ## Telemetry
//!
//! Load outcomes (`loads_ok` / `load_failures`) and the `errored` state persist
//! through the per-store ledger inside [`PluginManager`] (FR-022). The session
//! exposes [`PluginSession::reports`] so the caller can surface the load summary
//! and [`PluginSession::manager`] for `/plugins list` verbose telemetry.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::command_adapter::PluginCommandAdapter;
use crate::error::PluginError;
use crate::lifecycle::{LoadReport, PluginManager, UnloadedPlugin};
use crate::store::{LifecycleState, StoreDirs};
use crate::tool_adapter::PluginToolAdapter;

/// The live session surface a [`PluginSession`] registers into and deregisters
/// from. Implemented by the session layer (or a test double); the plugin crate
/// stays free of agent/TUI dependencies.
pub trait PluginSurface {
    /// Registry names already taken (built-in tools plus any tool already
    /// registered this session) — the tool collision surface (FR-024).
    fn existing_tool_names(&self) -> BTreeSet<String>;
    /// Triggers already taken (built-in `SLASH_COMMANDS` plus any command
    /// already registered this session) — the command collision surface
    /// (FR-024).
    fn existing_command_names(&self) -> BTreeSet<String>;
    /// Register one plugin-contributed tool.
    fn register_tool(&mut self, adapter: Arc<PluginToolAdapter>);
    /// Register one plugin-contributed slash command.
    fn register_command(&mut self, adapter: Arc<PluginCommandAdapter>);
    /// Deregister the tool under `registry_name`. Missing names are ignored.
    fn deregister_tool(&mut self, registry_name: &str);
    /// Deregister the slash command under `trigger`. Missing triggers are
    /// ignored.
    fn deregister_command(&mut self, trigger: &str);
}

/// Outcome of one plugin's registration attempt during session start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationOutcome {
    /// Plugin id.
    pub plugin_id: String,
    /// Registry tool names registered for this plugin.
    pub tools: Vec<String>,
    /// Command triggers registered for this plugin.
    pub commands: Vec<String>,
    /// `Some(cause)` when registration was refused (for example a name
    /// collision); the plugin is then `errored` and contributed nothing.
    pub error: Option<String>,
}

/// Outcome of a live `/plugins enable <id>` call (T-013; FR-011): the load
/// report plus the registration outcome for the newly loaded plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnableOutcome {
    /// Load report from the immediate load attempt.
    pub report: LoadReport,
    /// Registration outcome for the plugin's tools and commands.
    pub registration: RegistrationOutcome,
}

/// Outcome of a live `/plugins disable <id>` call (T-013; FR-012): the plugin
/// id plus how many tools and commands were deregistered from the surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisableOutcome {
    /// Plugin id.
    pub id: String,
    /// Tools deregistered from the session surface.
    pub tools_deregistered: usize,
    /// Commands deregistered from the session surface.
    pub commands_deregistered: usize,
}

/// A live plugin session: manager plus the names this session registered, so
/// shutdown can deregister exactly what was added.
#[derive(Debug)]
pub struct PluginSession {
    manager: PluginManager,
    /// Per-plugin load reports from session start (FR-008), in discovery order.
    reports: Vec<LoadReport>,
    /// Per-plugin registration outcomes (registrations and refusals).
    registrations: Vec<RegistrationOutcome>,
    /// Tool names registered this session (`plugin_<id>_<tool>`).
    registered_tools: BTreeSet<String>,
    /// Command triggers registered this session.
    registered_commands: BTreeSet<String>,
}

impl PluginSession {
    /// Start a plugin session: discover, load every enabled plugin, then
    /// register each loaded plugin's tools and commands into `surface`
    /// (FR-008, FR-022). Discovery and loading happen on the calling thread,
    /// which must remain the plugin-manager owner (rquickjs contexts are
    /// `!Send`).
    ///
    /// A plugin whose manifest fails to parse or whose entry fails is recorded
    /// `errored` by the manager and contributes nothing; a plugin whose
    /// contributed names collide with the surface is refused (all-or-nothing)
    /// and marked `errored` with cause `name-collision`.
    #[must_use]
    pub fn start(
        dirs: StoreDirs,
        config: ragent_config::PluginsConfig,
        surface: &mut impl PluginSurface,
    ) -> Self {
        // Master switch (SPEC configuration schema): with `plugins.enabled`
        // false the subsystem is inert — no discovery, no loading, nothing
        // registered (acceptance criterion 8).
        let enabled = config.is_enabled();
        let mut manager = PluginManager::new(dirs, config);
        let reports = if enabled {
            manager.load_all_enabled()
        } else {
            Vec::new()
        };

        let mut session = Self {
            manager,
            reports,
            registrations: Vec::new(),
            registered_tools: BTreeSet::new(),
            registered_commands: BTreeSet::new(),
        };
        if enabled {
            session.register_loaded(surface);
        }
        session
    }

    /// Register every loaded plugin's tools and commands into `surface`,
    /// recording one [`RegistrationOutcome`] per plugin. Tools register first
    /// (they are the agent-facing contribution), then commands; each stage is
    /// independently collision-checked and all-or-nothing for the plugin.
    fn register_loaded(&mut self, surface: &mut impl PluginSurface) {
        let loaded_ids: Vec<String> = self
            .manager
            .tracked_plugins()
            .filter(|p| p.state == LifecycleState::Loaded)
            .map(|p| p.descriptor.id.clone())
            .collect();

        for plugin_id in loaded_ids {
            let outcome = self.register_one(&plugin_id, surface);
            self.registrations.push(outcome);
        }
    }

    /// Register one loaded plugin's tools and commands. On a collision the
    /// manager records `errored` with cause `name-collision` and nothing is
    /// contributed (FR-024).
    fn register_one(
        &mut self,
        plugin_id: &str,
        surface: &mut impl PluginSurface,
    ) -> RegistrationOutcome {
        let mut tool_adapters = Vec::new();
        let existing_tools = surface.existing_tool_names();
        let tools =
            match self
                .manager
                .register_plugin_tools(plugin_id, &existing_tools, |adapter| {
                    tool_adapters.push(adapter);
                }) {
                Ok(names) => names,
                Err(error) => {
                    let cause = format!("name-collision: {error}");
                    self.manager
                        .record_registration_error(plugin_id, cause.clone());
                    return RegistrationOutcome {
                        plugin_id: plugin_id.to_string(),
                        tools: Vec::new(),
                        commands: Vec::new(),
                        error: Some(cause),
                    };
                }
            };

        let mut command_adapters = Vec::new();
        let existing_commands = surface.existing_command_names();
        let commands =
            match self
                .manager
                .register_plugin_commands(plugin_id, &existing_commands, |adapter| {
                    command_adapters.push(adapter);
                }) {
                Ok(names) => names,
                Err(error) => {
                    // The tools resolved above are not applied to the surface:
                    // the plugin is errored and must contribute nothing
                    // (all-or-nothing across both stages).
                    let cause = format!("name-collision: {error}");
                    self.manager
                        .record_registration_error(plugin_id, cause.clone());
                    return RegistrationOutcome {
                        plugin_id: plugin_id.to_string(),
                        tools: Vec::new(),
                        commands: Vec::new(),
                        error: Some(cause),
                    };
                }
            };

        for name in &tools {
            self.registered_tools.insert(name.clone());
        }
        for trigger in &commands {
            self.registered_commands.insert(trigger.clone());
        }
        for adapter in tool_adapters {
            surface.register_tool(adapter);
        }
        for adapter in command_adapters {
            surface.register_command(adapter);
        }

        RegistrationOutcome {
            plugin_id: plugin_id.to_string(),
            tools,
            commands,
            error: None,
        }
    }

    /// Unload every loaded plugin cleanly at session end (FR-008): drop the
    /// sandbox contexts and deregister the contributed tools and commands from
    /// `surface`. Returns the plugins that were unloaded with their contributed
    /// names.
    pub fn shutdown(&mut self, surface: &mut impl PluginSurface) -> Vec<UnloadedPlugin> {
        let unloaded = self.manager.unload_all();
        for plugin in &unloaded {
            for name in &plugin.tools {
                if self.registered_tools.remove(name) {
                    surface.deregister_tool(name);
                }
            }
            for trigger in &plugin.commands {
                if self.registered_commands.remove(trigger) {
                    surface.deregister_command(trigger);
                }
            }
        }
        unloaded
    }

    /// Enable a plugin in the live session (T-013; FR-011): mark it enabled
    /// through the lifecycle manager, immediately load it, and register its
    /// contributed tools and commands into `surface`.
    ///
    /// Enabling a plugin that is **already loaded** in this session is
    /// idempotent: the existing load report and registration outcome are
    /// re-reported and nothing is loaded or registered a second time (a plugin
    /// installed enabled, then explicitly enabled, must not collide with its own
    /// contributions).
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::UnknownPlugin`] when no store contains the id, or
    /// the ledger write failure.
    pub fn enable(
        &mut self,
        plugin_id: &str,
        surface: &mut impl PluginSurface,
    ) -> Result<EnableOutcome, PluginError> {
        // Idempotent re-enable: a plugin already loaded this session is reported
        // as-is without a second load or duplicate registration.
        if let Some(loaded) = self.manager.get(plugin_id)
            && loaded.state == LifecycleState::Loaded
        {
            let report = LoadReport {
                id: loaded.descriptor.id.clone(),
                state: LifecycleState::Loaded,
                error: None,
                tools: loaded.registered_tool_names(),
                commands: loaded
                    .commands
                    .iter()
                    .map(|c| c.decl.name.clone())
                    .collect(),
                declared_permissions: loaded.descriptor.requested_permissions.clone(),
            };
            let registration = self
                .registrations
                .iter()
                .find(|r| r.plugin_id == plugin_id)
                .cloned()
                .unwrap_or_else(|| RegistrationOutcome {
                    plugin_id: plugin_id.to_string(),
                    tools: report.tools.clone(),
                    commands: report.commands.clone(),
                    error: None,
                });
            return Ok(EnableOutcome {
                report,
                registration,
            });
        }

        let report = self.manager.enable(plugin_id)?;
        let registration = if report.state == LifecycleState::Loaded {
            self.register_one(plugin_id, surface)
        } else {
            RegistrationOutcome {
                plugin_id: plugin_id.to_string(),
                tools: Vec::new(),
                commands: Vec::new(),
                error: report.error.clone(),
            }
        };
        Ok(EnableOutcome {
            report,
            registration,
        })
    }

    /// Disable a plugin in the live session (T-013; FR-012): mark it disabled,
    /// unload its sandbox, and deregister exactly the tools and commands this
    /// session registered for it. Reports how many were deregistered.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::UnknownPlugin`] when no store contains the id, or
    /// the ledger write failure.
    pub fn disable(
        &mut self,
        plugin_id: &str,
        surface: &mut impl PluginSurface,
    ) -> Result<DisableOutcome, PluginError> {
        // Capture the contributed names before the manager drops the record.
        let prefix = format!("plugin_{plugin_id}_");
        let own_tools: Vec<String> = self
            .registered_tools
            .iter()
            .filter(|n| n.starts_with(&prefix))
            .cloned()
            .collect();
        let own_commands = self.own_command_names(plugin_id);

        let report = self.manager.disable(plugin_id)?;

        for name in own_tools {
            if self.registered_tools.remove(&name) {
                surface.deregister_tool(&name);
            }
        }
        for trigger in own_commands {
            if self.registered_commands.remove(&trigger) {
                surface.deregister_command(&trigger);
            }
        }
        self.registrations.retain(|r| r.plugin_id != plugin_id);
        Ok(DisableOutcome {
            id: report.id,
            tools_deregistered: report.tools_deregistered,
            commands_deregistered: report.commands_deregistered,
        })
    }

    /// The command triggers this session registered for `plugin_id`.
    fn own_command_names(&self, plugin_id: &str) -> Vec<String> {
        self.manager
            .get(plugin_id)
            .map(|p| p.commands.iter().map(|c| c.decl.name.clone()).collect())
            .unwrap_or_default()
    }

    /// The prompt-command definitions for a loaded plugin: `(trigger, prompt
    /// body)` pairs for every command whose behaviour is a prompt template
    /// (FR-031). Empty for an inline-only or absent plugin.
    #[must_use]
    pub fn prompt_commands(&self, plugin_id: &str) -> Vec<(String, String)> {
        self.manager
            .get(plugin_id)
            .map(|p| {
                p.commands
                    .iter()
                    .filter_map(|c| {
                        c.prompt()
                            .map(|body| (c.decl.name.clone(), body.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Per-plugin load reports from session start (FR-008).
    #[must_use]
    pub fn reports(&self) -> &[LoadReport] {
        &self.reports
    }

    /// Per-plugin registration outcomes from session start.
    #[must_use]
    pub fn registrations(&self) -> &[RegistrationOutcome] {
        &self.registrations
    }

    /// The live manager, for tool/command dispatch and `/plugins list`
    /// telemetry (FR-022).
    #[must_use]
    pub const fn manager(&self) -> &PluginManager {
        &self.manager
    }

    /// The live manager, for tool/command dispatch (mutable).
    pub const fn manager_mut(&mut self) -> &mut PluginManager {
        &mut self.manager
    }
}
