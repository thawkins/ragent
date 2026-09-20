//! Versioned host-API bridge presented to plugin JavaScript (spec `plugins`;
//! FR-004, FR-018, FR-020, FR-027).
//!
//! [`HostApiInstall`] captures everything the `ragent` host object needs for
//! one plugin invocation and installs it into a sandboxed context:
//!
//! ```text
//! ragent.api_version                          -> 1
//! ragent.plugin_id                            -> string
//! ragent.register_tool(def)                   -> recorded; wired by T-010
//! ragent.register_command(def)                -> recorded; wired by T-012
//! ragent.config.get(key)                      -> JSON text from plugin-scoped config, or undefined
//! ragent.message.info(text) / .warn(text) / .error(text)
//! ragent.log(level, text)
//! ragent.plugin.read_text_file(relative_path) -> string (restricted to the plugin directory)
//! ```
//!
//! All capability calls are recorded into a shared [`HostCalls`] sink; the
//! session layer drains it to register tools/commands (T-010/T-012) and to
//! render messages (FR-020). Nothing leaves the sandbox except through this
//! sink (FR-018).
//!
//! ## Permission gate (FR-020)
//!
//! Each installable capability group is keyed by one of the names in
//! [`crate::manifest::V1_CAPABILITIES`] (`tools`, `commands`, `config`,
//! `message`, `log`, `plugin.read_text_file`). Before installing a group,
//! [`HostApiInstall::install`] asks a [`PermissionGate`] for a decision;
//! ungranted capability groups are simply absent from the injected object.
//! The full permission key for a capability is
//! `plugin:<pluginid>:<capability>` (see [`capability_permission_key`]); the
//! gate closure is typically built by the lifecycle layer from the plugin's
//! declared permissions plus the `plugins.permissions` config grants and the
//! normal permission rule engine. Without a gate the full surface installs.
//!
//! ## Logging posture (FR-027)
//!
//! Host-level traces of capability use and gate decisions are emitted at
//! `debug!`/`trace!` only; nothing about plugin-internal file contents,
//! tool results, or message payloads is logged above that level. The only
//! user-visible plugin output is what the plugin itself emits through
//! `ragent.message.*` (surfaced in the message window) and `ragent.log`
//! (forwarded to the host log by the session layer).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rquickjs::Value as JsValue;
use serde_json::Value as JsonValue;

use crate::error::PluginError;
use crate::manifest::{PluginCommandDecl, PluginToolDecl, V1_CAPABILITIES};

/// Build the permission key for a capability query: `plugin:<pluginid>:<cap>`
/// (FR-020). The capability name is one of [`V1_CAPABILITIES`].
#[must_use]
pub fn capability_permission_key(plugin_id: &str, capability: &str) -> String {
    format!("plugin:{plugin_id}:{capability}")
}

/// Decision callback for [`HostApiInstall`]: given a v1 capability name (one
/// of [`V1_CAPABILITIES`]), decide whether that capability group is installed
/// into the plugin's `ragent` object (FR-020). Returning `false` makes the
/// capability simply absent from the injected object.
///
/// A boxed trait object is used so [`HostApiInstall`] stays storage-friendly
/// (no generic parameter) and the closure may capture user-grant maps,
/// config, or session state. `Send + Sync` lets an install plan travel with
/// the session's async tasks even though execution itself is in-thread.
pub type PermissionGate = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// A message emitted by plugin code through `ragent.message.*` (FR-020).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMessage {
    /// Severity: `info`, `warn`, or `error`.
    pub level: String,
    /// Message text.
    pub text: String,
}

/// A log line emitted by `ragent.log(level, text)` (FR-027: plugin-initiated
/// logging is expected and forwarded to the host log).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginLogLine {
    /// Level string supplied by the plugin (`debug`/`info`/`warn`/`error`).
    pub level: String,
    /// Log text.
    pub text: String,
}

/// Thread-safe sink collecting everything plugin code reported through the
/// host API during one execution plus any registrations it made (FR-020).
///
/// Shared with the harness/test surface so `/plugins test` can assert on
/// exactly what a plugin emitted.
#[derive(Debug, Default, Clone)]
pub struct HostCalls {
    inner: Arc<Mutex<HostCallsInner>>,
}

