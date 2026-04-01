# Claude Code vs ragent — Gap Analysis (CCGAP)

> Generated: 2026-04-01  
> CC source: `../claude-code-sourcemap/src/`  
> ragent source: `crates/`  
> All references to CLAUDE.md in CC are treated as AGENTS.md in ragent.

---

## Executive Summary

Claude Code (CC) is a TypeScript/React/Ink.js terminal AI assistant backed exclusively by
Anthropic models with tight Statsig/telemetry integration. ragent is a Rust/Ratatui terminal AI
assistant that supports multiple providers (Anthropic, OpenAI, GitHub Copilot, Ollama, generic
OpenAI-compatible), has a full swarm multi-agent team system, integrated LSP support, and document
(Office/PDF) tooling. 

This document compares both implementations feature by feature (with deep implementation notes
where both exist), identifies gaps, and provides a prioritised reconciliation plan.

---

## 1. Context Injection into System Prompt

### Claude Code implementation

`context.ts` — `getContext()` (memoised with lodash)

Runs 4 async queries in parallel and assembles a map of context sections:

| Key | Source | Notes |
|-----|--------|-------|
| `directoryStructure` | `LSTool.call()` | 1 000 ms timeout |
| `gitStatus` | 5 git commands in parallel (branch, main branch, `status --short`, `log --oneline -n5`, author log) | truncated at 200 lines |
| `codeStyle` | ripgrep for `.cursorrules` / `CLAUDE.md` (3 000 ms timeout) | cached separately |
| `claudeFiles` | ripgrep `**/*/CLAUDE.md` — returns **file paths only**, not content | skipped if `dontCrawlDirectory` |
| `readme` | reads `README.md` in cwd | |

User-supplied key-value context from `projectConfig.context` overlays the above.  
Context sections are injected into the system prompt only if non-empty (sparse spread pattern).

### ragent implementation

`agent/mod.rs` — `build_system_prompt(agent, working_dir, file_tree, skill_registry)`

Uses four template variables substituted into the agent's prompt text:

| Variable | Source | Notes |
|----------|--------|-------|
| `{{WORKING_DIR}}` | cwd passed in | |
| `{{FILE_TREE}}` | pre-built string passed in | built before call |
| `{{AGENTS_MD}}` | reads `AGENTS.md` in working_dir if present | **root only** — no recursive discovery |
| `{{DATE}}` | `Utc::now().format("%Y-%m-%d")` | |

After variable substitution, adds skills list and (for primary agents) sub-agent guidance.  
First-message-only: sends a separate "AGENTS.md has been loaded, acknowledge briefly" LLM call
(max 200 tokens) that is streamed but **not added to chat history**.

### Differences

| Aspect | CC | ragent |
|--------|-----|--------|
| Git status injection | ✅ Full (branch + status + 5 commits + author) | ❌ None |
| README injection | ✅ Reads and injects | ❌ None |
| AGENTS.md discovery | Multi-directory ripgrep (paths listed) | Single root file, full content injected |
| Code-style detection | ✅ Scans `.cursorrules` | ❌ None |
| Directory tree | ✅ Via LSTool (1 s timeout) | ✅ Pre-built and passed in |
| Memoization | ✅ Full session memoize; cleared on `/clear` and `/compact` | ❌ Rebuilt each call |
| User-defined context | ✅ `projectConfig.context` overlay | Partially via config `instructions` |

### Gap: G01 — Missing git context in system prompt
### Gap: G02 — Missing README injection
### Gap: G03 — No multi-directory AGENTS.md discovery
### Gap: G04 — No code-style auto-detection

---

## 2. Permission System and Command Safety

### Claude Code implementation

`permissions.ts` + `utils/commands.ts` + `tools/BashTool/prompt.ts`

**Multi-layer flow:**

```
Input command
 │
 ├─ 1. SAFE_COMMANDS whitelist (Set, exact match) ─── allow immediately
 │       { git status, git diff, git log, git branch, pwd, tree, date, which }
 │
 ├─ 2. BANNED_COMMANDS list ─── reject with error
 │       { alias, curl, curlie, wget, axel, aria2c, nc, telnet, lynx, w3m,
 │         links, httpie, xh, http-prompt, chrome, firefox, safari }
 │
 ├─ 3. Haiku LLM injection detection (memoised by command string)
 │       Parses ~60 example patterns; returns:
 │         commandInjectionDetected: true  → only exact whitelist allowed
 │         commandPrefix: "git diff"       → check prefix grants
 │         commandPrefix: null             → check blanket approval
 │
 ├─ 4. Compound-command safety
 │       safe: && || ; ;;   (command list)
 │       unsafe: | > #      (pipe/redirect/comment)
 │
 └─ 5. Permission store check
         Prefix approval: "BashTool(git diff:*)"
         Blanket approval: allowedTools.includes("Bash")
         Persisted in projectConfig.allowedTools (JSON on disk)
```

**Bash `validateInput()` also checks:**
- Directory escape: `cd` restricted to children of original cwd
- Syntax check: `sh -n -c {command}` (1 000 ms timeout) before execution

