# Ragent LSP Integration Planning Guide

## Executive Summary
Ragent is a Rust-based multi-agent AI coding assistant with a modular architecture. The project is organized into 3 crates (ragent-core, ragent-tui, ragent-server) with a well-defined tool system, event bus architecture, and existing MCP integration. For LSP integration, you'll want to leverage the existing tool registration system and event bus, likely adding LSP tools as native tools or MCP servers.

---

## 1. Crate Structure

### **Workspace Layout**
```
/home/thawkins/Projects/ragent/
├── Cargo.toml (workspace root)
├── src/main.rs (CLI entry point)
├── crates/
│   ├── ragent-core/      # Core library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs    # Main library exports
│   │       ├── agent/    # Agent definitions & registry
│   │       ├── config/   # Config loading & types
│   │       ├── event/    # Event bus & types
│   │       ├── llm/      # LLM abstraction layer
│   │       ├── mcp/      # MCP client integration
│   │       ├── message/  # Message types & history
│   │       ├── permission/ # Permission system
│   │       ├── provider/ # LLM provider registry
│   │       ├── session/  # Session management & processor
│   │       ├── skill/    # Skill loading & invocation
│   │       ├── storage/  # SQLite persistence
│   │       ├── task/     # Sub-agent task management
│   │       ├── tool/     # Tool trait & built-in tools
│   │       └── ...
│   ├── ragent-tui/       # Terminal UI
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── app.rs    # App state & slash commands
│   │       ├── input.rs  # Keyboard event handling
│   │       ├── ...
│   └── ragent-server/    # HTTP API server
│       ├── Cargo.toml
│       └── src/
│           ├── routes/   # API endpoints
│           └── ...
```

### **Key Dependencies**
- **Async Runtime**: tokio (full features)
- **Serialization**: serde, serde_json, serde_yaml
- **LLM Clients**: Custom abstraction (ragent_core::provider)
- **MCP SDK**: rmcp (v0.16 with child-process & HTTP transports)
- **TUI**: ratatui + crossterm
- **HTTP Server**: axum (Tokio-based)
- **Database**: rusqlite (SQLite)
- **File I/O**: zipfile, docx-rust, calamine, ooxmlsdk, pdf-extract for Office/PDF support

---

## 2. Tool System Architecture

### **Location**: `/home/thawkins/Projects/ragent/crates/ragent-core/src/tool/mod.rs`

### **Core Trait Definition**
```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;                      // Unique identifier
    fn description(&self) -> &str;               // Human-readable description
    fn parameters_schema(&self) -> Value;        // JSON Schema for params
    fn permission_category(&self) -> &str;       // e.g., "file:read"
    async fn execute(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput>;
}
```

### **Tool Context** (passed to each tool)
```rust
pub struct ToolContext {
    pub session_id: String,
    pub working_dir: PathBuf,
    pub event_bus: Arc<EventBus>,
    pub storage: Option<Arc<crate::storage::Storage>>,
    pub task_manager: Option<Arc<crate::task::TaskManager>>,
}
```

### **Tool Output**
```rust
pub struct ToolOutput {
    pub content: String,              // Main output text
    pub metadata: Option<Value>,      // Optional structured metadata
}
```