#[derive(Debug, Default)]
struct HostCallsInner {
    messages: Vec<PluginMessage>,
    logs: Vec<PluginLogLine>,
    tools: Vec<PluginToolDecl>,
    commands: Vec<PluginCommandDecl>,
}

impl HostCalls {
    /// Create an empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of recorded messages.
    #[must_use]
    pub fn messages(&self) -> Vec<PluginMessage> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .messages
            .clone()
    }

    /// Snapshot of recorded log lines.
    #[must_use]
    pub fn logs(&self) -> Vec<PluginLogLine> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .logs
            .clone()
    }

    /// Snapshot of tools registered by `register_tool`.
    #[must_use]
    pub fn tools(&self) -> Vec<PluginToolDecl> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tools
            .clone()
    }

    /// Snapshot of commands registered by `register_command`.
    #[must_use]
    pub fn commands(&self) -> Vec<PluginCommandDecl> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .commands
            .clone()
    }

    /// Record a message (called from the JS bridge).
    pub fn push_message(&self, level: &str, text: String) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .messages
            .push(PluginMessage {
                level: level.to_string(),
                text,
            });
    }

    /// Record a log line.
    pub fn push_log(&self, level: &str, text: String) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .logs
            .push(PluginLogLine {
                level: level.to_string(),
                text,
            });
    }

    /// Record a tool registration.
    pub fn push_tool(&self, decl: PluginToolDecl) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tools
            .push(decl);
    }

    /// Record a command registration.
    pub fn push_command(&self, decl: PluginCommandDecl) {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .commands
            .push(decl);
    }
}

/// Everything needed to install the `ragent` host object for one plugin
/// execution (FR-004, FR-018, FR-020).
pub struct HostApiInstall {
    plugin_id: String,
    calls: HostCalls,
    /// Plugin root directory; `plugin.read_text_file` resolves relative paths
    /// against this and refuses escapes.
    plugin_root: PathBuf,
    /// Plugin-scoped configuration map (`ragent.config.get` source).
    plugin_config: BTreeMap<String, JsonValue>,
    /// Optional permission gate consulted per capability group (FR-020);
    /// `None` installs the full v1 surface.
    gate: Option<PermissionGate>,
}

