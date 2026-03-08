# CODE_CLEANUP.md — ragent Code Quality Tasks

This document lists tasks to bring the ragent codebase up to recommended Rust project standards. Items are organized by category, prioritized within each section, and tagged with severity.

> **Legend**: 🔴 Critical · 🟠 High · 🟡 Medium · 🟢 Low

---

## 1. Clippy Lint Compliance

The project currently has **20 clippy warnings** across 4 categories. All should be resolved.

### 1.1 ✅ ~~Add workspace-wide lint configuration~~

**Files**: `Cargo.toml` (workspace root), each `crates/*/Cargo.toml`

Add to the workspace `Cargo.toml`:

```toml
[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"
unwrap_used = "warn"
expect_used = "warn"

[workspace.lints.rust]
missing_docs = "warn"
unsafe_code = "deny"
```

And in each crate's `Cargo.toml`:

```toml
[lints]
workspace = true
```

### 1.2 ✅ ~~Fix collapsible `if` statements (12 warnings)~~

Clippy warns about nested `if` blocks that should use `&&` chains.

| File | Line(s) |
|------|---------|
| `crates/ragent-core/src/config/mod.rs` | 214 |
| `crates/ragent-core/src/permission/mod.rs` | 66 |
| `crates/ragent-core/src/provider/openai.rs` | 293, 312, 339 |
| `crates/ragent-core/src/session/processor.rs` | 324 |
| `crates/ragent-tui/src/app.rs` | 191, 221, 250, 291 |

**Fix**: Collapse nested `if` blocks into single `if ... && ...` expressions.

### 1.3 ✅ ~~Use `&Path` instead of `&PathBuf` (6 warnings)~~

Functions accepting `&PathBuf` should accept `&Path` — a `PathBuf` auto-derefs to `&Path`, but `&Path` is more general (accepts both `Path` and `PathBuf`).

| File | Line |
|------|------|
| `crates/ragent-core/src/tool/edit.rs` | 100 |
| `crates/ragent-core/src/tool/glob.rs` | 129, 139 |
| `crates/ragent-core/src/tool/grep.rs` | 173, 217 |
| `crates/ragent-core/src/tool/list.rs` | 144 |
| `crates/ragent-core/src/tool/read.rs` | 97 |
| `crates/ragent-core/src/tool/write.rs` | 79 |

### 1.4 ✅ ~~Derive `Default` instead of manual impl~~

**File**: `crates/ragent-core/src/config/mod.rs:155`

`ExperimentalFlags::default()` manually sets all fields to their zero values. Replace with `#[derive(Default)]`.

### 1.5 🟢 Remove redundant closure

**File**: `crates/ragent-core/src/session/mod.rs:72`

Replace redundant closure with a direct function reference.

---

## 2. Error Handling

### 2.1 🔴 Replace `.unwrap()` on `Mutex::lock()` with proper error handling

**File**: `crates/ragent-core/src/storage/mod.rs` — 11 call sites (lines 37, 95, 108, 133, 159, 169, 181, 201, 245, 258, 269)

Every `Storage` method calls `self.conn.lock().unwrap()`. A poisoned mutex crashes the process.

**Fix**: Return `Result` from all storage methods and map the lock error:

```rust
let conn = self.conn.lock()
    .map_err(|e| anyhow::anyhow!("database lock poisoned: {e}"))?;
```

### 2.2 🔴 Define a crate-level error type with `thiserror`

The project has `thiserror` in its dependencies but never uses it. All errors are `anyhow::Error` with no structured variants.

**Fix**: Create `crates/ragent-core/src/error.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum RagentError {
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("provider error: {message}")]
    Provider { provider: String, message: String },
    #[error("tool error: {tool}: {message}")]
    Tool { tool: String, message: String },
    #[error("config error: {0}")]
    Config(String),
    #[error("permission denied: {permission} on {pattern}")]
    PermissionDenied { permission: String, pattern: String },
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
```

Use `RagentError` at module boundaries; keep `anyhow::Result` for internal convenience.

### 2.3 🟠 Add context to error returns

Many functions return bare errors without context. Use `.context()` or `.with_context()`:

