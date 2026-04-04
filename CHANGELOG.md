# Changelog

## [0.1.0-alpha.24] - 2026-04-04

### Fixed
- **Security Audit CI** — resolved all failures in both `cargo audit` and `cargo deny check` jobs:
  - Upgraded `criterion` 0.4→0.5 in `ragent-core` and `ragent-server` (eliminates `atty` RUSTSEC-2021-0145/2024-0375)
  - Updated `rustls-webpki` 0.103.9→0.103.10 (fixes RUSTSEC-2026-0049 CRL matching bug)
  - Updated `quinn-proto` 0.11.13→0.11.14 (fixes RUSTSEC-2026-0037 Quinn DoS)
  - Added `.cargo/audit.toml` to ignore RUSTSEC-2026-0002 (`lru` via ratatui; unexploitable)
  - Rewrote `deny.toml` for cargo-deny ≥0.19 schema (removed obsolete keys: `vulnerability`, `unmaintained`, `unlicensed`, `copyleft`)
  - Added `CDLA-Permissive-2.0` to allowed license list (webpki-roots 1.0.6)
  - Added advisory ignore entries for `paste` and `proc-macro-error` (transitive unmaintained, unfixable)
  - Added `license.workspace = true` to `prompt_opt/Cargo.toml`
  - Changed `wildcards = "deny"` → `"warn"` to avoid false-positives on workspace-internal path deps
- **Tool hallucination prevention** — inject `## Available Tools` section into every session's system prompt listing all registered tool names exactly, preventing models (especially Ollama-hosted) from calling non-existent tools like `search` instead of `grep`

## [0.1.0-alpha.23] - 2026-04-04

### Added
- **`/bash` slash command** — dynamic bash allowlist/denylist management:
  - `/bash add allow <cmd>` — exempt a banned command prefix (e.g. `curl`) from the built-in banned-command check
  - `/bash add deny <pattern>` — add a substring pattern that unconditionally rejects any matching command
  - `/bash remove allow|deny <entry>` — remove an entry from either list
  - `/bash show` — display current user-defined allowlist and denylist
  - `/bash help` — usage reference
  - Append `--global` to any add/remove to write to `~/.config/ragent/ragent.json` instead of the project `ragent.json`
  - Lists are persisted in `ragent.json` under a new `bash` top-level key (`allowlist`, `denylist` arrays)
  - Global and project configs are merged at startup; also reloaded on `/reload`
  - Allowlisted commands bypass the built-in banned-command check without entering YOLO mode
  - Denylisted patterns are checked after built-in DENIED_PATTERNS

### Fixed
- **CI warnings-as-errors** — resolved all 14 errors flagged by `cargo check -D warnings`:
  - `session/processor.rs` — renamed unused `finish_reason` variable to `_finish_reason`
  - `team/config.rs` — added missing doc comment on `last_spawn_error` field
  - `tool/github_issues.rs` — added doc comments to all 5 tool structs
  - `tool/github_prs.rs` — added doc comments to all 5 tool structs

## [0.1.0-alpha.22] - 2026-06-09

### Added
- **M5 Ecosystem Integration:**
  - **MCP Tool Wiring (5.1):** `McpToolWrapper` implementing the `Tool` trait; MCP server tools are discovered at startup and registered in the tool registry as `mcp_{server_id}_{tool_name}`; `ToolRegistry` now uses `RwLock` for interior mutability enabling runtime registration; `SessionProcessor` holds `OnceLock<Arc<RwLock<McpClient>>>` with `set_mcp_client()` wired into `main.rs`
  - **Hooks System (5.4):** `HookConfig` with `HookTrigger` enum (`on_session_start`, `on_session_end`, `on_error`, `on_permission_denied`); `fire_hooks()` async fire-and-forget via `tokio::spawn`; hooks configured in `ragent.json` under `"hooks"` array; 4 lifecycle trigger points wired into `session/processor.rs`; hooks merge (append) across config files
  - **Auto-Update (5.5):** `ragent_core::updater` module with `check_for_update()` (GitHub releases API, 10s timeout), `download_and_replace()` (300s timeout, replaces running binary), `is_newer()` semver comparison; `/update` slash command with interactive confirmation
  - **`/doctor` Diagnostic Command (5.6):** System diagnostics report including binary path, version, config validity, MCP server connectivity, provider/model selection, tool count, session store status



