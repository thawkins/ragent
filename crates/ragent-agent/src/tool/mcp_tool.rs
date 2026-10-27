//! Dynamic tool wrapper for MCP server tools.
//!
//! Adapts tools advertised by MCP servers into the ragent [`Tool`] trait,
//! so agents can invoke them transparently alongside built-in tools.

use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::mcp::{McpClient, McpToolDef};

use super::{Tool, ToolContext, ToolOutput};

/// Wraps a single MCP server tool as a ragent [`Tool`].
///
/// Delegates `execute()` to `McpClient::call_tool()` using the stored
/// server ID and tool name.
pub struct McpToolWrapper {
    /// The MCP server this tool belongs to.
    pub server_id: String,
    /// Tool name as reported by the MCP server.
    pub tool_name: String,
    /// Full tool identifier used as the ragent tool name: `mcp_{server}_{tool}`.
    pub ragent_name: String,
    /// Human-readable description from the MCP server.
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub input_schema: Value,
    /// Shared MCP client handle.
    pub client: Arc<RwLock<McpClient>>,
    /// Fallback config used for the enable check when the client no longer
    /// holds a record for this server (the tool is still registered from a
    /// previous connection). Always `disabled: false`, so the check then
    /// defers entirely to the global enable ledger.
    config_probe: ragent_config::McpServerConfig,
}

impl McpToolWrapper {
    /// The registry name (`mcp_<server>_<tool>`) an MCP tool is registered
    /// under, with `-`, `.` and `/` replaced by `_` in both segments.
    ///
    /// This is the single source of truth for the mangling: the TUI's `/mcp`
    /// surfaces and the registry reconcile pass must derive the same name
    /// without constructing a wrapper (which clones the schema and the client
    /// handle just to reach [`Self::ragent_name`]). The derivation itself lives
    /// in [`ragent_types::tool_bridge`], so the name FR-032 checks and the name
    /// this wrapper registers cannot disagree.
    #[must_use]
    pub fn ragent_name_for(server_id: &str, tool_name: &str) -> String {
        ragent_types::tool_bridge::mcp_tool_name(server_id, tool_name)
    }

    /// The registry-name prefix (`mcp_<server>_`) every tool of `server_id`
    /// carries.
    ///
    /// The TUI's stale-server scan matches registered names against this prefix
    /// and [`ragent_types::tool_bridge::check_dynamic_bridge`] demands it, so
    /// both read one derivation. Before this existed the scan reached the prefix
    /// through `ragent_name_for(server_id, "")`, which is a registry name that
    /// must never be registered.
    #[must_use]
    pub fn ragent_prefix_for(server_id: &str) -> String {
        ragent_types::tool_bridge::mcp_tool_prefix(server_id)
    }

    /// Create a new wrapper for a specific MCP tool.
    pub fn new(
        server_id: &str,
        tool_name: &str,
        description: &str,
        input_schema: Value,
        client: Arc<RwLock<McpClient>>,
    ) -> Self {
        Self {
            server_id: server_id.to_string(),
            tool_name: tool_name.to_string(),
            ragent_name: Self::ragent_name_for(server_id, tool_name),
            description: description.to_string(),
            input_schema,
            client,
            config_probe: ragent_config::McpServerConfig::default(),
        }
    }
}

#[async_trait::async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.ragent_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        let mut schema = self.input_schema.clone();
        if let Some(obj) = schema.as_object_mut() {
            if obj.contains_key("properties") {
                obj.entry("additionalProperties")
                    .or_insert_with(|| serde_json::json!(false));
            }
        }
        schema
    }

    fn permission_category(&self) -> &'static str {
        "mcp"
    }

    fn mcp_wrapper_info(&self) -> Option<(&str, &str)> {
        Some((&self.server_id, &self.tool_name))
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        // Read the ledger before taking the client lock: the check is a file
        // read, and tool calls run in a loop, so holding the client lock across
        // disk I/O would serialise unrelated MCP calls behind it.
        let ledger = crate::mcp::McpEnableLedger::load();
        let client = self.client.read().await;
        // A server switched off in the global enable ledger keeps its tool
        // registration until the next restart (`set_mcp_client` registers once),
        // so refuse to call it here rather than reaching a server the user has
        // disabled. The refusal names the fix.
        if !crate::mcp::enable_state::is_server_enabled(
            client
                .servers()
                .iter()
                .find(|s| s.id == self.server_id)
                .map_or(&self.config_probe, |s| &s.config),
            &ledger,
            &self.server_id,
        ) {
            anyhow::bail!(
                "MCP server `{}` is disabled; enable it with `/mcp connect {}` \
                 (or set `mcp.{}.disabled = false` in ragent.json) and restart",
                self.server_id,
                self.server_id,
                self.server_id,
            );
        }
        let result = client
            .call_tool(&self.server_id, &self.tool_name, input)
            .await?;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
        Ok(ToolOutput {
            content,
            metadata: None,
        })
    }
}