| File | Issue |
|------|-------|
| `provider/anthropic.rs:189` | `response.text().await.unwrap_or_default()` — silently swallows HTTP errors |
| `provider/openai.rs:245` | Same pattern |
| `session/processor.rs:221` | `serde_json::from_str(&tc.args_json).unwrap_or(json!({}))` — silently replaces malformed tool args |

**Fix**: Log errors at warn level and propagate with context.

### 2.4 🟠 Remove silent `.unwrap_or_default()` on serialization

**File**: `crates/ragent-server/src/routes/mod.rs` — lines 79, 109, 157, 192

`serde_json::to_value().unwrap_or_default()` silently produces `null` on failure. Either propagate with `?` or log the error.

---

## 3. Type Safety

### 3.1 🔴 Introduce newtype wrappers for IDs

Bare `String` is used for session IDs, message IDs, provider IDs, and tool call IDs — making it easy to accidentally swap them.

**Fix**: Create typed wrappers in a new `crates/ragent-core/src/id.rs`:

```rust
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl $name {
            pub fn new() -> Self { Self(uuid::Uuid::new_v4().to_string()) }
            pub fn as_str(&self) -> &str { &self.0 }
        }
    };
}

define_id!(SessionId);
define_id!(MessageId);
define_id!(ProviderId);
define_id!(ToolCallId);
```

### 3.2 🟠 Replace `String` permission names with an enum

**File**: `crates/ragent-core/src/permission/mod.rs`

The `permission` field in `PermissionRule` is a bare `String`. Typos like `"bah"` instead of `"bash"` would silently fail to match.

**Fix**: Create a `Permission` enum:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read, Edit, Bash, Web, Question,
    PlanEnter, PlanExit, Todo, ExternalDirectory, DoomLoop,
    #[serde(other)]
    Custom(String),
}
```

### 3.3 🟡 Reduce `serde_json::Value` as catch-all

These fields would benefit from typed alternatives:

| File | Field | Better Type |
|------|-------|-------------|
| `agent/mod.rs` | `options: HashMap<String, Value>` | Typed `AgentOptions` struct |
| `config/mod.rs` | `options: HashMap<String, Value>` | Typed `ProviderOptions` struct |
| `mcp/mod.rs` | `parameters: Value` | `JsonSchema` newtype |
| `llm/mod.rs` | `parameters: Value` | `JsonSchema` newtype |

### 3.4 🟡 Strengthen `PermissionReply` in server routes

**File**: `crates/ragent-server/src/routes/mod.rs:254`

`PermissionReply { decision: String }` should use `PermissionDecision` enum directly so invalid strings are rejected at deserialization.

---

## 4. Documentation

### 4.1 🟠 Add crate-level documentation

No crate has a top-level `//!` doc comment. Add to each `lib.rs`:

```rust
//! ragent-core — Core library for the ragent AI coding agent.
//!
//! Provides types, traits, and implementations for LLM providers,
//! tool execution, session management, and configuration.
```

### 4.2 🟠 Document all public items

**50+ public types, traits, and functions lack `///` doc comments.** Key items needing docs:

| Module | Missing Docs On |
|--------|----------------|
| `agent/mod.rs` | `AgentMode`, `ModelRef`, `AgentInfo`, `create_builtin_agents`, `resolve_agent`, `build_system_prompt` |
| `config/mod.rs` | `Config`, `ProviderConfig`, `ApiConfig`, `ModelConfig`, all `merge` / `load` functions |
| `event/mod.rs` | `Event` (all 14 variants), `EventBus`, `FinishReason` |
| `llm/mod.rs` | `StreamEvent`, `ChatRequest`, `ChatMessage`, `LlmClient` trait |
| `message/mod.rs` | `Message`, `MessagePart`, `ToolCallState`, `Role` |
| `permission/mod.rs` | `PermissionChecker`, `PermissionAction`, `PermissionRule` |
| `provider/mod.rs` | `ModelInfo`, `ProviderInfo`, `Provider` trait, `ProviderRegistry` |
| `session/mod.rs` | `Session`, `SessionManager`, all methods |
| `session/processor.rs` | `SessionProcessor`, `process_message` |
| `storage/mod.rs` | `Storage`, all CRUD methods |
| `tool/mod.rs` | `Tool` trait, `ToolRegistry`, `ToolOutput`, `ToolContext` |
| `tool/*.rs` | All 8 tool structs (`ReadTool`, `WriteTool`, etc.) |