### Added
- **M4 Advanced Features & Differentiation:**
  - **Persistent Shell (4.1+4.2):** Bash tool now preserves shell environment (env vars, cwd) between calls using temp state files; `BashResetTool` to wipe state; status bar shows shell cwd in yellow when it differs from project root via `ShellCwdChanged` event
  - **GitHub OAuth & API Client (4.3):** Device flow OAuth (`/github login`), token stored at `~/.ragent/github_token` (chmod 600); `/github logout` and `/github status`; `GitHubClient` with GET/POST/PATCH/PUT, rate-limit handling, auto-detects repo from `git remote`
  - **GitHub Issue Tools (4.4):** `github_list_issues`, `github_get_issue`, `github_create_issue`, `github_comment_issue`, `github_close_issue`
  - **GitHub PR Tools (4.5):** `github_list_prs`, `github_get_pr`, `github_create_pr`, `github_merge_pr`, `github_review_pr`
  - **Advanced Security (4.6):** 14 attack tools added to banned list (nmap, sqlmap, hydra, metasploit, etc.); 30+ new denied patterns (sudo, privilege escalation, /dev/tcp exfiltration, destructive git ops); `pushd`/`cd ~/$HOME` escape detection; `check_path_within_root` directory escape guard applied to create/edit/write/rm/patch tools

## [0.1.0-alpha.20] - 2026-05-21

### Added
- Input changes improvements
- New OpenAI generic provider support
- **CCGAP CC1 — Context & Safety Foundations:**
  - Git status injection: `{{GIT_STATUS}}` template variable with branch, status, and recent commits
  - README injection: `{{README}}` template variable reads from working directory
  - Bash safety: Safe-command whitelist (git, pwd, tree, date, which) for fast-path validation
  - Bash safety: Banned-command list (curl, wget, nc, telnet, axel, aria2c, lynx, w3m) blocks risky tools
  - Bash safety: Directory-escape guard rejects `cd` to parent or absolute paths
  - Bash syntax pre-check: `sh -n -c` validation (1s timeout) before execution
  - Output truncation: Head+tail truncation (15k + 15k chars) for large bash outputs

## [0.1.0-alpha.19] - 2026-03-19

### Changed
- Version bump.

### Added
- Teams UX and lifecycle enhancements:
  - Added `/team open <name>`, `/team close`, `/team delete <name>`, and `/team clear`
  - Updated `/team tasks` to render a tabular task/status view
  - Improved team-session reliability with TeamManager lazy initialization in TUI team flows
  - Fixed slash-input cursor behavior for `/team ...` entry
- Context management improvements:
  - Added automatic pre-send context compaction near context-window limits with queued message replay
- Copilot provider enhancements:
  - Added reasoning level selection support (`low`, `medium`, `high`, `none`)
  - Added model request-cost multiplier display in model selector
  - Improved model compatibility filtering for chat-completions endpoint usage
- Stability and docs:
  - Added read tool line-range validation to prevent runtime panics
  - Added `docs/howto_teams.md` comprehensive Teams user manual
  - Expanded tests around teams slash commands and task rendering

## [0.1.0-alpha.18] - 2026-03-19

### Changed
- Version bump — no functional changes since 0.1.0-alpha.17

### Added
- 2 major changes highlighted by the user: 1. Custom Agents 2. Agent Teams
  - Custom Agents: added support for user-defined agents via configuration and dynamic loading; new examples and tests included
  - Agent Teams: team coordination APIs and TUI panels for managing multi-agent teams; task claiming and messaging tools implemented




All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0-alpha.16] - 2026-03-17