**Permission storage key format:** `BashTool(git diff)` or `BashTool(git diff:*)` for prefix.

### ragent implementation

`permission/mod.rs` — `PermissionChecker`

```
Input (permission, path)
 │
 ├─ 1. always_grants HashMap (compiled GlobMatcher per permission)
 │       Permanent grants recorded via record_always(permission, pattern)
 │       Return Allow if any matcher matches
 │
 └─ 2. ruleset evaluation (last-match-wins, CSS-like)
         Rules: { permission, path_pattern, action }
         Action: Allow | Deny | Ask
         Permission::Custom("*") matches all permission types
         Default if no match: Ask
```

**Bash tool safety** (`tool/bash.rs`):

```
Layer 1 — DENIED_PATTERNS (hard-coded, ~12 entries):
  rm -rf /   mkfs   dd if=   chmod -R 777 /
  .bash_history   .ssh/id_   insmod   :(){ :|:&};:

Layer 2 — Obfuscation detection (validate_no_obfuscation):
  base64 | bash   eval $(...)   $'\xNN'   python/perl exec(

Layer 3 — YOLO mode (toggle via /yolo):
  Bypasses ALL checks when enabled
```

**No** safe-command whitelist — deny-list only.  
**No** Haiku LLM injection analysis.  
**No** banned network-tool list.  
**No** directory-escape enforcement.  
**No** syntax pre-check.  

### Differences

| Aspect | CC | ragent |
|--------|-----|--------|
| Safe-command whitelist | ✅ 8 always-approved commands | ❌ None |
| Banned commands | ✅ 14 network/browser tools | ❌ None (deny-pattern approach) |
| Injection detection | ✅ Haiku LLM analysis | ❌ None |
| Obfuscation detection | ❌ None | ✅ base64/eval/hex patterns |
| Directory escape guard | ✅ Validates cd path | ❌ None |
| Syntax pre-check | ✅ `sh -n` before exec | ❌ None |
| Permission persistence | ✅ JSON config (cross-session) | ❌ Session-only (always_grants map) |
| YOLO/bypass mode | ❌ None | ✅ `/yolo` toggle |
| Permission storage key | `BashTool(prefix:*)` | glob pattern per Permission type |

### Gap: G05 — No safe-command whitelist for bash
### Gap: G06 — No banned-command list for network/browser tools  
### Gap: G07 — No LLM-based injection detection
### Gap: G08 — No directory-escape guard in bash tool
### Gap: G09 — No syntax pre-check before execution
### Gap: G10 — Permission grants not persisted across sessions

---

## 3. Bash Execution

### Claude Code implementation

`tools/BashTool/BashTool.tsx` + `PersistentShell.ts` + `utils.ts`

**Architecture:** Singleton `PersistentShell` — one long-lived shell process per session.

**Execution flow:**
1. Spawn shell once, source `.bashrc`/`.zshrc`
2. Each command: write to shell stdin, poll temp files every **10 ms**
3. Atomic exit-code capture: `eval cmd; EXEC_EXIT_CODE=$?; pwd > cwdFile; echo $EXEC_EXIT_CODE > statusFile`
4. File-based IPC: `/tmp/claude-{id}-{stdout,stderr,status,cwd}`
5. On completion: read files, reset cwd if escaped, return structured result

**Timeouts:**
- Default: **30 minutes** (`DEFAULT_TIMEOUT = 30 * 60 * 1000`)
- User-specified max: **10 minutes** (Zod schema enforces ≤600 000 ms)
- Syntax check: **1 000 ms** (`sh -n`)
- SIGTERM on timeout (exit code 143), kills child PIDs via `pgrep -P`

**Output truncation:** `MAX_OUTPUT_LENGTH = 30 000`
- Keeps first 15 000 chars + `\n\n...[N lines truncated]...\n\n` + last 15 000 chars
- Preserves beginning AND end context

**File-path extraction:** Haiku LLM call (non-blocking `.then()`) extracts paths touched by the
command, stores mtime in `readFileTimestamps` for change detection.

**Environment:** `GIT_EDITOR=true` to prevent interactive prompts.

### ragent implementation

`tool/bash.rs`

**Architecture:** Each command spawns a fresh process via `tokio::process::Command`.

**Timeouts:**
- Default: **120 seconds**
- Configurable per-command (field in tool input)
- Uses `tokio::time::timeout`

**Output truncation:** `MAX_OUTPUT = 100 000` (100 KB)
- Truncates tail, appends `"\n... (output truncated)"`
- No head+tail preservation — beginning preserved, end dropped

**Output format:** `"Exit code: {exit_code}\nDuration: {elapsed_ms}ms\n\n{content}"`

**No** persistent shell — each call is a fresh process (loses shell state: cd, env vars, aliases).  
**No** file-path extraction.  
**No** directory-escape guard.

### Differences