### 4.3 🟡 Document error conditions

Functions returning `Result` should document what errors can occur:

```rust
/// Sends a message to the agent and processes the response.
///
/// # Errors
/// - Returns `RagentError::SessionNotFound` if the session ID is invalid.
/// - Returns `RagentError::Provider` if the LLM API call fails.
/// - Returns `RagentError::Tool` if a tool execution fails.
pub async fn process_message(...) -> Result<Message> { ... }
```

### 4.4 🟢 Add module-level docs to submodules

Each `mod.rs` should start with a `//!` comment explaining the module's purpose and key types.

---

## 5. Testing

### 5.1 🔴 Add unit tests to untested modules

**Only 3 of 22 modules have tests** (7 tests total). The following modules have zero test coverage:

| Module | Priority | Recommended Tests |
|--------|----------|-------------------|
| `session/processor.rs` | 🔴 | Agent loop with mock LLM, tool execution flow, doom loop protection |
| `provider/anthropic.rs` | 🔴 | SSE stream parsing, error handling, tool call extraction |
| `provider/openai.rs` | 🔴 | SSE stream parsing, function call handling |
| `agent/mod.rs` | 🟠 | `resolve_agent` with config overrides, `build_system_prompt` output |
| `config/mod.rs` | 🟠 | Config loading precedence, `merge` correctness, JSONC parsing |
| `event/mod.rs` | 🟠 | Publish/subscribe, multiple subscribers, buffer overflow |
| `tool/bash.rs` | 🟠 | Command execution, timeout, output capture |
| `tool/read.rs` | 🟠 | Line ranges, missing files, binary files |
| `tool/edit.rs` | 🟠 | Exact match replacement, no-match error, multiple matches |
| `tool/grep.rs` | 🟡 | Pattern matching, case sensitivity, max results |
| `tool/glob.rs` | 🟡 | Pattern matching, depth limits |
| `tool/list.rs` | 🟡 | Depth control, empty directories |
| `tool/write.rs` | 🟡 | Parent directory creation, overwrite |
| `message/mod.rs` | 🟡 | Serialization roundtrip, Display impl |
| `llm/mod.rs` | 🟢 | Type construction, serialization |
| `mcp/mod.rs` | 🟢 | Stub correctness |

### 5.2 🟠 Add integration tests

Create `tests/integration/` with:

- **Full message flow**: user input → mock LLM → tool execution → stored response
- **Concurrent sessions**: multiple sessions sharing storage and event bus
- **Permission flow**: Ask → grant → re-check behavior
- **Config precedence**: global + project config merging

### 5.3 🟡 Add a mock LLM server for testing

Create a test helper that returns canned SSE responses (text, tool calls, errors) so provider and processor tests can run without real API keys.

### 5.4 🟡 Enable `#[cfg(test)]` conditional compilation

The test utilities should be behind `#[cfg(test)]` to avoid shipping test code in release builds.

---

## 6. Security

### 6.1 🔴 Encrypt API keys at rest

**File**: `crates/ragent-core/src/storage/mod.rs`

API keys are stored as plaintext in SQLite. Anyone with read access to `~/.local/share/ragent/ragent.db` can extract all provider credentials.

**Fix**: Encrypt API keys before storage using a key derived from the user's OS keyring (via the `keyring` crate) or a passphrase. At minimum, use a fixed-key XOR obfuscation with a warning that it's not secure against targeted attacks.

### 6.2 🔴 Add authentication to HTTP server

**File**: `crates/ragent-server/src/routes/mod.rs`

All API endpoints are unauthenticated. Any local process can:
- Read/modify sessions and messages
- Approve/deny permission requests
- Access API keys

**Fix**: Generate a random bearer token on server start, require it on all endpoints, and pass it to the TUI client. Consider also binding to `127.0.0.1` only (already done) and Unix sockets.

### 6.3 🟠 Audit bash tool for command injection

**File**: `crates/ragent-core/src/tool/bash.rs`

Commands are passed directly to `bash -c`. While the permission system gates execution, there is no sanitization or logging.

**Fix**:
1. Log every bash command at `info` level before execution
2. Consider a deny-list for destructive patterns (`rm -rf /`, `mkfs`, `dd if=`, `:(){:|:&};:`)
3. Document that bash tool is trusted-user-only

