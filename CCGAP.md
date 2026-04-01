# CCGAP — Claude Code vs ragent Gap Analysis

> Generated: 2026-04-01  
> Claude Code source: `../claude-code-sourcemap/src/` (TypeScript / Ink.js)  
> ragent source: `crates/` (Rust / Ratatui)  
> Note: wherever Claude Code uses `CLAUDE.md`, the ragent equivalent is `AGENTS.md`.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Feature-by-Feature Comparison](#2-feature-by-feature-comparison)
   - 2.1 Context & System-Prompt Injection
   - 2.2 Permission & Safety System
   - 2.3 Slash Commands
   - 2.4 Cost & Token Tracking
   - 2.5 Multi-Agent Architecture
   - 2.6 Compaction
   - 2.7 Extended Thinking
   - 2.8 LLM / Streaming
   - 2.9 Input Handling & TUI
   - 2.10 Auto-Updater
   - 2.11 Tools Inventory
3. [Priority Gap List](#3-priority-gap-list)
4. [Reconciliation Plan — Milestones](#4-reconciliation-plan--milestones)
5. [Features ragent Has That Claude Code Lacks](#5-features-ragent-has-that-claude-code-lacks)

---

## 1. Executive Summary

Claude Code (CC) is Anthropic's TypeScript/React-Ink CLI agent for software engineering tasks.
ragent is a Rust/Ratatui multi-provider agent framework with team/swarm capabilities.

**Shared strengths:** both are interactive terminal agents, both support tool use, permission
prompting, session persistence, and multi-agent delegation.

**Key ragent gaps vs CC:**
- No deep project-context auto-injection (directory tree, README, code-style inference)
- No `ThinkTool` (structured "scratchpad" reasoning step)
- No `ArchitectTool` (dedicated planning sub-agent)
- No safe-command whitelist for bash (no always-approved git status/diff/log)
- No bash command-injection detection
- No `/init` (generate AGENTS.md from codebase)
- No `/doctor`, `/review`, `/pr-comments`, `/cost`, `/ctx-viz`, `/release-notes`
- No per-model cost calculation displayed to the user
- No extended-thinking / thinking-budget support

**Key CC gaps vs ragent:**
- Single provider only (Anthropic); ragent supports 5 providers
- No team/swarm multi-agent system (only single-level AgentTool)
- No LSP tools
- No office document tools (docx, xlsx, odt, pdf…)
- No skill / custom-agent profile system
- No snapshot/checkpoint
- No @Reference system
- No HTTP server mode

---

## 2. Feature-by-Feature Comparison

---

### 2.1 Context & System-Prompt Injection

#### Claude Code

**Key file:** `src/context.ts`, `src/constants/prompts.ts`

CC assembles a rich `getContext()` object (memoized) at conversation start and injects it into
the system prompt as `<context name="…">…</context>` XML tags.

Sections injected (in order):
1. **User-defined key-value pairs** — stored in `~/.claude/projects/<hash>/config.json` under
   `context`. Set via `setContext(key, value)` (e.g. from `/config` command).
2. **Directory structure snapshot** — calls `LSTool` with a 1-second timeout to produce a file
   tree, wrapped in the message: *"Below is a snapshot of this project's file structure at the
   start of the conversation. This snapshot will NOT update during the conversation."*
3. **Git status** — runs `git branch --show-current`, `git status --short`,
   `git log --oneline -n 5`, and the last 5 commits by the user's email.
   Formatted as a block with branch name, file changes, and recent commits.
4. **Code style** — inferred once via `getCodeStyle()` (ripgrep patterns over the project),
   then stored/cached in project config. Appended alongside any AGENTS.md content under a
   combined section.
5. **AGENTS.md files** — `getClaudeFiles()` uses ripgrep `--glob '**/AGENTS.md'` to find *all*
   AGENTS.md files in the tree; injects a note listing their paths so the model knows to read
   each one when working in those directories. It does **not** inline their content directly —
   instead, the model is expected to use `FileReadTool` to load them.
   The *root* AGENTS.md content is inlined via the code-style section.
6. **README.md** — reads `{cwd}/README.md` if present; injected as a named context section.

The full system prompt is: core sysprompt string + context sections as XML tags.

`dontCrawlDirectory` flag in project config disables directory-structure and AGENTS.md scanning.

#### ragent

**Key file:** `crates/ragent-core/src/agent/mod.rs` (`build_system_prompt`),
`crates/ragent-core/src/session/processor.rs`

`build_system_prompt(agent, working_dir, file_tree, skills)` assembles the prompt in order:

1. **Agent prompt / role** — the agent's custom `prompt` field with template variables substituted:
   `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`, `{{DATE}}`
2. **Working directory** (if not already in template) — `## Working Directory\n{path}`
3. **File tree** (if not already in template) — `## Project Structure\n\`\`\`{tree}\`\`\``
4. **AGENTS.md** (if not already in template) — reads `{working_dir}/AGENTS.md` synchronously
   and appends as `## Project Guidelines (AGENTS.md)\n{content}`. Only the **root** AGENTS.md
   is read; subdirectory AGENTS.md files are not discovered.
5. **Skills list** — from the skill registry, listed as `## Available Skills\n…`

Then in `processor.rs`, the first message of a new session triggers an "AGENTS.md init exchange":
if `AGENTS.md` exists, a synthetic user+assistant exchange is prepended:
- User: `"AGENTS.md project guidelines have been loaded. [content]"`
- Assistant: `"I've read the AGENTS.md file and will follow these guidelines."`

**Not injected:** README.md, git status, code-style inference, directory snapshot (file tree
comes from `ProcessorOptions.file_tree` passed in at startup).

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| Directory snapshot at startup | ✅ via LSTool | ✅ via file_tree param (but static) | low |
| Git status injection | ✅ branch + status + recent commits | ❌ | **HIGH** |
| README.md injection | ✅ | ❌ | medium |
| Recursive AGENTS.md discovery | ✅ lists all paths via ripgrep | ❌ root only | medium |
| Code-style inference & caching | ✅ | ❌ | medium |
| User-defined context key-values | ✅ `/config set key val` | ❌ | low |

---

### 2.2 Permission & Safety System

#### Claude Code

**Key files:** `src/permissions.ts`, `src/utils/commands.ts`,
`src/components/permissions/toolUseOptions.ts`

**Permission tiers:**
1. **Always-allowed safe commands** — a hardcoded `Set` of exact strings:
   `git status`, `git diff`, `git log`, `git branch`, `pwd`, `tree`, `date`, `which`.
   These never prompt; they bypass the entire approval flow.
2. **Exact-match approval** — if `allowedTools` includes `Bash(git commit -m "foo")` the exact
   command is auto-approved.
3. **Prefix-match approval** — if `allowedTools` includes `Bash(npm run:*)` then any command
   starting with `npm run` is auto-approved.
4. **Blanket approval** — if `allowedTools` includes `"Bash"` (no parens), all bash commands
   are auto-approved.
5. **Per-file session permissions** — `FileEdit`, `FileWrite`, and `NotebookEdit` require
   per-file approval scoped to the session (not persisted to project config by default).
6. **MCP server approval dialog** — first use of any MCP server triggers a dialog.

**Command injection detection** (`src/utils/commands.ts`):
- The `getCommandPrefix()` function sends a bash command to Claude Haiku via API to determine
  its "safe prefix" (e.g. `npm run` for `npm run build`).
- If the LLM returns `commandInjectionDetected: true`, the system falls back to exact-match
  only — the command will be shown to the user and cannot be blanket-approved.
- Patterns that trigger this: `$(...)`, `${...}`, backticks, semicolons concatenating unrelated
  commands, etc.
- Uses `shell-quote` library to split and analyse command structure.

**Banned commands** (hardcoded in BashTool prompt, model instructed to refuse):
`alias`, `curl`, `curlie`, `wget`, `axel`, `aria2c`, `nc`, `telnet`, `lynx`, `w3m`, `links`,
`httpie`, `xh`, `http-prompt`, `chrome`, `firefox`, `safari`.

**`--dangerously-skip-permissions`** flag bypasses all checks.

#### ragent

**Key files:** `crates/ragent-core/src/permission/mod.rs`,
`crates/ragent-core/src/tool/bash.rs`, `crates/ragent-core/src/sanitize.rs`

**Permission model:**
- Each tool implements `permission_category() -> &str` (e.g. `"bash"`, `"edit"`, `"read"`).
- `PermissionChecker` holds a `PermissionRuleset` — an ordered list of `PermissionRule` structs
  `{ permission: Permission, pattern: String, action: PermissionAction }`.
- `check(permission, path)` walks the rules in order; first match wins.
  Actions: `Allow`, `Deny`, `Ask`.
- Patterns are glob-style (`*` wildcard).
- Default ruleset: `read/**` → Allow, `edit/**` → Ask, `bash/*` → Ask.
- YOLO mode (`yolo::is_enabled()`) short-circuits to Allow for everything.
- Permissions can be recorded as "always allow" for a pattern via
  `record_always(permission, pattern)` — stored in session state only (not persisted to disk
  across sessions).

**No safe-command whitelist** — `git status`, `git diff`, etc. always prompt unless the user
has previously approved them.

**No command injection detection** — any bash string passes through as-is.

**No banned command list** — the model is not instructed to refuse `curl`, `wget`, etc.

**No blanket or prefix-match approval** — each rule is an exact glob pattern.

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| Safe-command whitelist (no prompt for git status/diff etc.) | ✅ | ❌ | **HIGH** |
| Command injection detection (LLM-assisted) | ✅ | ❌ | medium |
| Banned command list | ✅ | ❌ | medium |
| Prefix-match approval (`npm run:*`) | ✅ | ❌ | medium |
| Blanket tool approval (`"Bash"`) | ✅ | ❌ | low |
| Per-file session permissions | ✅ | partial (session only) | low |

---

### 2.3 Slash Commands

#### Claude Code (18 commands)

| Command | Type | Implementation |
|---------|------|----------------|
| `/help` | local-jsx | renders help screen |
| `/clear` | local | clears messages, resets context caches |
| `/compact` | local | calls Sonnet to summarise, forks conversation (see §2.6) |
| `/init` | prompt | sends LLM a prompt asking it to create/improve AGENTS.md |
| `/doctor` | local-jsx | renders `Doctor` screen (checks Node, npm, git, SSH, API key) |
| `/cost` | local | returns `formatTotalCost()` string |
| `/review` | prompt | asks LLM to run `gh pr view/diff` and produce review |
| `/pr-comments` | prompt | asks LLM to run `gh api …/comments` and format them |
| `/ctx-viz` | local | renders token-count table per context section |
| `/release-notes` | local-jsx | shows release notes for current version |
| `/listen` | local | activates speech recognition (macOS iTerm/Terminal only) |
| `/approvedTools` | internal | lists session-approved tools |
| `/terminal-setup` | local | prints Shift+Enter setup instructions for iTerm2/VSCode |
| `/config` | local-jsx | renders config editing screen |
| `/login` | local-jsx | triggers OAuth / API-key auth flow |
| `/logout` | local | clears stored credentials |
| `/onboarding` | local-jsx | shows project onboarding screen |
| `/bug` | local-jsx | opens bug-report flow |

#### ragent (40+ commands)

Key commands (from `crates/ragent-tui/src/app.rs`):

| Command | Notes |
|---------|-------|
| `/compact` | Calls `start_compaction()` (see §2.6) |
| `/swarm [task]` | Fleet-style auto-decomposition and parallel execution |
| `/agents` | List available agent profiles |
| `/team` / `/teams` | Team management (spawn, list, status, send) |
| `/skills` | List available skills |
| `/lsp` | Show LSP diagnostics |
| `/history` | Show/search conversation history |
| `/sessions` | Session management (list, resume, archive) |
| `/help` | Show help |
| `/clear` | Clear conversation |
| `/snapshot` | Save current session snapshot |
| `/resume` | Resume a previous session |
| `/yolo` | Toggle YOLO (skip-permissions) mode |

#### Gap Summary

| Command | CC | ragent | Priority |
|---------|-----|--------|---------|
| `/init` — generate AGENTS.md from codebase | ✅ | ❌ | **HIGH** |
| `/doctor` — health-check diagnostics | ✅ | ❌ | **HIGH** |
| `/cost` — show session cost | ✅ | ❌ | **HIGH** |
| `/review` — AI-assisted PR review | ✅ | ❌ | medium |
| `/pr-comments` — fetch+format GitHub PR comments | ✅ | ❌ | medium |
| `/ctx-viz` — context window token breakdown | ✅ | ❌ | medium |
| `/release-notes` — show version release notes | ✅ | ❌ | low |
| `/listen` — speech input | ✅ (macOS only) | ❌ | low |
| `/swarm` — parallel agent decomposition | ❌ | ✅ | CC gap |
| `/team` / `/teams` | ❌ | ✅ | CC gap |
| `/skills` | ❌ | ✅ | CC gap |
| `/lsp` | ❌ | ✅ | CC gap |
| `/snapshot` | ❌ | ✅ | CC gap |

---

### 2.4 Cost & Token Tracking

#### Claude Code

**Key file:** `src/cost-tracker.ts`

Global mutable `STATE` object: `{ totalCost: number, totalAPIDuration: number, startTime }`.
- `addToTotalCost(cost, duration)` called after every LLM response.
- `formatTotalCost()` returns a chalk-grey multi-line string showing:
  - `Total cost: $X.XXXX`
  - `Total duration (API): Xm Xs`
  - `Total duration (wall): Xm Xs`
- On process exit, writes `lastCost`, `lastAPIDuration`, `lastDuration`, `lastSessionId` to
  project config JSON.
- `/cost` command just calls `formatTotalCost()` and prints it.
- Cost-per-token values are baked into the model pricing table in the build.

#### ragent

**Key files:** `crates/ragent-core/src/session/processor.rs`,
event bus `TokenUsage` event, `crates/ragent-tui/src/app.rs` token bar.

- `Event::TokenUsage { input_tokens, output_tokens, … }` published after each LLM response.
- TUI subscribes and shows a live token count in the status bar.
- **No cost calculation** — token counts are shown but not converted to USD.
- **No session-end cost summary** printed on exit.
- **No `/cost` command**.

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| Per-request cost calculation (USD) | ✅ | ❌ | **HIGH** |
| Session-total cost display | ✅ | ❌ | **HIGH** |
| `/cost` command | ✅ | ❌ | **HIGH** |
| Live token display in TUI | partial (status bar) | ✅ | — |
| Cost saved to project config | ✅ | ❌ | low |

---

### 2.5 Multi-Agent Architecture

#### Claude Code — AgentTool

**Key file:** `src/tools/AgentTool/AgentTool.tsx`

CC has a single-level `AgentTool` that:
1. Accepts a `{ prompt }` input.
2. Creates a fresh message list `[createUserMessage(prompt)]`.
3. Loads the same tools as the parent (minus `AgentTool` itself, to prevent nesting).
4. Calls `getContext()` + `getAgentPrompt()` (same system prompt as parent).
5. Runs a full `query()` loop (with streaming) until the sub-agent is done.
6. Returns the final text as the tool result to the parent.
7. The sub-agent has no persistent memory or inter-agent communication — it is
   purely ephemeral, receives only the single `prompt`, and returns one response.
8. Logs messages to a sidechain file (`logs/…-sidechain-N.jsonl`).

Sub-agents **cannot** themselves spawn further sub-agents (AgentTool excluded from sub-agent
tool list). Context window, cost, and permissions are all inherited from parent.

#### CC — ArchitectTool (disabled by default)

**Key file:** `src/tools/ArchitectTool/ArchitectTool.tsx`

An experimental planning tool (`isEnabled()` returns `false` by default):
- Accepts `{ prompt, context? }`.
- Runs a separate LLM call with `ARCHITECT_SYSTEM_PROMPT` ("You are an expert software
  architect…") using only filesystem exploration tools (no bash write, no edit).
- Returns a structured implementation plan as text.
- The parent agent can then execute the plan with full tools.

#### ragent — Team System

**Key files:** `crates/ragent-core/src/team/manager.rs`, `crates/ragent-core/src/team/mailbox.rs`,
`crates/ragent-core/src/team/task.rs`, `crates/ragent-core/src/tool/team_*.rs`

ragent has a full multi-agent team architecture:

**Agent lifecycle:**
- A "lead" session creates teammates via `team_create` / `team_spawn` tools.
- Each teammate runs as an independent session (its own `Processor` with its own tool set,
  permission set, model, and provider).
- Teammates are persisted in `.ragent/teams/<team-name>/` directories.

**Inter-agent communication — file-backed mailbox:**
- Each agent has a mailbox directory on disk: `.ragent/teams/<name>/mailbox/<agent>/`.
- Messages are JSON files written atomically; the receiver polls via `team_idle` or is woken
  by a filesystem watch.
- Supports peer-to-peer messaging (`team_send`) and broadcast.
- The lead can route tasks; teammates can request help from each other.

**Shared task list:**
- `.ragent/teams/<name>/tasks.json` — a shared JSON array of tasks.
- `team_task_create` adds tasks; `team_task_complete` marks them done.
- Lead uses `team_status` to monitor progress.

**Swarm / auto-decomposition (`/swarm`):**
- Lead agent analyses the task, decomposes it into sub-tasks, spawns N teammates, assigns
  tasks, and coordinates completion.
- Teammates run in background TUI panes or as headless sessions.
- Progress shown via `/swarm status` with a task table and dependency graph.

**Per-teammate model override:**
- Each teammate can be assigned a different provider+model combination.

**Blueprint system:**
- Team templates in `.ragent/blueprints/teams/` define spawn-prompts and task structures.

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| Single sub-agent delegation | ✅ AgentTool | ✅ team_spawn + team tools | — |
| Persistent teammates (survive session) | ❌ | ✅ | CC gap |
| Inter-agent communication | ❌ | ✅ mailbox | CC gap |
| Shared task list | ❌ | ✅ | CC gap |
| Swarm / auto-decomposition | ❌ | ✅ `/swarm` | CC gap |
| Per-teammate model override | ❌ | ✅ | CC gap |
| ArchitectTool (planning sub-agent) | ✅ (disabled) | ❌ | medium |
| Sub-agent nesting prevention | ✅ explicit | partial | low |

---

### 2.6 Compaction

#### Claude Code

**Key file:** `src/commands/compact.ts`

When user runs `/compact`:
1. Gets the current full message list via `getMessagesGetter()()`.
2. Appends a synthetic user message: *"Provide a detailed but concise summary of our
   conversation above. Focus on information that would be helpful for continuing the
   conversation, including what we did, what we're doing, which files we're working on,
   and what we're going to do next."*
3. Calls `querySonnet()` (the "slow and capable model") with the full context + summary request.
4. Clears the terminal and message list.
5. Forks the conversation with two synthetic messages:
   - User: `"Use the /compact command to clear the conversation history, and start a new
     conversation with the summary in context."`
   - Assistant: the summary response.
6. Clears the `getContext` and `getCodeStyle` memoize caches so they re-inject fresh.
7. The summary response's token usage is zeroed (input=0) so the context-size warning resets.

**No auto-compact** — CC only compacts on user request.

#### ragent

**Key files:** `crates/ragent-tui/src/app.rs` (`start_compaction`, `should_auto_compact_before_send`),
`crates/ragent-core/src/agent/mod.rs` (compaction agent definition)

When user runs `/compact` or auto-compact triggers:
1. `start_compaction(auto_triggered)` is called.
2. Resolves the built-in `"compaction"` agent: temperature=0.2, model=`claude-3-5-haiku-latest`
   (Anthropic), max_steps=1, prompt:
   *"You are a compaction agent. Summarize the conversation into a shorter representation that
   preserves all important context, decisions, and state. Include file paths, key code changes,
   and outstanding tasks."*
3. Sends the current session messages to this agent.
4. The summary replaces the session message history.

**Auto-compact trigger** (`should_auto_compact_before_send`):
- Checks token count approaching the model's context limit (configurable threshold).
- Triggered before the next send, not mid-response.
- Uses the same compaction flow but with `auto_triggered=true`.

#### Key Differences

| Item | CC | ragent |
|------|----|--------|
| Compaction model | Same "slow and capable" model as main | Haiku (hardcoded, cheaper) |
| Auto-compact | ❌ manual only | ✅ auto-triggered near context limit |
| Summary prompt focus | "what we did, what we're doing, which files, what next" | "decisions, state, file paths, key changes, outstanding tasks" |
| Context cache reset after compact | ✅ clears memoize caches | ❌ caches not explicitly reset |
| Token zeroing after compact | ✅ resets usage estimate | ❌ |
| Model pinning | compaction uses same model as chat | compaction uses Haiku regardless of chat model |

**ragent gap:** The compaction agent is hardcoded to `claude-3-5-haiku-latest` even if the user
is using OpenAI or GitHub Copilot — this will fail if Anthropic credentials are unavailable.
The compaction model should fall back to the current session provider/model.

---

### 2.7 Extended Thinking / ThinkTool

#### Claude Code

**ThinkTool** (`src/tools/ThinkTool/ThinkTool.tsx`):
- A no-op "scratchpad" tool: the model calls it with a `thought` string to reason before acting.
- Inspired by the tau-bench paper on chain-of-thought tool use.
- The thought is displayed in the UI but not returned to the model — it's logged for visibility.
- Prompt: *"Use the tool to think about something. It will not obtain new information or make
  any changes to the repository, but just log the thought. Use it when complex reasoning or
  brainstorming is needed."*

**Extended thinking / budget** (`src/services/claude.ts`, `src/utils/thinking.ts`):
- `getMaxThinkingTokens(messages)` computes a thinking budget per model.
- Only enabled for the "ant" user type (`process.env.USER_TYPE === 'ant'`), i.e. Anthropic
  internal users. Not exposed to external users.
- When enabled, passes `thinking: { type: "enabled", budget_tokens: N }` to the API.

#### ragent

- **No ThinkTool** — the model cannot use a structured "scratchpad" step.
- **No extended thinking support** — the provider abstraction does not pass thinking budget
  parameters to Anthropic's API.

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| ThinkTool (scratchpad reasoning) | ✅ | ❌ | **HIGH** |
| Extended thinking budget (Anthropic) | ✅ (ANT-internal) | ❌ | low (internal only) |

---

### 2.8 LLM / Streaming

#### Claude Code

**Key files:** `src/services/claude.ts`, `src/query.ts`

- Uses `@anthropic-ai/sdk` directly; streams via `client.messages.stream()`.
- Handles `thinking` and `redacted_thinking` block types (filtered out before tool parsing).
- Tool results are appended as new user messages and the loop continues until no more tool calls.
- `normalizeMessagesForAPI()` strips ephemeral fields before each API call.
- Models tracked: `haiku` (fast), `sonnet` (slow-and-capable), with dynamic model resolution
  via `getSlowAndCapableModel()`.
- Single provider: Anthropic only (+ Bedrock/Vertex as config options).

#### ragent

**Key files:** `crates/ragent-core/src/session/processor.rs`,
`crates/ragent-core/src/provider/`

- Provider abstraction trait `Provider` with implementations for:
  Anthropic, OpenAI, GitHub Copilot, Ollama, Generic OpenAI-compatible.
- Streaming via `futures::Stream<Item = StreamEvent>`.
- Tool calls accumulated from stream deltas, executed in parallel where possible.
- `Event::TokenUsage` published to event bus after each response.
- Model selected via `ModelRef { provider_id, model_id }` — can differ per agent.
- No thinking/extended-thinking support in provider abstraction.

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| Multiple providers | ❌ Anthropic only | ✅ 5 providers | CC gap |
| Extended thinking API params | ✅ (ANT-only) | ❌ | low |
| Parallel tool execution | ❌ sequential | ✅ | CC gap |

---

### 2.9 Input Handling & TUI

#### Claude Code

**Key files:** `src/components/` (React/Ink), `src/history.ts`

- Built with React + Ink.js — rendered as React components to the terminal.
- Input: Ink's `TextInput` component; Shift+Enter handled via iTerm2/VSCode terminal integration
  (the `/terminal-setup` command configures the terminal to send a specific escape sequence).
- History: stored in `~/.claude/history` as a plain newline-delimited file.
  Multiline entries stored with literal `\n` characters; retrieval recreates them.
- No TUI layout panels — output scrolls vertically in the terminal.
- Context window visualisation via `/ctx-viz` shows token counts per section in a table.

#### ragent

**Key files:** `crates/ragent-tui/src/app.rs`, `crates/ragent-tui/src/app/state.rs`

- Built with Ratatui — full TUI with multiple panels (input, message view, output, log, status bar).
- Shift+Enter inserts a newline into the input field (via Kitty keyboard protocol).
- Multiline history: entries persisted with `\` → `\\` and `\n` → literal `\n` escaping;
  loaded with reverse unescaping. Navigation aware of logical lines (Up/Down moves within
  multiline entries before jumping to previous/next history entry).
- Keyboard shortcuts: Ctrl+A (select all), Ctrl+X (cut), Ctrl+C (copy), Ctrl+V (paste),
  Ctrl+Left/Right (word navigation), Shift+arrow (character selection), Shift+Enter (newline).
- Multiple TUI panels: message view, tool output panel, log panel, teammate strip.
- `/ctx-viz` equivalent: no; token count shown in status bar only.

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| `/ctx-viz` token breakdown table | ✅ | ❌ | medium |
| Full TUI panel layout | ❌ | ✅ | CC gap |
| Multiline input with correct history | ✅ | ✅ | — |
| Kitty keyboard protocol | ❌ | ✅ | CC gap |

---

### 2.10 Auto-Updater

#### Claude Code

**Key file:** `src/utils/autoUpdater.ts`

- On startup, checks `statsig` dynamic config `tengu_version_config` for a minimum required
  version. If current version is below minimum, prints an error and calls `process.exit(1)`.
- Periodic background update check: tries `npm install -g @anthropic-ai/claude-code@latest`.
- Uses a lock file (`.update.lock`) with 5-minute timeout to prevent concurrent updates.
- `AutoUpdaterStatus` state flows through: `idle` → `checking` → `installing` → `success/failed`.
- On update success, notifies user and suggests restarting.
- `assertMinVersion()` exported and called at app startup.

#### ragent

- **No auto-updater** — installed as a Rust binary; updates must be done manually via
  `cargo install` or distro package manager.
- No minimum-version enforcement from server config.

#### Gap Summary

| Item | CC | ragent | Priority |
|------|----|--------|----------|
| Auto-update on startup | ✅ npm-based | ❌ | low |
| Minimum-version enforcement | ✅ Statsig | ❌ | low |
| Update lock file | ✅ | ❌ | low |

---

### 2.11 Tools Inventory

#### Claude Code (16 tools)

| Tool | Notes |
|------|-------|
| `BashTool` | Shell execution; banned-command list; injection detection |
| `FileReadTool` | Read file with line-range support |
| `FileWriteTool` | Create/overwrite files (requires session permission per file) |
| `FileEditTool` | Edit existing files (diff-style; requires session permission per file) |
| `GlobTool` | File pattern matching |
| `GrepTool` | Regex content search |
| `LSTool` | Directory listing |
| `AgentTool` | Launch ephemeral sub-agent |
| **`ArchitectTool`** | Planning sub-agent (disabled by default) |
| **`ThinkTool`** | Scratchpad reasoning step |
| `NotebookReadTool` | Read Jupyter notebook cells |
| `NotebookEditTool` | Edit Jupyter notebook cells |
| `MCPTool` | Model Context Protocol tool bridge |
| `MemoryReadTool` | Read persistent memory (ANT-internal only) |
| `MemoryWriteTool` | Write persistent memory (ANT-internal only) |
| `StickerRequestTool` | Anthropic-internal sticker requests |

#### ragent (53 tools, grouped)

| Group | Tools |
|-------|-------|
| File ops | read, write, create, edit, glob, grep, ls, file_ops |
| Execution | bash, background_bash |
| Code intelligence | lsp_definition, lsp_hover, lsp_references, lsp_symbols, lsp_diagnostics |
| Web | webfetch, websearch |
| Planning | todo, plan, new_task, wait_tasks |
| Documents | office_read, office_write, libreoffice_read, libreoffice_write, pdf_read, pdf_write, csv_read |
| Background agents | background_agent, wait_agent, cancel_task, list_tasks |
| Team coordination | team_create, team_spawn, team_send, team_status, team_idle, team_task_create, team_task_complete, team_list, team_join, team_leave, team_chat, team_broadcast, team_assign, team_wait, team_cancel, team_heartbeat, team_config, team_checkpoint |
| MCP | mcp (bridge) |

#### Gap Summary

| Tool | CC | ragent | Priority |
|------|----|--------|----------|
| `ThinkTool` | ✅ | ❌ | **HIGH** |
| `ArchitectTool` | ✅ (disabled) | ❌ | medium |
| `NotebookReadTool` | ✅ | ❌ | medium |
| `NotebookEditTool` | ✅ | ❌ | medium |
| LSP tools | ❌ | ✅ | CC gap |
| Office/PDF document tools | ❌ | ✅ | CC gap |
| Team coordination tools | ❌ | ✅ | CC gap |
| Background agent tools | ❌ | ✅ | CC gap |
| Websearch | ❌ | ✅ | CC gap |

---

## 3. Priority Gap List

Ranked by impact × implementation effort:

| ID | Gap | Impact | Effort |
|----|-----|--------|--------|
| G01 | `ThinkTool` — scratchpad reasoning | High | Low |
| G02 | Safe-command whitelist (git status/diff/log/branch, pwd, tree) | High | Low |
| G03 | `/cost` command + per-request USD cost calculation | High | Medium |
| G04 | `/init` — generate AGENTS.md from codebase | High | Low |
| G05 | `/doctor` — health-check diagnostics | High | Low |
| G06 | Git status auto-injection into system prompt | High | Low |
| G07 | Compaction model fallback (use session provider, not hardcoded Haiku) | High | Low |
| G08 | Recursive AGENTS.md discovery (list all paths like CC does) | Medium | Low |
| G09 | README.md auto-injection into system prompt | Medium | Low |
| G10 | `/review` — AI-assisted PR code review | Medium | Low |
| G11 | `/pr-comments` — fetch + format GitHub PR comments | Medium | Low |
| G12 | `/ctx-viz` — context window token breakdown | Medium | Medium |
| G13 | Bash command injection detection | Medium | Medium |
| G14 | Banned-command list (curl/wget/nc etc.) | Medium | Low |
| G15 | Prefix-match permission approval (`npm run:*`) | Medium | Medium |
| G16 | `ArchitectTool` — planning sub-agent | Medium | Medium |
| G17 | Code-style inference and caching | Medium | High |
| G18 | `NotebookReadTool` + `NotebookEditTool` | Medium | High |
| G19 | Auto-updater | Low | Medium |
| G20 | User-defined context key-value pairs (`/config set`) | Low | Low |

---

## 4. Reconciliation Plan — Milestones

---

### Milestone CC1 — Quick Wins (Low Effort / High Value)

**Goal:** Add the most-impactful CC features that require minimal new infrastructure.

#### Tasks

**CC1-T1: ThinkTool**
- Add `think` tool to `crates/ragent-core/src/tool/`.
- Input: `{ thought: String }`. Output: empty string (no side effects).
- Returns `ToolResult::text("")` — thought is emitted as a `ToolCall` event so the TUI
  can display it.
- Add to all agent tool registries by default.

**CC1-T2: Safe-command whitelist for bash**
- In `crates/ragent-core/src/tool/bash.rs`, before the permission check, match the
  command string against a `SAFE_COMMANDS` set:
  `["git status", "git diff", "git log", "git branch", "git branch --show-current",
    "pwd", "tree", "date", "which"]`
- If matched exactly, return `PermissionAction::Allow` without consulting the checker.

**CC1-T3: Git status injection into system prompt**
- In `build_system_prompt()` (`agent/mod.rs`), add a call to a new
  `collect_git_context(working_dir) -> String` function.
- Run: `git branch --show-current`, `git status --short`, `git log --oneline -n 5`.
- Format as a `## Git Status` section and append to the prompt.
- Skip gracefully if `working_dir` is not a git repository.

**CC1-T4: README.md injection**
- In `build_system_prompt()`, check for `{working_dir}/README.md`.
- If present, read (truncate to ~4000 chars) and append as `## README\n{content}`.

**CC1-T5: Recursive AGENTS.md discovery**
- In `build_system_prompt()`, after injecting the root AGENTS.md, use `walkdir` (or
  `ignore` crate) to find all `**/AGENTS.md` files under `working_dir`.
- Append a note listing discovered paths (do not inline — keep consistent with CC approach).

**CC1-T6: `/init` command**
- Add `"init"` slash command in `app.rs`.
- Sends a user message: *"Please analyse this codebase and create an AGENTS.md file
  containing: 1. Build/lint/test commands. 2. Code style guidelines. 3. Key architectural
  decisions. If there is already an AGENTS.md, improve it."*
- Let the model use its file tools to complete the task.

**CC1-T7: `/doctor` command**
- Add `"doctor"` slash command.
- Checks: `cargo --version`, `rustc --version`, `git --version`, `gh --version`,
  active provider connectivity (send a minimal ping message), disk space, and
  whether `AGENTS.md` exists in the working directory.
- Display results in the TUI output panel.

**CC1-T8: Compaction model fallback**
- Change `start_compaction()` to use the current session's `ModelRef` instead of
  hardcoding `claude-3-5-haiku-latest`.
- If the current model is non-Anthropic, either use the same model, or add a
  `compaction_model` field to `ragent.json` / agent config.

---

### Milestone CC2 — Cost Tracking & Context Visibility

**Goal:** Give users visibility into how much the session costs and what's in the context.

#### Tasks

**CC2-T1: Per-model pricing table**
- Add a `ModelPricing { input_per_mtok: f64, output_per_mtok: f64 }` lookup table
  in `crates/ragent-core/src/provider/`.
- Cover: all major Anthropic models, GPT-4o/mini, Copilot (show tokens only, no USD),
  Ollama (show tokens only, $0 cost).

**CC2-T2: Cost accumulator**
- Add `SessionCost { total_input_tokens: u64, total_output_tokens: u64, total_usd: f64 }`
  to session state.
- Update on every `Event::TokenUsage` using the pricing table.

**CC2-T3: `/cost` command**
- Add `"cost"` slash command in `app.rs`.
- Displays: total input tokens, total output tokens, estimated total cost in USD,
  wall-clock session duration, API response time.

**CC2-T4: Session-end cost summary**
- On clean exit, print a one-line cost summary to stdout (similar to CC).

**CC2-T5: `/ctx-viz` command**
- Parse the assembled system prompt into sections (by `##` headings).
- Estimate token count per section (use `tiktoken` or a byte/4 approximation).
- Render a table in the TUI output panel:
  `Section | Chars | ~Tokens`

---

### Milestone CC3 — Permission & Safety Hardening

**Goal:** Match CC's safety model for bash command handling.

#### Tasks

**CC3-T1: Banned-command list**
- In `bash.rs`, reject commands that start with any banned executable:
  `curl`, `wget`, `nc`, `netcat`, `telnet`, `aria2c`, `axel`, `lynx`, `w3m`.
- Return a clear `ToolError` explaining the ban.
- Allow override via `ragent.json` `allowBannedCommands: true` if user explicitly opts in.

**CC3-T2: Prefix-match approval**
- Extend `PermissionRule.pattern` to support trailing `:*` syntax meaning
  "command starts with this prefix".
- Example rule: `bash: npm run:* → Allow`.
- Update `PermissionChecker.check()` accordingly.

**CC3-T3: Command injection heuristic detection**
- Add a static heuristic check in `bash.rs` before permission evaluation:
  scan for `$(`, `` ` ``, `${`, `eval `, `exec `.
- If found, downgrade any blanket-allow to Ask and log a warning in the TUI.
- (Full LLM-based detection like CC is optional stretch goal.)

**CC3-T4: Blanket tool approval**
- Allow `"Bash"` (no pattern) in the `allowedTools` list to mean "approve all bash".
- Map to a wildcard rule `bash: * → Allow` in the checker.

---

### Milestone CC4 — Git & PR Integration Commands

**Goal:** Add CC's git-integration slash commands.

#### Tasks

**CC4-T1: `/review` command**
- Add `"review [PR_NUMBER]"` slash command.
- If no PR number, runs `gh pr list` and shows results.
- If PR number given, asks the LLM to run `gh pr view <N>` and `gh pr diff <N>`
  then produce a structured code review.

**CC4-T2: `/pr-comments` command**
- Add `"pr-comments [PR_NUMBER]"` slash command.
- Asks LLM to call `gh api /repos/{owner}/{repo}/issues/{N}/comments` and
  `/repos/{owner}/{repo}/pulls/{N}/comments`, format and display all comments.

---

### Milestone CC5 — ArchitectTool & Enhanced Planning

**Goal:** Add structured planning capabilities.

#### Tasks

**CC5-T1: ArchitectTool**
- Add an `architect` tool to `crates/ragent-core/src/tool/`.
- Input: `{ prompt: String, context: Option<String> }`.
- Launches a sub-agent with a system prompt focused on architecture:
  *"You are an expert software architect. Your role is to analyse technical requirements
  and produce clear, actionable implementation plans."*
- Sub-agent has access only to read-only tools (grep, glob, read, ls).
- Returns the implementation plan as text.
- Disabled by default; enabled via `ragent.json` `enableArchitectTool: true`.

**CC5-T2: Code-style inference**
- After `/init`, or on first session startup if AGENTS.md is absent, run a background
  task to infer code style from the project (look for `.editorconfig`, `rustfmt.toml`,
  `clippy.toml`, `package.json` style fields, etc.).
- Cache in `ragent.json` `codeStyle` field; inject into system prompt.

---

### Milestone CC6 — Jupyter Notebook Support

**Goal:** Add Jupyter notebook tools to match CC.

#### Tasks

**CC6-T1: `notebook_read` tool**
- Read `.ipynb` files; parse JSON and return cells as structured text with cell type
  (markdown/code), source, and outputs.

**CC6-T2: `notebook_edit` tool**
- Edit individual cells in `.ipynb` files (update source, add/remove cells).
- Requires per-file session permission (same as `edit`).

---

## 5. Features ragent Has That Claude Code Lacks

For completeness — significant ragent capabilities not present in Claude Code:

| Feature | ragent | Notes |
|---------|--------|-------|
| **Multi-provider support** | Anthropic, OpenAI, GitHub Copilot, Ollama, Generic OpenAI | CC is Anthropic-only |
| **Multi-agent team system** | Full mesh — spawn, mailbox, task list, swarm | CC has only single-level ephemeral AgentTool |
| **`/swarm` auto-decomposition** | Policy-driven parallel multi-agent work | No CC equivalent |
| **Auto-compact** | Triggers before context limit; no user action needed | CC requires manual `/compact` |
| **LSP tools** | definition, hover, references, symbols, diagnostics | No CC equivalent |
| **Office document tools** | docx/xlsx/pptx read+write | No CC equivalent |
| **LibreOffice tools** | odt/ods/odp read+write | No CC equivalent |
| **PDF tools** | pdf_read, pdf_write | No CC equivalent |
| **HTTP server mode** | ragent-server (SSE/REST API) | No CC equivalent |
| **Skill system** | `.md` custom tool profiles with dynamic context injection | No CC equivalent |
| **Snapshot/checkpoint** | Save and restore full agent state | No CC equivalent |
| **@Reference system** | `@file`, `@url`, `@symbol` inline in prompts | No CC equivalent |
| **Custom agent profiles** | `.md` format with provider/model pinning per agent | No CC equivalent |
| **Per-teammate model override** | Each team member uses a different provider/model | No CC equivalent |
| **Todo/task tools** | todo, plan, new_task, wait_tasks | No CC equivalent |
| **Websearch tool** | Built-in web search | No CC equivalent |
| **Live token bar** | Real-time token count in TUI status bar | No CC equivalent |
| **Offline/local LLM** | Full Ollama support | CC requires Anthropic account |
| **Parallel tool execution** | Multiple tool calls run concurrently | CC is sequential |
| **Background bash** | Long-running shell commands in background | No CC equivalent |

---

*End of CCGAP.md*