### **Tool Registry**
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, tool: Arc<dyn Tool>);
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn list(&self) -> Vec<Arc<dyn Tool>>;
    pub fn export(&self) -> Vec<ToolDefinition>;
}
```

### **Built-in Tools** (in `/crates/ragent-core/src/tool/`)
- **bash.rs** - Shell command execution (with timeout & security checks)
- **read.rs** - File reading (with line range support)
- **write.rs** - File writing
- **edit.rs** - Edit file sections
- **create.rs** - Create new files
- **patch.rs** - Apply patches
- **multiedit.rs** - Batch edits
- **grep.rs** - Search file contents
- **glob.rs** - File pattern matching
- **list.rs** - Directory listing
- **question.rs** - User input dialog
- **todo.rs** - TODO tracking
- **plan.rs** - Planning support
- **webfetch.rs** - HTTP GET
- **websearch.rs** - Search integration
- **office_read.rs, office_write.rs, office_info.rs** - Office document handling
- **pdf_read.rs, pdf_write.rs** - PDF handling
- **new_task.rs, list_tasks.rs, cancel_task.rs** - Task management

### **Tool Registration Pattern**
```rust
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(BashTool));
    registry.register(Arc::new(ReadTool));
    // ... more tools
    registry
}
```

### **LSP Integration Strategy for Tools**
- **Option 1 (Native)**: Implement LSP as a built-in tool in `tool/lsp.rs`
- **Option 2 (MCP)**: Wrap LSP client as an MCP server configuration
- **Option 3 (Hybrid)**: Use both for different code-intelligence features

---

## 3. Slash Commands System

### **Location**: `/home/thawkins/Projects/ragent/crates/ragent-tui/src/app.rs` (lines 159+)

### **Slash Command Definition**
```rust
pub struct SlashCommandDef {
    pub trigger: &'static str,        // Command name (without /)
    pub description: &'static str,    // Help text
}

pub const SLASH_COMMANDS: &[SlashCommandDef] = &[
    SlashCommandDef { trigger: "about", description: "..." },
    SlashCommandDef { trigger: "agent", description: "..." },
    SlashCommandDef { trigger: "clear", description: "..." },
    SlashCommandDef { trigger: "compact", description: "..." },
    SlashCommandDef { trigger: "help", description: "..." },
    SlashCommandDef { trigger: "log", description: "..." },
    SlashCommandDef { trigger: "model", description: "..." },
    SlashCommandDef { trigger: "provider", description: "..." },
    SlashCommandDef { trigger: "provider_reset", description: "..." },
    SlashCommandDef { trigger: "quit", description: "..." },
    SlashCommandDef { trigger: "resume", description: "..." },
    SlashCommandDef { trigger: "system", description: "..." },
    SlashCommandDef { trigger: "tools", description: "..." },
    SlashCommandDef { trigger: "skills", description: "..." },
    SlashCommandDef { trigger: "cancel", description: "..." },
];
```

### **Command Handler** (lines 1158+)
```rust
pub fn execute_slash_command(&mut self, raw: &str) {
    let stripped = raw.strip_prefix('/').unwrap_or(raw).trim();
    let (cmd, args) = stripped.split_once(char::is_whitespace)
        .map_or((stripped, ""), |(c, a)| (c, a.trim()));
    
    match cmd {
        "about" => { /* ... */ }
        "agent" => { /* ... */ }
        "clear" => { /* ... */ }
        // ... more patterns
        _ => { /* unknown command */ }
    }
}
```

### **Input Routing** (`/home/thawkins/Projects/ragent/crates/ragent-tui/src/input.rs`, lines 58+)
```rust
pub enum InputAction {
    SendMessage(String),
    SlashCommand(String),   // Captured here
    SwitchAgent,
    ScrollUp,
    ScrollDown,
    // ...
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<InputAction> {
    // Permission dialog intercepts y/n/a
    // Slash menu intercepts up/down/enter
    // Otherwise returns SendMessage, SlashCommand, etc.
}
```

### **Slash Menu** (dynamic auto-complete)
- Lines 105-150 in input.rs show menu navigation
- Menu shows matching commands as user types `/`
- Line 994 in app.rs shows menu filtering logic

### **LSP Integration for Slash Commands**
You could add:
```rust
SlashCommandDef {
    trigger: "lsp",
    description: "Configure LSP servers or query symbols"
}
```
or more granular:
```rust
SlashCommandDef { trigger: "lsp_connect", description: "Connect to LSP server" },
SlashCommandDef { trigger: "lsp_symbols", description: "List symbols in file" },
SlashCommandDef { trigger: "lsp_hover", description: "Get hover info at cursor" },
```

---

## 4. Event System

### **Location**: `/home/thawkins/Projects/ragent/crates/ragent-core/src/event/mod.rs`

### **Event Bus**
```rust
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self { /* ... */ }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> { /* ... */ }
    pub fn publish(&self, event: Event) { /* ... */ }
}
```

### **Event Types** (enum with 30+ variants)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    // Session lifecycle
    SessionCreated { session_id: String },
    SessionUpdated { session_id: String },
    
    // Message streaming
    MessageStart { session_id: String, message_id: String },
    TextDelta { session_id: String, text: String },
    ReasoningDelta { session_id: String, text: String },
    MessageEnd { session_id: String, message_id: String, reason: FinishReason },
    
    // Tool calls
    ToolCallStart { session_id: String, call_id: String, tool: String },
    ToolCallEnd {
        session_id: String,
        call_id: String,
        tool: String,
        error: Option<String>,
        duration_ms: u64,
    },
    
    // Permissions
    PermissionRequested {
        session_id: String,
        request_id: String,
        permission: String,
        description: String,
    },
    PermissionReplied {
        session_id: String,
        request_id: String,
        allowed: bool,
    },
    
    // Agent switching
    AgentSwitched { session_id: String, from: String, to: String },
    AgentSwitchRequested { session_id: String, to: String, task: String, context: String },
    AgentRestoreRequested { session_id: String, summary: String },
    
    // Errors
    AgentError { session_id: String, error: String },
    
    // Token usage (for tracking costs)
    TokenUsage {
        session_id: String,
        provider: String,
        model: String,
        input_tokens: u64,
        output_tokens: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    ToolUse,
    Length,
    ContentFilter,
    Cancelled,
}
```