| Aspect | CC | ragent |
|--------|-----|--------|
| Shell persistence | ✅ Singleton (state preserved) | ❌ Fresh process per call |
| Default timeout | 30 minutes | 120 seconds |
| Output limit | 30 000 chars (head+tail) | 100 000 chars (head only) |
| Truncation strategy | Head + tail (preserves end) | Head only (end lost) |
| File-path extraction | ✅ Haiku LLM post-processing | ❌ None |
| Directory tracking | ✅ Reads cwd after each command | ❌ None |
| Environment setup | ✅ Sources .bashrc/.zshrc | ❌ Inherits parent env |
| Polling interval | 10 ms | tokio async |

### Gap: G11 — No persistent shell (shell state lost between calls)
### Gap: G12 — Output truncation drops end context (should preserve head + tail)

---

## 4. Compaction / History Management

### Claude Code implementation

`commands/compact.ts`

**Model:** `slowAndCapableModel` (Claude Sonnet — configurable, NOT hardcoded Haiku)

**Summary prompt:**
> "Provide a detailed but concise summary of our conversation above. Focus on what we did, what we're doing, which files we're working on, and what we're going to do next."

**Post-summary steps:**
1. Zero out token counts in response (`input_tokens: 0`, cache tokens: 0) — suppresses context warning
2. `clearTerminal()` — wipe terminal output
3. `getMessagesSetter()([])` — clear in-memory message list
4. `getContext.cache.clear()` + `getCodeStyle.cache.clear()` — clear memoised context
5. `setForkConvoWithMessagesOnTheNextRender([userMsg, summaryResponse])` — fork with 2 synthetic messages

**Result:** New conversation starts with summary as context; original history permanently discarded in memory (not stored in persistent file).

**No auto-compact** — manual only.

**Cost:** Charged normally; just token counts zeroed in display.

### ragent implementation

`app.rs` — `start_compaction()` + `Event::MessageEnd` handler