### 6.4 🟠 Sanitize log output to prevent secret leakage

**File**: `crates/ragent-core/src/session/processor.rs`

LLM error responses may include API keys in headers or URLs. Errors are logged without redaction.

**Fix**: Create a `redact_secrets(msg: &str) -> String` utility that strips known key patterns (`sk-...`, `key-...`, bearer tokens).

### 6.5 🟠 Validate working directory paths

**Files**: `src/main.rs`, `crates/ragent-server/src/routes/mod.rs`

The `directory` field in session creation accepts arbitrary paths from the API.

**Fix**: Canonicalize paths and reject those outside the user's home directory or a configured allow-list.

### 6.6 🟡 Add rate limiting to message endpoint

**File**: `crates/ragent-server/src/routes/mod.rs`

No rate limiting on `POST /sessions/:id/messages`. Add a token-bucket or sliding-window rate limiter using `tower::limit`.

---

## 7. Concurrency

### 7.1 🟠 Use `tokio::sync::Mutex` instead of `std::sync::Mutex` for async-accessed data

**Files**:
- `crates/ragent-core/src/storage/mod.rs` — `conn: Mutex<Connection>` (std)
- `src/main.rs` — `PermissionChecker` wrapped in `std::sync::Mutex`

Standard `Mutex` blocks the async executor thread while locked. If any `.lock()` call happens across an `.await` point, it can starve the runtime.

**Fix**: Replace `std::sync::Mutex` with `tokio::sync::Mutex` for data accessed in async contexts. Alternatively, use a dedicated blocking thread for SQLite via `tokio::task::spawn_blocking`.

### 7.2 🟡 Use `RwLock` for read-heavy data

`PermissionChecker` is read-heavy (checking permissions) and rarely written to (recording "always" grants). Wrap in `tokio::sync::RwLock` instead of `Mutex`.

### 7.3 🟡 Document EventBus overflow behavior

**File**: `crates/ragent-core/src/event/mod.rs`

The broadcast channel (capacity 256) silently drops events when subscribers lag. This could cause missed permission requests.

**Fix**: Document the buffer size, add warn-level logging when `send()` returns `Err`, and consider increasing capacity or using an unbounded channel for critical events.

---

## 8. API Design

### 8.1 🟠 Implement `From` conversions for type mapping

Several manual conversion functions should be `impl From`:

| From | To | File |
|------|-----|------|
| `SessionRow` | `Session` | `session/mod.rs:93` (`row_to_session()`) |
| `&[Message]` | `Vec<ChatMessage>` | `session/processor.rs:350` (`history_to_chat_messages()`) |
| `MessagePart` | `ContentPart` | `session/processor.rs:370` (`parts_to_chat_content()`) |

### 8.2 🟠 Consolidate helper functions with many parameters into context structs

| Function | File | Params | Suggested Struct |
|----------|------|--------|------------------|
| `search_directory()` | `tool/grep.rs:120` | 7 | `SearchContext { pattern, case_insensitive, glob, max_results }` |
| `list_recursive()` | `tool/list.rs:62` | 5 | `ListOptions { max_depth, prefix }` |

### 8.3 🟡 Remove dead code

| Item | File | Issue |
|------|------|-------|
| `get_storage()` | `session/processor.rs:337` | Always returns `None`, never called |
| `SessionRow` (as public) | `storage/mod.rs:280` | Implementation detail; should be `pub(crate)` or private |

### 8.4 🟡 Add `impl Display` for key enums

These types are user-facing and benefit from human-readable `Display`:

| Type | File |
|------|------|
| `PermissionAction` | `permission/mod.rs` |
| `ToolCallStatus` | `message/mod.rs` |
| `McpStatus` | `mcp/mod.rs` |
| `AgentMode` | `agent/mod.rs` |
| `FinishReason` | `event/mod.rs` |

### 8.5 🟢 Add `Default` impls where sensible

| Type | File |
|------|------|
| `AgentInfo` | `agent/mod.rs` |
| `ToolOutput` | `tool/mod.rs` |

---

## 9. Performance

### 9.1 🟠 Use `tokio::fs` instead of `std::fs` in async functions