### **Event Publishing Pattern**
Tools and the session processor publish events directly:
```rust
self.event_bus.publish(Event::ToolCallStart { /* ... */ });
```

### **TUI Event Subscription** (app.rs)
The TUI subscribes to events and updates UI accordingly. See `App::update_from_event()`.

### **LSP Integration with Events**
You could publish LSP-specific events:
```rust
LspServerConnected { session_id: String, server_id: String },
LspSymbolFound { session_id: String, symbol: String, location: String },
LspDiagnostic { session_id: String, file: String, line: u32, message: String },
```

---

## 5. Session & Configuration Management

### **Session Storage** (`/crates/ragent-core/src/session/mod.rs`, lines 18+)
```rust
pub struct Session {
    pub id: String,
    pub title: String,
    pub project_id: String,
    pub directory: PathBuf,
    pub parent_id: Option<String>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub summary: Option<SessionSummary>,
}

pub struct SessionManager {
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
}

impl SessionManager {
    pub fn create_session(&self, directory: PathBuf) -> anyhow::Result<Session>;
    pub fn get_session(&self, id: &str) -> anyhow::Result<Option<Session>>;
    pub fn get_messages(&self, session_id: &str) -> anyhow::Result<Vec<Message>>;
    pub fn update_session(&self, session: &Session) -> anyhow::Result<()>;
}
```

### **Configuration** (`/crates/ragent-core/src/config/mod.rs`)

**Load Precedence** (lines 252+):
1. Compiled defaults
2. Global file: `~/.config/ragent/ragent.json`
3. Project file: `./ragent.json`
4. Environment variable: `RAGENT_CONFIG`
5. Inline content: `RAGENT_CONFIG_CONTENT`

**Config Sections**:
```rust
pub struct Config {
    pub username: Option<String>,
    pub default_agent: String,
    pub provider: HashMap<String, ProviderConfig>,
    pub permission: Vec<PermissionRule>,
    pub agent: HashMap<String, AgentConfig>,
    pub command: HashMap<String, CommandDef>,  // Slash command definitions
    pub mcp: HashMap<String, McpServerConfig>,  // MCP servers
    pub instructions: Vec<String>,              // Extra system prompts
    pub skill_dirs: Vec<String>,
    pub experimental: ExperimentalFlags,
}
```