### Added
- **Milestone 5 — Multi-agent orchestration hardening** (F6 extensions)
  - **Pluggable transport** — `HttpRouter` routes jobs to remote agents via HTTP POST; `RouterComposite` chains multiple routers with first-success fallback
  - **Leader election** — `LeaderElector` provides vote-based in-process leader election with deterministic tie-breaking and `LeaderEvent` broadcast; `CoordinatorCluster` routes jobs to the elected leader node
  - **Policy-based conflict resolution** — `ConflictResolver` wraps any `ConflictPolicy` variant (`Concat`, `FirstSuccess`, `LastResponse`, `Consensus{threshold}`, `HumanReview`); `HumanFallback` trait with `LoggingFallback` default
  - `Coordinator::with_policy()` consuming builder wires a `ConflictResolver` into `start_job_sync`
  - 41 new tests across `test_orchestrator_transport.rs`, `test_orchestrator_leader.rs`, `test_orchestrator_policy.rs`
  - SPEC.md and QUICKSTART.md updated with M5 API reference and examples

## [0.1.0-alpha.15] - 2026-03-18

### Added
- **Image attachment support (Alt+V)** — paste images from clipboard or file URIs to include screenshots and visuals in conversations with the LLM
  - New `MessagePart::Image` variant stores image path and MIME type
  - Clipboard raw image data (RGBA pixels) encoded as PNG and saved to temp file
  - File URIs (`file:///...`) with image extensions are recognized and staged
  - Pending attachments displayed in input widget before sending
- **Keybindings help panel (`?`)** — type `?` on empty input to view all keyboard shortcuts
- **Right-click context menu** — Cut/Copy/Paste context menu for input and message panels
- **Context-window utilisation display** — status bar shows percentage of context window used (Copilot provider)
- **Session-prefixed step numbers** — tool call logs now show `[sid:step]` format for easier cross-session correlation

### Changed
- Provider layer extended with `supports_images()` capability check
- Anthropic provider supports image content blocks in message assembly
- Copilot provider supports vision-capable models with base64 image URLs
- Step map now stores `(short_session_id, step_number)` tuples

## [0.1.0-alpha.14] - 2026-03-17

### Added
- **MCP server auto-discovery** — new `/mcp discover` command scans PATH, npm global packages, and well-known MCP registry directories for installed MCP servers
  - Recognizes 18 known MCP servers (filesystem, GitHub, git, postgres, sqlite, memory, brave-search, fetch, puppeteer, slack, google-drive, google-maps, sentry, sequential-thinking, everything, time, aws-kb-retrieval, exa)
  - Scans `@modelcontextprotocol` npm scope for installed servers
  - Reads Claude Desktop, Cline, and generic MCP registry directories
  - Discovered servers can be added to `ragent.json` config
- **TUI MCP discovery panel** — F9 key opens discovery panel showing available MCP servers

### Changed
- MCP module structure reorganized with new `discovery.rs` submodule

## [0.1.0-alpha.13] - 2026-03-17

### Added
- **LSP test prompts** — 5 test prompts for LSP server integration testing (hover, go-to-definition, find-references, list-symbols, diagnostics) targeting Rust language servers
- **Office 365 test prompts** — 5 test prompts for Office document read/write testing (Word, Excel, PowerPoint, multi-sheet Excel, output formats)
- **LSP workspace folders support** — updated LSP client to use `workspace_folders` parameter instead of deprecated `root_uri`

### Fixed
- **Unused imports and dead code** — removed unused `LspDiscoverState` import and dead `get_json_str`/`get_json_u64` helper functions
- **Missing documentation** — added doc comments for `ExperimentalFlags` struct and `VsCodeExtension` variant field

## [0.1.0-alpha.12] - 2026-03-17

### Added
- **OpenSkills support** — extended skill loader to support OpenSkills-format skill files
- **Output file support for `/simplify` skill** — skill now accepts optional output path argument to save findings to a markdown file

### Fixed
- **Overly aggressive read function** — improved large file handling with section maps

## [0.1.0-alpha.11] - 2026-03-16

