//! Adapter registering plugin-contributed tools in the session tool registry
//! under `plugin_<pluginid>_<toolname>` (spec `plugins` T-010; FR-005,
//! FR-015, FR-024, FR-026).
//!
//! A plugin contributes a tool by declaring it in its manifest or by calling
//! `ragent.register_tool({ name, description, parameters, handler })` while
//! its entry point executes. For each declaration the session layer builds a
//! [`PluginToolAdapter`] (one per tool per loaded plugin) via
//! [`register_plugin_tools`]: the adapter implements the existing [`Tool`]
//! trait so plugin tools are first-class registry members with the
//! plugin-declared JSON schema (FR-005) under permission domain
//! `plugin:<pluginid>` (A5).
//!
//! ## Collision rejection (FR-024)
//!
//! Registered names follow `plugin_<pluginid>_<toolname>`. A collision with
//! a built-in tool, another plugin's tool, or a second declaration of the
//! same name by the same plugin is rejected with
//! [`PluginError::NameCollision`]; registration is all-or-nothing per plugin
//! and the session records the failure as an `errored` state with cause
//! `name-collision` (PLAN T-010).
//!
//! ## Execution and marshalling (FR-005, FR-015, FR-026)
//!
//! Plugin handlers run inside the plugin's **live entry sandbox**, which is
//! `!Send`; the session therefore dispatches tool calls through
//! [`PluginManager::execute_tool`] on the manager-owning thread rather than
//! through an adapter's async `execute`. The marshalling contract (host API
//! v1):
//!
//! - arguments cross into the sandbox as a JSON document bound to the
//!   scratch global `__ragent_plugin_args`; the handler receives them as a
//!   plain JavaScript value;
//! - `undefined`/`null` yield empty content; a bare string passes through;
//!   `{ content, isError? }` unwraps into content plus an error flag
//!   (a `true` flag fails the tool call); any other value is surfaced
//!   compactly as content and carried under [`ToolOutput::metadata`];
//! - when `JSON.stringify` of the return value throws (e.g. cyclic object)
//!   the tool call fails with a serialisation error instead of a panic
//!   (FR-026).

use std::collections::BTreeSet;
use std::sync::Arc;

use ragent_tools_core::{Tool, ToolContext, ToolOutput};
use serde_json::Value as JsonValue;

use crate::error::PluginError;
use crate::lifecycle::PluginManager;
use crate::manifest::PluginToolDecl;
use crate::runtime::SandboxContext;
use crate::store::LifecycleState;

/// Sandbox global receiving the JSON argument document before each handler
/// invocation; the dispatch wrapper `JSON.parse`s it for the handler.
const ARGS_GLOBAL: &str = "__ragent_plugin_args";

/// Build the registry name for a plugin tool: `plugin_<pluginid>_<toolname>`
/// (FR-005, A5).
#[must_use]
pub fn plugin_tool_name(plugin_id: &str, tool_name: &str) -> String {
    format!("plugin_{plugin_id}_{tool_name}")
}

/// A plugin-contributed tool bridged into the session tool registry
/// (FR-005). Registration-time metadata carrier: the adapter answers
/// `name` / `description` / `parameters_schema` from the plugin's
/// declaration so the registry, `/tools`, and LLM tool definitions see a
/// first-class tool.
///
/// Actual handler dispatch needs the live (`!Send`) sandbox owned by the
/// session's [`PluginManager`]; the adapter therefore reports an explicit
/// engine error from [`Tool::execute`] and the session routes invocations
/// through [`PluginManager::execute_tool`] instead.
#[derive(Debug)]
pub struct PluginToolAdapter {
    /// Full registry name (`plugin_<id>_<tool>`).
    name: String,
    /// Permission domain (`plugin:<pluginid>`, A5).
    permission: String,
    /// Owning plugin id.
    plugin_id: String,
    /// Declaration as supplied by the plugin (manifest or `register_tool`).
    decl: PluginToolDecl,
}

impl PluginToolAdapter {
    /// Build an adapter for `decl` contributed by `plugin_id`.
    #[must_use]
    pub fn new(plugin_id: &str, decl: PluginToolDecl) -> Self {
        Self {
            name: plugin_tool_name(plugin_id, &decl.name),
            permission: format!("plugin:{plugin_id}"),
            plugin_id: plugin_id.to_string(),
            decl,
        }
    }

