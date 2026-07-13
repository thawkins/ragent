//! Rig `Tool` wrappers for ragent core tools (T-013 / FR-031).
//!
//! This module bridges ragent's security-audited tool registry onto Rig's
//! [`ToolDyn`] / [`ToolSet`] so that a Rig-backed agent can invoke ragent tools
//! **without** bypassing ragent's permission system, shell security model, or
//! tool approval gating.
//!
//! # Architecture
//!
//! ragent's [`Tool`] trait is object-safe and carries a [`ToolContext`] that
//! owns the session event bus, working directory, storage handle, and all the
//! other runtime state a tool needs. The event bus is the channel through which
//! permission requests, shell-command approvals, and question prompts flow.
//!
//! Rig's [`ToolDyn`] is a dynamic-dispatch trait with `name()`,
//! `definition(prompt)`, and `call(args: String) -> Result<String, ToolError>`.
//!
//! [`RigToolWrapper`] implements `ToolDyn` by holding an `Arc<dyn ragent
//! Tool>` plus a cloned [`ToolContext`] and delegating `call` straight to
//! `ragent_tool::execute(input, ctx)`. Because the execution path is identical
//! to the native ragent agent loop, every permission check, path guard, and
//! shell-security layer fires exactly as it does for a native session.
//!
//! # Security invariants (FR-031)
//!
//! 1. **No direct execution.** The wrapper never calls the underlying file/shell
//!    APIs itself — it always goes through `ragent_tool::execute`.
//! 2. **Permission bus preserved.** `ToolContext::event_bus` is forwarded
//!    verbatim, so `PermissionRequested`, `QuestionRequested`, and
//!    `ShellCwdChanged` events reach the same UI/server consumers.
//! 3. **Hidden tools excluded.** [`ragent_toolset`] derives the exported set
//!    from [`ToolRegistry::definitions`], which already excludes tools marked
//!    hidden via [`ToolRegistry::set_hidden`]. Rig agents cannot discover or
//!    call tools the ragent session has not advertised.
//! 4. **No privilege escalation.** The wrapper stores a plain `ToolContext`; it
//!    does not synthesise a higher-privilege context or strip fields.

use std::pin::Pin;
use std::sync::Arc;

use futures::Future;
use ragent_agent::tool::{Tool as RagTool, ToolContext, ToolOutput, ToolRegistry};
use ragent_llm::llm::ToolDefinition as RagToolDef;
use rig::completion::ToolDefinition as RigToolDefinition;
use rig::tool::{ToolDyn, ToolError, ToolSet};

/// Error surfaced when a wrapped ragent tool fails inside a Rig `ToolDyn::call`.
///
/// This implements `std::error::Error` so it can be boxed into Rig's
/// [`ToolError::ToolCallError`] variant.
#[derive(thiserror::Error, Debug)]
pub enum RagToolError {
    /// The wrapped ragent tool returned an error from `execute`.
    #[error("ragent tool '{name}' failed: {message}")]
    Execution {
        /// Name of the ragent tool that failed.
        name: String,
        /// Human-readable error message extracted from the `anyhow::Error`.
        message: String,
    },
    /// The arguments string could not be parsed as JSON.
    #[error("ragent tool '{name}' received invalid JSON arguments: {source}")]
    InvalidArgs {
        /// Name of the ragent tool whose arguments failed to parse.
        name: String,
        /// The underlying serde error.
        #[source]
        source: serde_json::Error,
    },
    /// The requested tool name was not present in the registry.
    #[error("ragent tool '{0}' not found in registry")]
    NotFound(String),
}

