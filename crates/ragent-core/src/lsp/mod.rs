//! Language Server Protocol (LSP) client and types.
//!
//! Provides [`LspManager`] for managing connections to one or more LSP
//! servers over stdio JSON-RPC. After connecting, language servers provide
//! code-intelligence capabilities (symbols, hover, definitions, references,
//! diagnostics) that the agent can query via dedicated tools.
//!
//! # Configuration
//!
//! LSP servers are declared in `ragent.json` under the `lsp` key, mirroring
//! the `mcp` section:
//!
//! ```json
//! {
//!   "lsp": {
//!     "rust": {
//!       "command": "rust-analyzer",
//!       "extensions": ["rs"]
//!     },
//!     "typescript": {
//!       "command": "typescript-language-server",
//!       "args": ["--stdio"],
//!       "extensions": ["ts", "tsx", "js", "jsx"]
//!     }
//!   }
//! }
//! ```
//!
//! # Auto-Discovery
//!
//! Use [`LspManager::discover`] to scan `PATH` and VS Code extension
//! directories for available servers (see [`discovery`] module).

pub mod client;
pub mod discovery;
pub mod server;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use lsp_types::Diagnostic;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::config::LspServerConfig;
use crate::event::EventBus;

pub use client::LspClient;
pub use discovery::{DiscoveredServer, discover as discover_servers};
pub use server::{LspServer, LspStatus};

/// Manages connections to one or more LSP servers.
///
/// Mirrors the architecture of [`crate::mcp::McpClient`]: holds a list of
/// [`LspServer`] descriptors for status display and a map of live
/// [`LspClient`] connections for query dispatch.
pub struct LspManager {
    servers: Vec<LspServer>,
    clients: HashMap<String, Arc<LspClient>>,
    /// Workspace root sent to each server as `rootUri`.
    root_path: PathBuf,
    event_bus: Arc<EventBus>,
}

impl LspManager {
    /// Create a new manager with no registered servers.
    ///
    /// `root_path` is the workspace root passed to each LSP server during
    /// initialization. `event_bus` is used to publish [`crate::event::Event::LspStatusChanged`]
    /// events when a server's status changes.
    pub fn new(root_path: PathBuf, event_bus: Arc<EventBus>) -> Self {
        Self {
            servers: Vec::new(),
            clients: HashMap::new(),
            root_path,
            event_bus,
        }
    }

    /// Connect to a single LSP server.
    ///
    /// Registers the server descriptor, spawns the process, and completes
    /// the initialize handshake. Updates `LspStatus` on success or failure
    /// and publishes an [`crate::event::Event::LspStatusChanged`] event.
    ///
    /// Disabled servers are registered but not started.
    ///
    /// # Errors
    ///
    /// Returns an error if the server is already registered by this `id`.
    pub async fn connect(&mut self, id: &str, language: &str, config: LspServerConfig) {
        if self.servers.iter().any(|s| s.id == id) {
            warn!("LSP server '{}' is already registered — skipping", id);
            return;
        }

        let mut descriptor = LspServer::new(id.to_string(), language.to_string(), config.clone());

        if config.disabled {
            self.servers.push(descriptor);
            return;
        }

        self.publish_status(id, &LspStatus::Starting);

        match LspClient::start(&config, &self.root_path).await {
            Ok(client) => {
                let caps_summary = caps_summary(&client.capabilities);
                descriptor.status = LspStatus::Connected;
                descriptor.capabilities_summary = Some(caps_summary.clone());
                info!("LSP '{}' connected ({})", id, caps_summary);
                self.publish_status(id, &LspStatus::Connected);
                self.clients.insert(id.to_string(), Arc::new(client));
            }
            Err(err) => {
                let msg = err.to_string();
                error!("LSP '{}' failed to connect: {}", id, msg);
                descriptor.status = LspStatus::Failed { error: msg.clone() };
                self.publish_status(id, &LspStatus::Failed { error: msg });
            }
        }

        self.servers.push(descriptor);
    }

    /// Connect all servers declared in a `ragent.json` `lsp` section.
    ///
    /// Servers are started concurrently. Connection errors are recorded per-server
    /// but do not prevent other servers from starting.
    pub async fn connect_all(&mut self, configs: HashMap<String, LspServerConfig>) {
        // Collect entries to start (can't run them all truly concurrently because
        // we need &mut self per connect call; sequential start is fine in practice).
        let entries: Vec<(String, LspServerConfig)> = configs.into_iter().collect();
        for (id, config) in entries {
            // Derive the language from the id key if not otherwise specified.
            let language = id.clone();
            self.connect(&id, &language, config).await;
        }
    }

