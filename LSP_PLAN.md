# LSP Integration Plan for ragent

## Overview

Add Language Server Protocol (LSP) client support to ragent, enabling the LLM to query
live code-intelligence from installed language servers. This gives the agent precise,
compiler-backed answers for hover types, go-to-definition, find-references, workspace
symbols, and diagnostics — replacing fragile text-search heuristics with the same data
that IDEs use.

---

## Goals

1. **LSP client** — connect to one or more running LSP servers (rust-analyzer, pyright, ts-language-server, etc.)
2. **LLM-callable tools** — expose LSP queries as ragent tools the LLM can invoke
3. **System-prompt injection** — surface connected servers and live diagnostics summary in the agent system prompt
4. **`/lsp` slash command** — display connected servers, status, and per-language server info
5. **Auto-discovery** — detect installed LSP servers on `PATH` and in VS Code extension directories at startup
6. **Config** — mirror the existing `mcp` config section with a new `lsp` section in `ragent.json`

---

## Background: How ragent is Structured

| Layer | Crate | Key files |
|-------|-------|-----------|
| Core logic | `ragent-core` | `src/config/mod.rs`, `src/tool/`, `src/mcp/mod.rs`, `src/agent/mod.rs` |
| Terminal UI | `ragent-tui` | `src/app.rs` (slash commands + `App` struct), `src/layout.rs` |
| HTTP server | `ragent-server` | SSE event streaming |

### Relevant patterns to mirror

- **MCP client** (`ragent-core/src/mcp/mod.rs`) — connects to external servers over stdio/HTTP, discovers their tools, exposes them to the LLM. The LSP client follows the same architectural pattern.
- **Tool trait** (`ragent-core/src/tool/mod.rs`) — `async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>`. New LSP tools implement this.
- **Config** (`ragent-core/src/config/mod.rs`) — JSON, layered `~/.config/ragent/ragent.json` → `./ragent.json`. Has `mcp: HashMap<String, McpServerConfig>` we replicate with `lsp`.
- **Slash commands** (`ragent-tui/src/app.rs`) — `SLASH_COMMANDS` const array + `execute_slash_command()` match arm per command.
- **App struct** — has `mcp_servers: Vec<McpServer>`; we add `lsp_servers: Vec<LspServer>`.
- **System prompt** — `build_system_prompt()` in `agent/mod.rs`; we add an LSP section.

---

## Architecture

```
ragent-core/src/lsp/
  mod.rs          — LspManager (owns all server connections)
  client.rs       — LspClient (single server: JSON-RPC over stdio/socket)
  protocol.rs     — LSP request/response types (wraps `lsp-types` crate)
  discovery.rs    — auto-discovery of installed servers
  server.rs       — LspServer struct + LspStatus enum
```

The **LspManager** is owned by the session processor (like McpClient) and is passed through
to each tool via `ToolContext`. LSP servers are started as child processes communicating
over stdio using JSON-RPC 2.0 (the standard LSP transport).

### Crate dependency

Add to `Cargo.toml` workspace dependencies:
```toml
lsp-types = "0.97"   # LSP protocol type definitions
```

The `lsp-types` crate provides all standard request/response structs (InitializeParams,
TextDocumentIdentifier, SymbolInformation, Diagnostic, etc.) without pulling in a full
LSP server framework.

For the JSON-RPC transport, implement a lightweight `LspClient` that:
1. Spawns the server process (`tokio::process::Command`)
2. Writes `Content-Length: N\r\n\r\n{json}` framed messages to stdin
3. Reads the same framing from stdout in a background task
4. Maintains a pending-request map (id → oneshot sender) for response routing

---

## Phase 1 — LSP Client Core (`ragent-core/src/lsp/`)

### 1.1 Protocol types (`protocol.rs`)

Re-export and extend `lsp-types` with ragent-specific helpers:
```rust
pub use lsp_types::{
    InitializeParams, ServerCapabilities, TextDocumentIdentifier, Position,
    SymbolInformation, DocumentSymbol, Location, Hover, Diagnostic,
    WorkspaceSymbolParams, DocumentSymbolParams,
};

pub struct LspRequest { pub id: u64, pub method: String, pub params: Value }
pub struct LspResponse { pub id: u64, pub result: Option<Value>, pub error: Option<Value> }
pub struct LspNotification { pub method: String, pub params: Value }
```