**MCP Server Config**:
```rust
pub struct McpServerConfig {
    pub type_: McpTransport,           // Stdio, Sse, Http
    pub command: Option<String>,       // Executable path
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub url: Option<String>,
    pub disabled: bool,
}

pub enum McpTransport {
    Stdio,  // Child process stdin/stdout
    Sse,    // Server-Sent Events
    Http,   // Plain HTTP
}
```

**Example Config File**:
```json
{
  "username": "Developer",
  "default_agent": "general",
  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"],
      "models": {
        "claude-sonnet": {
          "name": "Claude 3.5 Sonnet",
          "cost": {"input": 3.0, "output": 15.0},
          "capabilities": {
            "reasoning": false,
            "streaming": true,
            "vision": true,
            "tool_use": true
          }
        }
      }
    }
  },
  "mcp": {
    "example-lsp": {
      "type": "stdio",
      "command": "/usr/local/bin/lsp-server",
      "args": ["--debug"],
      "env": {"LSP_DEBUG": "1"}
    }
  },
  "command": {
    "build": {
      "command": "cargo build --release",
      "description": "Build the project in release mode"
    }
  },
  "instructions": [
    "Always run tests before submitting code.",
    "Follow Rust naming conventions."
  ]
}
```

### **LSP Configuration Integration**
Add an `lsp` section to config:
```json
{
  "lsp": {
    "enabled": true,
    "servers": {
      "rust-analyzer": {
        "type": "stdio",
        "command": "rust-analyzer",
        "args": [],
        "settings": {
          "rust-analyzer.check.command": "clippy"
        }
      },
      "pylsp": {
        "type": "stdio",
        "command": "pylsp",
        "disabled": false
      }
    }
  }
}
```

---

## 6. Agent System Prompt Building

### **Location**: `/crates/ragent-core/src/agent/mod.rs` (lines 409+)

### **System Prompt Structure**
```rust
pub fn build_system_prompt(
    agent: &AgentInfo,
    working_dir: &Path,
    file_tree: &str,
    skills: Option<&crate::skill::SkillRegistry>,
) -> String {
    // Prompt assembly order (per SPEC):
    // 1. Agent role definition (from agent.prompt)
    // 2. Working directory context
    // 3. Project file tree structure
    // 4. AGENTS.md project guidelines (if present)
    // 5. Available skills (agent-invocable skills from registry)
    // 6. Tool usage guidelines
    // 7. File reading best practices
    
    let mut prompt = String::new();
    
    // 1. Role
    if let Some(ref agent_prompt) = agent.prompt {
        prompt.push_str(agent_prompt);
        prompt.push_str("\n\n");
    }
    
    // Skip context for single-step agents (max_steps <= 1)
    let has_tools = agent.max_steps.map_or(true, |s| s > 1);
    if !has_tools { return prompt; }
    
    // 2. Working directory
    prompt.push_str(&format!(
        "## Working Directory\nYou are operating in: {}\n\n",
        working_dir.display()
    ));
    
    // 3. File tree
    if !file_tree.is_empty() {
        prompt.push_str("## Project Structure\n```\n");
        prompt.push_str(file_tree);
        prompt.push_str("\n```\n\n");
    }
    
    // 4. AGENTS.md guidelines
    let agents_md = working_dir.join("AGENTS.md");
    if agents_md.is_file() {
        if let Ok(contents) = std::fs::read_to_string(&agents_md) {
            prompt.push_str("## Project Guidelines (AGENTS.md)\n");
            prompt.push_str(&contents);
            prompt.push_str("\n\n");
        }
    }
    
    // 5. Available skills
    if let Some(registry) = skills {
        let skill_list = if agent.skills.is_empty() {
            registry.list_agent_invocable()
        } else {
            registry.list_agent_invocable().into_iter()
                .filter(|s| agent.skills.contains(&s.name))
                .collect()
        };
        
        if !skill_list.is_empty() {
            prompt.push_str("## Available Skills\n\n");
            for skill in &skill_list {
                let desc = skill.description.as_deref().unwrap_or("(no description)");
                let hint = skill.argument_hint.as_deref().map(|h| format!(" {h}")).unwrap_or_default();
                prompt.push_str(&format!("- `/{}{}`  — {}\n", skill.name, hint, desc));
            }
            prompt.push('\n');
        }
    }
    
    // 6. Tool usage guidelines
    prompt.push_str("## Guidelines\n- Use tools to verify information...\n");
    
    // 7. File reading best practices
    prompt.push_str("## File Reading Best Practices\n...");
    
    prompt
}
```

### **Agent Definition** (lines 48+)
```rust
pub struct AgentInfo {
    pub name: String,
    pub description: String,
    pub mode: AgentMode,                // Primary, Subagent, All
    pub hidden: bool,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub model: Option<ModelRef>,
    pub prompt: Option<String>,         // System prompt template
    pub permission: PermissionRuleset,
    pub max_steps: Option<u32>,
    pub skills: Vec<String>,            // Preloaded skills
    pub options: HashMap<String, Value>, // Provider-specific options
}
```

### **Built-in Agents** (lines 132+)
- **ask** - Q&A without tools (max_steps=1)
- **general** - General coding (max_steps=500)
- **build** - Build/test specialist (max_steps=30)
- **plan** - Planning & analysis (max_steps=20)
- **explore** - Codebase exploration (max_steps=15)
- **title** - Session title generation (max_steps=1, hidden)
- **summary** - Session summarization (max_steps=1, hidden)
- **compaction** - History compaction (max_steps=1, hidden)

### **LSP Awareness in Prompts**
You could add a code-awareness section to agents that use LSP:
```rust
// In agent prompt building
if has_lsp {
    prompt.push_str("## Code Intelligence (LSP)\n\
        You have access to LSP-powered code analysis including:\n\
        - Symbol definitions and references\n\
        - Type information and hover details\n\
        - Diagnostics and errors\n\
        - Completion suggestions\n\n");
}
```

---

## 7. Existing External Process Tools

### **Bash Tool** (`/crates/ragent-core/src/tool/bash.rs`)

**Process Spawning**:
```rust
use tokio::process::Command;