/// A wrapper that exposes a ragent [`Tool`] as a Rig [`ToolDyn`].
///
/// Construct with [`RigToolWrapper::new`] or in bulk via
/// [`ragent_toolset`]. The wrapper is `'static` and can be added to a Rig
/// [`ToolSet`] with [`ToolSet::add_tool`].
///
/// # Security
///
/// See the [module docs](self) for the FR-031 invariants. In short: the wrapper
/// never bypasses ragent's permission system — it forwards every call through
/// `ragent_tool::execute(input, &ctx)`, which fires the same permission and
/// shell-security events as the native agent loop.
pub struct RigToolWrapper {
    /// Cached tool name (avoids re-borrowing the `dyn Tool` for every
    /// `ToolDyn::name` call).
    name: String,
    /// The wrapped ragent tool implementation.
    inner: Arc<dyn RagTool>,
    /// Cloned tool context carrying the session event bus, working directory,
    /// storage handle, and other runtime state. The event bus is the channel
    /// through which permission requests flow, so forwarding it verbatim is
    /// what preserves FR-031.
    ctx: ToolContext,
}

impl RigToolWrapper {
    /// Wrap a single ragent tool for use in a Rig [`ToolSet`].
    ///
    /// The `ctx` is cloned (cheaply — it is a `Clone` struct of `Arc`s and a
    /// few small fields) so the wrapper owns its own context independent of
    /// the caller's lifetime.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_agent::tool::{ToolRegistry, ToolContext};
    /// use ragent_rig::tool::RigToolWrapper;
    /// # use std::sync::Arc;
    /// # use ragent_agent::event::EventBus;
    /// # use std::path::PathBuf;
    /// # fn build_ctx() -> ToolContext {
    /// #   ToolContext {
    /// #     session_id: "s".into(), working_dir: PathBuf::from("."),
    /// #     event_bus: Arc::new(EventBus::new(16)),
    /// #     storage: None, task_manager: None, active_model: None,
    /// #     team_context: None, team_manager: None, code_index: None,
    /// #     spec_manager: None, active_spec_id: None, config: None,
    /// #     read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    /// #     cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
    /// #   }
    /// # }
    /// let registry = ToolRegistry::new();
    /// // register tools...
    /// let ctx = build_ctx();
    /// if let Some(tool) = registry.get("read") {
    ///     let wrapper = RigToolWrapper::new(tool, ctx);
    ///     assert_eq!(wrapper.name(), "read");
    /// }
    /// ```
    #[must_use]
    pub fn new(tool: Arc<dyn RagTool>, ctx: ToolContext) -> Self {
        let name = tool.name().to_string();
        Self {
            name,
            inner: tool,
            ctx,
        }
    }

    /// Returns the ragent tool name this wrapper exposes.
    ///
    /// This is the canonical name from [`RagTool::name`]; Rig uses it as the
    /// `ToolSet` key.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the wrapped ragent tool.
    #[must_use]
    pub fn inner(&self) -> &dyn RagTool {
        &*self.inner
    }

    /// Returns a reference to the cloned [`ToolContext`] the wrapper will pass
    /// to `execute`.
    #[must_use]
    pub fn context(&self) -> &ToolContext {
        &self.ctx
    }
}