### 1.2 Client transport (`client.rs`)

```rust
pub struct LspClient {
    process:  tokio::process::Child,
    writer:   tokio::io::BufWriter<ChildStdin>,
    next_id:  AtomicU64,
    pending:  Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    caps:     ServerCapabilities,  // from initialize response
}

impl LspClient {
    pub async fn start(command: &str, args: &[String], root_uri: Url) -> Result<Self>;
    pub async fn send_request<P: Serialize, R: DeserializeOwned>
        (&self, method: &str, params: P) -> Result<R>;
    pub async fn send_notification<P: Serialize>(&self, method: &str, params: P) -> Result<()>;
    pub async fn shutdown(&mut self) -> Result<()>;
}
```

Initialization sequence on `start()`:
1. Spawn process
2. Spawn reader task (parses Content-Length frames → routes to pending map or notification queue)
3. Send `initialize` request with `rootUri`, `capabilities`
4. Send `initialized` notification
5. Store `ServerCapabilities` from response

### 1.3 Server descriptor (`server.rs`)

```rust
pub struct LspServer {
    pub id:       String,          // e.g. "rust-analyzer"
    pub language: String,          // e.g. "rust"
    pub config:   LspServerConfig, // from ragent.json
    pub status:   LspStatus,
    pub caps:     Option<ServerCapabilities>,
}

pub enum LspStatus {
    Connected,
    Starting,
    Disabled,
    Failed { error: String },
}
```

### 1.4 Manager (`mod.rs`)

```rust
pub struct LspManager {
    servers: Vec<LspServer>,
    clients: HashMap<String, LspClient>,
}

impl LspManager {
    pub async fn connect(&mut self, id: &str, config: LspServerConfig, root: &Path) -> Result<()>;
    pub fn server_for_file(&self, path: &Path) -> Option<&LspClient>;
    pub fn servers(&self) -> &[LspServer];
    pub async fn disconnect_all(&mut self);
    pub fn connected_count(&self) -> usize;
    pub fn diagnostics_summary(&self, path: &Path) -> Vec<Diagnostic>;
}
```

`server_for_file()` maps a file extension to the appropriate connected client using the
language-to-extension mapping in each `LspServerConfig`.

---

## Phase 2 — Configuration

### 2.1 New config section (`ragent-core/src/config/mod.rs`)

Add to `Config`:
```rust
/// LSP server definitions keyed by language id (e.g. "rust", "typescript").
#[serde(default)]
pub lsp: HashMap<String, LspServerConfig>,
```

New config types:
```rust
pub struct LspServerConfig {
    /// Executable name or path (e.g. "rust-analyzer").
    pub command: String,
    /// Command-line arguments (e.g. ["--stdio"] for some servers).
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variable overrides for the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// File extensions this server handles (e.g. ["rs"]).
    #[serde(default)]
    pub extensions: Vec<String>,
    /// If true, server is configured but will not be started.
    #[serde(default)]
    pub disabled: bool,
    /// Maximum time in ms to wait for LSP responses (default: 5000).
    #[serde(default = "default_lsp_timeout_ms")]
    pub timeout_ms: u64,
}
```

Example `ragent.json`:
```json
{
  "lsp": {
    "rust": {
      "command": "rust-analyzer",
      "args": [],
      "extensions": ["rs"],
      "timeout_ms": 10000
    },
    "typescript": {
      "command": "typescript-language-server",
      "args": ["--stdio"],
      "extensions": ["ts", "tsx", "js", "jsx"]
    },
    "python": {
      "command": "pyright-langserver",
      "args": ["--stdio"],
      "extensions": ["py"]
    }
  }
}
```

---

## Phase 3 — Auto-Discovery (`lsp/discovery.rs`)

At startup (before connecting configured servers), auto-discovery:
1. Checks `PATH` for known LSP server executables
2. Scans `~/.vscode/extensions/` and `~/.vscode-server/extensions/` for bundled servers
3. Returns a list of `LspServerConfig` candidates with `disabled: true` (user opt-in)
   or `disabled: false` if `experimental.lsp_auto_connect` is set

### Known servers to detect