    /// Owning plugin id.
    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Declared tool name within the plugin (without the `plugin_<id>_`
    /// prefix).
    #[must_use]
    pub fn tool_name(&self) -> &str {
        &self.decl.name
    }
}

#[async_trait::async_trait]
impl Tool for PluginToolAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.decl.description
    }

    fn parameters_schema(&self) -> JsonValue {
        // FR-005: the plugin's declared JSON schema passes through unchanged;
        // absent declarations fall back to a permissive object schema.
        match &self.decl.parameters {
            JsonValue::Null => JsonValue::Object(serde_json::Map::from_iter([(
                "type".to_string(),
                JsonValue::String("object".to_string()),
            )])),
            schema => schema.clone(),
        }
    }

    fn permission_category(&self) -> &str {
        &self.permission
    }

    /// The registry contract requires the async `Tool::execute` surface, but
    /// dispatch must happen on the thread owning the plugin's `!Send`
    /// sandbox. The session wires that through
    /// [`PluginManager::execute_tool`]; a generic lookup reaching this
    /// wrapper gets a clear error rather than a hung runtime (FR-026).
    async fn execute(&self, _input: JsonValue, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        anyhow::bail!(
            "plugin tool '{}' is dispatched through PluginManager::execute_tool",
            self.name
        )
    }
}

