//! Dynamic tool wrapper for MCP server tools.
//!
//! Adapts tools advertised by MCP servers into the ragent [`Tool`] trait,
//! so agents can invoke them transparently alongside built-in tools.

use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::RwLock;

use crate::mcp::McpClient;

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
    /// handle just to reach [`Self::ragent_name`]).
    #[must_use]
    pub fn ragent_name_for(server_id: &str, tool_name: &str) -> String {
        let safe_server = server_id.replace(['-', '.', '/'], "_");
        let safe_tool = tool_name.replace(['-', '.', '/'], "_");
        format!("mcp_{safe_server}_{safe_tool}")
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