| Language | Executables to try |
|----------|--------------------|
| Rust | `rust-analyzer` |
| TypeScript/JavaScript | `typescript-language-server`, `tsserver` |
| Python | `pyright-langserver`, `pylsp`, `jedi-language-server` |
| Go | `gopls` |
| C/C++ | `clangd` |
| Java | `jdtls`, `java-language-server` |
| Lua | `lua-language-server` |
| HTML/CSS/JSON | `vscode-html-language-server`, `vscode-css-language-server`, `vscode-json-language-server` |
| Ruby | `solargraph` |
| C# | `OmniSharp`, `csharp-ls` |

```rust
pub struct DiscoveredServer {
    pub language: String,
    pub command:  String,
    pub args:     Vec<String>,
    pub extensions: Vec<String>,
    pub source:   DiscoverySource,  // Path | VsCodeExtension
}

pub async fn discover_lsp_servers() -> Vec<DiscoveredServer>;
```

Discovery is non-blocking; missing servers are silently skipped.

A `/lsp discover` subcommand (or at startup with `experimental.lsp_auto_discover`) can
print discovered servers and suggest adding them to `ragent.json`.

---

## Phase 4 — LLM Tools (`ragent-core/src/tool/`)

Five new tools the LLM can invoke. All require a connected LSP server for the file's language.
All gracefully degrade with a helpful message if no server is connected.

### 4.1 `lsp_symbols` — Document / workspace symbols

**Input**: `{ "path": "src/main.rs" }` or `{ "query": "LspClient" }`  
**Output**: List of symbol names, kinds, and line ranges

Calls `textDocument/documentSymbols` (for a specific file) or `workspace/symbol` (for a query).
Returns a formatted table the LLM can use to navigate without reading file contents.

### 4.2 `lsp_hover` — Type info and documentation

**Input**: `{ "path": "src/main.rs", "line": 42, "character": 15 }`  
**Output**: Hover markdown text (type signature + docs)

Calls `textDocument/hover`. Extremely useful for understanding what a symbol is without
reading surrounding code.

### 4.3 `lsp_definition` — Go to definition

**Input**: `{ "path": "src/main.rs", "line": 42, "character": 15 }`  
**Output**: `{ "path": "src/client.rs", "line": 88, "character": 4 }`

Calls `textDocument/definition`. Lets the LLM follow a symbol to its source without grep.

### 4.4 `lsp_references` — Find all usages

**Input**: `{ "path": "src/main.rs", "line": 42, "character": 15 }`  
**Output**: List of `{ "path", "line", "preview" }` for each reference

Calls `textDocument/references`. Critical for safe refactoring — agent can see all usage
sites before modifying.

### 4.5 `lsp_diagnostics` — Errors and warnings

**Input**: `{ "path": "src/main.rs" }` (or omit path for all files)  
**Output**: List of `{ "path", "line", "severity", "message", "code" }`

Calls `workspace/diagnostic` or uses push-diagnostics (`textDocument/publishDiagnostics`)
accumulated from server notifications. This gives the agent real compiler errors and
warnings without running `cargo check`.

### Tool registration

Register all five tools in `create_default_registry()` in `tool/mod.rs`. Each tool
receives the `LspManager` reference via `ToolContext` (add `lsp: Option<Arc<LspManager>>`
to `ToolContext`).

---

## Phase 5 — System Prompt Enhancement (`ragent-core/src/agent/mod.rs`)

Modify `build_system_prompt()` to accept an `LspManager` reference and conditionally
append an LSP section:

```
## Code Intelligence (LSP)

Connected language servers: rust-analyzer (Rust), typescript-language-server (TypeScript)

Use these tools for precise, compiler-accurate code intelligence:
- `lsp_symbols`     {"path": "src/main.rs"}          — list all symbols in a file
- `lsp_symbols`     {"query": "LspClient"}            — search symbols across workspace
- `lsp_hover`       {"path": "...", "line": N, "character": N}  — type info and docs
- `lsp_definition`  {"path": "...", "line": N, "character": N}  — go to definition
- `lsp_references`  {"path": "...", "line": N, "character": N}  — find all usages
- `lsp_diagnostics` {"path": "src/main.rs"}           — errors and warnings in a file
- `lsp_diagnostics` {}                                 — all workspace diagnostics

Prefer these over grep/read for: type information, definition lookup, finding usages,
and checking errors before editing.
```

