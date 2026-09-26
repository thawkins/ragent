//! Model Context Protocol (MCP) client and types.
//!
//! Provides [`McpClient`] for managing MCP server connections using the
//! official `rmcp` SDK. Supports stdio (child process) and HTTP transports.
//! After connecting, tools advertised by each server are discoverable via
//! [`McpClient::list_tools`] and invocable via [`McpClient::call_tool`].
//!
//! Use [`McpClient::discover`] to scan `PATH`, npm global packages, and
//! MCP registry directories for available servers (see [`discovery`] module).
//!
//! Whether a server is started is a durable, global choice recorded in
//! `<global state dir>/mcp_state.json`; see [`enable_state`].

pub mod discovery;
pub mod enable_state;
pub mod http;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, Tool as RmcpTool};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::ConfigureCommandExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{RwLock, Semaphore};

use ragent_config::{McpServerConfig, McpTransport};

pub use discovery::{DiscoveredMcpServer, McpDiscoverySource, discover as discover_servers};
pub use enable_state::{McpEnableLedger, global_state_path, is_server_enabled};

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
    if ragent_config::yolo::is_enabled() {
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
                // Char-boundary-safe preview: byte slicing can panic on multibyte URLs.
                let preview: String = trimmed.chars().take(60).collect();
                anyhow::bail!(
                    "[{id}] HTTP/SSE url must start with http:// or https://, got: '{}'",
                    preview
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

impl std::fmt::Display for McpStatus {
    /// Render a short, human-readable status label.
    ///
    /// The single source of truth for the wording every surface (`/mcp`,
    /// `/plugins list --mcp`) prints, so the two views cannot drift. `Failed`
    /// includes its error so a surface needs no second branch.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => f.write_str("connected"),
            Self::Disabled => f.write_str("disabled"),
            Self::NeedsAuth => f.write_str("needs auth"),
            Self::Failed { error } => write!(f, "failed: {error}"),
        }
    }
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

/// Backend trait shared by all MCP client implementations.
///
/// The existing [`McpClient`] and the new [`http::HttpMcpClient`] both implement
/// this trait so callers can dispatch tool discovery and invocation through a
/// single, transport-agnostic interface.
///
/// [`http::HttpMcpClient`]: crate::mcp::http::HttpMcpClient
#[async_trait]
pub trait McpClientBackend: Send + Sync {
    /// List tools from all connected servers.
    async fn list_tools(&self) -> Vec<McpToolDef>;

    /// List tools for a specific server by ID.
    async fn list_tools_for_server(&self, server_id: &str) -> Vec<McpToolDef>;

    /// Refresh the cached tool manifest for all connected servers.
    async fn refresh_tools(&mut self) -> anyhow::Result<()>;

    /// Refresh the cached tool manifest for a specific server.
    async fn refresh_tools_for_server(
        &mut self,
        server_id: &str,
    ) -> anyhow::Result<Vec<McpToolDef>>;

    /// Call a tool on a specific server.
    async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        input: Value,
    ) -> anyhow::Result<Value>;

    /// Call a tool by name, resolving the server automatically.
    async fn call_tool_by_name(&self, tool_name: &str, input: Value) -> anyhow::Result<Value>;
}

/// An active connection to a single MCP server.
///
/// Wraps either an rmcp [`RunningService`] (for stdio and SSE transports) or
/// an [`HttpMcpClient`] (for plain HTTP transport, FR-013). Both variants are
/// wrapped in `Arc` so the enum is cheaply `Clone` — needed when a connection
/// is pulled out of the `RwLock`-guarded map for a per-server refresh.
#[derive(Clone)]
enum McpConnection {
    /// rmcp-based connection (stdio child process or SSE).
    ///
    /// The optional pid is that of the spawned stdio child, kept so teardown
    /// can kill the process synchronously even when `KillOnDrop` cannot run
    /// (e.g. the process is force-exited before the async teardown completes).
    Rmcp(Arc<RunningService<RoleClient, ()>>, Option<u32>),
    /// Custom HTTP JSON-RPC connection (FR-013).
    Http(Arc<http::HttpMcpClient>),
}

/// Normalises model-supplied tool arguments into an MCP `arguments` map.
///
/// Shared by both transports (rmcp/stdio and HTTP) so a non-object payload is
/// normalised identically regardless of transport: objects pass through, null
/// becomes an empty map, and any other value is wrapped in an invented
/// `"value"` envelope key (documented so servers can detect it).
pub(crate) fn normalize_mcp_arguments(input: Value) -> serde_json::Map<String, Value> {
    match input {
        Value::Object(map) => map,
        Value::Null => serde_json::Map::new(),
        other => {
            let mut map = serde_json::Map::new();
            map.insert("value".to_string(), other);
            map
        }
    }
}

/// MCP client managing connections to one or more MCP servers.
///
/// Uses the official `rmcp` SDK for transport, handshake, tool discovery,
/// and tool invocation.
pub struct McpClient {
    servers: Vec<McpServer>,
    connections: Arc<RwLock<HashMap<String, McpConnection>>>,
    /// Shared HTTP client reused by every HTTP/SSE MCP transport connection,
    /// resolved on first use by the `HttpMcpClient` (`None` means "use the
    /// process-wide shared client").
    ///
    /// `reqwest::Client` is backed by an inner `Arc`, so cloning it is cheap
    /// and all `HttpMcpClient` instances share the same connection pool and
    /// TLS session cache (FR-007). It is built on the async request path rather
    /// than in [`Self::new`] because constructing it eagerly requires a Tokio
    /// runtime to be running.
    http_client: Option<reqwest::Client>,
    /// PERF-076: `tool_name -> server_id` lookup index derived from `servers`,
    /// rebuilt whenever the server list or a server's tool manifest changes.
    /// Turns `call_tool_by_name`'s nested `servers x tools` scan into an O(1)
    /// map lookup.  First connected server advertising a name wins, matching
    /// the previous linear-scan semantics.
    tool_index: HashMap<String, String>,
}

