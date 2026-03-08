use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::McpServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpStatus {
    Connected,
    Disabled,
    Failed { error: String },
    NeedsAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    pub config: McpServerConfig,
    pub status: McpStatus,
    pub tools: Vec<McpToolDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Stub MCP client for future implementation.
pub struct McpClient {
    servers: Vec<McpServer>,
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    /// Connect to an MCP server (stub — returns Ok immediately).
    pub async fn connect(&mut self, id: &str, config: McpServerConfig) -> anyhow::Result<()> {
        let server = McpServer {
            id: id.to_string(),
            config,
            status: McpStatus::Disabled,
            tools: Vec::new(),
        };
        self.servers.push(server);
        tracing::info!("MCP server '{}' registered (stub)", id);
        Ok(())
    }

    /// List tools from all connected servers (stub — returns empty).
    pub fn list_tools(&self) -> Vec<McpToolDef> {
        self.servers
            .iter()
            .flat_map(|s| s.tools.iter().cloned())
            .collect()
    }

    /// Call a tool on an MCP server (stub — returns empty result).
    pub async fn call_tool(
        &self,
        _server_id: &str,
        _tool_name: &str,
        _input: Value,
    ) -> anyhow::Result<Value> {
        tracing::warn!("MCP call_tool is a stub, returning empty object");
        Ok(serde_json::json!({}))
    }

    /// Get all registered servers and their statuses.
    pub fn servers(&self) -> &[McpServer] {
        &self.servers
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}