If LSP servers have active diagnostics (errors/warnings), also prepend a summary:
```
## Workspace Diagnostics (live)
  src/main.rs:42  error[E0382]  use of moved value: `client`
  src/lib.rs:17   warning       unused variable: `result`
```

---

## Phase 6 — `/lsp` Slash Command (`ragent-tui/src/app.rs`)

### Add to `SLASH_COMMANDS` const:
```rust
SlashCommandDef {
    trigger: "lsp",
    description: "Show LSP server status and code intelligence tools",
},
```

### Add match arm in `execute_slash_command()`:

```
/lsp                  — show all servers and status
/lsp discover         — scan PATH and VS Code extensions for available servers  
/lsp connect <id>     — connect a specific server (if stopped or disabled)
/lsp disconnect <id>  — disconnect a server
/lsp diagnostics      — show current workspace diagnostics from all servers
```

**Example `/lsp` output:**
```
LSP Servers

  rust-analyzer      rust      ● connected   capabilities: hover, definition, references,
                                             symbols, diagnostics, formatting
  typescript-ls      ts,js     ● connected   capabilities: hover, definition, references
  pyright            python    ✗ not found   install: pip install pyright
  gopls              go        ○ disabled    enable in ragent.json

Tools available: lsp_symbols, lsp_hover, lsp_definition, lsp_references, lsp_diagnostics
Run /lsp discover to scan for additional servers.
```

### Status bar addition (`ragent-tui/src/layout.rs`)

Optionally add an LSP indicator to the status bar — a small `⬡ N` (N = connected servers)
in the right section, similar to how MCP tools count could be shown. Only shown when ≥1
server is connected.

---

## Phase 7 — Session Processor Integration

In `ragent-core/src/session/processor.rs`:

1. Accept `lsp_manager: Option<Arc<RwLock<LspManager>>>` in `SessionProcessor::new()`
2. Pass it through to `ToolContext` on each tool call
3. On session start, trigger LSP `workspace/didOpen` or `workspace/didChangeConfiguration`
   for the working directory

In `ragent-tui/src/app.rs`:
1. Add `lsp_servers: Vec<LspServer>` to `App` struct (mirrors `mcp_servers`)
2. Subscribe to a new `Event::LspStatusChanged { server_id, status }` event
3. Start LSP servers after MCP servers during session initialisation

---

## File Change Map

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `lsp-types = "0.97"` to `[workspace.dependencies]` |
| `crates/ragent-core/Cargo.toml` | Add `lsp-types` dependency |
| `crates/ragent-core/src/lsp/mod.rs` | **New** — `LspManager` |
| `crates/ragent-core/src/lsp/client.rs` | **New** — JSON-RPC stdio client |
| `crates/ragent-core/src/lsp/protocol.rs` | **New** — protocol type re-exports + helpers |
| `crates/ragent-core/src/lsp/discovery.rs` | **New** — auto-discovery |
| `crates/ragent-core/src/lsp/server.rs` | **New** — `LspServer`, `LspStatus` |
| `crates/ragent-core/src/config/mod.rs` | Add `lsp: HashMap<String, LspServerConfig>` |
| `crates/ragent-core/src/tool/lsp_symbols.rs` | **New** tool |
| `crates/ragent-core/src/tool/lsp_hover.rs` | **New** tool |
| `crates/ragent-core/src/tool/lsp_definition.rs` | **New** tool |
| `crates/ragent-core/src/tool/lsp_references.rs` | **New** tool |
| `crates/ragent-core/src/tool/lsp_diagnostics.rs` | **New** tool |
| `crates/ragent-core/src/tool/mod.rs` | Register 5 new tools; add `lsp` to `ToolContext` |
| `crates/ragent-core/src/agent/mod.rs` | Extend `build_system_prompt()` with LSP section |
| `crates/ragent-core/src/session/processor.rs` | Accept + thread `LspManager` |
| `crates/ragent-core/src/event/mod.rs` | Add `LspStatusChanged` event |
| `crates/ragent-core/src/lib.rs` | Export `lsp` module |
| `crates/ragent-tui/src/app.rs` | Add `lsp_servers`, `/lsp` command, event handler |
| `crates/ragent-tui/src/layout.rs` | Optional: LSP indicator in status bar |
| `tests/` | New integration tests for LSP tools |
| `SPEC.md` | Document new LSP section and tools |
| `README.md` | Add LSP setup instructions |

---

## Implementation Order

