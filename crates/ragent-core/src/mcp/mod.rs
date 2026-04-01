//! Model Context Protocol (MCP) client and types.
//!
//! Provides [`McpClient`] for managing MCP server connections using the
//! official `rmcp` SDK. Supports stdio (child process) and HTTP transports.
//! After connecting, tools advertised by each server are discoverable via
//! [`McpClient::list_tools`] and invocable via [`McpClient::call_tool`].
//!
//! Use [`McpClient::discover`] to scan `PATH`, npm global packages, and
//! MCP registry directories for available servers (see [`discovery`] module).

pub mod discovery;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, Tool as RmcpTool};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::ConfigureCommandExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{RwLock, Semaphore};

use crate::config::{McpServerConfig, McpTransport};

pub use discovery::{DiscoveredMcpServer, McpDiscoverySource, discover as discover_servers};

// ── MCP config validation ────────────────────────────────────────────────────

/// Shell metacharacters that must not appear in MCP stdio command strings.
const SHELL_METACHARACTERS: &[char] = &['|', ';', '&', '$', '`', '(', ')', '{', '}', '<', '>'];

/// Maximum number of concurrent MCP server connections.
///
/// Prevents resource exhaustion from spawning too many child processes.
const MAX_CONCURRENT_MCP_CONNECTIONS: usize = 8;

/// Semaphore limiting concurrent MCP process spawns.
static MCP_SPAWN_SEMAPHORE: std::sync::LazyLock<Semaphore> =
    std::sync::LazyLock::new(|| Semaphore::new(MAX_CONCURRENT_MCP_CONNECTIONS));

/// Validate an MCP server configuration before connecting.
///
/// For **stdio** transports:
/// - The `command` field must be present and non-empty.
/// - If the command contains a path separator, the path must exist on disk.
/// - Shell metacharacters (`| ; & $ \` ( ) { } < >`) are rejected to prevent
///   command injection.
/// - Arguments are checked for shell metacharacters as well.
///
/// For **HTTP/SSE** transports:
/// - The `url` field must be present and begin with `http://` or `https://`.
///
/// # Errors
///
/// Returns a descriptive error if validation fails.
pub fn validate_mcp_config(id: &str, config: &McpServerConfig) -> anyhow::Result<()> {
    if crate::yolo::is_enabled() {
        tracing::warn!(id, "YOLO mode: skipping MCP config validation");
        return Ok(());
    }
    match config.type_ {
        McpTransport::Stdio => {
            let command_str = config.command.as_deref().ok_or_else(|| {
                anyhow::anyhow!("[{id}] stdio transport requires a 'command' field")
            })?;

            let trimmed = command_str.trim();
            if trimmed.is_empty() {
                anyhow::bail!("[{id}] stdio command must not be empty");
            }

            // Reject shell metacharacters in the command itself.
            if let Some(ch) = trimmed.chars().find(|c| SHELL_METACHARACTERS.contains(c)) {
                anyhow::bail!(
                    "[{id}] stdio command contains disallowed shell metacharacter '{ch}'"
                );
            }

            // If the command looks like a path, verify it exists.
            if trimmed.contains('/') || trimmed.contains('\\') {
                let path = Path::new(trimmed);
                if !path.exists() {
                    anyhow::bail!(
                        "[{id}] stdio command path '{}' does not exist",
                        path.display()
                    );
                }
            }

            // Validate arguments don't contain shell metacharacters.
            for (i, arg) in config.args.iter().enumerate() {
                if let Some(ch) = arg.chars().find(|c| SHELL_METACHARACTERS.contains(c)) {
                    anyhow::bail!(
                        "[{id}] stdio argument {i} contains disallowed shell metacharacter '{ch}'"
                    );
                }
            }

            tracing::info!(
                server_id = id,
                command = %crate::sanitize::redact_secrets(command_str),
                "MCP stdio config validated"
            );
        }
        McpTransport::Http | McpTransport::Sse => {
            let url = config.url.as_deref().ok_or_else(|| {
                anyhow::anyhow!("[{id}] HTTP/SSE transport requires a 'url' field")
            })?;

            let trimmed = url.trim();
            if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                anyhow::bail!(
                    "[{id}] HTTP/SSE url must start with http:// or https://, got: '{}'",
                    &trimmed[..trimmed.len().min(60)]
                );
            }

            tracing::info!(
                server_id = id,
                url = %crate::sanitize::redact_secrets(url),
                "MCP HTTP/SSE config validated"
            );
        }
    }

    Ok(())
}