async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
    let command = input["command"].as_str()?;
    let timeout_secs = input["timeout"].as_u64().unwrap_or(120);
    
    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(&ctx.working_dir)
            .output(),
    ).await;
    
    // Process output, exit code, and timing
    match result {
        Ok(Ok(output)) => { /* success */ }
        Ok(Err(e)) => { /* error */ }
        Err(_) => { /* timeout */ }
    }
}
```

**Key Features**:
- Timeout support (default 120s)
- Denied pattern checks (rm -rf /, mkfs, dd, etc.)
- Output truncation (100 KB max)
- Exit code & duration metadata
- stderr capture

### **MCP Client** (`/crates/ragent-core/src/mcp/mod.rs`)

**Architecture**:
```rust
pub struct McpClient {
    servers: Vec<McpServer>,
    connections: Arc<RwLock<HashMap<String, McpConnection>>>,
}

pub struct McpServer {
    pub id: String,
    pub config: McpServerConfig,
    pub status: McpStatus,  // Connected, Disabled, Failed, NeedsAuth
    pub tools: Vec<McpToolDef>,
}

impl McpClient {
    pub async fn connect(&mut self, id: &str, config: McpServerConfig) -> anyhow::Result<()>;
    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        args: Value,
    ) -> anyhow::Result<String>;
    pub fn list_tools(&self) -> Vec<(String, Vec<McpToolDef>)>;
}
```

**Transports**:
- **Stdio**: `Command::new()` to spawn child process
- **SSE**: HTTP Server-Sent Events
- **HTTP**: Plain request/response

**Implementation Details**:
- Uses official `rmcp` SDK (v0.16)
- Tokio-based async I/O
- 120s default timeout for tool calls
- Tool discovery after connection handshake

### **Web Tools**
- **webfetch.rs** - HTTP GET with reqwest
- **websearch.rs** - Search integration

### **LSP Integration Strategy**
LSP servers can be:
1. **Native Bash Calls**: `lsp-cli query-symbols` (via bash tool)
2. **MCP Server**: Wrap an LSP client as MCP server in config
3. **Native Tool**: Implement `/crates/ragent-core/src/tool/lsp.rs` communicating over stdio

Recommended: **Combination of #2 (MCP) and #3 (Native Tool)**
- MCP for external LSP servers
- Native tool for common LSP queries

---

## 8. Message/Context Flow

### **Message Types** (`/crates/ragent-core/src/message/mod.rs`)

```rust
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,              // User, Assistant
    pub parts: Vec<MessagePart>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum MessagePart {
    Text { text: String },
    ToolCall {
        tool: String,
        call_id: String,
        state: ToolCallState,
    },
    Reasoning { text: String },
}