impl PluginManager {
    /// All tool names this manager's loaded plugins currently register, in
    /// name order. Combined with the built-in registry list, this forms the
    /// collision surface checked by [`register_plugin_tools`] (FR-024).
    #[must_use]
    pub fn registered_plugin_tool_names(&self) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        for loaded in self.tracked_plugins() {
            for name in loaded.registered_tool_names() {
                set.insert(name);
            }
        }
        set
    }

    /// Build adapters for every tool a loaded plugin contributes and register
    /// them through `register` (the session passes a `ToolRegistry`-shaped
    /// closure so this crate never imports the agent layer). Collision-free
    /// registration returns the registered names; the first collision
    /// registers nothing and fails (all-or-nothing, FR-024).
    ///
    /// `existing` must contain the built-in registry list plus other loaded
    /// plugins' tool names (see [`PluginManager::registered_plugin_tool_names`]).
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::NameCollision`] on the first clashing name.
    pub fn register_plugin_tools(
        &self,
        plugin_id: &str,
        existing: &BTreeSet<String>,
        mut register: impl FnMut(Arc<PluginToolAdapter>),
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
        let mut adapters = Vec::with_capacity(loaded.tools.len());
        for decl in &loaded.tools {
            let name = plugin_tool_name(plugin_id, &decl.name);
            if existing.contains(&name) || !ours.insert(name.clone()) {
                return Err(PluginError::NameCollision(name));
            }
            adapters.push(Arc::new(PluginToolAdapter::new(plugin_id, decl.clone())));
        }
        let names = adapters.iter().map(|a| a.name().to_string()).collect();
        for adapter in adapters {
            register(adapter);
        }
        Ok(names)
    }

    /// Invoke the plugin tool under full registry name `registry_name`
    /// through the owning plugin's live sandbox, recording the outcome
    /// against the plugin's consecutive-failure telemetry and auto-unload
    /// threshold (FR-005, FR-015; SPEC error policy). Must run on the
    /// manager-owning thread because rquickjs contexts are `!Send`.
    ///
    /// The returned [`ToolOutput`] follows the v1 marshalling contract
    /// documented on this module. Any failure — exception, timeout, memory
    /// ceiling, serialisation error, stale adapter — is contained and
    /// reported as a [`PluginError`], never a panic (FR-026); the returned
    /// boolean is `true` when this call tripped the auto-unload threshold
    /// (the session then deregisters the plugin's tools and records
    /// `errored`).
    pub fn execute_tool(
        &mut self,
        registry_name: &str,
        args: JsonValue,
    ) -> (Result<ToolOutput, PluginError>, bool) {
        let Some((plugin_id, tool_name)) = self.find_tool(registry_name) else {
            return (
                Err(PluginError::UnknownPlugin(registry_name.to_string())),
                false,
            );
        };

        let context_result = self
            .get(&plugin_id)
            .and_then(|loaded| loaded.context())
            .ok_or_else(|| PluginError::Script {
                plugin: plugin_id.clone(),
                detail: format!("plugin '{plugin_id}' is not loaded"),
            });
        let outcome = context_result
            .and_then(|context| dispatch_sandbox(context, &plugin_id, &tool_name, &args));

        let unloaded = self.record_tool_outcome(&plugin_id, outcome.is_ok());
        (outcome, unloaded)
    }

    /// Resolve a full registry name (`plugin_<id>_<tool>`) against the loaded
    /// plugins' contributed tools. Errored plugins are skipped: their
    /// declarations are not invocable (FR-016-style inertness for failed
    /// loads). `None` when no loaded plugin contributes that name.
    fn find_tool(&self, registry_name: &str) -> Option<(String, String)> {
        self.tracked_plugins()
            .filter(|loaded| loaded.state == LifecycleState::Loaded)
            .find_map(|loaded| {
                loaded.tools.iter().find_map(|decl| {
                    let full = plugin_tool_name(&loaded.descriptor.id, &decl.name);
                    (full == registry_name)
                        .then(|| (loaded.descriptor.id.clone(), decl.name.clone()))
                })
            })
    }
}

/// Invoke the JavaScript handler for `tool_name` in the plugin's live entry
/// sandbox with `args` marshalled as JSON, converting the return value per
/// the v1 marshalling contract (FR-005).
///
/// The handler is resolved in the sandbox from `globalThis.__ragent_tools`
/// first (the map `register_tool` populates when the entry ran), then from a
/// global function of the tool's own name (manifest-declared tools with no
/// explicit `register_tool` call).
///
/// # Errors
///
/// Propagates sandbox failures — [`PluginError::Script`], `Timeout`,
/// `MemoryLimit`, `Engine` — plus the contract's serialisation and
/// invalid-JSON errors, all with `plugin_id` attached (FR-015, FR-026).
pub fn dispatch_sandbox(
    context: &SandboxContext,
    plugin_id: &str,
    tool_name: &str,
    args: &JsonValue,
) -> Result<ToolOutput, PluginError> {
    let args_json = serde_json::to_string(args).map_err(|e| PluginError::Script {
        plugin: plugin_id.to_string(),
        detail: format!("arguments could not be marshalled to JSON: {e}"),
    })?;
    context.set_global_str(ARGS_GLOBAL, &args_json)?;
    // FR-017 per-invocation budget: the entry deadline expired with the entry
    // execution; arm a fresh window for this dispatch.
    context.rearm_deadline();

    // Keep message literals plain (no quotes/braces concatenation): nested
    // eval of strings containing them trips rquickjs parsing (runtime.rs).
    let source = format!(
        "(function(){{ \
           const named = {name_lit}; \
           const reg = globalThis.__ragent_tools || {{}}; \
           let handler = reg[named]; \
           if (typeof handler !== 'function' && typeof globalThis[named] === 'function') {{ \
             handler = globalThis[named]; \
           }} \
           if (typeof handler !== 'function') throw new Error('no handler registered for tool ' + named); \
           const parsed = JSON.parse(globalThis.__ragent_plugin_args); \
           const result = handler(parsed); \
           if (typeof result === 'undefined' || result === null) return ''; \
           if (typeof result === 'string') return JSON.stringify({{ text: result }}); \
           let serialised; \
           try {{ serialised = JSON.stringify(result); }} catch (e) {{ \
             throw new Error('plugin tool result is not JSON-serialisable'); \
           }} \
           return serialised; \
         }})()",
        name_lit = js_literal(tool_name),
    );
    let returned = context
        .eval_to_string(&source)
        .map_err(|e| with_plugin(e, plugin_id))?;
    marshal_return(plugin_id, &returned)
}

/// Convert the handler's marshalled return document into a [`ToolOutput`].
/// Empty string means the handler returned `undefined`/`null`; a
/// single-field `{ text }` wrapper marks a bare-string return; a
/// `content`-carrying object follows the explicit contract (`isError: true`
/// fails the call); any other JSON value becomes compact content plus
/// structured metadata.
fn marshal_return(plugin_id: &str, returned: &str) -> Result<ToolOutput, PluginError> {
    if returned.is_empty() {
        return Ok(ToolOutput::default());
    }
    let value: JsonValue = serde_json::from_str(returned).map_err(|e| PluginError::Script {
        plugin: plugin_id.to_string(),
        detail: format!("plugin returned invalid JSON: {e}"),
    })?;

    if let JsonValue::Object(map) = &value {
        if map.len() == 1
            && let Some(JsonValue::String(text)) = map.get("text")
        {
            return Ok(ToolOutput {
                content: text.clone(),
                metadata: None,
            });
        }
        if let Some(JsonValue::String(content)) = map.get("content") {
            if map
                .get("isError")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false)
            {
                return Err(PluginError::Script {
                    plugin: plugin_id.to_string(),
                    detail: content.clone(),
                });
            }
            return Ok(ToolOutput {
                content: content.clone(),
                metadata: Some(value),
            });
        }
    }

    Ok(ToolOutput {
        content: value.to_string(),
        metadata: Some(value),
    })
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

/// Quote a Rust string as a JavaScript string literal (JSON escaping is
/// valid JS string-literal syntax for ASCII content). Shared with
/// [`crate::command_adapter`]'s dispatch wrapper.
pub(crate) fn js_literal(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
}