**Files**: `session/processor.rs`, `tool/read.rs`, `tool/write.rs`, `tool/edit.rs`, `tool/grep.rs`, `tool/glob.rs`, `tool/list.rs`

Synchronous filesystem calls (`std::fs::read_to_string`, `std::fs::read_dir`) block the tokio executor thread. In an async context, these should use `tokio::fs` equivalents or be wrapped in `tokio::task::spawn_blocking`.

### 9.2 🟡 Reduce unnecessary `.clone()` calls

| File | Context | Fix |
|------|---------|-----|
| `session/processor.rs:104` | `chat_messages.clone()` cloned per LLM call in agent loop | Build in place or use `Cow` |
| `session/processor.rs:81` | `history_to_chat_messages(&history)` rebuilds from full history each iteration | Incrementally append |
| `session/processor.rs:156` | `id.clone(), name.clone()` on already-owned values | Use `std::mem::take` or move |
| `routes/mod.rs:108` | `SessionRow → Session` conversion allocates per row | Pre-allocate vec with capacity |

### 9.3 🟡 Use `String` buffer reuse in search loops

**File**: `crates/ragent-core/src/tool/grep.rs:193`

Each search match creates a new `format!()` allocation. Use a reusable `String` buffer with `write!`.

### 9.4 🟢 Consider `Cow<'static, str>` for event fields

**File**: `crates/ragent-core/src/event/mod.rs`

Event variants use owned `String` fields even for static data (e.g., tool names). `Cow<'static, str>` avoids allocation for known-at-compile-time strings.

---

## 10. Rust Idioms

### 10.1 🟡 Use `let-else` and `if-let` chains

Several places use the anti-pattern:

```rust
if x.is_some() {
    let val = x.unwrap();
}
```

Replace with:

```rust
let Some(val) = x else { return; };
// or
if let Some(val) = x { ... }
```

Affected files: `agent/mod.rs`, `session/processor.rs`, `provider/openai.rs`.

### 10.2 🟡 Use `.ok_or_else()` instead of `match Some/None`

**File**: `session/processor.rs:238`

```rust
// Before
let result = match self.tool_registry.get(&tc.name) {
    Some(tool) => tool.execute(...).await,
    None => Err(anyhow::anyhow!("Unknown tool: {}", tc.name)),
};

// After
let tool = self.tool_registry.get(&tc.name)
    .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tc.name))?;
let result = tool.execute(...).await;
```

### 10.3 🟡 Use `?` operator instead of `.unwrap_or_else(|_| ...)`

**File**: `session/mod.rs:94-104`

Date parsing falls back to `Utc::now()` on error — this silently corrupts timestamps. Either propagate the error with `?` or log a warning.

### 10.4 🟢 Prefer `impl Into<String>` for constructor parameters

Functions like `AgentInfo::new(name: &str, description: &str)` force callers to pass `&str`. Using `impl Into<String>` is more flexible:

```rust
pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self { ... }
```

---

## 11. Project Structure & Configuration

### 11.1 🟡 Add a `README.md`

The project has `SPEC.md` but no `README.md` with quickstart instructions, build commands, and usage examples.

### 11.2 ✅ ~~Add a `.gitignore`~~

Ensure `target/`, `*.db`, and secret files are excluded from version control.

### 11.3 🟡 Add `LICENSE` file

`Cargo.toml` declares `license = "MIT"` but no `LICENSE` file exists.

### 11.4 🟢 Add CI configuration

Add a GitHub Actions workflow (`.github/workflows/ci.yml`) that runs:

```yaml
- cargo fmt --check
- cargo clippy --workspace -- -D warnings
- cargo test --workspace
- cargo build --release
```

### 11.5 🟠 Add `rustfmt.toml` matching AGENTS.md requirements

**AGENTS.md specifies**: 4 spaces, max 100 width, `reorder_imports=true`, Unix newlines.

No `rustfmt.toml` exists. Create one:

```toml
edition = "2024"
max_width = 100
tab_spaces = 4
reorder_imports = true
newline_style = "Unix"
use_field_init_shorthand = true
```

### 11.6 ✅ ~~Add `.gitignore` with AGENTS.md requirements~~

AGENTS.md requires `target/temp` to be gitignored. No `.gitignore` exists at all.

```gitignore
/target/
*.db
*.key
.env
```

