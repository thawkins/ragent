//! Shared isolated MCP probe helper (T-309).
//!
//! The CLI (`src/connectors.rs`) and TUI (`crates/ragent-tui/src/app/connector.rs`)
//! `/connectors test` harnesses both connect a brand-new, throwaway [`McpClient`]
//! to one server and list the tools it advertises. That connect-and-list step had
//! drifted into two byte-identical copies; it now has a single implementation
//! here, and each surface maps the result onto its own `ProbeTool`.

use ragent_config::McpServerConfig;
use serde_json::Value;

use super::McpClient;

/// Connect a throwaway [`McpClient`] to `server_id` and return the tools it
/// advertised as `(name, input_schema)` pairs.
///
/// The throwaway client is dropped before returning, so nothing the probe
/// started survives the call - the harness cannot observe or mutate the live
/// session (FR-015).
///
/// # Errors
///
/// Returns the connection failure cause when the server cannot be reached.
pub async fn connect_and_list(
    server_id: &str,
    config: McpServerConfig,
) -> Result<Vec<(String, Value)>, String> {
    let mut client = McpClient::new();
    client
        .connect(server_id, config)
        .await
        .map_err(|error| format!("{error:#}"))?;
    let tools = client
        .servers()
        .iter()
        .find(|server| server.id == server_id)
        .map(|server| {
            server
                .tools
                .iter()
                .map(|tool| (tool.name.clone(), tool.parameters.clone()))
                .collect()
        })
        .unwrap_or_default();
    Ok(tools)
}