/// Connection status of an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpStatus {
    /// The server is connected and operational.
    Connected,
    /// The server is registered but not connected.
    Disabled,
    /// The server failed to connect.
    Failed {
        /// Error message describing the failure.
        error: String,
    },
    /// The server requires authentication before connecting.
    NeedsAuth,
}

/// A registered MCP server with its configuration, connection status, and
/// advertised tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Unique identifier for this server.
    pub id: String,
    /// Configuration used to connect.
    pub config: McpServerConfig,
    /// Current connection status.
    pub status: McpStatus,
    /// Tools advertised by this server after connection.
    pub tools: Vec<McpToolDef>,
}

/// Definition of a tool exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    /// The tool name as registered with the MCP server.
    pub name: String,
    /// A human-readable description of the tool.
    pub description: String,
    /// JSON Schema defining the tool's expected input parameters.
    pub parameters: Value,
}

/// An active connection to a single MCP server, wrapping the rmcp
/// [`RunningService`].
struct McpConnection {
    service: RunningService<RoleClient, ()>,
}

/// MCP client managing connections to one or more MCP servers.
///
/// Uses the official `rmcp` SDK for transport, handshake, tool discovery,
/// and tool invocation.
pub struct McpClient {
    servers: Vec<McpServer>,
    connections: Arc<RwLock<HashMap<String, McpConnection>>>,
}

impl McpClient {
    /// Default timeout for tool calls in seconds.
    const TOOL_CALL_TIMEOUT_SECS: u64 = 120;

    /// Creates a new `McpClient` with no registered servers.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// assert!(client.servers().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Connect to an MCP server using the configured transport.
    ///
    /// For stdio servers, spawns a child process and communicates over
    /// stdin/stdout. For HTTP/SSE servers, connects to the configured URL.
    /// After the MCP `initialize` handshake completes, discovers available
    /// tools and populates the server's tool list.
    ///
    /// # Arguments
    ///
    /// * `id` — unique identifier for this server connection
    /// * `config` — transport and connection configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the transport cannot be established, the
    /// initialize handshake fails, or tool discovery fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # use ragent_core::config::McpServerConfig;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut client = McpClient::new();
    /// let config = McpServerConfig::default();
    /// client.connect("my-server", config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(&mut self, id: &str, config: McpServerConfig) -> anyhow::Result<()> {
        if config.disabled {
            let server = McpServer {
                id: id.to_string(),
                config,
                status: McpStatus::Disabled,
                tools: Vec::new(),
            };
            self.servers.push(server);
            tracing::info!(server_id = id, "MCP server registered as disabled");
            return Ok(());
        }

        // Validate config before attempting connection.
        validate_mcp_config(id, &config)?;

        // Acquire a spawn permit to limit concurrent MCP connections.
        let _permit = MCP_SPAWN_SEMAPHORE.acquire().await.map_err(|_| {
            anyhow::anyhow!("MCP spawn semaphore closed")
        })?;