### Added
- **Skills system** fully implemented (SPEC §3.19) across 10 phases
  - YAML frontmatter-based skill definitions (`SKILL.md` format)
  - Multi-scope skill registry with priority: Bundled < Enterprise < Personal < Project
  - Argument substitution: `$ARGUMENTS`, `$N` (0-indexed), `$ARGUMENTS[N]`, `${RAGENT_SESSION_ID}`, `${RAGENT_SKILL_DIR}`
  - Dynamic context injection via `` !`command` `` shell execution patterns
  - Forked subagent execution for `context: fork` skills
  - 4 bundled skills: `simplify`, `batch`, `debug`, `loop`
  - `/skills` slash command with table display (Command, Scope, Access, Description)
  - Skill autocomplete in TUI slash menu (skills rendered in yellow)
  - Config `skill_dirs` for additional skill search directories
  - System prompt integration: active skills injected into agent context
  - `release` project skill for automated version bump, commit, push, and tag
- ~110 new tests for skill system (parsing, args, context, discovery, invocation, TUI)

### Changed
- SPEC.md: Skills section (§3.19) marked ✅ with implementation details
- QUICKSTART.md expanded with additional usage guidance
- Total test count: 781 (up from 672)

## [0.1.0-alpha.10] - 2026-03-13

### Changed
- Version bump — no functional changes since 0.1.0-alpha.9

## [0.1.0-alpha.9] - 2026-03-13

### Changed
- Version bump — no functional changes since 0.1.0-alpha.8

## [0.1.0-alpha.8] - 2026-03-12

### Added
- Step numbers `[#N]` displayed next to tool calls in both message and log panels for cross-referencing
- Pretty-printed JSON for tool call parameters in log panel
- Restored session tool calls now appear in log panel with `(restored)` suffix
- Event bus lag warning when broadcast events are dropped

### Changed
- Message/log panel split ratio changed from 70/30 to 60/40
- Event bus capacity increased from 256 to 2048 events
- TUI event loop now drains all pending bus events per cycle using `try_recv()` instead of processing one at a time
- Log panel auto-scroll now uses rendered (wrapped) line count for correct bottom tracking

### Fixed
- Log panel missing entries when tool output wraps to many lines (scroll calculation used logical vs rendered line count)
- Event bus lag causing silent loss of tool call log entries during burst activity
- Resumed sessions not showing tool calls in log panel
- 148 build warnings across workspace (missing docs, unused variables, dead code) — all resolved

## [0.1.0-alpha.7] - 2026-03-11

### Added
- `rm` tool — delete a single file (no wildcards), with path and success/failure display in TUI (23 tools total)
- `/tools` command now shows indented parameter details (name + description) for each tool
- SPEC.md: `orchestrator` and `debug` agents with tool group restrictions
- SPEC.md: Task delegation via `new_task` tool for orchestrator-style workflows
- SPEC.md: Codebase indexing & semantic search (§3.22) — embeddings + vector DB + Tree-sitter
- SPEC.md: Post-edit diagnostics integration (§3.23) — write delay + LSP error detection
- SPEC.md: Task todo list (§3.24) — interactive in-session task tracking
- SPEC.md: Prompt enhancement (§3.25) — AI-powered prompt refinement before sending
- SPEC.md: Hierarchical custom instructions (§3.26) — global/project/agent-specific rules
- SPEC.md: File ignore patterns `.ragentignore` (§3.27) — agent file access control
- SPEC.md: Suggested responses (§3.28) — context-aware follow-up suggestions
- SPEC.md: Shadow git checkpoints in Snapshot & Undo (§3.16)
- SPEC.md: `--allow-tool`, `--deny-tool`, `--allow-all-tools` CLI flags
- SPEC.md: `-p`/`--prompt` programmatic mode flag
- SPEC.md: `/context`, `/checkpoint`, `/todo` slash commands
- SPEC.md: Auto-compaction at 95% context limit, message enqueueing, inline rejection feedback
- SPEC.md: Hooks (§3.17), custom agents (§3.18), skills (§3.19), persistent memory (§3.20), trusted directories (§3.21)
- SPEC.md: Future goals F11–F20 (ACP, marketplace, API profiles, concurrent ops, custom tools, etc.)
- Planned tools: `new_task`, `switch_agent`, `codebase_search`, `generate_image`

### Changed
- Tool registry now contains 23 tools (up from 22)
- SPEC.md expanded from ~1636 to ~2168 lines with Copilot CLI and Roo Code feature parity analysis

