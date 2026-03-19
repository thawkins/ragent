//! LSP JSON-RPC client over stdio.
//!
//! [`LspClient`] spawns an LSP server process, performs the `initialize`
//! handshake, and exposes typed request/notification helpers. A background
//! reader task routes incoming responses to their awaiting callers and
//! accumulates push diagnostics in [`LspClient::diagnostics`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context as _, Result};
use lsp_types::{
    ClientCapabilities, Diagnostic, InitializeParams, InitializeResult, InitializedParams,
    PublishDiagnosticsParams, ServerCapabilities,
};
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{Mutex, RwLock, oneshot};
use url::Url;

use crate::config::LspServerConfig;

/// Active JSON-RPC connection to a single LSP server process.
///
/// Spawns the server as a child process (stdio transport), completes the
/// LSP `initialize` handshake, and routes JSON-RPC messages between callers
/// and the reader background task.
pub struct LspClient {
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    next_id: Arc<AtomicU64>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    /// Accumulated push diagnostics keyed by file URI string.
    pub diagnostics: Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>,
    /// Server capabilities reported during `initialize`.
    pub capabilities: ServerCapabilities,
    timeout_ms: u64,
    // Kept alive so the process is killed when the client is dropped.
    _process: Child,
    _reader: tokio::task::JoinHandle<()>,
}

impl LspClient {
    /// Spawn `config.command`, complete the LSP initialize handshake, and
    /// return a connected client.
    ///
    /// `root_path` is the workspace root directory sent as `rootUri`.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be spawned, the initialize
    /// request times out, or the server returns an error response.
    pub async fn start(config: &LspServerConfig, root_path: &Path) -> Result<Self> {
        use std::process::Stdio;

        let command = config
            .command
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("LSP server config missing command"))?;

