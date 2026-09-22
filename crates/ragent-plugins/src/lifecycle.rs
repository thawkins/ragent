//! Plugin lifecycle management: enable, disable, load, unload, errored
//! state, and the consecutive-failure auto-unload threshold (spec `plugins`
//! T-009; FR-008, FR-011, FR-012, FR-016, FR-019).
//!
//! [`PluginManager`] is the session-facing owner of every plugin's lifecycle.
//! It composes the earlier layers:
//!
//! - discovery comes from [`crate::store::scan_dirs`] (manifest parse only,
//!   no JavaScript — FR-023);
//! - enable/disable state and telemetry counters persist through the per-store
//!   [`crate::store::StoreLedger`];
//! - loading checks the declared host-API version (FR-019), checks out a fresh
//!   [`crate::runtime::SandboxContext`] under the entry budget, installs the
//!   [`crate::host_api::HostApiInstall`] surface (gated per FR-020), and
//!   evaluates the entry point (FR-008);
//! - tool-invocation outcomes feed consecutive-failure telemetry; reaching the
//!   auto-unload threshold (default [`DEFAULT_AUTO_UNLOAD_THRESHOLD`]) unloads
//!   the plugin and marks it `errored` (SPEC error-handling policy).
//!
//! Lifecycle states follow [`crate::store::LifecycleState`]: `disabled`,
//! `enabled`, `loaded`, `errored`. A disabled plugin is fully inert: its code
//! never executes and it contributes no tools or commands, but it still scans
//! and lists (FR-016). Disabling takes effect immediately in the running
//! session (assumption 4 in the SPEC): the sandbox context is dropped and the
//! contributed tool/command names are reported so the session layer can
//! deregister them (FR-012).
//!
//! The manager is single-threaded by design: `SandboxContext` (rquickjs) is
//! not `Send`, so the session layer drives the manager from its own thread.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::descriptor::PluginDescriptor;
use crate::error::PluginError;
use crate::host_api::{HostApiInstall, HostCalls, PermissionGate};
use crate::manifest::{
    HOST_API_VERSION, ParsedManifest, PluginCommandDef, PluginToolDecl, check_api_version,
};
use crate::runtime::{RuntimePool, SandboxBudget, SandboxContext};
use crate::store::{LifecycleState, ScannedPlugin, StoreDirs, StoreLedger, scan_dirs};

/// Default consecutive tool-invocation failures before a plugin is
/// auto-unloaded and marked `errored` (SPEC error-handling policy). The value
/// becomes configurable when the `plugins` config block grows a field for it;
/// the manager accepts an override via [`PluginManager::with_auto_unload_threshold`].
pub const DEFAULT_AUTO_UNLOAD_THRESHOLD: u64 = 3;

/// A plugin currently tracked by the running session: descriptor, state, the
/// captured host-API sink, and the live sandbox (when loaded — kept so T-010
/// tool dispatch can invoke the plugin's handlers).
#[derive(Debug)]
pub struct LoadedPlugin {
    /// Normalised manifest descriptor.
    pub descriptor: PluginDescriptor,
    /// Current lifecycle state (`Loaded` or `Errored` once tracked).
    pub state: LifecycleState,
    /// Failure detail when `state == Errored` (cause class plus message).
    pub error: Option<String>,
    /// Tools the plugin contributes: manifest-declared plus any recorded via
    /// `ragent.register_tool` during entry execution (FR-005).
    pub tools: Vec<PluginToolDecl>,
    /// Slash commands the plugin contributes (manifest + `register_command`,
    /// each tagged with its inline or prompt-file source).
    pub commands: Vec<PluginCommandDef>,
    /// Sink of everything the plugin emitted through the host API.
    pub calls: HostCalls,
    /// Store directory this plugin was discovered in (ledger location).
    pub store: PathBuf,
    /// Live sandbox context; `Some` only while `state == Loaded` and the
    /// plugin contributed JavaScript (a non-JS plugin loads inertly with no
    /// context).
    context: Option<SandboxContext>,
}