pub struct ToolCallState {
    pub status: ToolCallStatus,  // Pending, Running, Completed, Error
    pub input: Value,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}
```

### **Session Processor** (`/crates/ragent-core/src/session/processor.rs`, lines 77+)

**Main Loop** (`process_message()`):
```
1. Store user message in database
2. Publish MessageStart event
3. Load message history
4. Build system prompt (with file tree, AGENTS.md, skills)
5. Create LLM client (with credentials from storage)
6. Stream LLM response via provider:
   - Publish TextDelta events for text chunks
   - Publish ToolCallStart/End for tool invocations
   - Execute tools: fetch output + embed in conversation
   - Feed tool results back to LLM (iteratively until stop)
7. Publish MessageEnd event with finish reason
8. Store complete message in database
```

### **Tool Execution Flow** (lines 200+)
```rust
// For each tool call from LLM:
1. Check permission (may prompt user via PermissionRequested event)
2. Create ToolContext { session_id, working_dir, event_bus, storage, task_manager }
3. Publish ToolCallStart event
4. Call tool.execute(input, &ctx)
5. Embed ToolOutput content back into conversation
6. Publish ToolCallEnd event with duration/error
7. Feed back to LLM as tool result
```

### **System Prompt Injection** (lines 165+)
```rust
let working_dir = self.session_manager.get_session(session_id)?.map(|s| s.directory).unwrap_or_default();
let file_tree = build_file_tree(&working_dir, 2);
let skill_registry = crate::skill::SkillRegistry::load(&working_dir, &skill_dirs);
let system_prompt = build_system_prompt(agent, &working_dir, &file_tree, Some(&skill_registry));
```

### **LLM Request** (lines 181+)
```rust
let chat_messages = history_to_chat_messages(&history);
let request = ChatRequest {
    model: model_ref.model_id.clone(),
    messages: chat_messages,
    tools: exported_tools,  // From tool registry
    temperature: agent.temperature,
    top_p: agent.top_p,
    max_tokens: None,
    system: Some(system_prompt),  // Injected here
    options: agent.options.clone(),
};
```

### **Message Context Flow Diagram**
```
User Input
    ↓
Store as Message (Role::User)
    ↓
history_to_chat_messages() converts Message → ChatMessage
    ↓
Append system_prompt
    ↓
LLM streams response (TextDelta, ToolCallStart/End)
    ↓
For each ToolCall:
    - Create ToolContext
    - Execute tool
    - Emit ToolCallEnd event
    - Embed output in conversation
    ↓
Loop until finish_reason != ToolUse
    ↓
Store as Message (Role::Assistant, with ToolCall parts)
    ↓