    /// Disconnect a specific server, shutting it down gracefully.
    ///
    /// # Errors
    ///
    /// Returns an error if the server id is not found.
    pub async fn disconnect(&mut self, id: &str) -> Result<()> {
        if let Some(client) = self.clients.remove(id) {
            let _ = client.shutdown().await; // ignore errors during shutdown
        }
        if let Some(s) = self.servers.iter_mut().find(|s| s.id == id) {
            s.status = LspStatus::Disabled;
            self.publish_status(id, &LspStatus::Disabled);
            Ok(())
        } else {
            anyhow::bail!("LSP server '{}' not found", id)
        }
    }

    /// Disconnect and shut down all connected servers.
    pub async fn disconnect_all(&mut self) {
        let ids: Vec<String> = self.clients.keys().cloned().collect();
        for id in ids {
            if let Some(client) = self.clients.remove(&id) {
                let _ = client.shutdown().await;
            }
        }
        for s in &mut self.servers {
            if s.status == LspStatus::Connected {
                s.status = LspStatus::Disabled;
            }
        }
        self.clients.clear();
    }

    /// Returns a slice of all registered server descriptors (connected, disabled, failed).
    pub fn servers(&self) -> &[LspServer] {
        &self.servers
    }

    /// Returns the number of currently connected servers.
    pub fn connected_count(&self) -> usize {
        self.clients.len()
    }

    /// Find the connected [`LspClient`] that handles files with the given
    /// extension (e.g. `"rs"`, `"ts"`).
    ///
    /// Returns `None` if no connected server declares this extension.
    pub fn client_for_extension(&self, ext: &str) -> Option<Arc<LspClient>> {
        // Find a connected server whose config.extensions contains `ext`.
        let server_id = self
            .servers
            .iter()
            .find(|s| s.status == LspStatus::Connected && s.config.extensions.iter().any(|e| e == ext))?
            .id
            .clone();
        self.clients.get(&server_id).cloned()
    }

    /// Find the connected [`LspClient`] best suited for `path` based on file extension.
    pub fn client_for_path(&self, path: &Path) -> Option<Arc<LspClient>> {
        let ext = path.extension()?.to_str()?;
        self.client_for_extension(ext)
    }

    /// Collect all accumulated diagnostics from all connected servers,
    /// filtered to `path` if provided.
    pub async fn diagnostics_for(&self, path: Option<&Path>) -> Vec<(String, Vec<Diagnostic>)> {
        let mut results = Vec::new();
        for client in self.clients.values() {
            let map = client.diagnostics.read().await;
            for (uri, diags) in map.iter() {
                if let Some(filter) = path {
                    // Match by URI suffix to avoid constructing full file URIs here.
                    let filter_str = filter.to_string_lossy();
                    if !uri.contains(filter_str.as_ref()) {
                        continue;
                    }
                }
                if !diags.is_empty() {
                    results.push((uri.clone(), diags.clone()));
                }
            }
        }
        results
    }

    /// Scan the system for available LSP servers and return them.
    ///
    /// Does not modify internal state — the caller decides what to do with results.
    pub async fn discover() -> Vec<DiscoveredServer> {
        discovery::discover().await
    }

    fn publish_status(&self, id: &str, status: &LspStatus) {
        self.event_bus
            .publish(crate::event::Event::LspStatusChanged {
                server_id: id.to_string(),
                status: status.clone(),
            });
    }
}

/// Shared, thread-safe handle to an [`LspManager`].
pub type SharedLspManager = Arc<RwLock<LspManager>>;

/// Build a human-readable capabilities summary from a server's `ServerCapabilities`.
fn caps_summary(caps: &lsp_types::ServerCapabilities) -> String {
    let mut parts = Vec::new();
    if caps.hover_provider.is_some() {
        parts.push("hover");
    }
    if caps.definition_provider.is_some() {
        parts.push("definition");
    }
    if caps.references_provider.is_some() {
        parts.push("references");
    }
    if caps.document_symbol_provider.is_some() {
        parts.push("symbols");
    }
    if caps.workspace_symbol_provider.is_some() {
        parts.push("workspace-symbols");
    }
    if caps.diagnostic_provider.is_some() {
        parts.push("diagnostics");
    }
    if caps.document_formatting_provider.is_some() {
        parts.push("formatting");
    }
    if caps.rename_provider.is_some() {
        parts.push("rename");
    }
    if caps.code_action_provider.is_some() {
        parts.push("code-actions");
    }
    if parts.is_empty() {
        "no advertised capabilities".to_string()
    } else {
        parts.join(", ")
    }
}