impl LoadedPlugin {
    /// Full tool-name list as registered in the session tool registry:
    /// `plugin_<pluginid>_<toolname>` (FR-005).
    #[must_use]
    pub fn registered_tool_names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|t| format!("plugin_{}_{}", self.descriptor.id, t.name))
            .collect()
    }

    /// Live sandbox for tool dispatch (T-010); `None` when not loaded.
    #[must_use]
    pub const fn context(&self) -> Option<&SandboxContext> {
        self.context.as_ref()
    }
}

/// Report returned by load/enable attempts (FR-008, FR-011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadReport {
    /// Plugin id.
    pub id: String,
    /// Resulting lifecycle state.
    pub state: LifecycleState,
    /// Failure detail when `state == Errored` (cause class plus message).
    pub error: Option<String>,
    /// Registered tool names (`plugin_<id>_<tool>`).
    pub tools: Vec<String>,
    /// Contributed command names.
    pub commands: Vec<String>,
    /// Permissions the manifest declared; surfaced so the enable path states
    /// them to the user before the plugin runs (SPEC security posture).
    pub declared_permissions: Vec<String>,
}

/// Report returned by [`PluginManager::disable`] (FR-012).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisableReport {
    /// Plugin id.
    pub id: String,
    /// Tools deregistered from the session.
    pub tools_deregistered: usize,
    /// Commands deregistered from the session.
    pub commands_deregistered: usize,
}

/// One plugin unloaded at session end (T-016; FR-008). Carries the contributed
/// names so the session can deregister them from the tool registry and the
/// command surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnloadedPlugin {
    /// Plugin id.
    pub plugin_id: String,
    /// Contributed registry tool names (`plugin_<id>_<tool>`).
    pub tools: Vec<String>,
    /// Contributed command triggers.
    pub commands: Vec<String>,
}

/// Session-facing lifecycle owner for all discovered plugins.
#[derive(Debug)]
pub struct PluginManager {
    dirs: StoreDirs,
    config: ragent_config::PluginsConfig,
    pool: RuntimePool,
    auto_unload_threshold: u64,
    /// Plugins with a session-visible state: loaded, errored, or (enabled but
    /// loading failed this session). Disabled plugins are absent: inertness is
    /// represented by having no runtime record at all (FR-016).
    tracked: BTreeMap<String, LoadedPlugin>,
}

impl PluginManager {
    /// Create a manager over `dirs` with the given `plugins` configuration.
    #[must_use]
    pub fn new(dirs: StoreDirs, config: ragent_config::PluginsConfig) -> Self {
        Self {
            dirs,
            config,
            pool: RuntimePool::new(),
            auto_unload_threshold: DEFAULT_AUTO_UNLOAD_THRESHOLD,
            tracked: BTreeMap::new(),
        }
    }

    /// Override the consecutive-failure auto-unload threshold (tests).
    #[must_use]
    pub const fn with_auto_unload_threshold(mut self, threshold: u64) -> Self {
        self.auto_unload_threshold = threshold;
        self
    }

    /// Discover plugins in the configured stores (manifest parse only, no
    /// execution — FR-023).
    #[must_use]
    pub fn discover(&self) -> Vec<ScannedPlugin> {
        scan_dirs(self.dirs.clone())
    }

    /// The `plugins` configuration this manager was built with. Used by the
    /// command surfaces to honour the master switch (`plugins.enabled`) before
    /// any discovery or loading (SPEC configuration schema; FR-016).
    #[must_use]
    pub const fn config(&self) -> &ragent_config::PluginsConfig {
        &self.config
    }

    /// Read-only access to one tracked plugin's runtime record.
    #[must_use]
    pub fn get(&self, plugin_id: &str) -> Option<&LoadedPlugin> {
        self.tracked.get(plugin_id)
    }

    /// Iterator over every tracked plugin (loaded or errored) in id order
    /// (T-010 tool registration iterates this for collision checks).
    pub fn tracked_plugins(&self) -> impl Iterator<Item = &LoadedPlugin> {
        self.tracked.values()
    }