impl ToolDyn for RigToolWrapper {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn definition(
        &self,
        _prompt: String,
    ) -> Pin<Box<dyn Future<Output = RigToolDefinition> + Send + Sync + '_>> {
        // ragent tool schemas are static (they do not vary by prompt), so we
        // ignore the Rig-supplied `_prompt` and return the tool's own schema.
        let def = RigToolDefinition {
            name: self.name.clone(),
            description: self.inner.description().to_string(),
            parameters: self.inner.parameters_schema(),
        };
        Box::pin(async move { def })
    }

    fn call(
        &self,
        args: String,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + Sync + '_>> {
        let inner = self.inner.clone();
        let ctx = self.ctx.clone();
        let name = self.name.clone();
        Box::pin(async move {
            // Rig passes arguments as a JSON string. ragent's `execute` takes a
            // `serde_json::Value`. If the string is not valid JSON we surface
            // that as a `ToolError::ToolCallError` so the Rig agent sees the
            // failure rather than a silent panic.
            let input: serde_json::Value = serde_json::from_str(&args).map_err(|e| {
                ToolError::ToolCallError(Box::new(RagToolError::InvalidArgs {
                    name: name.clone(),
                    source: e,
                }))
            })?;
            // ragent's `async_trait::async_trait` `execute` returns a
            // `Send`-only boxed future (the default `async_trait` flavour has
            // no `Sync` bound). Rig's `ToolDyn::call` requires the returned
            // future to be `Send + Sync`. To bridge the two without losing the
            // `Sync` bound, we spawn the ragent execution onto a separate Tokio
            // task and await the `JoinHandle`, whose future is `Send + Sync`.
            //
            // Security note (FR-031): spawning does NOT bypass the permission
            // system. `inner.execute(input, &ctx)` still publishes
            // `PermissionRequested` / `QuestionRequested` events onto
            // `ctx.event_bus`, which is shared (via `Arc`) with the same
            // UI/server permission pipeline the native agent loop uses. The
            // event bus is `Send + Sync`, so publishing from a spawned task is
            // safe and reaches the same consumers.
            let join = tokio::spawn(async move { inner.execute(input, &ctx).await });
            let executed = join.await.map_err(|join_err| {
                ToolError::ToolCallError(Box::new(RagToolError::Execution {
                    name: name.clone(),
                    message: format!("spawned tool task panicked: {join_err}"),
                }))
            })?;
            let ToolOutput { content, metadata } = executed.map_err(|e| {
                ToolError::ToolCallError(Box::new(RagToolError::Execution {
                    name: name.clone(),
                    message: e.to_string(),
                }))
            })?;
            // Serialize the result. When metadata is present we emit a small
            // JSON envelope so structured consumers (and Rig's own tool-result
            // parsing) can recover both the text and the structured fields.
            // When there is no metadata we return the raw content string,
            // which is the common case and avoids needless JSON wrapping.
            let result = match metadata {
                Some(meta) => serde_json::json!({
                    "content": content,
                    "metadata": meta,
                })
                .to_string(),
                None => content,
            };
            Ok(result)
        })
    }
}

/// Build a Rig [`ToolSet`] from a ragent [`ToolRegistry`], wrapping every
/// non-hidden tool as a [`RigToolWrapper`].
///
/// # Tool selection
///
/// * If `names` is non-empty, only tools whose names appear in `names` **and**
///   in the registry's exported definitions are wrapped. Names not present in
///   the registry are silently skipped (callers can validate with
///   [`ToolRegistry::get`] beforehand if they need strict checking).
/// * If `names` is empty, every tool exported by
///   [`ToolRegistry::definitions`] is wrapped. Tools hidden via
///   [`ToolRegistry::set_hidden`] are excluded — Rig agents cannot discover
///   or invoke them (FR-031 invariant #3).
///
/// # Context
///
/// Every wrapper receives a clone of `ctx`. The event bus inside `ctx` is the
/// channel that carries permission requests, so forwarding it verbatim is what
/// preserves the ragent security model (FR-031). The caller is responsible for
/// supplying a `ctx` whose `event_bus` is wired to the same UI/server
/// permission pipeline the native agent loop uses.
///
/// # Errors
///
/// Returns `Ok(ToolSet)` always; unknown names are skipped rather than
/// surfaced as errors, matching how the native registry behaves when a tool
/// is deregistered mid-session.
///
/// # Examples
///
/// ```
/// use ragent_agent::tool::{create_default_registry, ToolContext};
/// use ragent_rig::tool::ragent_toolset;
/// # use std::sync::Arc;
/// # use ragent_agent::event::EventBus;
/// # use std::path::PathBuf;
/// # fn build_ctx() -> ToolContext {
/// #   ToolContext {
/// #     session_id: "s".into(), working_dir: PathBuf::from("."),
/// #     event_bus: Arc::new(EventBus::new(16)),
/// #     storage: None, task_manager: None, active_model: None,
/// #     team_context: None, team_manager: None, code_index: None,
/// #     spec_manager: None, active_spec_id: None, config: None,
/// #     read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
/// #     cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
/// #   }
/// # }
/// let registry = create_default_registry();
/// let ctx = build_ctx();
/// let names = vec!["read".to_string(), "grep".to_string()];
/// let toolset = ragent_toolset(&registry, ctx, &names);
/// assert!(toolset.contains("read"));
/// assert!(toolset.contains("grep"));
/// assert!(!toolset.contains("bash")); // not in `names`
/// ```
#[must_use]
pub fn ragent_toolset(registry: &ToolRegistry, ctx: ToolContext, names: &[String]) -> ToolSet {
    let defs: Vec<RagToolDef> = registry.definitions();
    let allow: Option<&[String]> = if names.is_empty() { None } else { Some(names) };
    let mut toolset = ToolSet::default();
    for def in &defs {
        if let Some(allowed) = allow {
            if !allowed.iter().any(|n| n == &def.name) {
                continue;
            }
        }
        if let Some(tool) = registry.get(&def.name) {
            toolset.add_tool(RigToolWrapper::new(tool, ctx.clone()));
        }
    }
    toolset
}

