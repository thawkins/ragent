//! Adapter registering plugin-contributed slash commands with collision
//! rejection (spec `plugins` T-012; FR-004, FR-024).
//!
//! A plugin contributes a slash command by declaring it in its manifest
//! (`commands[]`) or by calling
//! `ragent.register_command({ name, description, usage?, handler? })` while
//! its entry point executes. [`PluginCommandAdapter`] is the session-facing
//! metadata carrier: it exposes the declared `name` (used verbatim as the
//! slash trigger), `description`, and `usage` so the TUI command surface can
//! list and dispatch the command.
//!
//! ## Collision rejection (FR-024)
//!
//! Command names register under their declared trigger; a collision with a
//! built-in `SLASH_COMMANDS` entry, another plugin's command, or a second
//! declaration of the same name by the same plugin is rejected with
//! [`PluginError::NameCollision`]. Registration through
//! [`PluginManager::register_plugin_commands`] is all-or-nothing per plugin;
//! the session records the failure as an `errored` state with cause
//! `name-collision`.
//!
//! ## Handler dispatch
//!
//! Like plugin tools, command handlers run inside the plugin's live
//! (`!Send`) entry sandbox, so invocation goes through
//! [`PluginManager::execute_command`] on the manager-owning thread. The
//! argument string typed after the trigger is marshalled into the sandbox as
//! the scratch global `__ragent_plugin_cmd_args` and handed to the handler
//! as a plain JavaScript string; the handler's return value is surfaced as
//! message-window text (bare strings pass through, `undefined`/`null`
//! produce empty text, anything else is `JSON.stringify`-compacted). Command
//! failures are contained like any plugin failure (FR-026) and count toward
//! the consecutive-failure auto-unload threshold.

use std::collections::BTreeSet;

use crate::error::PluginError;
use crate::lifecycle::PluginManager;
use crate::manifest::PluginCommandDef;
use crate::runtime::SandboxContext;
use crate::store::LifecycleState;
use crate::tool_adapter::js_literal;

/// Sandbox global receiving the command argument text before each command
/// handler invocation.
const CMD_ARGS_GLOBAL: &str = "__ragent_plugin_cmd_args";

/// A plugin-contributed slash command bridged into the session command
/// surface (FR-004). Metadata carrier only: registration-time listing,
/// autocomplete, and usage help read name/description/usage from the
/// declaration; actual handler dispatch needs the plugin's live sandbox and
/// is routed through [`PluginManager::execute_command`].
#[derive(Debug)]
pub struct PluginCommandAdapter {
    /// Slash trigger (the command's declared name, without a leading `/`).
    name: String,
    /// Owning plugin id.
    plugin_id: String,
    /// Dispatchable definition (inline sandbox handler or prompt file).
    def: PluginCommandDef,
}

impl PluginCommandAdapter {
    /// Build an adapter for `def` contributed by `plugin_id`.
    #[must_use]
    pub fn new(plugin_id: &str, def: PluginCommandDef) -> Self {
        Self {
            name: def.decl.name.clone(),
            plugin_id: plugin_id.to_string(),
            def,
        }
    }

    /// Slash trigger (no leading `/`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Owning plugin id.
    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Human-readable description for the autocomplete menu.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.def.decl.description
    }

    /// Usage hint for help output, when the plugin declared one.
    #[must_use]
    pub fn usage(&self) -> Option<&str> {
        self.def.decl.usage.as_deref()
    }

    /// Prompt body when this command is a prompt command, else `None`.
    #[must_use]
    pub fn prompt(&self) -> Option<&str> {
        self.def.prompt()
    }

    /// Whether this command dispatches into the plugin's JavaScript sandbox.
    #[must_use]
    pub fn is_inline(&self) -> bool {
        self.def.source == crate::manifest::CommandSource::Inline
    }
}