Events streamed to TUI/subscribers
```

### **LSP Tool Output Integration**
When an LSP tool returns results (e.g., symbol definitions), it becomes:
```rust
MessagePart::ToolCall {
    tool: "lsp_query_symbol",
    call_id: "call_123",
    state: ToolCallState {
        status: ToolCallStatus::Completed,
        input: json!({"symbol": "my_function", "file": "src/main.rs"}),
        output: Some(json!({
            "name": "my_function",
            "kind": "function",
            "location": "src/main.rs:42:5",
            "range": "42-50",
            "signature": "fn my_function(x: i32) -> bool"
        })),
        error: None,
        duration_ms: Some(5),
    }
}
```

This output is then fed back to the LLM as:
```json
{
    "type": "tool_result",
    "tool_use_id": "call_123",
    "content": "{\"name\": \"my_function\", ...}"
}
```

---

## 9. Key Integration Points for LSP

### **Option A: Native Tool** (Best for common LSP queries)
```rust
// Create /crates/ragent-core/src/tool/lsp.rs
pub struct LspQueryTool;

#[async_trait::async_trait]
impl Tool for LspQueryTool {
    fn name(&self) -> &str { "lsp_query_symbol" }
    fn description(&self) -> &str { "Query symbol definition via LSP" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {"type": "string"},
                "file": {"type": "string"},
                "line": {"type": "integer"}
            }
        })
    }
    
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Connect to configured LSP server
        // Query symbol definition
        // Return formatted output
    }
}
```

Register in `create_default_registry()`.

### **Option B: MCP Configuration** (Best for flexibility)
```json
{
  "mcp": {
    "lsp-rust-analyzer": {
      "type": "stdio",
      "command": "rust-analyzer",
      "env": {"RA_LOG": "debug"}
    },
    "lsp-pylsp": {
      "type": "stdio",
      "command": "pylsp"
    }
  }
}
```

Then MCP client auto-discovers tools from LSP servers.

### **Option C: Slash Commands** (Best for user control)
```rust
// In SLASH_COMMANDS array
SlashCommandDef {
    trigger: "lsp_connect",
    description: "Connect to LSP server (/lsp_connect <server_id>)"
},
SlashCommandDef {
    trigger: "lsp_symbol",
    description: "Query symbol definition (/lsp_symbol <symbol> [file])"
},