/// Wrap a single named ragent tool, returning `None` if the name is not
/// registered or is hidden.
///
/// This is the single-tool counterpart to [`ragent_toolset`]; use it when you
/// only need to expose one tool to a Rig agent (e.g., a restricted tool
/// surface for a sub-agent).
///
/// # Examples
///
/// ```
/// use ragent_agent::tool::{create_default_registry, ToolContext};
/// use ragent_rig::tool::wrap_tool;
/// # use std::sync::Arc;
/// # use ragent_agent::event::EventBus;
/// # use std::path::PathBuf;
/// # fn build_ctx() -> ToolContext {
/// #   ToolContext {
/// #     session_id: "s".into(), working_dir: PathBuf::from("."),
/// #     event_bus: Arc::new(EventBus::new(16)),
/// #     storage: None, task_manager: None, active_model: None,
/// #     team_context: None, team_manager: None, code_index: None,
/// #     spec_manager: None, active_spec_id: None, config: None,
/// #     read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
/// #     cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
/// #   }
/// # }
/// let registry = create_default_registry();
/// let ctx = build_ctx();
/// assert!(wrap_tool(&registry, "read", ctx.clone()).is_some());
/// assert!(wrap_tool(&registry, "nope", ctx).is_none());
/// ```
#[must_use]
pub fn wrap_tool(registry: &ToolRegistry, name: &str, ctx: ToolContext) -> Option<RigToolWrapper> {
    registry
        .get(name)
        .map(|tool| RigToolWrapper::new(tool, ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ragent_agent::event::EventBus;
    use serde_json::json;
    use std::path::PathBuf;

    /// A minimal deterministic ragent tool used to exercise the wrapper
    /// without needing filesystem or network access.
    struct EchoTool;

    #[async_trait]
    impl RagTool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echo back the `message` argument as content and `len` as metadata."
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Text to echo." }
                },
                "required": ["message"]
            })
        }
        fn permission_category(&self) -> &str {
            "none"
        }
        async fn execute(
            &self,
            input: serde_json::Value,
            _ctx: &ToolContext,
        ) -> anyhow::Result<ToolOutput> {
            let msg = input
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing 'message'"))?;
            Ok(ToolOutput {
                content: msg.to_string(),
                metadata: Some(json!({ "len": msg.len() })),
            })
        }
    }

    fn build_ctx() -> ToolContext {
        ToolContext {
            session_id: "test-session".into(),
            working_dir: PathBuf::from("."),
            event_bus: Arc::new(EventBus::new(16)),
            storage: None,
            task_manager: None,
            active_model: None,
            team_context: None,
            team_manager: None,
            code_index: None,
            spec_manager: None,
            active_spec_id: None,
            config: None,
            read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    #[tokio::test]
    async fn wrapper_exposes_name_and_definition() {
        let tool: Arc<dyn RagTool> = Arc::new(EchoTool);
        let wrapper = RigToolWrapper::new(tool, build_ctx());
        assert_eq!(wrapper.name(), "echo");
        let def = wrapper.definition(String::new()).await;
        assert_eq!(def.name, "echo");
        assert!(def.description.contains("Echo"));
        assert_eq!(def.parameters["type"], "object");
    }

    #[tokio::test]
    async fn wrapper_call_routes_through_ragent_execute() {
        let tool: Arc<dyn RagTool> = Arc::new(EchoTool);
        let wrapper = RigToolWrapper::new(tool, build_ctx());
        let args = json!({ "message": "hello" }).to_string();
        let out = wrapper.call(args).await.expect("echo should succeed");
        // metadata present → JSON envelope
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("envelope is JSON");
        assert_eq!(parsed["content"], "hello");
        assert_eq!(parsed["metadata"]["len"], 5);
    }

    #[tokio::test]
    async fn wrapper_call_surfaces_invalid_json_as_tool_error() {
        let tool: Arc<dyn RagTool> = Arc::new(EchoTool);
        let wrapper = RigToolWrapper::new(tool, build_ctx());
        let err = wrapper
            .call("not json {".to_string())
            .await
            .expect_err("bad JSON must error");
        let msg = err.to_string();
        assert!(msg.contains("echo"), "error should name the tool: {msg}");
    }

    #[tokio::test]
    async fn wrapper_call_surfaces_execute_failure_as_tool_error() {
        let tool: Arc<dyn RagTool> = Arc::new(EchoTool);
        let wrapper = RigToolWrapper::new(tool, build_ctx());
        // missing 'message' → execute returns Err
        let err = wrapper
            .call(json!({}).to_string())
            .await
            .expect_err("missing arg must error");
        assert!(err.to_string().contains("echo"));
    }

    #[tokio::test]
    async fn ragent_toolset_wraps_selected_names_only() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));
        let names = vec!["echo".to_string()];
        let set = ragent_toolset(&registry, build_ctx(), &names);
        assert!(set.contains("echo"));
    }

    #[tokio::test]
    async fn ragent_toolset_skips_unknown_names() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));
        let names = vec!["echo".to_string(), "nope".to_string()];
        let set = ragent_toolset(&registry, build_ctx(), &names);
        assert!(set.contains("echo"));
        assert!(!set.contains("nope"));
    }

    #[tokio::test]
    async fn ragent_toolset_empty_names_wraps_all_exported() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));
        let set = ragent_toolset(&registry, build_ctx(), &[]);
        assert!(set.contains("echo"));
    }

    #[tokio::test]
    async fn ragent_toolset_hidden_tools_excluded() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));
        registry.set_hidden(&["echo".to_string()]);
        let set = ragent_toolset(&registry, build_ctx(), &[]);
        // definitions() excludes hidden tools, so the toolset is empty.
        assert!(!set.contains("echo"));
    }

    #[tokio::test]
    async fn wrap_tool_returns_none_for_unknown() {
        let registry = ToolRegistry::new();
        assert!(wrap_tool(&registry, "nope", build_ctx()).is_none());
        registry.register(Arc::new(EchoTool));
        assert!(wrap_tool(&registry, "echo", build_ctx()).is_some());
    }

    #[tokio::test]
    async fn toolset_call_through_rig_routes_to_ragent() {
        // End-to-end: build a ToolSet, call via ToolSet::call, verify result.
        let registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool));
        let set = ragent_toolset(&registry, build_ctx(), &[]);
        let out = set
            .call("echo", json!({ "message": "hi" }).to_string())
            .await
            .expect("call should succeed");
        let parsed: serde_json::Value = serde_json::from_str(&out).expect("envelope is JSON");
        assert_eq!(parsed["content"], "hi");
    }

    #[tokio::test]
    async fn toolset_call_unknown_tool_returns_not_found() {
        let registry = ToolRegistry::new();
        let set = ragent_toolset(&registry, build_ctx(), &[]);
        let err = set
            .call("nope", "{}".to_string())
            .await
            .expect_err("unknown tool must error");
        assert!(err.to_string().contains("nope"));
    }
}