impl HostApiInstall {
    /// Build an install plan for one plugin execution.
    #[must_use]
    pub fn new(plugin_id: &str, plugin_root: PathBuf) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            calls: HostCalls::new(),
            plugin_root,
            plugin_config: BTreeMap::new(),
            gate: None,
        }
    }

    /// Attach a shared sink (used by the harness when the caller needs to
    /// observe messages after execution).
    #[must_use]
    pub fn with_calls(mut self, calls: HostCalls) -> Self {
        self.calls = calls;
        self
    }

    /// Attach plugin-scoped configuration (`ragent.config.get`).
    #[must_use]
    pub fn with_config(mut self, config: BTreeMap<String, JsonValue>) -> Self {
        self.plugin_config = config;
        self
    }

    /// Attach a permission gate consulted per capability group (FR-020).
    ///
    /// The gate receives one of [`V1_CAPABILITIES`] and returns whether the
    /// corresponding group is installed; ungranted groups are simply absent
    /// from the injected `ragent` object. The lifecycle layer (T-009) builds
    /// the closure from the plugin's declared permissions, the
    /// `plugins.permissions` config grants, and the normal permission rule
    /// engine, consulting [`capability_permission_key`] for the full key.
    #[must_use]
    pub fn with_gate(mut self, gate: PermissionGate) -> Self {
        self.gate = Some(gate);
        self
    }

    /// The sink this install records into.
    #[must_use]
    pub fn calls(&self) -> HostCalls {
        self.calls.clone()
    }

    /// Ask the gate for one capability; an absent gate grants everything.
    fn granted(&self, capability: &str) -> bool {
        debug_assert!(
            V1_CAPABILITIES.contains(&capability),
            "unknown host capability: {capability}"
        );
        match &self.gate {
            Some(gate) => {
                let allowed = gate(capability);
                // FR-027: gate decisions are debug-level operational traces,
                // never payload-carrying.
                tracing::debug!(
                    plugin = %self.plugin_id,
                    capability,
                    key = %capability_permission_key(&self.plugin_id, capability),
                    granted = allowed,
                    "host API capability gate decision"
                );
                allowed
            }
            None => true,
        }
    }

    /// Install the `ragent` global into an rquickjs context (FR-004).
    ///
    /// Installs `api_version` and `plugin_id` unconditionally, then each
    /// capability group subject to the gate (FR-020). A denied group is
    /// simply absent from the object; the plugin still loads.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Engine`] when any function/object cannot be
    /// created or set.
    pub fn install(&self, ctx: rquickjs::Ctx<'_>) -> Result<(), PluginError> {
        let engine = PluginError::Engine;
        let ragent = rquickjs::Object::new(ctx.clone()).map_err(|e| engine(e.to_string()))?;

        // api_version / plugin_id always exist: they carry no capability.
        ragent
            .set("api_version", crate::manifest::HOST_API_VERSION)
            .map_err(|e| engine(e.to_string()))?;
        ragent
            .set("plugin_id", self.plugin_id.as_str())
            .map_err(|e| engine(e.to_string()))?;

        if self.granted("config") {
            self.install_config(&ctx, &ragent)?;
        }
        if self.granted("message") {
            self.install_message(&ctx, &ragent)?;
        }
        if self.granted("log") {
            self.install_log(&ctx, &ragent)?;
        }
        if self.granted("tools") {
            self.install_register_tool(&ctx, &ragent)?;
        }
        if self.granted("commands") {
            self.install_register_command(&ctx, &ragent)?;
        }
        if self.granted("plugin.read_text_file") {
            self.install_plugin_reader(&ctx, &ragent)?;
        }

        ctx.globals()
            .set("ragent", ragent)
            .map_err(|e| engine(e.to_string()))?;
        Ok(())
    }

    /// `ragent.config.get(key) -> JSON string or undefined`.
    ///
    /// The sandbox boundary is strings-only in v1, so the JSON value crosses
    /// as its serialised text and plugin code parses it itself.
    fn install_config<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        ragent: &rquickjs::Object<'js>,
    ) -> Result<(), PluginError> {
        let engine = PluginError::Engine;
        let plugin_id = self.plugin_id.clone();
        let config_map = self.plugin_config.clone();
        let config_obj = rquickjs::Object::new(ctx.clone()).map_err(|e| engine(e.to_string()))?;
        let get_fn = rquickjs::Function::new(ctx.clone(), move |key: String| {
            tracing::trace!(plugin = %plugin_id, "host API call: config.get");
            config_map.get(&key).map(JsonValue::to_string)
        })
        .map_err(|e| engine(e.to_string()))?;
        config_obj
            .set("get", get_fn)
            .map_err(|e| engine(e.to_string()))?;
        ragent
            .set("config", config_obj)
            .map_err(|e| engine(e.to_string()))?;
        Ok(())
    }

    /// `ragent.message.info(text) / .warn(text) / .error(text)`.
    fn install_message<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        ragent: &rquickjs::Object<'js>,
    ) -> Result<(), PluginError> {
        let engine = PluginError::Engine;
        let plugin_id = self.plugin_id.clone();
        let message_obj = rquickjs::Object::new(ctx.clone()).map_err(|e| engine(e.to_string()))?;
        for level in ["info", "warn", "error"] {
            let calls = self.calls.clone();
            let plugin_for_call = plugin_id.clone();
            let f = rquickjs::Function::new(ctx.clone(), move |text: String| {
                // FR-027: payload text goes to the sink, never to the log.
                tracing::trace!(plugin = %plugin_for_call, level, "host API call: message");
                calls.push_message(level, text);
            })
            .map_err(|e| engine(e.to_string()))?;
            message_obj
                .set(level, f)
                .map_err(|e| engine(e.to_string()))?;
        }
        ragent
            .set("message", message_obj)
            .map_err(|e| engine(e.to_string()))?;
        Ok(())
    }

    /// `ragent.log(level, text)`.
    fn install_log<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        ragent: &rquickjs::Object<'js>,
    ) -> Result<(), PluginError> {
        let engine = PluginError::Engine;
        let calls = self.calls.clone();
        let plugin_id = self.plugin_id.clone();
        let log_fn = rquickjs::Function::new(ctx.clone(), move |level: String, text: String| {
            tracing::trace!(plugin = %plugin_id, level, "host API call: log");
            calls.push_log(&level, text);
        })
        .map_err(|e| engine(e.to_string()))?;
        ragent
            .set("log", log_fn)
            .map_err(|e| engine(e.to_string()))?;
        Ok(())
    }

    /// `ragent.register_tool(def)` — records the declaration in the host
    /// sink and stashes the handler (when `def.handler` is a function) into
    /// the sandbox's `__ragent_tools` map so [`crate::tool_adapter`] can
    /// invoke it at dispatch time (T-010, FR-005). The handler value itself
    /// never crosses the boundary as JSON; only name/description/parameters
    /// are extracted for the declaration.
    fn install_register_tool<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        ragent: &rquickjs::Object<'js>,
    ) -> Result<(), PluginError> {
        let engine = PluginError::Engine;
        let calls = self.calls.clone();
        let plugin_id = self.plugin_id.clone();
        let f = rquickjs::Function::new(ctx.clone(), move |def: JsValue<'js>| {
            tracing::trace!(plugin = %plugin_id, "host API call: register_tool");
            let decl = tool_decl_from_js(&def);
            // Stash the declared handler for tool dispatch (T-010): the
            // adapter's wrapper looks up `__ragent_tools[name]`. A def
            // without a function handler still records the declaration —
            // dispatch then fails with a "no handler registered" error instead
            // of a panic (FR-026).
            // Handler capture goes through the sandbox's own `eval` so the
            // function's JS references stay valid for later dispatch; reading
            // the `handler` property through `Object::get` here produced a
            // value QuickJS could not resolve once the registration call was
            // suspended (the stash silently never landed).
            let assign = format!(
                "(function(d){{ \
                   if (d && typeof d.handler === 'function') {{ \
                     const name = String(d.name || ''); \
                     if (name) {{ \
                       const m = globalThis.__ragent_tools || \
                         (globalThis.__ragent_tools = {{}}); \
                       m[name] = d.handler; \
                     }} \
                   }} \
                 }})({})",
                value_to_scratch(&def)
            );
            let eval_ctx = def.ctx().clone();
            let _ = eval_ctx.eval::<rquickjs::Value<'_>, _>(assign);
            calls.push_tool(decl);
        })
        .map_err(|e| engine(e.to_string()))?;
        ragent
            .set("register_tool", f)
            .map_err(|e| engine(e.to_string()))?;
        Ok(())
    }

    /// `ragent.register_command(def)` — records the declaration in the host
    /// sink and stashes the handler (when `def.handler` is a function) into
    /// the sandbox's `__ragent_commands` map so [`crate::command_adapter`] can
    /// invoke it at dispatch time (T-012, FR-004). Uses the same
    /// sandbox-internal eval pattern as `register_tool`: reading the `handler`
    /// property through `Object::get` here produced a value QuickJS could not
    /// resolve once the registration call was suspended.
    fn install_register_command<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        ragent: &rquickjs::Object<'js>,
    ) -> Result<(), PluginError> {
        let engine = PluginError::Engine;
        let calls = self.calls.clone();
        let plugin_id = self.plugin_id.clone();
        let f = rquickjs::Function::new(ctx.clone(), move |def: JsValue<'js>| {
            tracing::trace!(plugin = %plugin_id, "host API call: register_command");
            let assign = format!(
                "(function(d){{ \
                   if (d && typeof d.handler === 'function') {{ \
                     const name = String(d.name || ''); \
                     if (name) {{ \
                       const m = globalThis.__ragent_commands || \
                         (globalThis.__ragent_commands = {{}}); \
                       m[name] = d.handler; \
                     }} \
                   }} \
                 }})({})",
                value_to_scratch(&def)
            );
            let eval_ctx = def.ctx().clone();
            let _ = eval_ctx.eval::<rquickjs::Value<'_>, _>(assign);
            calls.push_command(command_decl_from_js(&def));
        })
        .map_err(|e| engine(e.to_string()))?;
        ragent
            .set("register_command", f)
            .map_err(|e| engine(e.to_string()))?;
        Ok(())
    }

    /// `ragent.plugin.read_text_file(relative)` restricted to `plugin_root`.
    fn install_plugin_reader<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        ragent: &rquickjs::Object<'js>,
    ) -> Result<(), PluginError> {
        let engine = PluginError::Engine;
        let plugin_obj = rquickjs::Object::new(ctx.clone()).map_err(|e| engine(e.to_string()))?;
        let root = self.plugin_root.clone();
        // Err surfaces to the plugin as a thrown exception carrying the
        // refusal / IO message (FR-018: escape attempts are visible to
        // plugin code as errors).
        let read_fn = rquickjs::Function::new(ctx.clone(), move |relative: String| {
            read_within(&root, &relative).map_err(|message| {
                rquickjs::Error::new_from_js_message("plugin", "read_text_file", message)
            })
        })
        .map_err(|e| engine(e.to_string()))?;
        plugin_obj
            .set("read_text_file", read_fn)
            .map_err(|e| engine(e.to_string()))?;
        ragent
            .set("plugin", plugin_obj)
            .map_err(|e| engine(e.to_string()))?;
        Ok(())
    }
}