// In execute_slash_command()
"lsp_connect" => {
    // Connect to configured LSP server
    // Publish event
}
"lsp_symbol" => {
    // Query symbol from active LSP connection
}
```

### **Option D: Agent-Specific Skill** (Best for structured workflows)
Create a skill in `skills/lsp_analysis.toml` that the agent can invoke inline.

---

## 10. File Paths Summary

### **Core Module Files**
```
/home/thawkins/Projects/ragent/crates/ragent-core/src/
├── lib.rs                           Main library exports
├── agent/mod.rs                     Agent definitions & system prompt building
├── config/mod.rs                    Configuration loading & types
├── event/mod.rs                     Event bus & event types
├── llm/mod.rs                       LLM abstraction layer
├── mcp/mod.rs                       MCP client integration
├── message/mod.rs                   Message types & conversation history
├── permission/mod.rs                Permission checking & request handling
├── provider/mod.rs                  LLM provider registry & abstractions
├── session/mod.rs                   Session management
├── session/processor.rs             Agentic loop & message processing
├── skill/mod.rs                     Skill loading & invocation
├── storage/mod.rs                   SQLite persistence layer
├── task/mod.rs                      Sub-agent task management
├── tool/mod.rs                      Tool trait & registry
├── tool/bash.rs                     Bash command execution
├── tool/read.rs                     File reading
├── tool/write.rs                    File writing
├── tool/edit.rs                     File editing
├── tool/grep.rs                     File searching
├── tool/glob.rs                     File pattern matching
└── ... (more tools)
```

### **TUI Files**
```
/home/thawkins/Projects/ragent/crates/ragent-tui/src/
├── app.rs                           App state & slash command execution (line 159: SLASH_COMMANDS)
├── input.rs                         Keyboard event handling (line 32: InputAction enum)
├── layout.rs                        UI rendering
├── main.rs                          TUI entry point
└── tests/test_slash_commands.rs     Slash command tests
```

### **Server Files**
```
/home/thawkins/Projects/ragent/crates/ragent-server/src/
└── routes/mod.rs                    HTTP API endpoints
```

### **Configuration Files**
```
/home/thawkins/Projects/ragent/
├── Cargo.toml                       Workspace configuration
├── src/main.rs                      CLI entry point (line 141: main fn)
├── ragent.json                      Project-level config (if exists)
└── ~/.config/ragent/ragent.json     Global config (if exists)
```

---

## 11. Configuration Examples for LSP

### **Simple LSP Configuration**
```json
{
  "mcp": {
    "rust-analyzer": {
      "type": "stdio",
      "command": "rust-analyzer",
      "args": []
    },
    "pylsp": {
      "type": "stdio",
      "command": "pylsp"
    }
  }
}
```

### **Advanced LSP Configuration**
```json
{
  "lsp": {
    "enabled": true,
    "default_server": "rust-analyzer",
    "fallback_to_bash": true,
    "cache_symbols": true,
    "cache_ttl_seconds": 3600,
    "servers": {
      "rust-analyzer": {
        "type": "stdio",
        "command": "rust-analyzer",
        "args": ["--log-file", "/tmp/ra.log"],
        "env": {
          "RA_LOG": "debug",
          "RA_PROFILE": "cpu,memory"
        },
        "settings": {
          "rust-analyzer.check.command": "clippy",
          "rust-analyzer.checkOnSave.allFeatures": true
        },
        "disabled": false,
        "timeout_seconds": 30
      },
      "pylsp": {
        "type": "stdio",
        "command": "pylsp",
        "args": [],
        "disabled": false
      }
    }
  }
}
```

---

## 12. Integration Checklist

- [ ] **Decide Integration Approach**: MCP, Native Tool, Slash Command, or Skill
- [ ] **Create LSP Configuration Schema** in config/mod.rs
- [ ] **Implement LSP Client Library** (wrap lsp-types, jsonrpc, etc.)
- [ ] **Add LSP Tool** (if native) in tool/lsp.rs
- [ ] **Register LSP Tool** in create_default_registry()
- [ ] **Add LSP Slash Commands** (if needed) in app.rs SLASH_COMMANDS
- [ ] **Publish LSP Events** (optional) in event/mod.rs
- [ ] **Update System Prompt** to mention LSP capabilities
- [ ] **Add LSP Tests** in tests/test_lsp_integration.rs
- [ ] **Document LSP Usage** in README/SPEC
- [ ] **Test with Real LSP Servers** (rust-analyzer, pylsp, etc.)

---

## 13. Recommended LSP Tool Interface

```rust
pub struct LspTool {
    clients: Arc<RwLock<HashMap<String, Box<dyn LspClient>>>>,
}

#[async_trait::async_trait]
pub trait LspClient: Send + Sync {
    async fn initialize(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
    async fn goto_definition(&self, file: &str, line: u32, col: u32) -> Result<Vec<Location>>;
    async fn find_references(&self, file: &str, line: u32, col: u32) -> Result<Vec<Location>>;
    async fn hover(&self, file: &str, line: u32, col: u32) -> Result<Option<String>>;
    async fn completion(&self, file: &str, line: u32, col: u32) -> Result<Vec<CompletionItem>>;
    async fn diagnostics(&self, file: &str) -> Result<Vec<Diagnostic>>;
    async fn document_symbols(&self, file: &str) -> Result<Vec<SymbolInfo>>;
    async fn workspace_symbols(&self, query: &str) -> Result<Vec<SymbolInfo>>;
}

impl Tool for LspTool {
    fn name(&self) -> &str { "lsp" }
    fn description(&self) -> &str { "Query LSP servers for code intelligence" }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["goto_definition", "find_references", "hover", "completion", "diagnostics", "symbols"]
                },
                "file": {"type": "string"},
                "line": {"type": "integer"},
                "column": {"type": "integer"},
                "query": {"type": "string"}
            },
            "required": ["action"]
        })
    }
    
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Dispatch based on action parameter
        // Return formatted results
    }
}
```

---

**End of LSP Integration Planning Guide**