### 11.7 🟡 Create `target/temp` directory for temporary files

**AGENTS.md requirement**: Use `target/temp/` for all temporary files instead of `/tmp`.

The directory does not exist and is not referenced anywhere in the code.

### 11.8 🟡 Add `LICENSE` file

`Cargo.toml` declares `license = "MIT"` but no `LICENSE` file exists.

### 11.9 🟢 Add CI configuration

Add a GitHub Actions workflow (`.github/workflows/ci.yml`) that runs:

```yaml
- cargo fmt --check
- cargo clippy --workspace -- -D warnings
- timeout 600 cargo test --workspace
- cargo build
```

---

## 12. AGENTS.md Compliance

This section covers requirements from `AGENTS.md` that are **not currently implemented** in the codebase. Each item references the specific AGENTS.md rule it violates.

### 12.1 🔴 Migrate all inline tests to `tests/` directories

**AGENTS.md rule**: *"All tests MUST be located in the `tests/` inside each crate... NOT inline in source files."*

**Current state**: All 7 tests are inline (`#[cfg(test)]` modules inside source files):

| File | Inline Tests |
|------|-------------|
| `crates/ragent-core/src/permission/mod.rs` | 3 tests (`test_default_action_is_ask`, `test_allow_rule`, `test_always_grant`) |
| `crates/ragent-core/src/snapshot/mod.rs` | 1 test (`test_snapshot_roundtrip`) |
| `crates/ragent-core/src/storage/mod.rs` | 3 tests (`test_storage_roundtrip`, `test_provider_auth`, `test_archive_session`) |

**No crate has a `tests/` directory.** Required structure:

```
crates/ragent-core/tests/
├── permission/
│   └── test_permission_checker.rs
├── snapshot/
│   └── test_snapshot_roundtrip.rs
└── storage/
    └── test_storage_crud.rs
```

**Fix**: For each crate:
1. Create `crates/<crate>/tests/` directory with subfolders by component
2. Move all `#[cfg(test)]` module contents into separate test files
3. Replace `use super::*` with `use ragent_core::<module>::*` (public API imports)
4. Remove inline `#[cfg(test)]` blocks from source files
5. Verify tests still pass via `cargo test --workspace`

### 12.2 🔴 Remove all `println!` / `eprintln!` — use `tracing` instead

**AGENTS.md rule**: *"Use `tracing` crate with structured logging, avoid `println!` or `eprintln!` in any phase of development."*

**Current state**: `src/main.rs` has **16 occurrences** of `println!` / `eprintln!`:

| Line(s) | Usage | Replacement |
|---------|-------|-------------|
| 161-162 | "ragent interactive mode" startup message | `tracing::info!("Starting interactive mode")` |
| 175 | Print message response | `tracing::info!(response = %msg.text_content())` |
| 176 | Print error | `tracing::error!(error = %e, "Failed to process message")` |
| 189 | Print run result | `tracing::info!(response = %msg.text_content())` |
| 191 | Print run error | `tracing::error!(error = %e, "Run failed")` |
| 209 | "No sessions found" | `tracing::info!("No sessions found")` |
| 212 | List sessions | `tracing::info!(id = %s.id, title = %s.title)` |
| 223 | "Resuming session" | `tracing::info!(id = %id, "Resuming session")` |
| 229 | Print export JSON | Write to stdout via `std::io::Write` (acceptable for data output) |
| 235 | "Imported session" | `tracing::info!(file = %file, "Imported session")` |
| 241 | "Stored API key" | `tracing::info!(provider = %provider, "Stored API key")` |
| 246-250 | List models | Use structured output |
| 258 | Print config JSON | Write to stdout via `std::io::Write` (acceptable for data output) |

**Note**: Data output commands (`config`, `export`) may use stdout writes, but informational/error messages must use `tracing`.

### 12.3 🔴 Add DOCBLOCK comments to every function and module

**AGENTS.md rule**: *"For all functions create DOCBLOCK documentation comments above each function... For all modules place a DOCBLOCK at the top of the file."*

**Current state**:
- **0 of 35 source files** have module-level `//!` docblocks
- **0 of ~120 public functions** in ragent-core have `///` doc comments (except 23 lines in `src/main.rs` from clap derives)
- No function documents its arguments or return values

