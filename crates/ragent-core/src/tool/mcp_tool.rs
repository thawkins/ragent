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
}

impl McpToolWrapper {
    /// Create a new wrapper for a specific MCP tool.
    pub fn new(
        server_id: &str,
        tool_name: &str,
        description: &str,
        input_schema: Value,
        client: Arc<RwLock<McpClient>>,
    ) -> Self {
        let safe_server = server_id.replace(['-', '.', '/'], "_");
        let safe_tool = tool_name.replace(['-', '.', '/'], "_");
        Self {
            server_id: server_id.to_string(),
            tool_name: tool_name.to_string(),
            ragent_name: format!("mcp_{safe_server}_{safe_tool}"),
            description: description.to_string(),
            input_schema,
            client,
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
        self.input_schema.clone()
    }

    fn permission_category(&self) -> &str {
        "mcp"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let client = self.client.read().await;
        let result = client.call_tool(&self.server_id, &self.tool_name, input).await?;
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
        Ok(ToolOutput {
            content,
            metadata: None,
        })
    }
}