impl Drop for McpClient {
    /// R-15: On drop, kill any MCP stdio child processes.
    ///
    /// `Drop` is synchronous but graceful cancellation is async, so this is the
    /// last-resort path: it hard-kills every recorded stdio child pid (see
    /// [`Self::shutdown`] for the deterministic path) and then drops the
    /// connections map, whose `KillOnDrop` transports would kill the children
    /// anyway when the map is uniquely owned. Both steps are needed because a
    /// child whose pid was recorded but whose transport is shared elsewhere
    /// cannot be reached through `Arc::get_mut`, and a leaked child keeps the
    /// stdio pipe open and prints its own teardown errors after ragent exits.
    fn drop(&mut self) {
        let Some(conns) = Arc::get_mut(&mut self.connections) else {
            // The connections map is shared with an `McpToolWrapper` that
            // outlived the client. There is nothing safe to reach here, so the
            // pids cannot be recovered; callers must run [`Self::shutdown`]
            // before dropping the last handle.
            tracing::debug!(
                "MCP client dropped with connections still shared; stdio children rely on the process supervisor"
            );
            return;
        };
        let guard = conns.get_mut();
        for connection in guard.values() {
            if let McpConnection::Rmcp(_, Some(pid)) = connection {
                kill_stdio_child(*pid);
            }
        }
        guard.clear();
    }
}

impl McpClient {
    /// Default timeout for tool calls in seconds.
    const TOOL_CALL_TIMEOUT_SECS: u64 = 120;