### Milestone 1 — Core client (foundation)
1. Add `lsp-types` to workspace `Cargo.toml`
2. Create `ragent-core/src/lsp/` module skeleton
3. Implement `LspClient` (JSON-RPC stdio transport)
4. Implement `LspServer` + `LspStatus`
5. Implement `LspManager` with `connect()` / `disconnect_all()`
6. Add `LspServerConfig` to `Config` struct

### Milestone 2 — Auto-discovery
7. Implement `discovery.rs` with PATH and VS Code extension scanning
8. Wire auto-discovery into startup in `app.rs`

### Milestone 3 — LLM tools
9. Implement `lsp_symbols` tool
10. Implement `lsp_hover` tool
11. Implement `lsp_definition` tool
12. Implement `lsp_references` tool
13. Implement `lsp_diagnostics` tool
14. Register tools; add `lsp` to `ToolContext`

### Milestone 4 — UX
15. Add `/lsp` slash command (with subcommands)
16. Extend system prompt with LSP section
17. Wire `LspManager` into `SessionProcessor`
18. Add `lsp_servers` to `App` + event handling
19. Add LSP indicator to status bar

### Milestone 5 — Tests & docs
20. Integration tests for each LSP tool (using rust-analyzer against the repo itself)
21. Update `SPEC.md`, `README.md`, `CHANGELOG.md`

---

## Testing Strategy

- **Unit tests**: LspClient JSON-RPC framing, `server_for_file()` extension matching, config parsing
- **Integration tests** (using `rust-analyzer` which is installed at `~/.cargo/bin/rust-analyzer`):
  - Connect to rust-analyzer against the ragent workspace
  - Call `lsp_symbols` on a known file, assert expected symbols present
  - Call `lsp_diagnostics` on a clean build, assert empty
  - Call `lsp_hover` at a known position, assert non-empty result
- **Discovery tests**: Mock PATH with temp dir, assert correct servers detected

---

## Error Handling & Graceful Degradation

- If no LSP server is connected for a file's language → tools return a helpful message:
  `"No LSP server connected for .rs files. Add rust-analyzer to the lsp section of ragent.json"`
- Server crash → `LspStatus::Failed { error }`, reconnect attempt on next tool call
- Timeout → configurable per-server `timeout_ms`, returns partial result with warning
- Server startup failure → shown in `/lsp` output, does not block ragent startup
- Missing `lsp-types` feature → tools simply not registered (feature-gated)

---

## Notes on LSP Protocol Specifics

- **File URIs**: LSP uses `file:///absolute/path` URIs. Use the `url` crate (`Url::from_file_path()`) for conversion.
- **Positions**: LSP uses 0-based line/character (UTF-16 offsets). ragent tools expose 1-based line numbers to the LLM; conversion happens inside the tool implementation.
- **`didOpen`**: Before querying a file, send `textDocument/didOpen` with the file content. Servers may not respond to queries for files they haven't been told about.
- **Diagnostics**: Some servers push diagnostics via `textDocument/publishDiagnostics` notifications rather than responding to requests. The `LspClient` reader task accumulates these in a `Mutex<HashMap<Url, Vec<Diagnostic>>>`.
- **`workspace/diagnostic`** (LSP 3.17+): Pull-based diagnostics — not all servers support it. Fall back to push-diagnostics for older servers.
- **Initialization**: Each server must be initialized with `rootUri` pointing to the project root before any document queries.

---

## Open Questions for Discussion

1. **Opt-in vs opt-out for auto-connected servers**: Should auto-discovered servers connect automatically (behind `experimental.lsp_auto_connect` flag) or require explicit config? → Recommended: require explicit config; auto-discovery only suggests.
2. **Single workspace root**: LSP servers are initialized with one `rootUri`. For multi-root projects, do we start multiple instances? → Start with single root (cwd), revisit later.
3. **File sync**: Should ragent send `textDocument/didChange` notifications after every file edit? → Yes for `create`/`edit`/`write` tools, to keep diagnostics fresh.
4. **Performance**: LSP requests add latency to tool calls (typically 10–500ms for rust-analyzer). Should tool calls be async with a timeout fallback? → Yes, use `timeout_ms` per server config.
5. **Permission model**: Should LSP tools be gated by the same `file:read` permission as the `read` tool? → Yes, since they expose file contents indirectly.