impl PluginManager {
    /// All command names this manager's loaded plugins currently register,
    /// in name order. Combined with the built-in `SLASH_COMMANDS` list, this
    /// forms the collision surface checked by
    /// [`PluginManager::register_plugin_commands`] (FR-024).
    #[must_use]
    pub fn registered_plugin_command_names(&self) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        for loaded in self.tracked_plugins() {
            for def in &loaded.commands {
                set.insert(def.decl.name.clone());
            }
        }
        set
    }

    /// Build adapters for every slash command a loaded plugin contributes
    /// and register them through `register` (the session passes a
    /// command-surface-shaped closure so this crate never imports the TUI
    /// layer). Collision-free registration returns the registered names; the
    /// first collision registers nothing and fails (all-or-nothing, FR-024).
    ///
    /// `existing` must contain the built-in trigger list plus other loaded
    /// plugins' command names (see
    /// [`PluginManager::registered_plugin_command_names`]).
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::NameCollision`] on the first clashing name.
    pub fn register_plugin_commands(
        &self,
        plugin_id: &str,
        existing: &BTreeSet<String>,
        mut register: impl FnMut(std::sync::Arc<PluginCommandAdapter>),
    ) -> Result<Vec<String>, PluginError> {
        let loaded = self
            .get(plugin_id)
            .ok_or_else(|| PluginError::UnknownPlugin(plugin_id.to_string()))?;
        if loaded.state != LifecycleState::Loaded {
            return Err(PluginError::Script {
                plugin: plugin_id.to_string(),
                detail: format!(
                    "plugin '{plugin_id}' is not loaded (state: {})",
                    loaded.state
                ),
            });
        }
        let mut ours = BTreeSet::new();
        let mut adapters = Vec::with_capacity(loaded.commands.len());
        for def in &loaded.commands {
            if existing.contains(&def.decl.name) || !ours.insert(def.decl.name.clone()) {
                return Err(PluginError::NameCollision(def.decl.name.clone()));
            }
            adapters.push(PluginCommandAdapter::new(plugin_id, def.clone()));
        }
        let names: Vec<String> = adapters.iter().map(|a| a.name().to_string()).collect();
        for adapter in adapters {
            register(std::sync::Arc::new(adapter));
        }
        Ok(names)
    }

    /// Invoke the plugin slash command `registry_name` through the owning
    /// plugin's live sandbox, with `args` (the text typed after the trigger)
    /// marshalled in as a plain string. The handler's return value is
    /// converted to message-window text; failures are contained and recorded
    /// against the plugin's consecutive-failure telemetry, tripping the
    /// auto-unload threshold exactly like tool failures (FR-004, FR-026).
    ///
    /// Must run on the manager-owning thread because rquickjs contexts are
    /// `!Send`. The returned boolean is `true` when this call tripped the
    /// auto-unload threshold.
    pub fn execute_command(
        &mut self,
        registry_name: &str,
        args: &str,
    ) -> (Result<String, PluginError>, bool) {
        let Some((plugin_id, command_name)) = self.find_command(registry_name) else {
            return (
                Err(PluginError::UnknownPlugin(registry_name.to_string())),
                false,
            );
        };

        // A prompt command has no JavaScript handler: its body is the rendered
        // result. Returning the body here keeps the TUI dispatch uniform — the
        // session decides whether to show it or inject it into the agent (FR-031).
        if let Some(body) = self.prompt_for(registry_name) {
            return (
                Ok(substitute_command_args(
                    &body,
                    args,
                    &plugin_id,
                    &command_name,
                )),
                false,
            );
        }

        let context_result = self
            .get(&plugin_id)
            .and_then(|loaded| loaded.context())
            .ok_or_else(|| PluginError::Script {
                plugin: plugin_id.clone(),
                detail: format!("plugin '{plugin_id}' is not loaded"),
            });
        let outcome = context_result
            .and_then(|context| dispatch_command_sandbox(context, &plugin_id, &command_name, args));

        let unloaded = self.record_tool_outcome(&plugin_id, outcome.is_ok());
        (outcome, unloaded)
    }

    /// Resolve a slash trigger against the loaded plugins' contributed
    /// commands. Errored plugins are skipped: their commands are not
    /// invocable (FR-016-style inertness for failed loads). `None` when no
    /// loaded plugin contributes that trigger.
    fn find_command(&self, registry_name: &str) -> Option<(String, String)> {
        self.tracked_plugins()
            .filter(|loaded| loaded.state == LifecycleState::Loaded)
            .find_map(|loaded| {
                loaded.commands.iter().find_map(|def| {
                    (def.decl.name == registry_name)
                        .then(|| (loaded.descriptor.id.clone(), def.decl.name.clone()))
                })
            })
    }

    /// The prompt body for a registered prompt command, else `None`. Used by
    /// [`PluginManager::execute_command`] to short-circuit prompt commands away
    /// from the sandbox and by callers that need the rendered prompt itself.
    #[must_use]
    pub fn prompt_for(&self, registry_name: &str) -> Option<String> {
        self.tracked_plugins()
            .filter(|loaded| loaded.state == LifecycleState::Loaded)
            .find_map(|loaded| {
                loaded.commands.iter().find_map(|def| {
                    (def.decl.name == registry_name)
                        .then(|| def.prompt().map(str::to_string))
                        .flatten()
                })
            })
    }
}

/// Substitute `$ARGUMENTS`, `$1..`, and `${PLUGIN_...}` placeholders in a prompt
/// command body (FR-031). Mirrors the skill arg-substitution subset the Claude
/// command templates use; unknown placeholders are left untouched.
#[must_use]
pub fn substitute_command_args(body: &str, args: &str, plugin_id: &str, command: &str) -> String {
    let mut result = body
        .replace("${PLUGIN_ID}", plugin_id)
        .replace("${PLUGIN_COMMAND}", command);
    // $ARGUMENTS[N] and $N positional access, longest patterns first.
    let parsed: Vec<&str> = args.split_whitespace().collect();
    for (i, arg) in parsed.iter().enumerate() {
        result = result
            .replace(&format!("$ARGUMENTS[{i}]"), arg)
            .replace(&format!("${i}"), arg);
    }
    result.replace("$ARGUMENTS", args)
}

/// Invoke the JavaScript handler for `command_name` in the plugin's live
/// entry sandbox with `args` marshalled as a plain string, converting the
/// return value to message-window text (FR-004).
///
/// The handler is resolved from `globalThis.__ragent_commands[name]` (the
/// map `register_command` populates when the entry ran). Returns
/// `Ok(text)`: bare strings pass through, `undefined`/`null` produce an
/// empty string, and any other JSON value is `JSON.stringify`-compacted —
/// the session prints the text into the message window.
///
/// # Errors
///
/// Propagates sandbox failures — [`PluginError::Script`], `Timeout`,
/// `MemoryLimit`, `Engine` — plus the contract's serialisation errors, all
/// with `plugin_id` attached (FR-015, FR-026).
pub fn dispatch_command_sandbox(
    context: &SandboxContext,
    plugin_id: &str,
    command_name: &str,
    args: &str,
) -> Result<String, PluginError> {
    context.set_global_str(CMD_ARGS_GLOBAL, args)?;
    // FR-017 per-invocation budget: arm a fresh window for this dispatch.
    context.rearm_deadline();

    // Keep message literals plain (no quotes/braces concatenation): nested
    // eval of strings containing them trips rquickjs parsing (runtime.rs).
    let source = format!(
        "(function(){{ \
           const named = {name_lit}; \
           const reg = globalThis.__ragent_commands || {{}}; \
           const handler = reg[named]; \
           if (typeof handler !== 'function') throw new Error('no handler registered for command ' + named); \
           const argText = globalThis.__ragent_plugin_cmd_args; \
           const result = handler(typeof argText === 'string' ? argText : ''); \
           if (typeof result === 'undefined' || result === null) return ''; \
           if (typeof result === 'string') return result; \
           let serialised; \
           try {{ serialised = JSON.stringify(result); }} catch (e) {{ \
             throw new Error('plugin command result is not JSON-serialisable'); \
           }} \
           return serialised; \
         }})()",
        name_lit = js_literal(command_name),
    );
    context
        .eval_to_string(&source)
        .map_err(|e| with_plugin(e, plugin_id))
}

/// Attach the plugin id to a sandbox error (the runtime classifies errors
/// without knowing which plugin it served).
fn with_plugin(error: PluginError, plugin_id: &str) -> PluginError {
    match error {
        PluginError::Script { detail, .. } => PluginError::Script {
            plugin: plugin_id.to_string(),
            detail,
        },
        other => other,
    }
}
