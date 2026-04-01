# Claude Code → ragent Gap Analysis

This document compares [Claude Code](https://github.com/anthropics/claude-code) (`../claude-code-sourcemap`)
with ragent and identifies major functional gaps, then provides a prioritised milestone plan to reconcile them.

---

## 1. Feature Comparison Matrix

### 1.1 Core Tools

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| Bash execution | ✅ BashTool | ✅ bash | — |
| File read | ✅ FileReadTool | ✅ read | — |
| File write (full overwrite) | ✅ FileWriteTool | ✅ write | — |
| File edit (targeted diff) | ✅ FileEditTool | ✅ edit, multiedit, patch | ragent richer |
| Glob file search | ✅ GlobTool | ✅ glob | — |
| Grep content search | ✅ GrepTool | ✅ grep | — |
| Directory listing | ✅ LSTool | ✅ list | — |
| Web fetch | ❌ | ✅ webfetch | CC gap |
| Web search | ❌ | ✅ websearch | CC gap |
| **Think tool** (extended reasoning scratch-pad) | ✅ ThinkTool | ❌ | **ragent gap** |
| **Architect tool** (plan-only sub-agent mode) | ✅ ArchitectTool | ❌ | **ragent gap** |
| **Notebook read** (Jupyter .ipynb) | ✅ NotebookReadTool | ❌ | **ragent gap** |
| **Notebook edit** (Jupyter .ipynb) | ✅ NotebookEditTool | ❌ | **ragent gap** |
| **Memory read** (CLAUDE.md/project memory) | ✅ MemoryReadTool | ✅ team_memory_read | partial — different model |
| **Memory write** (CLAUDE.md/project memory) | ✅ MemoryWriteTool | ✅ team_memory_write | partial — different model |
| Office documents (docx/xlsx/pptx) | ❌ | ✅ office_write/read | CC gap |
| LibreOffice (odt/ods/odp) | ❌ | ✅ libre_write/read | CC gap |
| PDF read/write | ❌ | ✅ pdf_read/write | CC gap |
| LSP tools (definition/hover/references/symbols/diagnostics) | ❌ | ✅ 5 LSP tools | CC gap |
| Todo management | ❌ | ✅ todo | CC gap |
| Plan/question tools | ❌ | ✅ plan, question | CC gap |
| Task management (new/cancel/list/wait) | ❌ | ✅ 4 task tools | CC gap |
| Team tools (spawn/message/status/broadcast/etc.) | ❌ | ✅ ~20 team tools | CC gap |
| MCP tool integration | ✅ MCPTool | ✅ mcp | — |

### 1.2 Context & Memory

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| **CLAUDE.md auto-injection** — project config file auto-loaded as context | ✅ Full support; recursively finds all CLAUDE.md files in tree | ❌ ragent.json exists but not auto-injected into LLM context | **ragent gap** |
| **Directory structure snapshot** — project file hierarchy injected at start | ✅ Injected as context section every conversation | ❌ | **ragent gap** |
| **README.md auto-injection** — project readme added to context | ✅ | ❌ | **ragent gap** |
| **Code style inference** — auto-detected from project and injected | ✅ Inferred via ripgrep patterns + stored in project config | ❌ | **ragent gap** |
| **Custom context key-value pairs** — user-defined context sections | ✅ `setContext(key, value)` stored in project config | ❌ | **ragent gap** |
| **Conversation compaction** (`/compact`) — summarise + reset context window | ✅ `/compact` command calls model to summarise then forks conversation | ❌ | **ragent gap** |
| **Context window visualisation** (`/ctx-viz`) — breakdown by section/tool | ✅ Renders table of context sections with token estimates | ❌ | **ragent gap** |
| **Extended thinking / thinking budget** | ✅ `getMaxThinkingTokens()` per model | ❌ | **ragent gap** |
| Persistent agent memory | ✅ via CLAUDE.md (MemoryRead/Write are Anthropic-internal only) | ✅ T8 agent memory system | different model |
| Session resume | ✅ ResumeConversation screen | ✅ session manager | — |
| Snapshot / checkpoint | ❌ | ✅ snapshot system | CC gap |
| @Reference resolution | ❌ | ✅ reference system | CC gap |

### 1.3 Slash Commands (TUI / CLI)

| Command | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| `/help` | ✅ | ✅ | — |
| `/clear` | ✅ | ✅ | — |
| `/compact` | ✅ summarise + compact | ✅ `/compact` + auto-compact near context limit | — |
| `/init` | ✅ generate CLAUDE.md | ❌ | **ragent gap** |
| `/doctor` | ✅ health check | ❌ | **ragent gap** |
| `/review` | ✅ PR code review | ❌ | **ragent gap** |
| `/pr-comments` | ✅ fetch GitHub PR comments | ❌ | **ragent gap** |
| `/ctx-viz` | ✅ context window breakdown | ❌ | **ragent gap** |
| `/cost` | ✅ show session cost | ❌ | **ragent gap** |
| `/release-notes` | ✅ show version release notes | ❌ | **ragent gap** |
| `/listen` | ✅ speech recognition (macOS only) | ❌ | **ragent gap** |
| `/approvedTools` | ✅ list session-approved tools | ❌ | **ragent gap** |
| `/terminal-setup` | ✅ iTerm2/VSCode Shift+Enter binding | ✅ built-in (kitty protocol) | similar |
| `/memory` | ✅ CLAUDE.md management | partial | **ragent gap** |
| `/lsp` | ❌ | ✅ shows LSP diagnostics | CC gap |
| `/agents` | ❌ | ✅ | CC gap |
| `/teams` | ❌ | ✅ | CC gap |
| `/sessions` | ❌ | ✅ | CC gap |
| `/skills` | ❌ | ✅ | CC gap |

### 1.4 Permission & Safety

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| Per-tool permission prompts | ✅ | ✅ | — |
| YOLO / dangerouslySkipPermissions mode | ✅ | ✅ | — |
| **Command injection detection** in bash permissions | ✅ Detects `${var}`, `$(...)` patterns; fails-closed | ❌ No injection detection | **ragent gap** |
| **Safe command whitelist** (git status, diff, log...) | ✅ Always-allowed safe commands | ❌ | **ragent gap** |
| **Command prefix matching** — approve entire `npm run` prefix | ✅ via `getCommandSubcommandPrefix()` | ❌ exact match only | **ragent gap** |
| **Blanket tool approval** — approve all bash / all file writes at once | ✅ e.g. `allowedTools: ["Bash"]` | ❌ | **ragent gap** |
| **Per-file session permissions** — FileEdit/Write require per-file approval | ✅ Session-scoped, not globally persistent | ✅ similar | — |
| **MCP server approval dialog** — approve new MCP server on first use | ✅ | ✅ | — |
| Project-level allowedTools config | ✅ | ✅ | — |
| Per-session approved tools list | ✅ | ✅ | — |

### 1.5 Cost & Telemetry

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| **Real-time cost tracking** (tokens → $) | ✅ Running total displayed; saved to project config | ❌ | **ragent gap** |
| **Cost threshold warning dialog** | ✅ Prompts when session cost exceeds threshold | ❌ | **ragent gap** |
| **Usage analytics / Statsig** | ✅ Feature flags + event telemetry | ❌ | **ragent gap** |
| **Error reporting / Sentry** | ✅ SentryErrorBoundary | ❌ | **ragent gap** |
| Token/usage display in TUI | ❌ | ✅ usage bar | CC gap |

### 1.6 Authentication & Providers

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| **OAuth browser flow** for Anthropic auth | ✅ Full PKCE OAuth via local HTTP server | ❌ API key only | **ragent gap** |
| Anthropic provider | ✅ | ✅ | — |
| OpenAI provider | ❌ (via MCP workaround) | ✅ | CC gap |
| Ollama (local models) | ❌ | ✅ | CC gap |
| GitHub Copilot provider | ❌ | ✅ | CC gap |
| Generic OpenAI-compatible provider | ❌ | ✅ | CC gap |
| API key configuration | ✅ | ✅ | — |

### 1.7 Git Integration

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| **Git-aware context injection** (status, branch, email) | ✅ Automatically injects git info into system prompt | ❌ | **ragent gap** |
| **PR review slash command** | ✅ `/review` | ❌ | **ragent gap** |
| **PR comments slash command** | ✅ `/pr-comments` | ❌ | **ragent gap** |
| Uses `gh` CLI for PR operations | ✅ | ✅ (in bash tool) | — |

### 1.8 Project Initialisation & Onboarding

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| **Project onboarding wizard** | ✅ First-run setup; prompts for API key; suggests /init | ❌ | **ragent gap** |
| **`/init` — generate CLAUDE.md** | ✅ Analyses codebase; writes build/lint/test commands + code style | ❌ | **ragent gap** |
| **`/doctor` — health check** | ✅ Checks API key, network, model access, MCP servers | ❌ | **ragent gap** |
| **Auto-updater** | ✅ Checks for new npm package version | ❌ | **ragent gap** |

### 1.9 UI / UX

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| UI framework | React + Ink (virtual DOM, JSX) | Ratatui (immediate mode, Rust) | different approach |
| **Binary feedback** (👍/👎 on responses) | ✅ | ❌ | **ragent gap** |
| **Message selector** (select/quote earlier messages) | ✅ | ❌ | **ragent gap** |
| **Slash command typeahead** | ✅ Full typeahead with descriptions | ✅ Basic slash menu | ragent simpler |
| Multiline input | ✅ (terminal-setup required on some terms) | ✅ Shift+Enter / Alt+Enter + kitty protocol | — |
| Markdown rendering | ✅ | ✅ | — |
| Theming | ✅ (dark/light) | ✅ | — |
| Spinner / progress | ✅ | ✅ | — |

### 1.10 Architecture

| Feature | Claude Code | ragent | Gap |
|---------|-------------|--------|-----|
| Multi-agent team system | ❌ | ✅ Full team infrastructure | CC gap |
| Swarm/orchestration | ❌ | ✅ orchestrator, swarm | CC gap |
| Agent sub-spawning | ✅ AgentTool (single level) | ✅ Multi-level team spawn | ragent richer |
| Server / API mode | ❌ | ✅ ragent-server (SSE/HTTP) | CC gap |
| Skill / custom tool extension | ❌ | ✅ skill system + .md profiles | CC gap |
| LSP integration | ❌ | ✅ | CC gap |

---

## 2. Priority Gaps Summary

The gaps most valuable to close (from a user-experience and code-engineering-assistant perspective):

| # | Gap | Impact | Effort |
|---|-----|--------|--------|
| G1 | **ThinkTool** — dedicated reasoning scratch-pad | High | Low |
| G2 | **CLAUDE.md / project memory auto-injection** | High | Medium |
| G3 | **Conversation compaction (`/compact`)** | High | Medium |
| G4 | **Cost tracking** (real-time $ display) | High | Medium |
| G5 | **ArchitectTool** (plan-only sub-agent) | Medium | Medium |
| G6 | **Git-aware context** (auto-inject git info) | Medium | Low |
| G7 | **Command injection detection** in bash permissions | Medium | Low |
| G8 | **Safe command whitelist** + prefix matching | Medium | Low |
| G9 | **PR review / PR comments** slash commands | Medium | Low |
| G10 | **Context window visualisation (`/ctx-viz`)** | Medium | Medium |
| G11 | **`/init` — generate project config** | Medium | Medium |
| G12 | **`/doctor` — health check** | Low | Low |
| G13 | **Jupyter Notebook support** | Medium | High |
| G14 | **Binary feedback** (👍/👎) | Low | Low |
| G15 | **OAuth browser auth flow** | Low | High |
| G16 | **Extended thinking / thinking budget** | Medium | Medium |
| G17 | **Cost threshold warning** | Low | Low |
| G18 | **Message selector** (quote earlier messages) | Low | Medium |
| G19 | **Project onboarding wizard** | Low | Medium |
| G20 | **Auto-updater** | Low | Medium |

---

## 3. Reconciliation Plan

### Milestone CC1 — Quick Wins: Safety, Git Context & Cost Display
*Prerequisites: none. All low-effort, high-impact.*

**Tasks:**

- **CC1.1** Add `ThinkTool` — a no-op tool that accepts a `thought` string parameter and returns it unchanged. Gives the LLM an explicit scratch-pad for chain-of-thought without emitting visible output. Register in default tool set.
- **CC1.2** Add a safe-command whitelist to `BashTool` — always-allow short read-only commands (`git status`, `git diff`, `git log`, `git branch`, `pwd`, `ls`, `cat`, `tree`, `date`, `which`, `echo`) without prompting.
- **CC1.3** Add command injection detection to `BashTool` permission check — detect `$(...)`, backtick substitution, `${var@...}` patterns; when detected, require exact-match approval (no prefix matching).
- **CC1.4** Add bash command prefix matching — when a command like `npm run test` is approved, auto-approve future invocations of the same prefix rather than requiring re-approval each time.
- **CC1.5** Auto-inject git context into system prompt — when CWD is a git repo, prepend a `<context name="git">` block with `git branch --show-current`, `git status --short`, author email. Mirrors CC's `getIsGit()` + git info injection.
- **CC1.6** Add `/doctor` slash command — checks: API key set, provider reachable (single ping), MCP servers responding, history file readable. Outputs coloured pass/fail per check.
- **CC1.7** Add cost tracking — track `input_tokens`, `output_tokens`, `cache_read_tokens` per LLM response; compute cost using per-model pricing table; display running total in status bar (e.g. `$0.0234 · 1,204 tok`).
- **CC1.8** Add `/cost` slash command — show formatted session cost breakdown (model, tokens in/out, cache hits, $total, wall time, API time).

---

### Milestone CC2 — Project Memory & Context Management
*Prerequisites: CC1 complete.*

**Tasks:**

- **CC2.1** Auto-inject `CLAUDE.md` — on session start, walk the CWD tree upward and collect all `CLAUDE.md` files (project root wins; subdirectory files noted). Inject content as `<context name="project_memory">` block in system prompt. Respect a max-bytes limit.
- **CC2.2** Add `MemoryReadTool` — reads the project's `CLAUDE.md` (or `~/.config/ragent/CLAUDE.md` for global). Returns contents as tool output.
- **CC2.3** Add `MemoryWriteTool` — appends or replaces a keyed section in the project's `CLAUDE.md`. Prompts for user approval (file:write permission).
- **CC2.4** Add `/memory` slash command — shows current CLAUDE.md contents; offers to open editor.
- **CC2.5** Implement `/compact` slash command — sends current conversation + `"Please summarise..."` prompt; on response, clears the message history and replaces it with an assistant message containing the summary. Preserves the summary in the session file.
- **CC2.6** Add context window visualisation `/ctx-viz` — render a table showing: system prompt sections (by `<context name>` tag), each message role/size, tool results, estimated total tokens, percentage of model context limit used.
- **CC2.7** Extended thinking budget — add per-model `max_thinking_tokens` table; when calling Anthropic Claude 3.5+ or Claude 4 models, include `thinking: {type: "enabled", budget_tokens: N}` in the request when budget is non-zero.

---

### Milestone CC3 — ArchitectTool & Git Commands
*Prerequisites: CC1 complete.*

**Tasks:**

- **CC3.1** Add `ArchitectTool` — a tool that spawns a read-only sub-agent with no write tools; the sub-agent analyses the task and returns a structured plan (files to change, approach, risks). The parent agent receives the plan and can proceed to implement it. Register as optional/configurable tool.
- **CC3.2** Add `/review` slash command — sends a prompt instructing the agent to: `gh pr list` (if no PR number given), `gh pr view <N>`, `gh pr diff <N>`, then synthesise a structured code review with sections for overview, quality, suggestions, risks.
- **CC3.3** Add `/pr-comments` slash command — sends a prompt instructing the agent to fetch PR-level and review comments via `gh api`, format them by file/line, and present them to the user.
- **CC3.4** Add `/init` slash command — sends a prompt instructing the agent to analyse the codebase (directory structure, existing config, CI files) and write or improve `CLAUDE.md` with: build/lint/test commands, code style, naming conventions, key directories.

---

### Milestone CC4 — Jupyter Notebook Support
*Prerequisites: none (independent).*

**Tasks:**

- **CC4.1** Add `notebook_read` tool — reads a `.ipynb` JSON file and renders it as human-readable text: cell type (code/markdown), source, and (for code cells) outputs (stdout, stderr, display_data text). Returns formatted string.
- **CC4.2** Add `notebook_edit` tool — accepts a notebook path, cell index, new source string, and optional cell type. Reads the `.ipynb` JSON, replaces or inserts the cell, writes back. Supports: edit cell, insert cell at index, delete cell, clear outputs.
- **CC4.3** Register notebook tools in default tool set with `notebook_read` in read-only set.
- **CC4.4** Add notebook format auto-detection — when `read` tool is called on a `.ipynb` file, delegate to `notebook_read` automatically.

---

### Milestone CC5 — UX: Feedback, Message Selection & Onboarding
*Prerequisites: none (independent).*

**Tasks:**

- **CC5.1** Add binary feedback (👍/👎) — after each assistant response, display a subtle feedback prompt; capture rating; log locally to session file (and optionally to analytics). Accessible via keyboard shortcut (e.g. `+` / `-`).
- **CC5.2** Add cost threshold warning — configurable `max_session_cost` in config; when exceeded, show a dismissible dialog asking if the user wants to continue or compact.
- **CC5.3** Add message selector — allow user to press a key (e.g. `s`) to enter "select mode" in the message pane; navigate messages with arrows, press Enter to quote-reply (prepend `> <message excerpt>`).
- **CC5.4** Add first-run onboarding — on first launch (no config, no API key), display a welcome screen that: explains ragent, prompts for API key or provider selection, offers to run `/init` on the current project. Store completion state in global config.
- **CC5.5** Add auto-update check — on startup, check the crates.io (or GitHub releases) API for a newer ragent version; if found, show a non-blocking notice with the version number and update command.

---

### Milestone CC6 — OAuth & Authentication
*Prerequisites: none (independent, lower priority).*

**Tasks:**

- **CC6.1** Implement PKCE OAuth flow for Anthropic Console — spawn a local HTTP server on a random port; open browser to Anthropic OAuth URL; receive callback with authorization code; exchange for access token; store in global config.
- **CC6.2** Add `--login` / `--logout` CLI flags and `/login` slash command.
- **CC6.3** Account info display — show logged-in account email in TUI status bar when authenticated via OAuth.
- **CC6.4** Token refresh — handle expired access tokens; silently refresh using stored refresh token before each API call.

---

## 4. Features ragent Has That Claude Code Lacks

For completeness, these are significant ragent capabilities not present in Claude Code:

| Feature | ragent | Notes |
|---------|--------|-------|
| **Multi-provider support** | Anthropic, OpenAI, Ollama, Copilot, Generic OpenAI | CC is Anthropic-only |
| **Multi-agent team system** | Spawn, task distribution, mailbox, swarm | CC has only single-level AgentTool |
| **Orchestrator / swarm** | Policy-driven multi-agent coordination; `/swarm` auto-decomposes work | No CC equivalent |
| **Auto-compact + context limit awareness** | Auto-triggers compaction before hard context limit | CC requires manual `/compact` |
| **LSP tools** | definition, hover, references, symbols, diagnostics | No CC equivalent |
| **Office document tools** | docx/xlsx/pptx read+write | No CC equivalent |
| **LibreOffice tools** | odt/ods/odp read+write | No CC equivalent |
| **PDF tools** | pdf_read, pdf_write | No CC equivalent |
| **HTTP server mode** | ragent-server (SSE/REST API) | No CC equivalent |
| **Skill system** | .md custom tool profiles | No CC equivalent |
| **Snapshot / checkpoint** | Save and restore agent state | No CC equivalent |
| **@Reference system** | `@file`, `@url`, `@symbol` in prompts | No CC equivalent |
| **Custom agent profiles** | .md format with provider/model pinning | No CC equivalent |
| **Todo / task tools** | todo, plan, new_task, wait_tasks | No CC equivalent |
| **Web search** | websearch tool | No CC equivalent |
| **Token usage bar** | Live token display in TUI | No CC equivalent |
| **Offline / local LLM** | Full Ollama support | CC requires Anthropic account |
| **Per-teammate model override** | Each team member can use a different provider/model | No CC equivalent |

---

## 5. Effort Estimates

| Milestone | Gaps closed | Estimated effort |
|-----------|-------------|-----------------|
| CC1 — Safety, Git, Cost | G1, G6, G7, G8, G12 (partial G4) | ~3–5 days |
| CC2 — Memory & Context | G2, G3, G10, G16 | ~5–8 days |
| CC3 — Architect & Git Commands | G5, G9, G11 | ~3–5 days |
| CC4 — Notebook Support | G13 | ~3–5 days |
| CC5 — UX Improvements | G14, G17, G18, G19, G20 | ~5–8 days |
| CC6 — OAuth | G15 | ~3–5 days |

**Total estimated effort: ~22–36 developer-days**

---

*Generated: 2026-04-01 by ragent gap analysis comparing claude-code-sourcemap and ragent source trees.*