## [0.1.0-alpha.6] - 2026-03-11

### Added
- TUI display summaries for `office_read`, `office_write`, `office_info`, `pdf_read`, and `pdf_write` tools (file path + line count)

### Fixed
- Panic in text selection when selecting lines containing multi-byte UTF-8 characters (e.g., `●`) — byte offsets now snap to valid char boundaries
- `office_read`, `office_write`, `office_info`, `pdf_read`, `pdf_write` tools now show file path and line count in the messages panel

## [0.1.0-alpha.5] - 2026-03-11

### Added
- `create` tool — create a new file with content, truncating if it already exists; creates parent directories as needed (22 tools total)
- Slash command output headers — all slash commands (`/about`, `/help`, `/system`, `/tools`) now prefix output with `From: /<command>` for clarity
- Each slash command now produces a separate message block with its own indicator dot

### Fixed
- Slash command output truncation — messages panel scroll calculation now uses `Paragraph::line_count()` to account for word-wrapped lines instead of logical line count
- Slash command viewport not scrolling to bottom — `scroll_offset` now resets to 0 when any slash command is executed
- `ratatui` `unstable-rendered-line-info` feature enabled for accurate wrapped-line measurement

### Changed
- Tool registry now contains 22 tools (up from 21)
- TUI display summaries added for `create` tool (input path + result line count)

## [0.1.0-alpha.4] - 2026-03-11

### Added
- `multiedit` tool — apply multiple edits to a single file atomically with line-based targeting
- `patch` tool — apply unified diff patches to files with fuzzy matching
- `webfetch` tool — fetch URLs and convert HTML to clean markdown via `htmd`
- `websearch` tool — web search via Tavily API with structured results (titles, URLs, snippets)
- `plan_enter` tool — delegate a task to the planning agent via event-driven agent switching
- `plan_exit` tool — return from planning agent to previous agent with summary injection
- `todo_read` tool — list session-scoped TODO items with optional status filter
- `todo_write` tool — add, update, remove, or clear TODO items with persistent SQLite storage
- Agent delegation architecture: `AgentSwitchRequested` / `AgentRestoreRequested` events, agent stack in TUI, pending action dispatch on `MessageEnd`
- `ToolContext.storage` field (`Option<Arc<Storage>>`) for tools needing database access
- `todos` table in Storage with full CRUD (`create_todo`, `get_todos`, `update_todo`, `delete_todo`, `clear_todos`)
- `todo` permission rule (Allow) in default agent permissions
- TUI display summaries for all 8 new tools (input descriptions + result summaries)
- SSE serialization for `AgentSwitchRequested` and `AgentRestoreRequested` events
- 111 new tests across 7 test files (538 total)

### Changed
- Tool registry now contains 21 tools (up from 13)
- Processor detects `agent_switch` / `agent_restore` metadata in tool results and breaks agent loop

## [0.1.0-alpha.3] - 2026-03-10

### Added
- AGENTS.md auto-loading on session start — project guidelines are injected into the system prompt for all multi-step agents (general, build, plan, explore); excluded for ask and internal utility agents
- AGENTS.md init exchange — model acknowledges project guidelines with a visible greeting in the message window on first message
- TUI tool call display improvements:
  - Tool names capitalized (Read, Write, Bash, Grep, etc.)
  - File paths shown relative to project root instead of absolute
  - Result summary lines with "└" prefix (e.g., "└ 72 lines read", "└ 45 lines written to INDEX.md")
  - Per-message-part spacing for visual separation
- INDEX.md document index with summaries of all root-level markdown files
- `content_line_count` field on `ToolResult` event for accurate line counts
- `force_new_message` flag in TUI to separate init exchange from main response
- History reconstruction now generates `tool_result` messages for `/compact` compatibility