/// The outcome of bridging one MCP server's advertised tools (FR-032).
///
/// Containment is structural rather than a convention: a tool that FR-032 refuses
/// simply has no entry in [`wrappers`](Self::wrappers), so a caller cannot
/// accidentally register it alongside its siblings.
pub struct BridgedServerTools {
    /// One wrapper per tool that may be registered, in advertisement order.
    pub wrappers: Vec<McpToolWrapper>,
    /// One refusal per tool that may not be, in advertisement order.
    pub refusals: Vec<ragent_types::tool_bridge::BridgeRefusal>,
}

/// Bridges one MCP server's advertised tools into ragent wrappers, refusing any
/// tool whose surfaced name breaks FR-032 and reporting each refusal once.
///
/// This is the single MCP registration path: [`set_mcp_client`] and the TUI's
/// `register_mcp_tools` both call it, so a connector's servers - which reach the
/// registry as the bridged `<connector-id>.<server-id>` and therefore arrive here
/// under a dotted id - are covered by the same rule.
///
/// A refused tool costs exactly one tool: the rest of the server still bridges and
/// the session stays usable. Refusals are logged the first time this server offers
/// them and silently afterwards.
///
/// [`set_mcp_client`]: crate::session::SessionProcessor::set_mcp_client
pub fn bridge_server_tools(
    server_id: &str,
    tools: &[McpToolDef],
    client: Arc<RwLock<McpClient>>,
) -> BridgedServerTools {
    bridge_server_tools_named(server_id, tools, client, McpToolWrapper::ragent_name_for)
}

/// [`bridge_server_tools`] with the name mangler supplied by the caller.
///
/// With the real mangler every surfaced name is `mcp_<server>_<tool>`, so FR-032
/// can never refuse and the containment property would be untestable. Taking the
/// function as a parameter lets a test drive the regression the backstop exists
/// for - a call site that stopped namespacing - and observe that it costs one
/// tool rather than the server.
#[doc(hidden)]
pub fn bridge_server_tools_named(
    server_id: &str,
    tools: &[McpToolDef],
    client: Arc<RwLock<McpClient>>,
    name_for: impl Fn(&str, &str) -> String,
) -> BridgedServerTools {
    let source = ragent_types::tool_bridge::mcp_source_label(server_id);
    let prefix = McpToolWrapper::ragent_prefix_for(server_id);
    let mut wrappers = Vec::with_capacity(tools.len());
    let mut refusals = Vec::new();
    for tool in tools {
        let surfaced = name_for(server_id, &tool.name);
        if let Err(refusal) =
            ragent_types::tool_bridge::check_dynamic_bridge(&source, &prefix, &surfaced)
        {
            refusals.push(refusal);
            continue;
        }
        let mut wrapper = McpToolWrapper::new(
            server_id,
            &tool.name,
            &tool.description,
            tool.parameters.clone(),
            Arc::clone(&client),
        );
        // The wrapper registers the name the check just approved, not a second
        // derivation of it: otherwise the mangler and the check could disagree and
        // a refused name would still reach the registry.
        wrapper.ragent_name = surfaced;
        tracing::debug!(
            server_id = %server_id,
            tool = %tool.name,
            ragent_name = %wrapper.ragent_name,
            "Bridging MCP tool"
        );
        wrappers.push(wrapper);
    }
    ragent_types::tool_bridge::report_bridge_refusals_once(&refusals);
    BridgedServerTools { wrappers, refusals }
}