    /// The manager's view of one plugin's lifecycle state: runtime state wins
    /// over the ledger (a plugin loaded or errored this session is `loaded` /
    /// `errored`; an enabled-but-not-yet-loaded plugin is `enabled`; anything
    /// else is `disabled`).
    #[must_use]
    pub fn state_of(&self, plugin_id: &str) -> LifecycleState {
        if let Some(tracked) = self.tracked.get(plugin_id) {
            return tracked.state;
        }
        if self.enabled_in_ledger(plugin_id) {
            LifecycleState::Enabled
        } else {
            LifecycleState::Disabled
        }
    }

    /// Enable a plugin and immediately load it into the current session
    /// (FR-011). The ledger is updated first so a plugin that fails to load
    /// stays enabled for the next session while this session marks it
    /// `errored`. A plugin already loaded in `self.tracked` is returned as-is
    /// (the session layer's `enable` short-circuits before this call).
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::UnknownPlugin`] when no store contains `id`, or
    /// the ledger write failure.
    pub fn enable(&mut self, plugin_id: &str) -> Result<LoadReport, PluginError> {
        let scanned = self.discover();
        let plugin = scanned
            .into_iter()
            .find(|p| scan_id(p) == plugin_id)
            .ok_or_else(|| PluginError::UnknownPlugin(plugin_id.to_string()))?;

        set_enabled(&plugin.store, plugin_id, true)?;
        let report = match plugin.outcome {
            Ok(parsed) => self.load(parsed, &plugin.store),
            Err(failure) => self.mark_errored(
                plugin_id,
                &plugin.store,
                format!("manifest-parse: {}", failure.error),
            ),
        };
        tracing::debug!(
            plugin = plugin_id,
            state = %report.state,
            "plugin enabled"
        );
        Ok(report)
    }

    /// Disable a plugin: mark it disabled in the ledger, unload its sandbox,
    /// and report how many tools and commands must be deregistered (FR-012).
    /// Disabling an already-disabled or never-loaded plugin is a no-op with
    /// zero counts.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::UnknownPlugin`] when no store contains `id`, or
    /// the ledger write failure.
    pub fn disable(&mut self, plugin_id: &str) -> Result<DisableReport, PluginError> {
        let scanned = self.discover();
        let plugin = scanned
            .into_iter()
            .find(|p| scan_id(p) == plugin_id)
            .ok_or_else(|| PluginError::UnknownPlugin(plugin_id.to_string()))?;

        set_enabled(&plugin.store, plugin_id, false)?;

        let (tools, commands) = match self.tracked.remove(plugin_id) {
            Some(loaded) => (loaded.tools.len(), loaded.commands.len()),
            None => (0, 0),
        };
        tracing::debug!(
            plugin = plugin_id,
            tools_deregistered = tools,
            commands_deregistered = commands,
            "plugin disabled and unloaded"
        );
        Ok(DisableReport {
            id: plugin_id.to_string(),
            tools_deregistered: tools,
            commands_deregistered: commands,
        })
    }

    /// Load every enabled plugin discovered in the stores (FR-008 session
    /// start). Disabled plugins stay inert; plugins whose manifests failed to
    /// parse at discovery are marked `errored` only when enabled.
    pub fn load_all_enabled(&mut self) -> Vec<LoadReport> {
        let scanned = self.discover();
        let mut reports = Vec::new();
        for plugin in scanned {
            if !plugin.enabled {
                continue;
            }
            let store = plugin.store.clone();
            let id = scan_id(&plugin);
            let report = match plugin.outcome {
                Ok(parsed) => self.load(parsed, &store),
                Err(failure) => {
                    self.mark_errored(&id, &store, format!("manifest-parse: {}", failure.error))
                }
            };
            reports.push(report);
        }
        reports
    }

    /// Unload every tracked plugin at session end (T-016; FR-008): drop the
    /// live sandbox context and clear the runtime records so no plugin code
    /// remains reachable, returning the contributed tool and command names per
    /// plugin so the session can deregister them from the tool registry and the
    /// command surface.
    ///
    /// The ledger is left untouched: unloading does not change the enable flag
    /// (a plugin disabled earlier stays disabled; an enabled plugin loads again
    /// next session). Errored plugins carry no live context and contribute no
    /// names, so they are skipped.
    pub fn unload_all(&mut self) -> Vec<UnloadedPlugin> {
        let mut unloaded = Vec::new();
        for (plugin_id, loaded) in std::mem::take(&mut self.tracked) {
            if loaded.state != LifecycleState::Loaded {
                continue;
            }
            unloaded.push(UnloadedPlugin {
                plugin_id,
                tools: loaded.registered_tool_names(),
                commands: loaded
                    .commands
                    .iter()
                    .map(|c| c.decl.name.clone())
                    .collect(),
            });
        }
        tracing::debug!(
            count = unloaded.len(),
            "plugin session shutdown unloaded plugins"
        );
        unloaded
    }

    /// Record that a plugin could not have all its contributed tools/commands
    /// registered (typically a name collision, FR-024): its state becomes
    /// `errored` with `cause` and its context is dropped, since a partially
    /// registered plugin cannot be dispatched. A no-op when the plugin id is
    /// not tracked.
    pub fn record_registration_error(&mut self, plugin_id: &str, cause: String) {
        let Some(tracked) = self.tracked.get_mut(plugin_id) else {
            return;
        };
        tracked.context = None;
        tracked.state = LifecycleState::Errored;
        tracked.error = Some(cause);
    }

    /// Record one tool-invocation outcome for a loaded plugin: success resets
    /// the consecutive-failure counter; failure increments it and, past the
    /// auto-unload threshold, unloads the plugin and marks it `errored`
    /// (SPEC error-handling policy). Returns `true` when this call triggered
    /// the auto-unload.
    pub fn record_tool_outcome(&mut self, plugin_id: &str, ok: bool) -> bool {
        let Some(tracked) = self.tracked.get_mut(plugin_id) else {
            return false;
        };
        let store = tracked.store.clone();
        let mut ledger = StoreLedger::load(&store);
        let state = ledger.state_mut(plugin_id);
        state.counters.tool_invocations += 1;
        if ok {
            state.counters.consecutive_failures = 0;
        } else {
            state.counters.tool_failures += 1;
            state.counters.consecutive_failures += 1;
        }
        let trips = !ok && state.counters.consecutive_failures >= self.auto_unload_threshold;
        if let Err(e) = ledger.save(&store) {
            tracing::warn!(
                plugin = plugin_id,
                error = %e,
                "plugin telemetry counters could not be persisted"
            );
        }
        if !trips {
            return false;
        }
        let reason = format!(
            "auto-unload: {} consecutive tool failures",
            self.auto_unload_threshold
        );
        tracing::debug!(plugin = plugin_id, "plugin auto-unloaded");
        if let Some(tracked) = self.tracked.get_mut(plugin_id) {
            tracked.context = None;
            tracked.state = LifecycleState::Errored;
            tracked.error = Some(reason);
        }
        true
    }

    /// Load one parsed plugin into the session (FR-008 procedure): version
    /// check (FR-019), sandbox checkout under the entry budget (FR-017), host
    /// API install (gated per FR-020), entry evaluation, then collection of
    /// the pending tool/command registrations.
    ///
    /// On failure the plugin's partial registrations are discarded with the
    /// context (FR-015: the sandbox goes away entirely) and telemetry is
    /// recorded.
    fn load(&mut self, parsed: ParsedManifest, store: &std::path::Path) -> LoadReport {
        let id = parsed.descriptor.id.clone();
        let declared_permissions = parsed.descriptor.requested_permissions.clone();

        let report = self
            .load_inner(parsed, store)
            .map(|loaded| {
                let tools = loaded.registered_tool_names();
                let commands = loaded
                    .commands
                    .iter()
                    .map(|c| c.decl.name.clone())
                    .collect::<Vec<_>>();
                self.tracked.insert(id.clone(), loaded);
                LoadReport {
                    id: id.clone(),
                    state: LifecycleState::Loaded,
                    error: None,
                    tools,
                    commands,
                    declared_permissions: declared_permissions.clone(),
                }
            })
            .unwrap_or_else(|cause| {
                self.mark_errored(&id, store, cause.clone());
                LoadReport {
                    id,
                    state: LifecycleState::Errored,
                    error: Some(cause),
                    tools: Vec::new(),
                    commands: Vec::new(),
                    declared_permissions,
                }
            });

        // Telemetry: persist load outcome (FR-022).
        let mut ledger = StoreLedger::load(store);
        let counters = &mut ledger.state_mut(&report.id).counters;
        if report.state == LifecycleState::Loaded {
            counters.loads_ok += 1;
            counters.consecutive_failures = 0;
        } else {
            counters.load_failures += 1;
        }
        if let Err(e) = ledger.save(store) {
            tracing::warn!(
                plugin = %report.id,
                error = %e,
                "plugin load telemetry could not be persisted"
            );
        }
        report
    }

    /// The fallible loading core of [`PluginManager::load`]; on success the
    /// [`LoadedPlugin`] is ready for tracking.
    fn load_inner(
        &self,
        parsed: ParsedManifest,
        store: &std::path::Path,
    ) -> Result<LoadedPlugin, String> {
        let descriptor = parsed.descriptor;

        // FR-019: refuse a newer declared host-API version.
        check_api_version(descriptor.api_version, HOST_API_VERSION)
            .map_err(|mismatch| format!("api-version: {mismatch}"))?;

        // A non-JS plugin (skill-only / MCP-only) contributes no JavaScript:
        // it has no entry point, so it loads inertly with no sandbox context
        // and no contributions. `enable`/`load` still report it `loaded`.
        let Some(entry) = descriptor.entry.clone() else {
            return Ok(LoadedPlugin {
                descriptor,
                state: LifecycleState::Loaded,
                error: None,
                tools: parsed.tools,
                commands: parsed.commands,
                calls: HostCalls::new(),
                store: store.to_path_buf(),
                context: None,
            });
        };

        // FR-017 entry budget; FR-003 sandbox context.
        let budget = SandboxBudget::from_config(&self.config).with_deadline(
            std::time::Duration::from_millis(self.config.max_entry_ms.max(1)),
        );
        let context = self
            .pool
            .checkout(budget)
            .map_err(|e| format!("entry: engine checkout failed: {e}"))?;

        // FR-004 + FR-020: gated host API surface.
        let calls = HostCalls::new();
        let install =
            HostApiInstall::new(&descriptor.id, descriptor.root.clone()).with_calls(calls.clone());
        let install = match build_gate(&descriptor, &self.config) {
            Some(gate) => install.with_gate(gate),
            None => install,
        };
        context
            .install_host_api(&install)
            .map_err(|e| format!("entry: host API install failed: {e}"))?;

        // FR-018: the entry file is read host-side; the sandbox itself has no
        // filesystem access. The interrupt gets a fresh entry-budget window so
        // a slow entry cannot starve later tool dispatches of deadline headroom
        // (FR-017).
        let source = std::fs::read_to_string(&entry)
            .map_err(|e| format!("entry: {}: {e}", entry.display()))?;
        context.reset_interrupt();
        context.eval(&source).map_err(|e| entry_cause(&e))?;

        let mut tools = parsed.tools;
        tools.extend(calls.tools());
        let mut commands = parsed.commands;
        // Runtime `register_command` contributions are inline (sandbox handlers).
        commands.extend(
            calls
                .commands()
                .into_iter()
                .map(crate::manifest::PluginCommandDef::inline),
        );
        Ok(LoadedPlugin {
            descriptor,
            state: LifecycleState::Loaded,
            error: None,
            tools,
            commands,
            calls,
            store: store.to_path_buf(),
            context: Some(context),
        })
    }

    /// Record an `errored` runtime state for a plugin that could not be
    /// loaded (manifest-parse at discovery, version refusal, entry failure).
    fn mark_errored(
        &mut self,
        plugin_id: &str,
        store: &std::path::Path,
        cause: String,
    ) -> LoadReport {
        let tracked = self
            .tracked
            .entry(plugin_id.to_string())
            .or_insert_with(|| {
                // Descriptor fields that discovery could not produce are
                // placeholders; the errored state and cause carry the report.
                LoadedPlugin {
                    descriptor: PluginDescriptor {
                        id: plugin_id.to_string(),
                        name: plugin_id.to_string(),
                        version: String::new(),
                        dialect: crate::descriptor::PluginDialect::Codex,
                        entry: None,
                        requested_permissions: Vec::new(),
                        api_version: HOST_API_VERSION,
                        unsupported_capabilities: Vec::new(),
                        manifest_path: PathBuf::new(),
                        root: PathBuf::new(),
                    },
                    state: LifecycleState::Errored,
                    error: None,
                    tools: Vec::new(),
                    commands: Vec::new(),
                    calls: HostCalls::new(),
                    store: store.to_path_buf(),
                    context: None,
                }
            });
        tracked.state = LifecycleState::Errored;
        tracked.error = Some(cause.clone());
        LoadReport {
            id: plugin_id.to_string(),
            state: LifecycleState::Errored,
            error: Some(cause),
            tools: Vec::new(),
            commands: Vec::new(),
            declared_permissions: tracked.descriptor.requested_permissions.clone(),
        }
    }

    /// Whether the ledger in either store marks the plugin enabled.
    fn enabled_in_ledger(&self, plugin_id: &str) -> bool {
        [&self.dirs.global, &self.dirs.project]
            .into_iter()
            .flatten()
            .any(|store| {
                StoreLedger::load(store)
                    .state(plugin_id)
                    .is_some_and(|s| s.enabled)
            })
    }
}