**Fix**: Every `.rs` file needs a module docblock (`//!`), and every `pub fn` / `pub struct` / `pub enum` / `pub trait` needs a `///` docblock describing purpose, arguments, and return values.

### 12.4 🔴 Remove wildcard imports (`use super::*`)

**AGENTS.md rule**: *"No wildcard imports"*

**Current state**: 3 wildcard imports in test modules:

| File | Line |
|------|------|
| `permission/mod.rs` | `use super::*` (line 91) |
| `snapshot/mod.rs` | `use super::*` (line 52) |
| `storage/mod.rs` | `use super::*` (line 296) |

**Fix**: Replace with explicit imports. This will also be resolved by task 12.1 (migrating tests out of source files).

### 12.5 ✅ ~~Set version to `0.1.0-alpha` during development~~

**AGENTS.md rule**: *"During development the release number will have `-alpha` appended to the end."*

**Current state**: Version is `"0.1.0"` without the `-alpha` suffix.

**Fix**: Update `Cargo.toml`:

```toml
[workspace.package]
version = "0.1.0-alpha"
```

### 12.6 🟠 Add `CHANGELOG.md` in Keep a Changelog format

**AGENTS.md rule**: *"Maintain a changelog... Follow Keep a Changelog format."*

**Current state**: No `CHANGELOG.md` exists.

**Fix**: Create `CHANGELOG.md` at project root:

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha] - Unreleased

### Added
- Initial project scaffolding with Cargo workspace
- Core library (ragent-core): agent, config, event, llm, mcp, message, permission, provider, session, snapshot, storage, tool modules
- Provider adapters for Anthropic and OpenAI
- 8 built-in tools: read, write, edit, bash, grep, glob, list, question
- Permission system with glob-based rule matching
- SQLite storage for sessions, messages, and provider auth
- HTTP server (ragent-server) with REST + SSE endpoints
- Terminal UI (ragent-tui) with ratatui
- CLI entry point with clap (run, serve, session, auth, models, config commands)
```

### 12.7 🟠 Add `RELEASE.md`

**AGENTS.md rule**: *"Write the version number and the most recent CHANGELOG.md entry to the RELEASE.md file."*

**Current state**: No `RELEASE.md` exists.

### 12.8 🟠 Create `docs/` directory for documentation

**AGENTS.md rule**: *"All documentation markdown files MUST be located in the `docs/` folder, except for STATS.md, SPEC.md, AGENTS.md, README.md, PLAN.md and CHANGELOG.md."*

**Current state**: No `docs/` directory exists. `CODE_CLEANUP.md` is in the project root but is not in the exemption list — it should be at `docs/CODE_CLEANUP.md`.

**Fix**:
1. Create `docs/` directory
2. Move `CODE_CLEANUP.md` → `docs/CODE_CLEANUP.md`
3. Any future documentation files go into `docs/`

### 12.9 🟠 Add `README.md`

**AGENTS.md rule**: `README.md` is listed as a required root file.

**Current state**: No `README.md` exists. Should include: project description, quickstart, build commands, usage examples, and a link to `SPEC.md`.

### 12.10 🟠 Enforce import grouping: std → external → local

**AGENTS.md rule**: *"Group std, external crates, then local modules; reorder automatically."*

**Current state**: Import ordering is inconsistent across files. The `rustfmt.toml` (once created per 11.5) with `reorder_imports = true` will handle automatic reordering, but `group_imports = "StdExternalCrate"` should also be set (requires nightly rustfmt or manual enforcement).

**Fix**: Add to `rustfmt.toml`:

```toml
group_imports = "StdExternalCrate"
```

And manually audit files where imports mix `std::`, external crates, and `crate::` imports in the same block.

### 12.11 ✅ ~~Enable `warn(missing_docs)` and cognitive complexity lint~~

**AGENTS.md rule**: *"warn on missing docs"* and *"cognitive complexity ≤30"*

**Current state**: No lint attributes set. No `#![warn(missing_docs)]` in any crate.

**Fix**: Add to each crate's `lib.rs`:

```rust
#![warn(missing_docs)]
#![warn(clippy::cognitive_complexity)]
```

Or configure in workspace `Cargo.toml`:

```toml
[workspace.lints.rust]
missing_docs = "warn"

[workspace.lints.clippy]
cognitive_complexity = { level = "warn", priority = -1 }
```

### 12.12 🟡 Use `thiserror` for custom error types (not just `anyhow`)

**AGENTS.md rule**: *"Use `Result<T, E>` with `?`, `anyhow::Result` for main, `thiserror` for custom errors."*

**Current state**: `thiserror` is a dependency but is never used. All errors are `anyhow::Error` with no structured variants. See also task 2.2 for the proposed `RagentError` type.

### 12.13 🟡 Add `STATS.md`

**AGENTS.md rule**: STATS.md is listed as a required root file (exempted from `docs/`).

**Current state**: No `STATS.md` exists. Should contain project statistics (lines of code, test count, binary size, etc.).

### 12.14 🟡 Test naming convention: `test_<component>_<scenario>`

**AGENTS.md rule**: *"Follow naming convention: `test_<component>_<scenario>`"*

**Current state**: Most test names follow this pattern, but some are vague:

| Current Name | Better Name |
|-------------|-------------|
| `test_default_action_is_ask` | `test_permission_default_action_is_ask` |
| `test_allow_rule` | `test_permission_allow_rule_matches` |
| `test_always_grant` | `test_permission_always_grant_overrides` |
| `test_storage_roundtrip` | ✅ Already follows pattern |
| `test_provider_auth` | `test_storage_provider_auth_crud` |
| `test_archive_session` | `test_storage_archive_session` |
| `test_snapshot_roundtrip` | ✅ Already follows pattern |

### 12.15 🟢 Apply Rust best practices from AGENTS.md reference

**AGENTS.md rule**: *"Read the best practices at https://www.djamware.com/post/... and apply to the project."*

This reference covers project structure and clean code practices. A review against that guide should be performed and findings incorporated.

---

## Summary

| Category | 🔴 Critical | 🟠 High | 🟡 Medium | 🟢 Low | ✅ Done | Total |
|----------|-------------|---------|-----------|--------|--------|-------|
| Clippy Compliance | 0 | 0 | 2 | 1 | **1** | 4 |
| Error Handling | 2 | 2 | 0 | 0 | 0 | 4 |
| Type Safety | 1 | 1 | 2 | 0 | 0 | 4 |
| Documentation | 0 | 2 | 1 | 1 | 0 | 4 |
| Testing | 1 | 1 | 2 | 0 | 0 | 4 |
| Security | 2 | 3 | 1 | 0 | 0 | 6 |
| Concurrency | 0 | 1 | 2 | 0 | 0 | 3 |
| API Design | 0 | 2 | 2 | 1 | 0 | 5 |
| Performance | 0 | 1 | 2 | 1 | 0 | 4 |
| Rust Idioms | 0 | 0 | 3 | 1 | 0 | 4 |
| Project Config | 0 | 1 | 1 | 1 | **2** | 5 |
| AGENTS.md Compliance | 4 | 5 | 3 | 1 | **2** | 15 |
| **Total** | **10** | **19** | **21** | **7** | **5** | **62** |

### Recommended Priority Order

1. **AGENTS.md Compliance (Critical)** — Migrate inline tests to `tests/` dirs, remove `println!`/`eprintln!`, add docblocks, remove wildcard imports
2. **Security** — Encrypt API keys, add server auth, validate paths
3. **Error Handling** — Replace `.unwrap()` on locks, create `RagentError` with `thiserror`
4. **AGENTS.md Compliance (High)** — Set `-alpha` version, add CHANGELOG.md, RELEASE.md, README.md, `docs/` directory, enforce import grouping
5. **Testing** — Add unit tests to processor, providers, and tools (in `tests/` dirs per AGENTS.md)
6. **Type Safety** — Introduce newtype IDs, permission enum
7. **Documentation** — Doc comments on all public items (required by AGENTS.md)
8. **Clippy** — Resolve all 20 warnings, add `rustfmt.toml`, enable pedantic lints
9. **Concurrency** — Switch to `tokio::sync::Mutex`, use `RwLock`
10. **Performance** — Use `tokio::fs`, reduce cloning
11. **API Design** — `From` impls, context structs, remove dead code
12. **Project Config** — Add LICENSE, CI, STATS.md
13. **Idioms** — Clean up patterns, apply best practices reference