/// Extract a [`PluginToolDecl`] from a JS registration object.
fn tool_decl_from_js(def: &JsValue<'_>) -> PluginToolDecl {
    let declaration = value_to_json(def);
    PluginToolDecl {
        name: string_field(&declaration, "name"),
        description: string_field(&declaration, "description"),
        parameters: declaration
            .get("parameters")
            .cloned()
            .unwrap_or(JsonValue::Null),
    }
}

/// Extract a [`PluginCommandDecl`] from a JS registration object.
fn command_decl_from_js(def: &JsValue<'_>) -> PluginCommandDecl {
    let declaration = value_to_json(def);
    PluginCommandDecl {
        name: string_field(&declaration, "name"),
        description: string_field(&declaration, "description"),
        usage: declaration
            .get("usage")
            .and_then(JsonValue::as_str)
            .map(str::to_string),
    }
}

fn string_field(declaration: &JsonValue, field: &str) -> String {
    declaration
        .get(field)
        .and_then(JsonValue::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Convert an rquickjs value to `serde_json::Value` via in-sandbox
/// `JSON.stringify` (host API v1 strings-only boundary).
fn value_to_json(value: &JsValue<'_>) -> JsonValue {
    let ctx = value.ctx().clone();
    let text: Result<String, _> = ctx.eval(format!(
        "JSON.stringify({})",
        // Store the value under a scratch global so stringify can reference it
        // without interpolation of arbitrary JS text.
        value_to_scratch(value)
    ));
    text.ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(JsonValue::Null)
}

/// Stash a value under a scratch global name and return the expression.
fn value_to_scratch(value: &JsValue<'_>) -> String {
    let ctx = value.ctx().clone();
    let globals = ctx.globals();
    let _ = globals.set("__ragent_host_scratch", value.clone());
    "__ragent_host_scratch".to_string()
}

/// Read a text file inside the plugin root, refusing escapes (`..` segments
/// or absolute paths). Used by `ragent.plugin.read_text_file` (FR-018).
///
/// Returns `Err(message)` on refusal; the JavaScript wrapper throws that
/// message so plugin code sees an exception for escape attempts rather than
/// silently reading nothing (FR-025 reporting style).
pub fn read_within(root: &std::path::Path, relative: &str) -> Result<String, String> {
    use std::path::Component;
    let rel = std::path::Path::new(relative);
    if rel.is_absolute() {
        return Err(format!(
            "plugin.read_text_file: absolute path refused: {relative}"
        ));
    }
    for component in rel.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(format!(
                "plugin.read_text_file: path escapes the plugin directory: {relative}"
            ));
        }
    }
    std::fs::read_to_string(root.join(rel))
        .map_err(|e| format!("plugin.read_text_file: {relative}: {e}"))
}