        match self.connect_inner(id, &config).await {
            Ok((service, tools)) => {
                let tool_defs: Vec<McpToolDef> = tools
                    .iter()
                    .map(|t| McpToolDef {
                        name: t.name.to_string(),
                        description: t.description.as_deref().unwrap_or_default().to_string(),
                        parameters: serde_json::to_value(&*t.input_schema)
                            .unwrap_or(Value::Object(serde_json::Map::new())),
                    })
                    .collect();

                let tool_count = tool_defs.len();
                let server = McpServer {
                    id: id.to_string(),
                    config,
                    status: McpStatus::Connected,
                    tools: tool_defs,
                };
                self.servers.push(server);

                let mut conns = self.connections.write().await;
                conns.insert(id.to_string(), McpConnection { service });

                tracing::info!(
                    server_id = id,
                    tool_count,
                    "MCP server connected and tools discovered"
                );
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("{e:#}");
                let server = McpServer {
                    id: id.to_string(),
                    config,
                    status: McpStatus::Failed {
                        error: error_msg.clone(),
                    },
                    tools: Vec::new(),
                };
                self.servers.push(server);
                tracing::error!(
                    server_id = id,
                    error = %error_msg,
                    "MCP server connection failed"
                );
                Err(e)
            }
        }
    }

    /// Internal connection logic, separated for clean error handling.
    ///
    /// # Arguments
    ///
    /// * `id` — server identifier for logging
    /// * `config` — transport configuration
    ///
    /// # Returns
    ///
    /// The running service and discovered tools on success.
    async fn connect_inner(
        &self,
        id: &str,
        config: &McpServerConfig,
    ) -> anyhow::Result<(RunningService<RoleClient, ()>, Vec<RmcpTool>)> {
        let service =
            match config.type_ {
                McpTransport::Stdio => {
                    let command_str = config.command.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("stdio transport requires a 'command' field")
                    })?;

                    let args = config.args.clone();
                    let env = config.env.clone();

                    let transport = rmcp::transport::TokioChildProcess::new(
                        Command::new(command_str).configure(|cmd| {
                            for arg in &args {
                                cmd.arg(arg);
                            }
                            for (k, v) in &env {
                                cmd.env(k, v);
                            }
                        }),
                    )?;

                    tracing::info!(
                        server_id = id,
                        command = %crate::sanitize::redact_secrets(command_str),
                        "Spawning stdio MCP server"
                    );
                    ().serve(transport).await?
                }
                McpTransport::Http | McpTransport::Sse => {
                    let url = config.url.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("HTTP/SSE transport requires a 'url' field")
                    })?;

                    let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(url);

                    tracing::info!(server_id = id, url = %crate::sanitize::redact_secrets(url), "Connecting to HTTP MCP server");
                    ().serve(transport).await?
                }
            };

        let tools = service.peer().list_all_tools().await?;

        Ok((service, tools))
    }

    /// List tools from all connected servers (cached).
    ///
    /// Returns an aggregated list of tool definitions from every server
    /// that has status [`McpStatus::Connected`]. Uses the tool list
    /// discovered at connection time. Call [`Self::refresh_tools`] to
    /// re-query servers for updated tool manifests.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// let tools = client.list_tools();
    /// assert!(tools.is_empty());
    /// ```
    pub fn list_tools(&self) -> Vec<McpToolDef> {
        self.servers
            .iter()
            .filter(|s| s.status == McpStatus::Connected)
            .flat_map(|s| s.tools.iter().cloned())
            .collect()
    }

    /// List tools for a specific server by ID (cached).
    ///
    /// Returns the cached tool definitions for the given server, or an
    /// empty list if the server is not found or not connected.
    ///
    /// # Arguments
    ///
    /// * `server_id` — the ID of the server to query
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// let tools = client.list_tools_for_server("my-server");
    /// assert!(tools.is_empty());
    /// ```
    pub fn list_tools_for_server(&self, server_id: &str) -> Vec<McpToolDef> {
        self.servers
            .iter()
            .find(|s| s.id == server_id && s.status == McpStatus::Connected)
            .map(|s| s.tools.clone())
            .unwrap_or_default()
    }

    /// Re-query all connected servers for their current tool manifests.
    ///
    /// Sends `tools/list` to each connected server and updates the cached
    /// tool definitions. Servers that fail to respond keep their existing
    /// tool list and log a warning.
    ///
    /// # Errors
    ///
    /// Returns `Ok(())` even if individual servers fail to respond; errors
    /// are logged per-server. Only returns `Err` if the connection lock
    /// cannot be acquired.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut client = McpClient::new();
    /// client.refresh_tools().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn refresh_tools(&mut self) -> anyhow::Result<()> {
        let conns = self.connections.read().await;

        for server in &mut self.servers {
            if server.status != McpStatus::Connected {
                continue;
            }

            if let Some(conn) = conns.get(&server.id) {
                match conn.service.peer().list_all_tools().await {
                    Ok(tools) => {
                        let tool_defs: Vec<McpToolDef> = tools
                            .iter()
                            .map(|t| McpToolDef {
                                name: t.name.to_string(),
                                description: t
                                    .description
                                    .as_deref()
                                    .unwrap_or_default()
                                    .to_string(),
                                parameters: serde_json::to_value(&*t.input_schema)
                                    .unwrap_or(Value::Object(serde_json::Map::new())),
                            })
                            .collect();

                        tracing::info!(
                            server_id = %server.id,
                            tool_count = tool_defs.len(),
                            "Refreshed tools from MCP server"
                        );
                        server.tools = tool_defs;
                    }
                    Err(e) => {
                        tracing::warn!(
                            server_id = %server.id,
                            error = %e,
                            "Failed to refresh tools from MCP server"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Re-query a specific server for its current tool manifest.
    ///
    /// Sends `tools/list` to the specified server and updates its cached
    /// tool definitions.
    ///
    /// # Arguments
    ///
    /// * `server_id` — the ID of the server to refresh
    ///
    /// # Errors
    ///
    /// Returns an error if the server is not connected or the query fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut client = McpClient::new();
    /// let tools = client.refresh_tools_for_server("my-server").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn refresh_tools_for_server(
        &mut self,
        server_id: &str,
    ) -> anyhow::Result<Vec<McpToolDef>> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(server_id)
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' is not connected", server_id))?;

        let tools = conn.service.peer().list_all_tools().await?;
        let tool_defs: Vec<McpToolDef> = tools
            .iter()
            .map(|t| McpToolDef {
                name: t.name.to_string(),
                description: t.description.as_deref().unwrap_or_default().to_string(),
                parameters: serde_json::to_value(&*t.input_schema)
                    .unwrap_or(Value::Object(serde_json::Map::new())),
            })
            .collect();

        drop(conns);

        if let Some(server) = self.servers.iter_mut().find(|s| s.id == server_id) {
            server.tools = tool_defs.clone();
        }

        tracing::info!(
            server_id,
            tool_count = tool_defs.len(),
            "Refreshed tools from MCP server"
        );

        Ok(tool_defs)
    }

    /// Call a tool on a specific MCP server.
    ///
    /// Routes the invocation to the server identified by `server_id`,
    /// serializes the `input` as tool arguments, and returns the server's
    /// response as a JSON value.
    ///
    /// # Arguments
    ///
    /// * `server_id` — the ID of the target server
    /// * `tool_name` — the name of the tool to invoke
    /// * `input` — JSON arguments matching the tool's input schema
    ///
    /// # Errors
    ///
    /// Returns an error if the server is not connected, the tool call
    /// fails, times out, or the response cannot be serialized.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = McpClient::new();
    /// let input = serde_json::json!({"query": "test"});
    /// let result = client.call_tool("my-server", "search", input).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        input: Value,
    ) -> anyhow::Result<Value> {
        let conns = self.connections.read().await;
        let conn = conns
            .get(server_id)
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' is not connected", server_id))?;

        let arguments = match input {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => {
                let mut map = serde_json::Map::new();
                map.insert("value".to_string(), other);
                Some(map)
            }
        };

        let params = CallToolRequestParams {
            meta: None,
            name: tool_name.to_string().into(),
            arguments,
            task: None,
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(Self::TOOL_CALL_TIMEOUT_SECS),
            conn.service.peer().call_tool(params),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "MCP tool call '{}' on server '{}' timed out after {}s",
                tool_name,
                server_id,
                Self::TOOL_CALL_TIMEOUT_SECS,
            )
        })??;

        Self::format_call_result(&result)
    }

    /// Call a tool by name, automatically resolving which server owns it.
    ///
    /// Searches all connected servers for a tool matching `tool_name` and
    /// dispatches the call to the first server that advertises it.
    ///
    /// # Arguments
    ///
    /// * `tool_name` — the name of the tool to invoke
    /// * `input` — JSON arguments matching the tool's input schema
    ///
    /// # Errors
    ///
    /// Returns an error if no connected server advertises the named tool,
    /// or if the tool call itself fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = McpClient::new();
    /// let input = serde_json::json!({"query": "test"});
    /// let result = client.call_tool_by_name("search", input).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call_tool_by_name(&self, tool_name: &str, input: Value) -> anyhow::Result<Value> {
        let server_id = self
            .servers
            .iter()
            .find(|s| {
                s.status == McpStatus::Connected && s.tools.iter().any(|t| t.name == tool_name)
            })
            .map(|s| s.id.clone())
            .ok_or_else(|| {
                anyhow::anyhow!("No connected MCP server provides tool '{}'", tool_name)
            })?;

        self.call_tool(&server_id, tool_name, input).await
    }

    /// Format a [`CallToolResult`] into a JSON [`Value`].
    fn format_call_result(result: &rmcp::model::CallToolResult) -> anyhow::Result<Value> {
        let content_values: Vec<Value> = result
            .content
            .iter()
            .map(|c| serde_json::to_value(c).unwrap_or(Value::Null))
            .collect();

        let response = serde_json::json!({
            "content": content_values,
            "is_error": result.is_error.unwrap_or(false),
        });

        Ok(response)
    }

    /// Get all registered servers and their statuses.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_core::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// assert!(client.servers().is_empty());
    /// ```
    pub fn servers(&self) -> &[McpServer] {
        &self.servers
    }

    /// Disconnect a specific server by ID.
    ///
    /// Cancels the running service and removes the connection.
    ///
    /// # Arguments
    ///
    /// * `server_id` — the ID of the server to disconnect
    ///
    /// # Errors
    ///
    /// Returns an error if the service cancellation fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut client = McpClient::new();
    /// client.disconnect("my-server").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn disconnect(&mut self, server_id: &str) -> anyhow::Result<()> {
        let conn = {
            let mut conns = self.connections.write().await;
            conns.remove(server_id)
        };

        if let Some(conn) = conn {
            conn.service
                .cancel()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to cancel MCP service: {e}"))?;

            if let Some(server) = self.servers.iter_mut().find(|s| s.id == server_id) {
                server.status = McpStatus::Disabled;
                server.tools.clear();
            }

            tracing::info!(server_id, "MCP server disconnected");
        }

        Ok(())
    }

    /// Disconnect all connected servers and clean up.
    ///
    /// # Errors
    ///
    /// Returns an error if any service cancellation fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut client = McpClient::new();
    /// client.disconnect_all().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn disconnect_all(&mut self) -> anyhow::Result<()> {
        let server_ids: Vec<String> = {
            let conns = self.connections.read().await;
            conns.keys().cloned().collect()
        };

        for id in server_ids {
            self.disconnect(&id).await?;
        }

        Ok(())
    }

    /// Scan the system for available MCP servers and return them.
    ///
    /// Does not modify internal state — the caller decides what to do with results.
    /// Scans `PATH` for known executables, npm global packages, and MCP registry
    /// directories.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_core::mcp::McpClient;
    /// # async fn example() {
    /// let servers = McpClient::discover().await;
    /// for server in servers {
    ///     println!("Found: {} at {:?}", server.name, server.executable);
    /// }
    /// # }
    /// ```
    pub async fn discover() -> Vec<DiscoveredMcpServer> {
        discovery::discover().await
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}