    /// Creates a new `McpClient` with no registered servers.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_agent::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// assert!(client.servers().is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            // `reqwest::Client` is built lazily by `HttpMcpClient` itself:
            // `reqwest::Client::new()` creates the connection pool's sockets
            // eagerly, so calling it here panics with "there is no reactor
            // running" when an `McpClient` is constructed outside a Tokio
            // runtime. The `HttpMcpClient` resolves a process-wide shared client
            // on the first request instead.
            http_client: None,
            tool_index: HashMap::new(),
        }
    }

    /// PERF-076: rebuild the `tool_name -> server_id` index from the current
    /// server list.  Called after any mutation of `servers` or a server's tool
    /// manifest.  The first connected server advertising a name wins, so a
    /// duplicate tool name does not override the earlier server.
    fn rebuild_tool_index(&mut self) {
        self.tool_index.clear();
        for server in &self.servers {
            if server.status != McpStatus::Connected {
                continue;
            }
            for tool in &server.tools {
                self.tool_index
                    .entry(tool.name.clone())
                    .or_insert_with(|| server.id.clone());
            }
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
    /// # use ragent_agent::mcp::McpClient;
    /// # use ragent_agent::McpServerConfig;
    /// # async fn example() -> anyhow::Result<()> {
    /// let mut client = McpClient::new();
    /// let config = McpServerConfig::default();
    /// client.connect("my-server", config).await?;
    /// # Ok(())
    /// # }
    /// ```
    /// Register an MCP server that exists but must not be started.
    ///
    /// The server is recorded as [`McpStatus::Disabled`] with no tools and no
    /// connection, so every surface (`/mcp`, `/plugins`) can list it and offer
    /// to re-enable it. Used for a server switched off in `ragent.json`
    /// (`disabled: true`) or in the global enable-state ledger.
    pub fn register_disabled(&mut self, id: &str, config: McpServerConfig) {
        let server = McpServer {
            id: id.to_string(),
            config,
            status: McpStatus::Disabled,
            tools: Vec::new(),
        };
        self.servers.push(server);
        self.rebuild_tool_index();
        tracing::info!(server_id = id, "MCP server registered as disabled");
    }

    /// Connect to an MCP server using the configured transport.
    pub async fn connect(&mut self, id: &str, config: McpServerConfig) -> anyhow::Result<()> {
        if config.disabled {
            self.register_disabled(id, config);
            return Ok(());
        }

        // Validate config before attempting connection.
        validate_mcp_config(id, &config)?;

        // Adopt an already-running instance of this server before spawning our
        // own. A plugin (or a hand-configured entry) declares how the server is
        // *normally* reached; a user who already runs that server separately - a
        // shared MongoDB MCP server on a fixed port, a company-wide HTTP MCP
        // gateway - must not be shadowed by a second copy ragent starts. The
        // probe is transport-aware (see `adopt_candidate`): a declared HTTP/SSE
        // URL is contacted directly, and a stdio entry is contacted only when its
        // command line names a port to try. A stdio child can never be adopted -
        // its stdio pipes are private to its parent - so a plain stdio server
        // with no endpoint always falls through to a normal spawn.
        match config.type_ {
            McpTransport::Stdio => {
                if let Some(killed) = kill_orphaned_stdio(id, &config) {
                    tracing::info!(
                        server_id = id,
                        pids = ?killed,
                        "Killed orphaned MCP stdio server processes matching the configured command"
                    );
                }
            }
            McpTransport::Http | McpTransport::Sse => {}
        }
        if let Some(endpoint) = self.adopt_running(id, &config).await {
            tracing::info!(
                server_id = id,
                url = %crate::sanitize::redact_secrets(&endpoint.url),
                "Adopted an already-running MCP server"
            );
            let mut client = http::HttpMcpClient::new(&endpoint.url, config.headers.clone());
            if let Some(http_client) = self.http_client.clone() {
                client = client.with_client(http_client);
            }
            client = client.with_session_id(endpoint.session_id);
            let tool_defs = client.list_tools().await;
            let tool_count = tool_defs.len();
            self.servers.push(McpServer {
                id: id.to_string(),
                config,
                status: McpStatus::Connected,
                tools: tool_defs,
            });
            self.rebuild_tool_index();
            self.connections
                .write()
                .await
                .insert(id.to_string(), McpConnection::Http(Arc::new(client)));
            tracing::info!(
                server_id = id,
                tool_count,
                "Already-running MCP server adopted and tools discovered"
            );
            return Ok(());
        }

        // Acquire a spawn permit to limit concurrent MCP connections.
        let _permit = MCP_SPAWN_SEMAPHORE
            .acquire()
            .await
            .map_err(|_| anyhow::anyhow!("MCP spawn semaphore closed"))?;

        match self.connect_inner(id, &config).await {
            Ok((connection, tool_defs)) => {
                let tool_count = tool_defs.len();
                let server = McpServer {
                    id: id.to_string(),
                    config,
                    status: McpStatus::Connected,
                    tools: tool_defs,
                };
                self.servers.push(server);
                self.rebuild_tool_index();

                let mut conns = self.connections.write().await;
                conns.insert(id.to_string(), connection);
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
                self.rebuild_tool_index();
                tracing::error!(
                    server_id = id,
                    error = %error_msg,
                    "MCP server connection failed"
                );
                Err(e)
            }
        }
    }

    /// Look for an already-running MCP server that `config` would otherwise
    /// duplicate, returning the endpoint to adopt it through.
    ///
    /// The candidate URLs come from the config's own declared transport, so this
    /// is generic across servers and never hard-coded to one product:
    ///
    /// - `http` / `sse` — the configured [`McpServerConfig::url`] is the server's
    ///   endpoint, so it is the only candidate.
    /// - `stdio` — a child's stdio pipes are private to its parent, so a stdio
    ///   child cannot be adopted; orphaned copies are instead killed by
    ///   [`kill_orphaned_stdio`] before spawn. The command line is still
    ///   inspected for an `--httpPort <n>` / `--port <n>` style flag; a server
    ///   that also listens on HTTP is adopted through that port, and a server
    ///   that does not leaves this returning `None` (a normal spawn).
    ///
    /// The first URL whose `initialize` handshake round-trips wins. Probing is
    /// best-effort: a connection refusal just means "nothing is listening there
    /// yet", so the caller falls through to starting its own instance.
    async fn adopt_running(
        &self,
        id: &str,
        config: &McpServerConfig,
    ) -> Option<http::LiveEndpoint> {
        for url in adopt_candidate_urls(config) {
            if let Some(endpoint) = http::probe(&url).await {
                return Some(endpoint);
            }
        }
        tracing::debug!(
            server_id = id,
            "No already-running MCP server answered; starting a new instance"
        );
        None
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
    /// The active connection and discovered tools on success.
    async fn connect_inner(
        &self,
        id: &str,
        config: &McpServerConfig,
    ) -> anyhow::Result<(McpConnection, Vec<McpToolDef>)> {
        match config.type_ {
            McpTransport::Stdio => {
                let command_str = config
                    .command
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("stdio transport requires a 'command' field"))?;

                let args = config.args.clone();
                let env = config.env.clone();

                // `KillOnDrop` makes the child die with its transport. Without
                // it, the process is abandoned to the OS when the client goes
                // away: it can outlive ragent holding the stdio pipe open, and
                // its teardown writes then land on a dead pipe, printing an
                // unhandled `write EPIPE` Node stack dump after ragent exits.
                // A hard kill is deliberate - a graceful one makes the server
                // log to a client that is already gone, which is the dump.
                let mut wrap = process_wrap::tokio::CommandWrap::from(
                    Command::new(command_str).configure(|cmd| {
                        for arg in &args {
                            cmd.arg(arg);
                        }
                        for (k, v) in &env {
                            cmd.env(k, v);
                        }
                        // The child leads its own process group so shutdown
                        // can `killpg` the whole tree: launcher commands
                        // (`npx`, `npm`, `bunx`) fork the real server (`node`)
                        // and exit immediately, so the transport id alone
                        // tracks a process that is already gone. Without a
                        // per-child group, `killpg` would target ragent's own
                        // group (see the same pattern in the bash timeout).
                        #[cfg(unix)]
                        {
                            cmd.process_group(0);
                        }
                    }),
                );
                wrap.wrap(process_wrap::tokio::KillOnDrop);
                let transport = rmcp::transport::TokioChildProcess::new(wrap)?;
                let child_pid = transport.id();

                tracing::info!(
                    server_id = id,
                    command = %crate::sanitize::redact_secrets(command_str),
                    "Spawning stdio MCP server"
                );
                let service = Arc::new(().serve(transport).await?);
                let tools = service.peer().list_all_tools().await?;
                let tool_defs = rmcp_tools_to_defs(&tools);
                Ok((McpConnection::Rmcp(service, child_pid), tool_defs))
            }
            McpTransport::Sse => {
                let url = config
                    .url
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("SSE transport requires a 'url' field"))?;

                let transport = rmcp::transport::StreamableHttpClientTransport::from_uri(url);

                tracing::info!(
                    server_id = id,
                    url = %crate::sanitize::redact_secrets(url),
                    "Connecting to SSE MCP server"
                );
                let service = Arc::new(().serve(transport).await?);
                let tools = service.peer().list_all_tools().await?;
                let tool_defs = rmcp_tools_to_defs(&tools);
                Ok((McpConnection::Rmcp(service, None), tool_defs))
            }
            McpTransport::Http => {
                let url = config
                    .url
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("HTTP transport requires a 'url' field"))?;

                tracing::info!(
                    server_id = id,
                    url = %crate::sanitize::redact_secrets(url),
                    "Connecting to HTTP MCP server via HttpMcpClient"
                );
                let mut client = http::HttpMcpClient::new(url, config.headers.clone());
                if let Some(http_client) = self.http_client.clone() {
                    client = client.with_client(http_client);
                }
                // The `initialize` handshake is mandatory, not best-effort. A
                // sessionful Streamable-HTTP server (the MongoDB MCP server on
                // its 2025-era path) rejects `tools/list` unless the request
                // carries the `mcp-session-id` this handshake yields, and a
                // server that cannot complete the handshake is not an MCP server
                // ragent can talk to at all. Treating it as optional previously
                // let `list_tools` swallow the failure, so a dead endpoint
                // registered a tool-less server as `Connected`.
                client.initialize().await.map_err(|error| {
                    anyhow::anyhow!("HTTP MCP server at '{url}' failed to initialize: {error}")
                })?;
                let tool_defs = client.list_tools().await;

                tracing::info!(
                    server_id = id,
                    tool_count = tool_defs.len(),
                    "HTTP MCP server connected and tools discovered"
                );

                Ok((McpConnection::Http(Arc::new(client)), tool_defs))
            }
        }
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
    /// use ragent_agent::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// let tools = client.list_tools();
    /// assert!(tools.is_empty());
    /// ```
    #[must_use]
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
    /// use ragent_agent::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// let tools = client.list_tools_for_server("my-server");
    /// assert!(tools.is_empty());
    /// ```
    #[must_use]
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
    /// # use ragent_agent::mcp::McpClient;
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
                let tools_result = match conn {
                    McpConnection::Rmcp(service, _) => {
                        match service.peer().list_all_tools().await {
                            Ok(tools) => Ok(rmcp_tools_to_defs(&tools)),
                            Err(e) => Err(e),
                        }
                    }
                    McpConnection::Http(client) => {
                        // list_tools never errors; it returns an empty vec on failure.
                        Ok(client.list_tools().await)
                    }
                };

                match tools_result {
                    Ok(tool_defs) => {
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

        // PERF-076: the manifests changed — rebuild the lookup index. Drop the
        // connections read guard first so `&mut self` is free to borrow.
        drop(conns);
        self.rebuild_tool_index();
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
    /// # use ragent_agent::mcp::McpClient;
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
        let conn = {
            let conns = self.connections.read().await;
            conns.get(server_id).cloned()
        };

        let Some(conn) = conn else {
            anyhow::bail!("MCP server '{server_id}' is not connected");
        };

        let tool_defs = match &conn {
            McpConnection::Rmcp(service, _) => {
                let tools = service.peer().list_all_tools().await?;
                rmcp_tools_to_defs(&tools)
            }
            McpConnection::Http(client) => client.list_tools().await,
        };

        if let Some(server) = self.servers.iter_mut().find(|s| s.id == server_id) {
            server.tools = tool_defs.clone();
        }
        // PERF-076: the manifest changed — rebuild the lookup index.
        self.rebuild_tool_index();

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
    /// # use ragent_agent::mcp::McpClient;
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
            .ok_or_else(|| anyhow::anyhow!("MCP server '{server_id}' is not connected"))?;

        match conn {
            McpConnection::Rmcp(service, _) => {
                let arguments = normalize_mcp_arguments(input);

                let params =
                    CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments);

                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(Self::TOOL_CALL_TIMEOUT_SECS),
                    service.peer().call_tool(params),
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
            McpConnection::Http(client) => {
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(Self::TOOL_CALL_TIMEOUT_SECS),
                    client.call_tool(server_id, tool_name, input),
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

                Ok(result)
            }
        }
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
    /// # use ragent_agent::mcp::McpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = McpClient::new();
    /// let input = serde_json::json!({"query": "test"});
    /// let result = client.call_tool_by_name("search", input).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call_tool_by_name(&self, tool_name: &str, input: Value) -> anyhow::Result<Value> {
        // PERF-076: O(1) lookup in the maintained `tool_name -> server_id`
        // index instead of a nested `servers x tools` scan.
        let Some(server_id) = self.tool_index.get(tool_name) else {
            anyhow::bail!("No connected MCP server provides tool '{tool_name}'");
        };

        self.call_tool(server_id, tool_name, input).await
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
    /// use ragent_agent::mcp::McpClient;
    ///
    /// let client = McpClient::new();
    /// assert!(client.servers().is_empty());
    /// ```
    #[must_use]
    pub fn servers(&self) -> &[McpServer] {
        &self.servers
    }

    /// Record a connected server without spawning a child process.
    ///
    /// Test seam for the UI paths that read the client's server list as the
    /// authoritative MCP state (`/mcp`), so a test can assert what the display
    /// shows for a connected server without starting a real MCP process.
    #[doc(hidden)]
    pub fn register_connected_for_tests(&mut self, id: &str, tools: Vec<McpToolDef>) {
        self.servers.push(McpServer {
            id: id.to_string(),
            config: McpServerConfig::default(),
            status: McpStatus::Connected,
            tools,
        });
        self.rebuild_tool_index();
    }

    /// Append a fully-formed server entry, leaving the tool index untouched.
    ///
    /// Test seam for callers that need a specific status/transport pairing
    /// (e.g. asserting the startup report labels `sse`/`http` servers); unlike
    /// [`Self::register_connected_for_tests`] the config is caller-supplied.
    /// Tool registration is irrelevant on those paths because no tools are
    /// seeded.
    #[doc(hidden)]
    pub fn push_server_for_tests(&mut self, server: McpServer) {
        self.servers.push(server);
    }

    /// Mutable access to the recorded servers for tests that need to flip a
    /// seeded server's status (e.g. simulating the post-disconnect state when
    /// no real connection exists for `disconnect()` to tear down).
    #[doc(hidden)]
    pub fn servers_mut_for_tests(&mut self) -> &mut [McpServer] {
        &mut self.servers
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
    /// Disconnect is best-effort for teardown: a graceful `cancel()` failure is
    /// logged as a warning (the stdio child is hard-killed right after either
    /// way) rather than returned. Infallible in practice today, but the
    /// `Result` keeps the door open for stricter failure modes later.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use ragent_agent::mcp::McpClient;
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
            match conn {
                McpConnection::Rmcp(service, child_pid) => {
                    // cancel() takes ownership; unwrap the Arc.
                    // If there are other holders, the Arc strong count > 1
                    // and we skip cancellation (the service will be
                    // dropped when the last Arc is released), which also
                    // drops the transport and kills its child (`KillOnDrop`).
                    // A cancel failure is logged, not propagated: the pid is
                    // hard-killed right after either way, and the server must
                    // still leave the live registry as Disabled.
                    if let Some(service) = Arc::into_inner(service)
                        && let Err(error) = service.cancel().await
                    {
                        tracing::warn!(
                            server_id,
                            error = %error,
                            "MCP service failed to cancel during disconnect"
                        );
                    }
                    // `cancel()` stops the rmcp client but does not reliably
                    // SIGKILL the child on every platform, so reap the pid we
                    // recorded at spawn. Hard-killing here is deliberate and
                    // matches the `KillOnDrop` transport: a graceful signal
                    // makes a server log its teardown to a client that is
                    // already gone, which is the unhandled `write EPIPE` dump.
                    if let Some(pid) = child_pid {
                        kill_stdio_child(pid);
                    }
                }
                McpConnection::Http(_) => {
                    // HTTP connections are stateless; nothing to cancel.
                }
            }
            if let Some(server) = self.servers.iter_mut().find(|s| s.id == server_id) {
                server.status = McpStatus::Disabled;
                server.tools.clear();
            }
            // PERF-076: drop the disconnected server's tools from the index.
            self.rebuild_tool_index();

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
    /// # use ragent_agent::mcp::McpClient;
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

    /// Terminate every MCP connection at process teardown.
    ///
    /// Called on the shutdown path (TUI exit, headless run end, SIGTERM) so the
    /// stdio server children ragent spawned are reaped *before* the process
    /// exits. Without it a stdio child outlives ragent as an orphan (`init`
    /// becomes its parent), keeps the stdio pipe open, and prints its own
    /// teardown errors into a dead pipe - and the next ragent start finds a
    /// stale server still bound to whatever endpoint it held.
    ///
    /// Unlike [`Self::disconnect_all`], this is lossy by design: a failed
    /// graceful cancel on one server must not abort the teardown of the rest,
    /// and every recorded stdio pid is hard-killed afterwards so the child is
    /// gone even when its transport is still shared. The call is safe to repeat.
    ///
    /// This is idempotent and best-effort; it never returns an error.
    pub async fn shutdown(&mut self) {
        let conns: Vec<(String, McpConnection)> = {
            let mut conns = self.connections.write().await;
            conns.drain().collect()
        };
        // Each drained connection is torn down inline (rather than reinserted
        // for `disconnect`) so a failed `cancel()` - the Arc is shared by a
        // `McpToolWrapper` and never yields - still kills the recorded pid.
        for (id, conn) in conns {
            let (service, pid) = match &conn {
                McpConnection::Rmcp(service, pid) => (Some(service.clone()), *pid),
                McpConnection::Http(_) => (None, None),
            };
            if let Some(service) = service {
                // `cancel()` takes ownership, so try the Arc-unwrap first; a
                // shared service only loses the graceful path, never the kill.
                match Arc::into_inner(service) {
                    Some(service) => {
                        if let Err(error) = service.cancel().await {
                            tracing::warn!(server_id = %id, error = %error, "MCP service failed to cancel during shutdown");
                        }
                    }
                    None => {
                        tracing::debug!(
                            server_id = %id,
                            "MCP service Arc is shared; skipping cancel, killing the pid directly"
                        );
                    }
                }
            }
            if let Some(pid) = pid {
                // The child leads its own process group: `kill_stdio_child`
                // SIGKILLs the pid and the whole group. Both syscalls are
                // ESRCH-safe, so a child already reaped by `/mcp disable`
                // needs no liveness probe or special-casing here.
                kill_stdio_child(pid);
            }
        }
        let mut servers = std::mem::take(&mut self.servers);
        for server in &mut servers {
            if server.status == McpStatus::Connected {
                server.status = McpStatus::Disabled;
                server.tools.clear();
            }
        }
        self.servers = servers;
        self.rebuild_tool_index();
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
    /// # use ragent_agent::mcp::McpClient;
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

/// Hard-kill an MCP stdio child by pid.
///
/// The child is created with `KillOnDrop`, so the transport's own teardown
/// normally reaps it; this is the belt-and-braces path for the cases the
/// transport cannot cover - a force-exit that never runs the async teardown, or
/// a child whose transport is still shared when the client is dropped.
///
/// `SIGKILL` is deliberate: a graceful signal makes the server log to a client
/// that is already gone, which is the unhandled `write EPIPE` Node stack dump.
/// The error is ignored: an already-dead child (`ESRCH`) is the expected
/// outcome when `KillOnDrop` won the race.
#[cfg(unix)]
fn kill_stdio_child(pid: u32) {
    // The child leads its own process group (`process_group(0)` at spawn), so
    // SIGKILL the whole group: launcher commands (`npx`, ...) fork the real
    // server process and exit, and killing only the recorded pid would orphan
    // that grandchild on a dead stdio pipe.
    // SAFETY: `libc::killpg` takes a plain C int and has no pointer arguments.
    // `pid` was recorded at spawn as the group leader's pid, so this signals
    // the child's own group, never ragent's.
    #[allow(unsafe_code)]
    unsafe {
        libc::killpg(pid as libc::pid_t, libc::SIGKILL);
    }
    // The leader itself too: already covered by killpg, but the call is
    // harmless if the group is gone (`ESRCH`), and it guards a child that
    // left its group after spawn.
    #[allow(unsafe_code)]
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
}

/// Hard-kill an MCP stdio child by pid (Windows: no pid-based hard kill).
///
/// There is no safe, direct `TerminateProcess` equivalent in std, and the
/// `KillOnDrop` transport already terminates the child on Windows, so this is a
/// no-op there.
#[cfg(not(unix))]
fn kill_stdio_child(_pid: u32) {}

/// Parent pid of `pid`, parsed from `/proc/<pid>/stat` (field 4, `ppid`).
///
/// The `comm` field (field 2) is parenthesised and may itself contain spaces
/// or `)`, so the fields are split after the *last* `)`. Returns `None` when
/// the process exited mid-scan or the file cannot be read.
///
/// `#[doc(hidden)] pub` so the integration tests can parse synthetic `stat`
/// contents via [`ppid_from_stat_contents`] without standing up real pids.
#[cfg(unix)]
#[doc(hidden)]
pub fn read_ppid_of(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    ppid_from_stat_contents(&stat)
}

/// Parse the ppid from the contents of a `/proc/<pid>/stat` file.
///
/// Split out from [`read_ppid_of`] so tests can exercise the parsing rule
/// (including hostile `comm` values) without a real process.
#[cfg(unix)]
#[doc(hidden)]
pub fn ppid_from_stat_contents(stat: &str) -> Option<u32> {
    let after_comm = stat.rsplit_once(") ")?.1;
    // Fields after comm: state(3) ppid(4) ...
    after_comm.split_whitespace().nth(1)?.parse::<u32>().ok()
}

/// Pids of orphaned processes belonging to a stdio MCP server, excluding
/// `exclude` (the current process). Reads `/proc` on Unix; a directory entry
/// that cannot be read means the process exited mid-scan and is skipped.
///
/// A process counts as an orphan only when it has been re-parented to init
/// (`Ppid == 1`): that proves its spawning ragent exited. A matching process
/// whose parent is still alive belongs to a *running* ragent instance and
/// must be left alone - killing it would sever that instance's stdio pipes
/// and its tools would return `Transport closed`. The ppid is read *before*
/// the cmdline so the common case (another live instance's child) is rejected
/// without parsing argv.
#[cfg(unix)]
fn find_orphaned_stdio_pids(command: &str, args: &[String], exclude: u32) -> Vec<u32> {
    let mut pids = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return pids;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        if pid == exclude {
            continue;
        }
        if read_ppid_of(pid) != Some(1) {
            continue;
        }
        let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        if orphan_cmdline_matches(&raw, command, args) {
            pids.push(pid);
        }
    }
    pids.sort_unstable();
    pids
}

/// Kill orphaned copies of a stdio MCP server's command left running by an
/// earlier ragent exit, and return the killed pids.
///
/// A stdio server's pipes are private to the parent that spawned it, so an
/// orphaned copy is unreachable (it can never serve another client) but keeps
/// its resources - file locks, ports it also binds, API rate-limit slots,
/// memory. `connect` therefore tears orphans down before spawning a fresh
/// child, freeing what they hold. The current process is always excluded, so
/// in-process tests and a hypothetical embedded server are never killed.
///
/// Matching is `/proc/<pid>/cmdline`-based (see [`orphan_cmdline_matches`]) and
/// deliberately narrow: launcher-mediated servers match on the extracted
/// package token, not the shared launcher name. On non-Unix platforms (or
/// without a `command`) there is no safe process-table walk, so nothing is
/// killed and the normal spawn proceeds.
fn kill_orphaned_stdio(id: &str, config: &McpServerConfig) -> Option<Vec<u32>> {
    #[cfg(unix)]
    {
        let command = config.command.as_deref()?;
        let pids = find_orphaned_stdio_pids(command, &config.args, std::process::id());
        if pids.is_empty() {
            return None;
        }
        for pid in &pids {
            kill_stdio_child(*pid);
        }
        tracing::warn!(
            server_id = id,
            killed = pids.len(),
            "Orphaned MCP stdio processes killed before spawning a fresh child"
        );
        Some(pids)
    }
    #[cfg(not(unix))]
    {
        let _ = id;
        let _ = config;
        None
    }
}

/// The HTTP endpoint URLs to probe for an already-running instance of a server.
///
/// See [`McpClient::adopt_running`] for the transport-aware rationale. Returns
/// an empty vector for a stdio entry whose command line names no port, which is
/// the common case and the correct one: nothing about that entry advertises an
/// address a separately-running copy would be reachable at.
pub fn adopt_candidate_urls(config: &McpServerConfig) -> Vec<String> {
    match config.type_ {
        McpTransport::Http | McpTransport::Sse => config
            .url
            .as_deref()
            .map(str::to_string)
            .into_iter()
            .collect(),
        McpTransport::Stdio => {
            // Only a command line that explicitly binds an HTTP port is a
            // candidate; the port flag name is read from the arguments, not the
            // command, so a server id or package name cannot be mistaken for one.
            let Some(port) = declared_port(&config.args) else {
                return Vec::new();
            };
            vec![
                format!("http://127.0.0.1:{port}/mcp"),
                format!("http://127.0.0.1:{port}"),
            ]
        }
    }
}

/// Extract the TCP port a stdio server's arguments declare it will listen on.
///
/// Recognises the flag spellings the common MCP servers use (`--httpPort 3000`,
/// `--http-port=3000`, `--port 3000`). Only a bare decimal port in the range
/// `1..=65535` is accepted, so a `--port` that names a device path or a
/// non-numeric value is ignored rather than producing a nonsense probe.
pub fn declared_port(args: &[String]) -> Option<u16> {
    /// Flag names that declare an HTTP listen port, lower-cased for comparison.
    const PORT_FLAGS: &[&str] = &["--httpport", "--http-port", "--port", "-p"];

    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let lower = arg.to_ascii_lowercase();
        // `--flag=3000` form.
        if let Some((flag, value)) = lower.split_once('=') {
            if PORT_FLAGS.contains(&flag) {
                if let Some(port) = parse_port(value) {
                    return Some(port);
                }
            }
            continue;
        }
        // `--flag 3000` form.
        if PORT_FLAGS.contains(&lower.as_str()) {
            if let Some(value) = iter.peek() {
                if let Some(port) = parse_port(value) {
                    return Some(port);
                }
            }
        }
    }
    None
}

/// Parse a decimal TCP port, rejecting out-of-range or non-numeric values.
pub fn parse_port(value: &str) -> Option<u16> {
    value.trim().parse::<u16>().ok().filter(|port| *port != 0)
}

// ── Orphaned stdio child cleanup ────────────────────────────────────────────

/// Whether `basename` names a launcher binary that mediates an MCP stdio
/// server's real child process (`npx`, `npm exec`, ...).
///
/// A server launched through a wrapper (`npx -y some-mcp-server`) leaves two
/// process trees behind when its parent dies: the launcher and the real node
/// child. Matching then keys on the package argument (see
/// [`orphan_match_tokens`]) rather than the launcher name, because every
/// MCP server on the machine shares the same `npx` executable name.
fn is_launcher_basename(basename: &str) -> bool {
    let stem = basename.strip_suffix(".exe").unwrap_or(basename);
    matches!(stem, "npx" | "npm" | "bunx" | "pnpx")
}

/// The executable basenames of a single `/proc/<pid>/cmdline` buffer
/// (NUL-separated argv). Leading-dash arguments (`-y`, `--httpPort`) are
/// dropped - they are launch configuration, not identity. Every other argv
/// part reduces to its basename (`/usr/bin/node` -> `node`), because the
/// process's own executable name - not the script path it runs - is the
/// stable, comparable token. A path part that is purely a script (`server.js`)
/// still contributes its basename, which is harmless: only the *first*
/// basename (argv[0]'s executable) is used by the plain-command matcher, and
/// the launcher matcher keys on the extracted package token rather than
/// arbitrary basenames.
fn cmdline_basenames(raw: &[u8]) -> Vec<String> {
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .filter(|part| !part.starts_with(b"-"))
        .map(|part| {
            let text = String::from_utf8_lossy(part);
            text.rsplit(['/', '\\']).next().unwrap_or(&text).to_string()
        })
        .collect()
}

/// Extract a match token from one argv fragment when the fragment is exactly
/// the package name, or a path whose **basename** is the package name. A
/// fragment that merely contains the name deeper in its path (e.g.
/// `.../node_modules/mongodb-mcp-server/dist/index.js`, basename
/// `index.js`) yields nothing; [`part_matches_token`] handles that case with
/// the exact-segment rule below.
fn package_match_token<'a>(fragment: &'a str, package: &str) -> Option<&'a str> {
    if fragment.is_empty() {
        return None;
    }
    if fragment == package {
        return Some(fragment);
    }
    let base = fragment.rsplit(['/', '\\']).next().unwrap_or(fragment);
    if base == package { Some(base) } else { None }
}

/// Whether one argv part names the package. An argv part identifies the
/// package when it is the bare token (`mongodb-mcp-server`), a path whose
/// basename is the token (`.../node_modules/mongodb-mcp-server`), or a path
/// containing the token as **one exact segment**
/// (`.../node_modules/mongodb-mcp-server/dist/index.js`: the segment
/// `mongodb-mcp-server` appears verbatim among the part's `/`-separated
/// segments). The exact-segment rule is deliberately strict: a segment that
/// merely *contains* the token (`mongodb-mcp-server-2`) does not match, so
/// one package's name cannot sweep another's.
fn part_matches_token(part: &str, token: &str) -> bool {
    if package_match_token(part, token).is_some() {
        return true;
    }
    // Version-suffixed argv (the real npx command line shows the full
    // `mongodb-mcp-server@<3` argument): split off the trailing `@<version>`
    // specifier and try again.
    let stripped = match part.rsplit_once('@') {
        Some((base, _)) if !base.is_empty() && !part.starts_with('@') => base,
        _ => part,
    };
    if stripped != part {
        return package_match_token(stripped, token).is_some()
            || stripped
                .split(['/', '\\'])
                .any(|segment| !segment.is_empty() && segment == token);
    }
    part.split(['/', '\\'])
        .any(|segment| !segment.is_empty() && segment == token)
}

/// The match token set that identifies a process started from `command` +
/// `args`.
///
/// - A launcher command (`npx`, ...) yields the launcher name plus the
///   package token extracted from `args`, because the launcher shows
///   `npx -y pkg` while the child it spawns shows `node .../pkg/.../bin`.
/// - A plain command yields only the command basename, matched against the
///   basenames of the running process.
///
/// Returns `None` when no safe identifier extractable from the config remains:
/// a launcher command with no positional package argument, or a `command`
/// whose basename cannot be told apart from any same-named process.
fn orphan_match_tokens(
    command: &str,
    args: &[String],
) -> Option<std::collections::HashSet<String>> {
    let command_base = command.rsplit(['/', '\\']).next().unwrap_or(command);
    if command_base.is_empty() {
        return None;
    }
    let mut tokens = std::collections::HashSet::new();
    if is_launcher_basename(command_base) {
        tokens.insert(command_base.to_string());
        for arg in args {
            let lower = arg.to_ascii_lowercase();
            // Skip flags (`-y`, `--httpPort`) and numbers: they are launch
            // configuration, not identity.
            if lower.starts_with('-') || lower.parse::<u16>().is_ok() {
                continue;
            }
            // Strip a *trailing* version specifier (`mongodb-mcp-server@<3` ->
            // `mongodb-mcp-server`) only when the `@` is not the very first
            // character; a scoped package's leading `@` (`@scope/pkg`) is part
            // of the name and is preserved.
            let package = match arg.rsplit_once('@') {
                Some((base, _)) if !base.is_empty() => base,
                _ => arg.as_str(),
            };
            if package.is_empty() {
                continue;
            }
            let package_base = package.rsplit(['/', '\\']).next().unwrap_or(package);
            if package_base.is_empty() {
                continue;
            }
            tokens.insert(package_base.to_string());
        }
        // A launcher with no extractable package token cannot be identified
        // safely - matching on `npx` alone would sweep every npx process on
        // the machine.
        if tokens.len() == 1 {
            return None;
        }
    } else {
        tokens.insert(command_base.to_string());
    }
    Some(tokens)
}

/// Whether one `/proc/<pid>/cmdline` buffer identifies a process started from
/// a stdio `command` + `args` pair. Extracted for testability, and exported
/// `doc(hidden)` for the integration tests.
///
/// - Launcher command: the package-match token must appear as an argv part
///   (`npx -y pkg`) or as a path segment (`.../node_modules/pkg/dist/...`).
///   The launcher name alone is **not** sufficient to identify the child,
///   because the launcher's own argv shows the package, and its child must be
///   tied to that package, not to every npx process on the machine.
/// - Plain command: the first argv basename (the executable) must be the
///   command name, regardless of what script the process runs.
#[cfg(unix)]
#[doc(hidden)]
pub fn orphan_cmdline_matches(raw: &[u8], command: &str, args: &[String]) -> bool {
    let Some(tokens) = orphan_match_tokens(command, args) else {
        return false;
    };
    let basenames = cmdline_basenames(raw);
    if basenames.is_empty() {
        return false;
    }
    let command_base = command.rsplit(['/', '\\']).next().unwrap_or(command);
    if is_launcher_basename(command_base) {
        tokens
            .iter()
            .filter(|t| t.as_str() != command_base)
            .any(|token| {
                if basenames
                    .iter()
                    .any(|base| package_match_token(base, token).is_some())
                {
                    return true;
                }
                // Path / version-suffixed probe: the package name may be a
                // non-basename path segment of an argv part
                // (`.../node_modules/pkg/dist/index.js`), or an argv part may
                // carry the trailing `@<version>` specifier
                // (`mongodb-mcp-server@<3`). [`part_matches_token`] owns both
                // rules.
                raw.split(|b| *b == 0)
                    .filter(|part| !part.is_empty())
                    .any(|part| part_matches_token(&String::from_utf8_lossy(part), token))
            })
    } else {
        basenames
            .first()
            .is_some_and(|first| tokens.contains(first))
    }
}

/// Convert rmcp tool descriptors to ragent's [`McpToolDef`] format.
fn rmcp_tools_to_defs(tools: &[RmcpTool]) -> Vec<McpToolDef> {
    tools
        .iter()
        .map(|t| McpToolDef {
            name: t.name.to_string(),
            description: t.description.as_deref().unwrap_or_default().to_string(),
            parameters: serde_json::to_value(&*t.input_schema)
                .unwrap_or(Value::Object(serde_json::Map::new())),
        })
        .collect()
}

#[async_trait]
impl McpClientBackend for McpClient {
    async fn list_tools(&self) -> Vec<McpToolDef> {
        self.list_tools()
    }

    async fn list_tools_for_server(&self, server_id: &str) -> Vec<McpToolDef> {
        self.list_tools_for_server(server_id)
    }

    async fn refresh_tools(&mut self) -> anyhow::Result<()> {
        self.refresh_tools().await
    }

    async fn refresh_tools_for_server(
        &mut self,
        server_id: &str,
    ) -> anyhow::Result<Vec<McpToolDef>> {
        self.refresh_tools_for_server(server_id).await
    }

    async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        input: Value,
    ) -> anyhow::Result<Value> {
        self.call_tool(server_id, tool_name, input).await
    }

    async fn call_tool_by_name(&self, tool_name: &str, input: Value) -> anyhow::Result<Value> {
        self.call_tool_by_name(tool_name, input).await
    }
}