/// Build the FR-020 permission gate for one plugin from the `plugins`
/// configuration. When `plugins.permissions` lists the plugin id, only the
/// listed v1 capability names (or `"*"`) are granted; with no entry the full
/// surface installs (no gate). The plugin's `requested_permissions` are not
/// capability names (they describe external grants such as
/// `network.outbound`); they are surfaced on [`LoadReport`] for the enable
/// path to state to the user.
#[must_use]
pub fn build_gate(
    descriptor: &PluginDescriptor,
    config: &ragent_config::PluginsConfig,
) -> Option<PermissionGate> {
    let grants = config.permissions.get(&descriptor.id)?.clone();
    let plugin_id = descriptor.id.clone();
    let consulted = Arc::new(Mutex::new(Vec::<String>::new()));
    let consulted_in_gate = Arc::clone(&consulted);
    Some(Arc::new(move |capability: &str| {
        consulted_in_gate
            .lock()
            .map(|mut seen| seen.push(capability.to_string()))
            .ok();
        let allowed = grants.iter().any(|g| g == "*" || g == capability);
        tracing::debug!(
            plugin = %plugin_id,
            capability,
            allowed,
            "lifecycle capability gate decision"
        );
        allowed
    }))
}

/// Map an entry-execution failure to its cause string (SPEC error handling:
/// cause class `entry` plus the JavaScript detail; timeout and memory-limit
/// aborts name the resource).
fn entry_cause(error: &PluginError) -> String {
    match error {
        PluginError::Timeout => "entry: timeout: plugin execution timed out".to_string(),
        PluginError::MemoryLimit => "entry: memory-limit: plugin memory limit exceeded".to_string(),
        PluginError::Script { detail, .. } => format!("entry: script: {detail}"),
        other => format!("entry: {other}"),
    }
}

/// The identity key for a scanned plugin: descriptor id when parsed, else the
/// directory name. Shared with the `/plugins test` harness (T-015) so both
/// resolve a plugin's identity identically.
pub(crate) fn scan_id(plugin: &ScannedPlugin) -> String {
    match &plugin.outcome {
        Ok(parsed) => parsed.descriptor.id.clone(),
        Err(_) => plugin
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// Persist the enable flag for one plugin in its store ledger.
fn set_enabled(store: &std::path::Path, plugin_id: &str, enabled: bool) -> Result<(), PluginError> {
    let mut ledger = StoreLedger::load(store);
    ledger.state_mut(plugin_id).enabled = enabled;
    ledger.save(store)
}