        let mut process = tokio::process::Command::new(command)
            .args(&config.args)
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("Failed to start LSP server '{command}'"))?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to obtain stdin for '{command}'"))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to obtain stdout for '{command}'"))?;

        let writer = Arc::new(Mutex::new(BufWriter::new(stdin)));
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let diagnostics: Arc<RwLock<HashMap<String, Vec<Diagnostic>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let next_id = Arc::new(AtomicU64::new(1));

        let reader_handle = tokio::spawn(reader_loop(
            stdout,
            Arc::clone(&pending),
            Arc::clone(&diagnostics),
        ));

        // ── initialize handshake ──────────────────────────────────────────
        let init_id = next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        pending.lock().await.insert(init_id, tx);

        let root_uri: lsp_types::Uri = Url::from_directory_path(root_path)
            .map_err(|()| {
                anyhow::anyhow!("Cannot convert root path to URI: {}", root_path.display())
            })?
            .as_str()
            .parse()
            .context("Root path URI parse failed")?;

        let workspace_folder = lsp_types::WorkspaceFolder {
            uri: root_uri.clone(),
            name: root_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "workspace".to_string()),
        };

        #[allow(deprecated)]
        let init_params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(root_uri),
            workspace_folders: Some(vec![workspace_folder]),
            capabilities: ClientCapabilities::default(),
            ..Default::default()
        };

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": init_id,
            "method": "initialize",
            "params": serde_json::to_value(&init_params)?
        });
        write_framed(&writer, &request).await?;

        let response = tokio::time::timeout(Duration::from_millis(config.timeout_ms), rx)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "LSP 'initialize' timed out after {}ms for '{command}'",
                    config.timeout_ms
                )
            })?
            .map_err(|_| anyhow::anyhow!("LSP initialize channel closed for '{command}'"))?;

        if let Some(err) = response.get("error") {
            anyhow::bail!("LSP 'initialize' error from '{command}': {err}");
        }

        let init_result: InitializeResult =
            serde_json::from_value(response.get("result").cloned().unwrap_or(Value::Null))
                .context("Failed to parse LSP InitializeResult")?;

        let capabilities = init_result.capabilities;

        // Acknowledge initialization
        let initialized = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": serde_json::to_value(InitializedParams {})?
        });
        write_framed(&writer, &initialized).await?;

        Ok(Self {
            writer,
            next_id,
            pending,
            diagnostics,
            capabilities,
            timeout_ms: config.timeout_ms,
            _process: process,
            _reader: reader_handle,
        })
    }

    /// Send a JSON-RPC request and await the typed response.
    ///
    /// # Errors
    ///
    /// Returns an error on timeout, channel failure, or an LSP error response.
    pub async fn request<P, R>(&self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: serde::de::DeserializeOwned,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": serde_json::to_value(&params)?
        });
        write_framed(&self.writer, &message).await?;

        let response = tokio::time::timeout(Duration::from_millis(self.timeout_ms), rx)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "LSP request '{method}' timed out after {}ms",
                    self.timeout_ms
                )
            })?
            .map_err(|_| anyhow::anyhow!("LSP response channel closed for '{method}'"))?;

        if let Some(err) = response.get("error") {
            anyhow::bail!("LSP error for '{method}': {err}");
        }

        serde_json::from_value(response.get("result").cloned().unwrap_or(Value::Null))
            .with_context(|| format!("Failed to parse LSP response for '{method}'"))
    }

    /// Send a JSON-RPC notification (no response expected).
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or writing fails.
    pub async fn notify<P: Serialize>(&self, method: &str, params: P) -> Result<()> {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": serde_json::to_value(&params)?
        });
        write_framed(&self.writer, &message).await
    }

    /// Notify the server that a document has been opened.
    ///
    /// Must be called before sending any queries against a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the notification fails.
    pub async fn open_document(&self, path: &Path) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("LSP didOpen: cannot read {}", path.display()))?;

        let uri: lsp_types::Uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Cannot convert path to URI: {}", path.display()))?
            .as_str()
            .parse()
            .context("File path URI parse failed")?;

        let language_id = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("text")
            .to_string();

        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri,
                language_id,
                version: 1,
                text: content,
            },
        };
        self.notify("textDocument/didOpen", params).await
    }

    /// Notify the server that a document has been closed.
    ///
    /// # Errors
    ///
    /// Returns an error if the notification cannot be sent.
    pub async fn close_document(&self, path: &Path) -> Result<()> {
        let uri: lsp_types::Uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Cannot convert path to URI: {}", path.display()))?
            .as_str()
            .parse()
            .context("File path URI parse failed")?;

        let params = lsp_types::DidCloseTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier { uri },
        };
        self.notify("textDocument/didClose", params).await
    }

    /// Send a graceful shutdown request then an exit notification.
    ///
    /// # Errors
    ///
    /// Returns an error if the shutdown request or exit notification fails.
    pub async fn shutdown(&self) -> Result<()> {
        let _: Value = self.request("shutdown", Value::Null).await?;
        self.notify("exit", Value::Null).await
    }

    /// Build a [`lsp_types::TextDocumentIdentifier`] for `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be converted to a file URI.
    pub fn text_document_id(&self, path: &Path) -> Result<lsp_types::TextDocumentIdentifier> {
        let uri: lsp_types::Uri = Url::from_file_path(path)
            .map_err(|()| anyhow::anyhow!("Cannot convert path to URI: {}", path.display()))?
            .as_str()
            .parse()
            .context("File path URI parse failed")?;
        Ok(lsp_types::TextDocumentIdentifier { uri })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Write a single Content-Length framed JSON-RPC message to the server's stdin.
async fn write_framed(writer: &Arc<Mutex<BufWriter<ChildStdin>>>, message: &Value) -> Result<()> {
    let body = serde_json::to_vec(message)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut w = writer.lock().await;
    w.write_all(header.as_bytes()).await?;
    w.write_all(&body).await?;
    w.flush().await?;
    Ok(())
}

/// Background task that reads Content-Length framed messages from the server's
/// stdout and routes them to pending request senders or accumulates diagnostics.
async fn reader_loop(
    stdout: ChildStdout,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    diagnostics: Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>,
) {
    let mut reader = BufReader::new(stdout);

    loop {
        // ── read headers ──────────────────────────────────────────────────
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => return, // EOF or broken pipe — server died
                Ok(_) => {}
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break; // blank line separates headers from body
            }
            if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
                content_length = len_str.trim().parse().ok();
            }
        }

        let len = match content_length {
            Some(n) if n > 0 => n,
            _ => continue,
        };

        // ── read body ─────────────────────────────────────────────────────
        let mut body = vec![0u8; len];
        if reader.read_exact(&mut body).await.is_err() {
            return;
        }

        let value: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Responses have an "id"; server-originated notifications do not.
        if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
            let mut map = pending.lock().await;
            if let Some(tx) = map.remove(&id) {
                let _ = tx.send(value);
            }
        } else if let Some(method) = value.get("method").and_then(|v| v.as_str()) {
            handle_notification(method, &value, &diagnostics).await;
        }
    }
}

/// Dispatch a server-originated notification.
async fn handle_notification(
    method: &str,
    message: &Value,
    diagnostics: &Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>,
) {
    match method {
        "textDocument/publishDiagnostics" => {
            if let Some(params) = message.get("params") {
                if let Ok(p) = serde_json::from_value::<PublishDiagnosticsParams>(params.clone()) {
                    diagnostics
                        .write()
                        .await
                        .insert(p.uri.to_string(), p.diagnostics);
                }
            }
        }
        // window/logMessage, $/progress, etc. are silently ignored.
        _ => {}
    }
}