**Model resolution** (priority order):
1. `selected_model` (current TUI selection, format `"provider/model"`)
2. `agent_info.model` (agent's pinned model)
3. Fallback: `"anthropic/claude-3-5-haiku-latest"`

**Summary prompt:**
> "Summarise the conversation so far into a concise representation that preserves all important context, decisions, code changes, file paths, and outstanding tasks."

**Post-summary steps** (in `Event::MessageEnd` when `compact_in_progress`):
1. Extract last assistant message text
2. `storage.delete_messages(session_id)` — delete ALL messages from SQLite
3. Create new assistant message: `"[Conversation compacted]\n\n{summary}"`
4. Store in SQLite
5. Update `self.messages` in memory

**Auto-compact:** Triggered before send when `last_input_tokens >= 92% of context_window`.  
**Queue-after-compact:** Pending message stored in `pending_send_after_compact`; dispatched after summary completes.

### Differences

| Aspect | CC | ragent |
|--------|-----|--------|
| Default model | Sonnet (capable) | Haiku fallback (fast/cheap) |
| Auto-compact | ❌ Manual only | ✅ At 92% context threshold |
| Queue-after-compact | ❌ User must re-send | ✅ Queued message auto-sent |
| Context cache clear | ✅ Clears memoised context/style | ❌ Context rebuilt next call |
| Token display | Zeroes input count in UI | Resets fully |
| Storage | In-memory only | SQLite — persisted |
| Fork mechanism | Two synthetic messages | Single summary assistant message |
| Terminal clear | ✅ Clears terminal on compact | ❌ Continues in-place |

---

## 5. Cost Tracking

### Claude Code implementation

`cost-tracker.ts` + `/cost` command

**Global state:**
```typescript
STATE = { totalCost: number, totalAPIDuration: number, startTime: number }
```

**Per-request cost calculation** (from token counts + model):
- Haiku input: $0.80 / 1M tokens
- Haiku output: $4.00 / 1M tokens  
- Haiku cache write: $1.00 / 1M, cache read: $0.08 / 1M
- Sonnet input: $3.00 / 1M tokens
- Sonnet output: $15.00 / 1M tokens
- Sonnet cache write: $3.75 / 1M, cache read: $0.30 / 1M

**Persistence:** On process exit, writes `lastCost`, `lastAPIDuration`, `lastDuration`, `lastSessionId` to `projectConfig`.

**Display (`/cost`):**
```
Total cost: $X.XX
Total duration (API): Xm Ys
Total duration (wall): Xm Ys
```

### ragent implementation

`app.rs` — `Event::TokenUsage` handler + `usage_display()`

**Tracking:** Token counts only (no USD conversion):
```rust
token_usage: (u64, u64)  // (total_input, total_output) accumulated
last_input_tokens: u64    // most recent request
quota_percent: Option<f32> // Copilot plan quota %
```

**Display** (status bar, `usage_display()`):
- Copilot: `"{plan} {quota%}"` 
- Ollama: `"local ctx: {%}"`
- Others: `"ctx: {%}"` (context-window utilisation percent)

**No USD cost calculation.**  
**No persistence** across sessions.  
**No `/cost` slash command.**

### Differences

| Aspect | CC | ragent |
|--------|-----|--------|
| USD cost tracking | ✅ Per-model pricing | ❌ Token counts only |
| Session total cost | ✅ Accumulated | ❌ None |
| Persistence across sessions | ✅ Written to config on exit | ❌ None |
| `/cost` command | ✅ Formatted report | ❌ None |
| Context % display | Via `/ctx-viz` command | ✅ Status bar |
| Per-model pricing table | ✅ Haiku + Sonnet | ❌ None |
| Wall-clock vs API duration | ✅ Both tracked | ❌ None |

### Gap: G13 — No USD cost tracking or /cost command

---

## 6. Sub-agents / Teams

### Claude Code implementation

`tools/AgentTool/AgentTool.tsx` — single ephemeral sub-agent

**Spawning:**
- Tool name: `dispatch_agent`
- Input: `prompt: string`
- Spawns ONE agent with task-specific tool access
- Generator-based streaming; yields `progress` messages during execution

**Tool access control:**
- `dangerouslySkipPermissions = false` (default): agent gets **read-only tools only**
- `dangerouslySkipPermissions = true`: agent gets all tools
- **Recursive agents blocked:** filters out `AgentTool.name` from sub-agent's toolset
- Tool access scoped by `forkNumber` and `sidechainNumber` for log isolation

**Context isolation:**
- Own `abortController` for cancellation
- Own `messageLogName` for sidechain file logging
- Own `readFileTimestamps` map

**No persistent state, no task graph, no mailbox.**

**ArchitectTool** (`tools/ArchitectTool/`):
- Separate "plan-only" tool (no code generation)
- System prompt: analyse requirements → concrete actionable steps
- Disabled by default; opt-in via `enableArchitect` flag

### ragent implementation

`team/` — full swarm orchestration system

**Architecture:** Persistent TeamStore on disk; agents communicate via mailbox.

**Components:**
| Component | Purpose |
|-----------|---------|
| `TeamStore` | Disk-backed config, member list, task list |
| `TaskStore` | DAG of tasks with status (Pending/InProgress/Completed/Cancelled) |
| `Mailbox` | Per-member message queue (JSON files) |
| `MemberStatus` | Spawning/Working/Idle/Blocked/Failed/Stopped |
| `Blueprints` | Reusable team definitions (spawn-prompts + task-seeds) |

**Tools available to agents:**
`team_create`, `team_spawn`, `team_idle`, `team_message`, `team_read_messages`,
`team_broadcast`, `team_status`, `team_task_create`, `team_task_list`, `team_task_claim`,
`team_task_complete`, `team_assign_task`, `team_wait`, `team_shutdown_teammate`,
`team_shutdown_ack`, `team_memory_read`, `team_memory_write`, `team_cleanup`,
`team_submit_plan`, `team_approve_plan`

**Swarm decomposition** (`/swarm <prompt>`):
- LLM decomposes goal into parallel subtasks → spawns ephemeral team
- DAG dependency resolution blocks/unblocks spawns

**Differences:**

| Aspect | CC AgentTool | ragent Teams |
|--------|-------------|--------------|
| Concurrency | Sequential or 10-concurrent (read-only) | Full parallel swarm |
| Persistence | In-session only | Disk-backed TeamStore |
| Communication | Tool input/output | Mailbox (P2P + broadcast) |
| Task tracking | Implicit in tool results | Explicit TaskStore with DAG |
| Memory | None | `team_memory_read/write` |
| Blueprints | None | Reusable spawn-prompt + task-seed JSON |
| Max depth | 1 (recursive blocked) | N levels (configured) |
| Goal decomposition | Manual in prompt | `/swarm` auto-decomposes via LLM |
| Oversight | `focus` command for teammate output | None |

---

## 7. Tool Inventory Comparison

### CC tools (18)

| Tool | CC | ragent equivalent |
|------|----|--------------------|
| `AgentTool` (dispatch_agent) | ✅ | ✅ (team_spawn) |
| `ArchitectTool` | ✅ | ✅ (plan tool) |
| `BashTool` | ✅ | ✅ (bash) |
| `FileReadTool` | ✅ | ✅ (read) |
| `FileEditTool` | ✅ | ✅ (edit, multiedit, patch) |
| `FileWriteTool` | ✅ | ✅ (write, create) |
| `GlobTool` | ✅ | ✅ (glob) |
| `GrepTool` | ✅ | ✅ (grep) |
| `LSTool` | ✅ | ✅ (list) |
| `MCPTool` | ✅ | ✅ (MCP integration) |
| `MemoryReadTool` | ✅ (ANT-only) | ✅ (team_memory_read) |
| `MemoryWriteTool` | ✅ (ANT-only) | ✅ (team_memory_write) |
| `NotebookEditTool` | ✅ | ❌ None |
| `NotebookReadTool` | ✅ | ❌ None |
| `ThinkTool` | ✅ (gate-controlled) | ❌ None |
| `StickerRequestTool` | ✅ (internal) | ❌ N/A |

### ragent-only tools (no CC equivalent)

| Tool | Purpose |
|------|---------|
| `lsp_definition/hover/references/symbols/diagnostics` | Full LSP integration |
| `office_read/write/info` | MS Office (DOCX, XLSX, PPTX) |
| `libreoffice_read/write/info` | LibreOffice formats |
| `pdf_read/pdf_write` | PDF manipulation |
| `file_ops_tool` | Concurrent batch file operations |
| `websearch` | Web search |
| `webfetch` | URL fetch |
| `question` | Ask user yes/no |
| `todo` | Create TODO item |
| `new_task/list_tasks/cancel_task/wait_tasks` | Background task management |
| All `team_*` tools | Swarm orchestration (20 tools) |

### Gap: G14 — No ThinkTool / scratchpad reasoning
### Gap: G15 — No Jupyter notebook support

---

## 8. Slash Commands Comparison

### CC commands (12)

| CC Command | ragent equivalent |
|------------|-------------------|
| `/init` (generate AGENTS.md) | ❌ None |
| `/doctor` (health check) | ❌ None |
| `/cost` (session cost) | ❌ None (see G13) |
| `/compact` | ✅ `/compact` |
| `/clear` | ✅ `/clear` |
| `/review` (PR review) | ❌ None |
| `/pr-comments` (PR comments) | ❌ None |
| `/terminal-setup` (Shift+Enter binding) | ❌ N/A (handled in TUI) |
| `/listen` (speech, macOS) | ❌ None |
| `/release-notes` (disabled) | ❌ None |
| `/ctx-viz` (token breakdown, ANT-only) | Partial (status bar %) |

### ragent-only commands (no CC equivalent)

| Command | Purpose |
|---------|---------|
| `/agent [name]` | Switch agent or show picker |
| `/agents` | List all agents |
| `/swarm <prompt>` | Auto-decompose + spawn team |
| `/team` (12 subcommands) | Full team management |
| `/model` | Select model |
| `/provider` | Select provider |
| `/provider_reset` | Clear API credentials |
| `/system [prompt]` | Show/update system prompt |
| `/lsp` (discover/connect/disconnect) | LSP server management |
| `/mcp` (discover/connect/disconnect) | MCP server management |
| `/skills` | List registered skills |
| `/opt` | Prompt optimisation |
| `/reload [all\|config\|agents\|mcp\|skills]` | Reload config from disk |
| `/yolo` | Toggle safety checks |
| `/todos` | List TODO items |
| `/tasks` | List background tasks |
| `/cancel <task_id>` | Cancel background task |
| `/resume` | Resume halted agent |
| `/inputdiag` | Input diagnostics |
| `/browse_refresh` | Refresh @ file picker cache |

### Gap: G16 — No `/init` command (generate AGENTS.md from codebase)
### Gap: G17 — No `/doctor` health-check command
### Gap: G18 — No PR review or PR-comments commands

---

## 9. System Prompt Construction

### Claude Code

System prompt is an array of strings (sections), assembled in `query.ts`:
1. Agent system prompt
2. Context sections from `getContext()` (each key → `\n\n## {key}\n{value}`)
3. Tool prompts concatenated (each tool has a `prompt()` async function)
4. AGENTS.md file list (paths, not contents)

Tool descriptions are also dynamic: `BashTool.description(input)` returns a custom string per call.

### ragent

System prompt is a single string built in `build_system_prompt()`:
1. Agent prompt template with `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`, `{{DATE}}` substituted
2. Skills list appended
3. Sub-agent guidance appended (primary agents only)
4. Max-steps guard: for `max_steps <= 1` agents, returns early (no context sections)

AGENTS.md content is fully inlined (not just path list).

### Differences

| Aspect | CC | ragent |
|--------|-----|--------|
| Prompt structure | Array of sections | Single substituted string |
| Dynamic tool descriptions | ✅ Per-call | ❌ Static definitions |
| AGENTS.md handling | Path list (multi-dir) | Full content (root only) |
| Git/README context | ✅ Always injected | ❌ Not injected |
| Skills integration | ❌ Not applicable | ✅ Skills appended |

---

## 10. History Management

### Claude Code

`history.ts` — wraps `projectConfig.history: string[]`

- **Max:** 100 items
- **Storage:** JSON config file (persisted immediately on each add)
- **Deduplication:** Skip if identical to most recent
- **Order:** LIFO (newest at index 0)
- **Navigation:** Up/Down arrows in REPL
- **Scope:** Per-project (tied to project config file)

### ragent

`app.rs` — `input_history: Vec<String>` + `history_index: Option<usize>`

- **Max:** 100 items
- **Storage:** Via `Storage` (SQLite or file, with 2-second debounce save)
- **Draft preservation:** `history_draft: String` saves unsent input while browsing
- **Navigation:** Up/Down arrows (smart: moves cursor in multiline input first)
- **Picker:** `/history` command opens overlay picker (newest first)
- **Scope:** Per-session (not shared across projects)

### Differences

| Aspect | CC | ragent |
|--------|-----|--------|
| Persistence | JSON config (project-scoped) | SQLite/file (session-scoped) |
| Draft preservation | ❌ | ✅ Preserves unsent draft |
| History picker | ❌ | ✅ `/history` overlay |
| Multiline history | ❌ (single-line REPL) | ✅ Supported |

---

## 11. Auto-Update

### Claude Code

`utils/autoUpdater.ts`

- **Check interval:** 30 minutes (background timer)
- **Method:** `npm install -g {PACKAGE_URL}` (self-update via npm)
- **Lock file:** `/tmp/.update.lock` with 5-minute staleness timeout
- **Min-version enforcement:** Statsig gate `tengu_version_config.minVersion` — forces exit if below
- **Permission check:** `/doctor` command verifies npm prefix is writable, offers 3 fix options
- **Telemetry:** Statsig events on success/failure

### ragent

**No auto-update mechanism.** Version is `Cargo.toml`-defined; users update manually.

### Gap: G19 — No auto-update mechanism

---

## 12. Multi-Provider Support

### Claude Code

Supports **Anthropic only** natively (1P API, AWS Bedrock, Google Vertex).  
No OpenAI, Ollama, or GitHub Copilot support.  
All cost calculations hard-coded for Haiku/Sonnet pricing.

### ragent

Supports **5 providers** with unified `Provider` trait:
- Anthropic (`claude-sonnet-4`, `claude-3-5-haiku-latest`)
- OpenAI (`gpt-4o`, `gpt-4o-mini`)
- GitHub Copilot (OAuth exchange → session JWT; free models)
- Ollama (local, auto-discovers models via `/api/tags`)
- GenericOpenAI (any OpenAI-compatible endpoint)

**ragent advantage:** Multi-provider is a core feature; CC is Anthropic-only.

---

## 13. MCP Integration

### Claude Code

`services/mcpClient.ts` — `getMCPTools()` and `getMCPCommands()` (both memoised)

- Discovers MCP servers from: `.mcprc` → global config → project config
- Wraps MCP tools as CC tools (same interface)
- Wraps MCP prompts as slash commands
- Transport: stdio and SSE

### ragent

`mcp/mod.rs` + `mcp/discovery.rs`

- `/mcp discover` — auto-discovers MCP servers on system
- `/mcp connect <server>` — connects to a server
- `/mcp disconnect <server>` — disconnects
- MCP tools available alongside built-in tools
- Transport: stdio (SSE in progress)

**Both have MCP support**; CC wraps MCP as slash commands too (ragent does not).

---

## 14. LSP Integration

### Claude Code

**No LSP integration** — code intelligence via ripgrep/file reading only.

### ragent

Full LSP integration via `lsp/mod.rs`:
- `/lsp discover` — finds language servers on system
- `/lsp connect <language>` — starts a language server
- 5 LSP tools: `lsp_definition`, `lsp_hover`, `lsp_references`, `lsp_symbols`, `lsp_diagnostics`
- Used automatically by agents when LSP is connected

**ragent advantage:** LSP is a significant differentiator.

---

## 15. ThinkTool / Scratchpad Reasoning

### Claude Code

`tools/ThinkTool/ThinkTool.tsx`

- Tool name: `Think`
- Input: `{ thought: string }`
- Implementation: **No-op** — logs thought to Statsig, returns `"Your thought has been logged."`
- Enabled via Statsig gate `tengu_think_tool` AND `THINK_TOOL` env var
- **Purpose:** Allows model to reason without affecting state; logged for analysis
- `isReadOnly: true`, `needsPermissions: false`

### ragent

**No ThinkTool equivalent.**

Extended reasoning is supported via `Event::ReasoningDelta` (Anthropic extended thinking API),
stored as `MessagePart::Reasoning { text }` and displayed in UI — but this is provider-streamed
reasoning, not a model-invoked scratchpad tool.

### Gap: G14 — Confirmed: No ThinkTool

---

## 16. Jupyter Notebook Support

### Claude Code

`tools/NotebookReadTool/` + `tools/NotebookEditTool/`

- `NotebookReadTool`: Reads `.ipynb` files, renders cells with types and outputs
- `NotebookEditTool`: Edits specific cells by index, insert/delete/replace
- Both handle code, markdown, and raw cell types

### ragent

**No Jupyter notebook support.**

### Gap: G15 — Confirmed: No notebook tools

---

## 17. Credential / Config Management

### Claude Code

`projectConfig` (JSON file, per-project): stores history, context, allowedTools, lastCost, etc.  
No credential encryption — API key stored in env var or Anthropic SDK keychain.  
`projectConfig.allowedTools` persists permission grants across sessions.

### ragent

- Credentials in SQLite `provider_auth` table
- **Encryption:** Blake3 XOF keyed with `"ragent credential encryption v2"` + machine identity
- Machine-local: decryption tied to username+home (prevents credential theft via DB copy)
- V1 legacy: repeating-key XOR for backward compat
- Config: layered JSON (`~/.config/ragent/ragent.json` → `./ragent.json` → env vars)

**ragent advantage:** Significantly stronger credential security.

---

## 18. Query / Streaming Architecture

### Claude Code

`query.ts` — recursive async generator

- `async function* query(...)` yields `Message` items to React UI
- Tool concurrency: up to 10 concurrent for read-only tools, serial for writes
- Retry: 10 attempts (100 for SWE-bench), exponential backoff (500 ms base, 32 s max)
- Respects `x-should-retry` header from API
- Binary feedback (ANT-only): generates 2 responses in parallel, user picks better one
- Thinking block preservation: maintained across tool_use → tool_result → next response

### ragent

`session/processor.rs` — `process_user_message()` event loop

- `tokio::spawn` based, publishes events via `EventBus`
- Tool concurrency: parallel via `futures::future::join_all()` (bounded by semaphore)
- Retry: delegated to provider implementations
- Extended reasoning: `MessagePart::Reasoning` stored and displayed
- Background task injection: completed background tasks injected as user messages each loop

**Key difference:** CC uses React generator streams (pull model); ragent uses Tokio async + EventBus (push model).

---

## Gap Summary

| ID | Feature | Priority | Effort |
|----|---------|---------|--------|
| G01 | Git status/branch/log injection into system prompt | High | Low |
| G02 | README.md injection into system prompt | Medium | Low |
| G03 | Multi-directory AGENTS.md discovery (ripgrep) | Medium | Low |
| G04 | Code-style auto-detection (`.cursorrules`) | Low | Medium |
| G05 | Safe-command whitelist (git status, pwd, etc.) | High | Low |
| G06 | Banned-command list (curl, wget, nc, etc.) | High | Low |
| G07 | LLM-based injection detection | Medium | High |
| G08 | Directory-escape guard in bash tool | High | Medium |
| G09 | Syntax pre-check before bash execution | Medium | Low |
| G10 | Permission grants persisted across sessions | Medium | Medium |
| G11 | Persistent shell (state preserved across calls) | Low | High |
| G12 | Bash output truncation — preserve head + tail | Medium | Low |
| G13 | USD cost tracking and `/cost` command | Medium | Medium |
| G14 | ThinkTool / scratchpad reasoning | Medium | Low |
| G15 | Jupyter notebook read/edit tools | Low | High |
| G16 | `/init` command (generate AGENTS.md) | High | Medium |
| G17 | `/doctor` health-check command | Low | Low |
| G18 | PR review and PR-comments slash commands | Low | Medium |
| G19 | Auto-update mechanism | Low | Medium |

---

## Reconciliation Plan

### Milestone CC1 — Context & Safety Foundations (High Priority)

**Goal:** Close the highest-impact gaps in context injection and command safety.

| Task | Gap | Notes |
|------|-----|-------|
| CC1-T1 | Add git status/branch/commits to system prompt | G01 | Run `git branch --show-current`, `git status --short`, `git log --oneline -n5`; inject as `{{GIT_STATUS}}` template var |
| CC1-T2 | Add README.md injection to system prompt | G02 | Read `README.md` from working dir; inject as `{{README}}` template var; skip if absent |
| CC1-T3 | Safe-command whitelist for bash | G05 | Exact-match set: `git status`, `git diff`, `git log`, `git branch`, `pwd`, `tree`, `date`, `which`; skip all checks |
| CC1-T4 | Banned-command list for bash | G06 | Block: `curl`, `wget`, `nc`, `telnet`, `axel`, `aria2c`, `lynx`, `w3m`; validate in `bash.rs` before exec |
| CC1-T5 | Directory-escape guard in bash | G08 | Reject `cd` to paths outside session working directory |
| CC1-T6 | Bash syntax pre-check | G09 | Run `sh -n -c {cmd}` (1 s timeout) before executing; return error on parse failure |
| CC1-T7 | Head+tail output truncation for bash | G12 | Keep first 15 000 + last 15 000 chars; insert `\n...[N lines omitted]...\n` in middle |

### Milestone CC2 — ThinkTool, Init, Cost (High Value / Low Effort)

| Task | Gap | Notes |
|------|-----|-------|
| CC2-T1 | Implement ThinkTool | G14 | Tool `think` with `thought: string` input; no-op execution; `is_readonly: true`; display in UI as collapsible block |
| CC2-T2 | Implement `/init` command | G16 | Analyse codebase; generate `AGENTS.md` with build/test/lint commands, code-style notes, project structure; ~20 lines |
| CC2-T3 | USD cost tracking | G13 | Add per-model pricing table; accumulate session total; expose `get_total_cost_usd()` |
| CC2-T4 | `/cost` slash command | G13 | Report `total cost: $X.XX`, API duration, wall duration |
| CC2-T5 | Multi-dir AGENTS.md discovery | G03 | Ripgrep `**/AGENTS.md` from working dir; list paths in system prompt; fully inline root AGENTS.md |

### Milestone CC3 — Safety Deep Work (Medium Priority)

| Task | Gap | Notes |
|------|-----|-------|
| CC3-T1 | Permission persistence across sessions | G10 | Store `allowed_tools` table in SQLite; load on session start; apply as `always_grants` |
| CC3-T2 | LLM-based injection detection | G07 | Use fast model (Haiku/GPT-4o-mini); memoize by command string; detect backtick/`$()`/newline injection |
| CC3-T3 | Code-style auto-detection | G04 | Scan for `.cursorrules`; parse and inject as code style guidelines |

### Milestone CC4 — PR Workflow Commands (Medium Priority)

| Task | Gap | Notes |
|------|-----|-------|
| CC4-T1 | `/review` command | G18 | `gh pr view` + `gh pr diff`; send to LLM for review; stream result |
| CC4-T2 | `/pr-comments` command | G18 | `gh api` for issue + pull review comments; format threaded output |
| CC4-T3 | `/doctor` command | G17 | Check: provider API key set, AGENTS.md present, git repo detected, MCP config valid |

### Milestone CC5 — Persistent Shell (Low Priority / High Effort)

| Task | Gap | Notes |
|------|-----|-------|
| CC5-T1 | Persistent shell implementation | G11 | Long-lived `tokio::process::Child`; stdin command queue; file-based or pipe-based IPC; cwd tracking |
| CC5-T2 | Directory tracking across calls | G11 | Capture cwd after each command; expose to subsequent calls as context |
| CC5-T3 | File-path extraction | Bonus | Optional: extract touched files from command + output; track mtimes |

### Milestone CC6 — Notebook & Auto-Update (Low Priority)

| Task | Gap | Notes |
|------|-----|-------|
| CC6-T1 | Jupyter notebook read tool | G15 | Parse `.ipynb` JSON; render cells (code/markdown/raw) with outputs |
| CC6-T2 | Jupyter notebook edit tool | G15 | Edit cells by index; insert/delete/replace |
| CC6-T3 | Auto-update mechanism | G19 | Background check for new versions on crates.io or GitHub releases; notify in status bar |

---

## ragent-Only Advantages (Not in CC)

These are features ragent has that CC does not — **do not remove or regress these**.

| Feature | ragent Implementation | Impact |
|---------|-----------------------|--------|
| **Multi-provider** | 5 providers (Anthropic, OpenAI, Copilot, Ollama, Generic) | Core differentiator |
| **LSP integration** | 5 LSP tools; `/lsp` management commands | Code intelligence |
| **Swarm teams** | Full multi-agent orchestration with TaskStore, Mailbox, Blueprints | Enterprise use cases |
| **Document tools** | Office, LibreOffice, PDF read/write | Office productivity |
| **Credential security** | Blake3 machine-local encryption | Security |
| **Auto-compaction** | At 92% context threshold with queue-after-compact | Reliability |
| **Prompt optimisation** | `/opt` command via `prompt_opt` crate | Quality |
| **Multiline input** | Full cursor navigation, Shift+Enter, clipboard ops | Usability |
| **History picker** | `/history` overlay, draft preservation | Usability |
| **YOLO mode** | `/yolo` toggle for trusted sessions | Power users |
| **MCP discovery** | Auto-discovers MCP servers on system | Extensibility |
| **Skills system** | Scoped skills with access levels | Customisation |
| **Session archiving** | Soft-delete with SQLite persistence | Data management |
| **Background tasks** | Long-running tasks with task manager | Concurrency |

---

## Implementation Notes

### Template Variable Pattern (for CC1-T1, CC1-T2)

The simplest path for git/README injection is to add new template variables in `build_system_prompt()`:

```rust
// In agent/mod.rs build_system_prompt()
let git_status = read_git_status(&working_dir).await.unwrap_or_default();
let readme = read_file_if_exists(&working_dir.join("README.md")).unwrap_or_default();

prompt = prompt.replace("{{GIT_STATUS}}", &git_status);
prompt = prompt.replace("{{README}}", &readme);
```

Add these vars to the built-in `general` agent's prompt template. Custom agents that don't include the variable simply don't get the injection.

### ThinkTool Pattern (for CC2-T1)

```rust
// In tool/think.rs
pub struct ThinkTool;
impl Tool for ThinkTool {
    fn name(&self) -> &str { "think" }
    fn description(&self) -> &str { "Log your reasoning without affecting state" }
    fn is_readonly(&self) -> bool { true }
    async fn execute(&self, input: Value, _ctx: ToolContext) -> Result<ToolOutput> {
        let thought = input["thought"].as_str().unwrap_or("");
        Ok(ToolOutput::text(format!("Thought logged ({} chars).", thought.len())))
    }
}
```

The thought text should be displayed in the UI as a collapsible "thinking" block (already exists for `MessagePart::Reasoning` — reuse that renderer).

### `/init` Command Pattern (for CC2-T2)

The `/init` command should:
1. Walk the project directory; sample key files
2. Detect build system (Cargo.toml → `cargo build/test`, package.json → `npm run build/test`, etc.)
3. Send to LLM: "Generate an AGENTS.md for this project with build commands, lint commands, test commands, and coding conventions"
4. Write result to `./AGENTS.md` (or update if exists)
5. Reload AGENTS.md in current session

### Safe-Command Whitelist Implementation (for CC1-T3)

```rust
// In tool/bash.rs
const SAFE_COMMANDS: &[&str] = &[
    "git status", "git diff", "git log", "git branch",
    "pwd", "tree", "date", "which",
];

fn is_safe_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    SAFE_COMMANDS.iter().any(|safe| trimmed == *safe || trimmed.starts_with(&format!("{} ", safe)))
}
```