#[cfg(all(test, unix))]
mod orphan_sweep_tests {
    use super::*;

    /// A stdio child leads its own process group (`process_group(0)` at spawn)
    /// and shutdown kills the *group*: launcher commands (`npx`, ...) fork the
    /// real server (`node`) and exit, so killing only the recorded pid orphans
    /// the process actually holding the stdio pipes. Proven with a `sh` child
    /// that forks a grandchild `sleep` before replacing itself.
    ///
    /// No rmcp transport is used: `().serve(transport)` would block on a
    /// JSON-RPC handshake `sleep` never answers. The shutdown path the fix
    /// targets only reads the recorded pid and kills the group, which a plain
    /// child exercises exactly.
    #[tokio::test]
    async fn shutdown_kills_the_stdio_child_process_group() {
        let mut child = tokio::process::Command::new("sh")
            .args(["-c", "sh -c 'sleep 600' & exec sleep 600"])
            .process_group(0)
            .kill_on_drop(true)
            .spawn()
            .expect("spawn sh");
        let leader_pid = child.id().expect("child pid is recorded");

        // Directly exercise the group-kill primitives: this is the same pair
        // `kill_stdio_child` issues for the pid the stdio spawn records.
        kill_stdio_child(leader_pid);
        let _ = child.wait().await;

        // The group must be gone: probing the leader pid (kill with signal 0)
        // and the process group (killpg with signal 0) must both return ESRCH.
        #[allow(unsafe_code)]
        // approved: kill/killpg signal-0 probes are harmless; no safe std alternative
        unsafe {
            assert_eq!(
                libc::kill(leader_pid as libc::pid_t, 0),
                -1,
                "leader pid {leader_pid} must be killed by shutdown"
            );
            assert_eq!(
                libc::killpg(leader_pid as libc::pid_t, 0),
                -1,
                "process group {leader_pid} must be empty after shutdown"
            );
        }
    }
}