### Fixed
- `/compact` slash command "tool_use ids without tool_result" error — `history_to_chat_messages()` now injects synthetic user messages with `ToolResult` parts for each assistant tool call
- Read tool line count showing truncated count (e.g., "5 lines" for a 1593-line file) — now uses full content line count before truncation
- Write tool showing "1 line written" regardless of actual content — now uses metadata `lines` field from tool output
- Write tool missing filename in display — `ToolCallArgs` event now sends full JSON args instead of truncated 200-char preview
- AGENTS.md init exchange no longer interferes with tool call decoding — uses isolated message list without the user's actual message
- Tool input parsing for write/edit tools with large content — full args JSON sent to TUI for proper field extraction

### Changed
- `ToolCallArgs` event now carries full args JSON (truncation moved to log display only)
- `content_line_count` computation uses tool metadata `lines` field when available, falls back to result content line count
- `build_system_prompt()` loads AGENTS.md from working directory for multi-step agents
- `history_to_chat_messages()` rewritten from iterator `.map()` to imperative loop with tool result injection

## [0.1.0-alpha.2] - 2025-07-25

### Added
- `/provider_reset` slash command with interactive provider selection UI
- Persistent provider disable flag — reset providers stay disabled across restarts
- Clipboard copy support on Copilot device code screen (`c` key, Linux-aware via `arboard`)
- Storage methods: `delete_provider_auth()`, `delete_setting()` with full test coverage
- `discover_api_base_multi_source()` for robust Copilot API base resolution
- VS Code-compatible headers for Copilot chat API (fixes 400 errors on plan-specific endpoints)

### Fixed
- Copilot "Unknown model" error — DB-stored device flow token now prioritised over `gh` CLI token
- Copilot API base URL resolution uses plan-specific endpoint (`api.individual.githubcopilot.com`)
- Provider reset now properly persists by storing a disabled flag in settings

### Changed
- `CopilotDeviceFlowComplete` event now carries `api_base` field
- Token exchange returns `TokenExchangeResult` with optional endpoints
- `resolve_api_key` for Copilot checks DB-stored token first, then falls back to other sources

## [0.1.0-alpha.1] - 2026-03-09

### Added
- TUI home screen with ASCII logo, random tips, and centered prompt
- Interactive provider setup dialog (select provider → enter API key → choose model)
- Provider health indicator (green/yellow/red) on home and chat screens
- Slash-command autocomplete dropdown (`/agent`, `/model`, `/provider`)
- Agent cycling with Tab/Shift+Tab across non-hidden agents
- `ask` agent — quick Q&A without tools (single-step, no project context)
- Settings key-value table in SQLite for persisting user preferences
- Input history navigation with Up/Down arrow keys
- API key resolution from database (fallback after env vars and auto-discovery)
- Extended thinking control forwarded to all providers via agent options
- `SessionManager::storage()` accessor for direct storage access
- Messages now persisted to database on send and receive

### Changed
- Default agent changed from `build` to `general`
- `build` agent demoted from Primary to Subagent mode
- Improved error handling in `SessionProcessor` — errors now emit both `AgentError` and `MessageEnd` events so the TUI always resets
- Single-step agents omit tool definitions from LLM requests
- Copilot provider falls through to database key check instead of hard-failing
- API key error messages now suggest `ragent auth` command

### Fixed
- TUI no longer hangs when provider or API key is missing (error events always emitted)
- `resolve_api_key` iterates env var list by reference (avoids move)

## [0.1.0-alpha.0] - 2026-03-01

### Added
- Initial project scaffolding with Cargo workspace (3 crates)
- Core library (ragent-core): agent, config, event, llm, mcp, message, permission, provider, session, snapshot, storage, tool modules
- Provider adapters for Anthropic and OpenAI with SSE streaming
- GitHub Copilot provider with auto token discovery
- Ollama provider for local/remote LLM support
- 8 built-in tools: read, write, edit, bash, grep, glob, list, question
- Permission system with glob-based rule matching
- SQLite storage for sessions, messages, and provider auth
- HTTP server (ragent-server) with REST + SSE endpoints via axum
- Terminal UI (ragent-tui) with ratatui
- CLI entry point with clap (run, serve, session, auth, models, config commands)
- Event bus for real-time internal pub/sub
- File snapshot/restore for undo support
- Workspace-wide lint configuration (clippy pedantic, nursery, missing_docs)
