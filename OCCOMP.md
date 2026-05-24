# OpenClaw vs ragent — Comparative Analysis

**Document:** OCCOMP.md  
**Date:** 2025-01-19  
**OpenClaw rev:** main (v2026.5.20+)  
**ragent rev:** v0.1.0-alpha.91

---

## 1. Executive Summary

| Dimension | OpenClaw | ragent |
|-----------|----------|--------|
| **Language / Runtime** | TypeScript (Node.js 22.19+) | Rust (edition 2024), statically linked binary |
| **Binary size** | ~MBs of JS + npm deps | Single static binary, zero runtime deps |
| **Primary UI** | Terminal CLI + Web Control UI (Vite+Lit) | Full-screen TUI (ratatui) + HTTP REST API |
| **Architecture** | Gateway daemon (loopback/WebSocket) + plugins | Standalone binary (TUI/server/run modes) |
| **Channels** | 20+ messaging channels (WhatsApp, Telegram, Slack, Discord, Signal, iMessage, Teams, etc.) | None (terminal-only input) |
| **Multi-user** | Multi-agent with isolated workspaces per user | Single-user (teams are task-coordination, not isolation) |
| **Sandboxing** | Docker, SSH, OpenShell backends | Permission-layer + bash security (no container sandbox) |
| **Plugin ecosystem** | npm/ClawHub-based plugins (channels, tools, providers, hooks) | MCP client only; no plugin system |
| **Target user** | Personal assistant across all your devices/channels | Terminal-centric coding agent for developers |

**Bottom line:** OpenClaw is a broad personal AI assistant platform; ragent is a focused coding agent. OpenClaw has ~20× the surface area in channels, media, and automation. ragent's strengths are Rust performance, zero-dependency deployment, and deep code-index integration.

---

## 2. Side-by-Side Feature Matrix

### 2.1 Core Platform

| Feature | OpenClaw | ragent | Gap |
|---------|----------|--------|-----|
| **Language** | TypeScript | Rust | — |
| **Binary deps** | Node.js + npm + platform libs | Zero (statically linked) | ragent wins |
| **Start time** | Gateway daemon warm-up | Instant | ragent wins |
| **Memory footprint** | Higher (Node + V8) | Lower (Rust native) | ragent wins |
| **Daemon mode** | ✅ Yes (systemd/launchd) | ❌ No (foreground only) | OpenClaw wins |
| **Config format** | JSON5 (comments, trailing commas) | JSON/JSONC | Tie |
| **Config location** | `~/.openclaw/openclaw.json` | `.ragent/ragent.json` + `~/.config/ragent/` | Tie |

### 2.2 User Interfaces

| Feature | OpenClaw | ragent | Gap |
|---------|----------|--------|-----|
| **Terminal UI** | Basic CLI output | Full ratatui with panels, markdown, syntax highlighting | ragent wins |
| **Web UI** | Control UI (Vite + Lit, built-in) | HTTP API only (no built-in UI) | OpenClaw wins |
| **Mobile UI** | Companion apps (planned) | None | OpenClaw wins |
| **Slash commands** | Rich `/` commands | Rich `/` commands | Tie |
| **Autocomplete** | Yes | Yes | Tie |

### 2.3 Channels / Input Methods

| Feature | OpenClaw | ragent | Gap |
|---------|----------|--------|-----|
| **WhatsApp** | ✅ Native | ❌ | OpenClaw only |
| **Telegram** | ✅ Native | ❌ | OpenClaw only |
| **Slack** | ✅ Native | ❌ | OpenClaw only |
| **Discord** | ✅ Native (incl. voice) | ❌ | OpenClaw only |
| **Signal** | ✅ Native | ❌ | OpenClaw only |
| **iMessage** | ✅ Native | ❌ | OpenClaw only |
| **Microsoft Teams** | ✅ Native | ❌ | OpenClaw only |
| **Google Chat** | ✅ Native | ❌ | OpenClaw only |
| **IRC** | ✅ Native | ❌ | OpenClaw only |
| **Matrix** | ✅ Native | ❌ | OpenClaw only |
| **LINE / Zalo / WeChat / QQ** | ✅ Native | ❌ | OpenClaw only |
| **Email** | ❌ Not listed | ❌ | Tie |
| **Web chat** | ✅ Control UI | ❌ HTTP API only | OpenClaw wins |

### 2.4 LLM Providers

| Provider | OpenClaw | ragent |
|----------|----------|--------|
| Anthropic | ✅ | ✅ |
| OpenAI | ✅ | ✅ |
| OpenRouter | ✅ | ❌ |
| Google Gemini | ✅ | ✅ |
| Ollama (local) | ✅ | ✅ |
| Ollama Cloud | ❌ | ✅ |
| HuggingFace | ❌ | ✅ |
| GitHub Copilot | ❌ | ✅ |
| Azure AI Foundry | ❌ | ✅ |
| Azure Resource (File) | ❌ | ✅ |
| Generic OpenAI-compatible | ✅ | ✅ |
| Mistral / Voyage | ✅ (embeddings) | ❌ |
| LiteLLM | ✅ (pricing) | ❌ |

**Gap:** ragent lacks OpenRouter, LiteLLM, and Mistral/Voyage embedding providers. OpenClaw lacks Ollama Cloud, HuggingFace, Copilot, and Azure-specific providers.

### 2.5 Agent Runtimes

| Feature | OpenClaw | ragent | Gap |
|---------|----------|--------|-----|
| **Built-in runtime** | ✅ "PI" embedded | ✅ Built-in loop | Tie |
| **Codex app-server** | ✅ Native | ❌ | OpenClaw wins |
| **Claude Code harness** | ✅ via ACP | ❌ | OpenClaw wins |
| **Gemini CLI harness** | ✅ via ACP | ❌ | OpenClaw wins |
| **OpenCode harness** | ✅ via ACP | ❌ | OpenClaw wins |
| **Cursor harness** | ✅ via ACP | ❌ | OpenClaw wins |
| **Custom harness plugins** | ✅ Plugin SDK | ❌ | OpenClaw wins |

### 2.6 Tools

| Category | OpenClaw | ragent |
|----------|----------|--------|
| **File ops** | read, write, edit, apply_patch | read, write, create, edit, multiedit, patch, rm, move, copy, mkdir, append, file_info, diff, glob, list |
| **Shell/exec** | exec, process, code_execution | bash, bash_reset (7-layer security) |
| **Web** | web_search, web_fetch, x_search | webfetch, websearch, http_request |
| **Browser** | browser (sandboxed) | ❌ |
| **Search** | — | grep |
| **Code intelligence** | ❌ | codeindex_search, codeindex_symbols, codeindex_references, codeindex_dependencies, codeindex_status, codeindex_reindex |
| **Messaging** | message (channel replies) | ❌ (no channels) |
| **Sessions/agents** | sessions_*, agents_list, subagents | session management, new_task, cancel_task, wait_tasks |
| **Automation** | cron, heartbeat_respond | ❌ |
| **Media** | image, image_generate, music_generate, video_generate, tts | ❌ |
| **GitHub/GitLab** | ❌ (via skills/plugins) | 29 native VCS tools |
| **Office/PDF** | ❌ | office_read/write/info, libre_read/write/info, pdf_read, pdf_write |
| **Memory** | memory_search, memory_get | memory_read, memory_write, memory_replace, memory_store, memory_recall, memory_forget, memory_search, memory_migrate |
| **Teams** | ❌ | 20 team lifecycle tools |
| **Calculator** | ❌ | ✅ |
| **Planning** | ❌ | plan_enter |
| **Interactive** | — | question, think, todo_read, todo_write |
| **Large tool catalog** | tool_search_code, tool_search, tool_describe | ❌ |

### 2.7 Multi-Agent / Teams

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Model** | Fully isolated agents (workspaces, auth, sessions) | Task-coordination teams (shared context) |
| **Isolation** | Per-agent workspace + state dir | Shared workspace |
| **Auth separation** | Per-agent auth profiles | Global auth |
| **Session store** | Per-agent | Global |
| **Cross-agent memory** | QMD extraCollections | Shared memory system |
| **Bindings** | Channel→agent routing | N/A |
| **Sub-agents** | sessions_spawn (background runs) | new_task (background tasks) |
| **Max nesting** | 5 levels | Not explicitly limited |
| **Concurrency** | 8 default (configurable) | 8 concurrent background tasks |

### 2.8 Memory System

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Long-term memory file** | MEMORY.md | MEMORY.md (file blocks) |
| **Daily notes** | memory/YYYY-MM-DD.md | ❌ (structured store instead) |
| **Dreaming** | ✅ (opt-in background consolidation) | ❌ |
| **Structured memory** | ❌ (file-based only) | ✅ (SQLite store with categories, tags, confidence) |
| **Semantic search** | ✅ (multiple backends) | ✅ (optional embeddings) |
| **Backends** | Builtin SQLite, QMD, Honcho, LanceDB | SQLite + optional embedding provider |
| **Hybrid search** | ✅ (vector + keyword) | ✅ (FTS5 + optional embeddings) |
| **Memory wiki** | ✅ (separate plugin) | ❌ |
| **Auto-extraction** | ✅ (memory flush before compaction) | ✅ (automatic extraction) |
| **Compaction integration** | ✅ (memory flush turn) | ✅ (atomic tool call preservation) |

### 2.9 Skills System

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Format** | SKILL.md with YAML frontmatter (AgentSkills) | YAML skill packs |
| **Sources** | Workspace, project-agent, personal-agent, managed, bundled, extra dirs, plugins | Bundled or custom YAML files |
| **Discovery** | ClawHub registry | Local files only |
| **Auto-generation** | Skill Workshop plugin (experimental) | ❌ |
| **Hot reload** | ✅ (file watcher) | ❌ |

### 2.10 Plugin System

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Plugin architecture** | Full plugin SDK | ❌ (MCP client only) |
| **Plugin sources** | ClawHub, npm, git, local | N/A |
| **Plugin types** | Channels, providers, tools, hooks, skills, speech, voice, media | N/A |
| **MCP client** | ✅ (consume + manage servers) | ✅ (consume only) |
| **MCP server** | ✅ (expose OpenClaw over MCP) | ❌ |
| **Runtime hooks** | before_tool_call, after_tool_call, lifecycle | ❌ |

### 2.11 Sandboxing / Security

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Sandbox backends** | Docker, SSH, OpenShell | ❌ (permission-layer only) |
| **Sandbox scope** | per-agent, per-session, shared | N/A |
| **Browser sandbox** | ✅ | ❌ |
| **Tool policy** | allow/deny lists + profiles | Permission rules with glob patterns |
| **Exec approval** | ✅ | ✅ (7-layer bash security) |
| **Elevated exec** | ✅ (escape hatch) | ✅ (YOLO mode) |
| **DM pairing** | ✅ (pairing codes for unknown senders) | N/A (no channels) |
| **Directory escape prevention** | ✅ | ✅ |
| **Obfuscation detection** | ❌ | ✅ |
| **Syntax validation** | ❌ | ✅ (sh -n) |
| **Safe command whitelist** | ❌ | ✅ (51 commands) |

### 2.12 Automation / Background Work

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Cron / scheduled tasks** | ✅ | ❌ |
| **Heartbeat** | ✅ (periodic main-session turn) | ❌ |
| **Background tasks ledger** | ✅ | ✅ (tasks list) |
| **Task Flow orchestration** | ✅ (multi-step flows) | ❌ |
| **Hooks** | ✅ (lifecycle event scripts) | ❌ |
| **Standing orders** | ✅ (AGENTS.md injection) | ✅ (AGENTS.md) |
| **Inferred commitments** | ✅ (opt-in follow-ups) | ❌ |
| **Sub-agent spawn** | sessions_spawn | new_task |
| **Delivery to channel** | ✅ (announce back) | ❌ (TUI only) |

### 2.13 Context Management

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Auto-compaction** | ✅ | ✅ |
| **Manual compaction** | ✅ (/compact) | ✅ |
| **Pluggable compaction** | ✅ (plugin providers) | ❌ |
| **Session pruning** | ✅ (tool result trimming) | ❌ |
| **Identifier preservation** | ✅ (strict/custom/off) | ❌ |
| **Memory flush before compaction** | ✅ | ❌ |
| **Context engine plugins** | ✅ | ❌ |

### 2.14 Voice / Media

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **TTS** | ✅ (multiple providers) | ❌ |
| **Speech recognition** | ✅ (Talk mode) | ❌ |
| **Realtime voice** | ✅ (Discord voice, OpenAI Realtime) | ❌ |
| **Image generation** | ✅ | ❌ |
| **Music generation** | ✅ | ❌ |
| **Video generation** | ✅ | ❌ |
| **Media understanding** | ✅ | ❌ |

### 2.15 Web / Network

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Web Control UI** | ✅ (Vite + Lit) | ❌ |
| **HTTP API** | Admin RPC (plugin) | REST + SSE (first-class) |
| **Webhooks** | ✅ | ❌ |
| **Tailscale integration** | ✅ (Serve, Funnel) | ❌ |
| **TLS** | ✅ | ❌ |

### 2.16 Codebase Intelligence

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Code indexing** | ❌ | ✅ (tree-sitter, Tantivy, 15+ languages) |
| **Symbol search** | ❌ | ✅ |
| **Reference finding** | ❌ | ✅ |
| **Dependency queries** | ❌ | ✅ |
| **Incremental updates** | ❌ | ✅ (file watcher) |
| **Language support** | N/A | Rust, Python, TS/JS, Go, C/C++, Java, OpenSCAD, Terraform, CMake, Gradle, Maven |

### 2.17 Git Integration

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Git tools** | ❌ (via exec) | 29 native tools (issues, PRs, pipelines, etc.) |
| **GitHub** | ❌ | ✅ Native |
| **GitLab** | ❌ | ✅ Native |

### 2.18 Office / Document Support

| Feature | OpenClaw | ragent |
|---------|----------|--------|
| **Office read/write** | ❌ | ✅ |
| **LibreOffice** | ❌ | ✅ |
| **PDF read/write** | ❌ | ✅ |

---

## 3. Detailed Capability Analysis

### 3.1 Where OpenClaw Excels

1. **Channel ubiquity**: OpenClaw's 20+ native messaging channels make it a true "personal assistant" that follows you across devices. ragent is terminal-bound.

2. **Voice and realtime**: OpenClaw has TTS, STT, realtime voice on Discord, and OpenAI Realtime integration. ragent has no voice capabilities.

3. **Media generation**: Image, music, and video generation tools are native in OpenClaw. ragent has none.

4. **Sandboxing**: Docker/SSH/OpenShell sandbox backends provide real isolation for untrusted code execution. ragent relies on permission layers and bash security.

5. **Plugin ecosystem**: OpenClaw's npm-based plugin system with ClawHub discovery enables community extensions. ragent has no plugin system.

6. **Automation infrastructure**: Cron, heartbeat, hooks, Task Flow, and inferred commitments create a true "always-on" assistant. ragent requires manual invocation.

7. **Codex integration**: Native Codex app-server runtime + ACP adapters for Claude Code, Gemini CLI, etc. ragent has only its built-in loop.

8. **Multi-agent isolation**: Per-agent workspaces, auth profiles, and session stores enable multi-user deployments. ragent's teams are coordination-only.

9. **Web Control UI**: Built-in browser UI for configuration and chat. ragent's HTTP API has no built-in frontend.

### 3.2 Where ragent Excels

1. **Zero dependencies**: Single static binary with no runtime requirements. OpenClaw needs Node.js + npm + daemon setup.

2. **Performance**: Rust native code with lower memory footprint and faster startup. Node.js/V8 has higher baseline overhead.

3. **Code intelligence**: Deep tree-sitter + Tantivy integration for symbol search, references, and dependencies across 15+ languages. OpenClaw has no code indexing.

4. **Terminal UX**: Full ratatui interface with streaming markdown, syntax highlighting, permission countdown timers, and live tool call display. OpenClaw's CLI is simpler.

5. **GitHub/GitLab native tools**: 29 built-in VCS tools for issues, PRs/MRs, pipelines. OpenClaw delegates to exec/skills.

6. **Office/PDF support**: Native document reading and writing. OpenClaw has none.

7. **Bash security**: 7-layer defense with safe-command whitelist, banned commands, denied patterns, directory escape prevention, syntax validation, obfuscation detection, and user allowlists. OpenClaw has simpler exec approval.

8. **Team coordination**: 20 team tools with shared task lists, mailbox messaging, and swarm decomposition. OpenClaw's sub-agents are simpler background runs.

9. **Prompt optimization**: `/opt` command transforms prompts into 12 structured frameworks instantly. OpenClaw has no equivalent.

---

## 4. Functional Parity Plan

This section provides a phased roadmap to close the most impactful gaps between ragent and OpenClaw. Priorities are based on user value and implementation effort.

### Priority Legend
- **P0** — Critical (blocks core use cases)
- **P1** — High (major feature gaps)
- **P2** — Medium (nice-to-have parity)
- **P3** — Low (differentiation, not parity)

---

## Phase 1: Foundation (P0 — Core Platform)

### Milestone 1.1: Daemon Mode
**Goal:** Enable ragent to run as a background daemon with lifecycle management.

| Task | Description | Effort |
|------|-------------|--------|
| T-001 | Implement `ragent daemon start` command | M |
| T-002 | Implement `ragent daemon stop` / `status` commands | S |
| T-003 | Add systemd service file generation (`--install-service`) | S |
| T-004 | Add macOS launchd plist generation | S |
| T-005 | PID file management and crash recovery | S |
| T-006 | Graceful shutdown with in-flight request draining | M |

**Rationale:** OpenClaw's gateway daemon is central to its always-on model. ragent currently requires a foreground TUI or explicit server command.

---

### Milestone 1.2: Web Control UI
**Goal:** Provide a built-in web interface for configuration and chat.

| Task | Description | Effort |
|------|-------------|--------|
| T-007 | Design Control UI architecture (embed static files in binary) | M |
| T-008 | Implement session listing and resumption in web UI | M |
| T-009 | Implement chat interface with SSE streaming | M |
| T-010 | Implement config editor with schema validation | L |
| T-011 | Add provider setup wizard to web UI | M |
| T-012 | Bundle static assets into release binary | S |

**Rationale:** OpenClaw's Control UI lowers the barrier to entry for non-terminal users. ragent's HTTP API is powerful but has no visual interface.

---

### Milestone 1.3: Enhanced Model Provider Support
**Goal:** Close provider coverage gaps.

| Task | Description | Effort |
|------|-------------|--------|
| T-013 | Add OpenRouter provider with dynamic model discovery | M |
| T-014 | Add Mistral provider | S |
| T-015 | Add Voyage AI embedding provider | S |
| T-016 | Add LiteLLM proxy provider | M |
| T-017 | Implement model failover with auth profile rotation | L |
| T-018 | Implement provider cooldown/backoff system | M |

**Rationale:** OpenClaw's OpenRouter integration provides access to 100+ models. ragent's provider list is narrower.

---

## Phase 2: Communication (P1 — Channels)

### Milestone 2.1: Discord Channel
**Goal:** Add Discord as the first messaging channel.

| Task | Description | Effort |
|------|-------------|--------|
| T-019 | Implement Discord gateway WebSocket client | L |
| T-020 | Implement DM and guild channel message routing | M |
| T-021 | Implement pairing/approval flow for unknown senders | M |
| T-022 | Implement slash command bridge (`/subagents`, etc.) | M |
| T-023 | Implement thread binding support | M |
| T-024 | Add Discord voice channel support (optional) | L |

**Rationale:** Discord is a high-value channel for developers and has the most complete OpenClaw reference implementation.

---

### Milestone 2.2: Additional Channels
**Goal:** Expand channel coverage based on demand.

| Task | Description | Effort |
|------|-------------|--------|
| T-025 | Add Telegram channel | L |
| T-026 | Add Slack channel | L |
| T-027 | Add Signal channel | L |
| T-028 | Add WhatsApp channel | L |
| T-029 | Add Matrix channel | M |
| T-030 | Add generic webhook channel | S |

**Rationale:** Each channel is substantial work. Discord first, then prioritize by user demand.

---

## Phase 3: Automation (P1 — Background Work)

### Milestone 3.1: Cron / Scheduled Tasks
**Goal:** Enable time-based agent execution.

| Task | Description | Effort |
|------|-------------|--------|
| T-031 | Implement cron expression parser and scheduler | M |
| T-032 | Implement task persistence (SQLite) | M |
| T-033 | Implement one-shot reminder (`--at`) support | S |
| T-034 | Add `ragent cron` CLI commands (list, add, remove) | M |
| T-035 | Implement task delivery to channels | M |

**Rationale:** OpenClaw's cron enables "daily reports" and "remind me in 20 minutes." ragent has no scheduling.

---

### Milestone 3.2: Heartbeat
**Goal:** Periodic main-session turns for monitoring.

| Task | Description | Effort |
|------|-------------|--------|
| T-036 | Implement heartbeat timer (default 30 min) | S |
| T-037 | Implement HEARTBEAT.md checklist parsing | M |
| T-038 | Implement heartbeat deferral when busy | S |
| T-039 | Add inbox/calendar check hooks (extensible) | M |

**Rationale:** Heartbeat batches periodic checks into one agent turn, enabling proactive assistance.

---

### Milestone 3.3: Hooks System
**Goal:** Event-driven scripts for lifecycle events.

| Task | Description | Effort |
|------|-------------|--------|
| T-040 | Implement hook discovery from `~/.ragent/hooks/` | S |
| T-041 | Implement lifecycle events: session_new, session_reset, compaction | M |
| T-042 | Implement hook execution with sandboxing | M |
| T-043 | Add `ragent hooks` CLI commands | S |

**Rationale:** Hooks let users run custom scripts on agent lifecycle events.

---

## Phase 4: Voice & Media (P2 — Rich Output)

### Milestone 4.1: Text-to-Speech
**Goal:** Add speech output capability.

| Task | Description | Effort |
|------|-------------|--------|
| T-044 | Add OpenAI TTS provider integration | M |
| T-045 | Add Piper (local) TTS provider | M |
| T-046 | Implement TTS tool and audio playback | M |

**Rationale:** TTS enables hands-free interaction. Lower priority than channels.

---

### Milestone 4.2: Image Generation
**Goal:** Add image creation capability.

| Task | Description | Effort |
|------|-------------|--------|
| T-047 | Add DALL-E provider | S |
| T-048 | Add Stability AI provider | S |
| T-049 | Implement image generation tool | S |
| T-050 | Implement image display in TUI (sixel/kitty protocol) | M |

**Rationale:** Image generation is a "wow" feature but not core to coding workflows.

---

## Phase 5: Advanced Agent Features (P2 — Deepening)

### Milestone 5.1: Sub-agent Enhancement
**Goal:** Richer background agent runs with delivery.

| Task | Description | Effort |
|------|-------------|--------|
| T-051 | Implement thread-bound sub-agent sessions | M |
| T-052 | Implement sub-agent completion delivery to parent | M |
| T-053 | Implement `/subagents` slash commands | S |
| T-054 | Implement sub-agent context fork mode | M |
| T-055 | Implement sub-agent nesting depth limits | S |

**Rationale:** OpenClaw's sub-agents have richer delivery and thread binding. ragent's `new_task` is simpler.

---

### Milestone 5.2: Memory Enhancements
**Goal:** Close memory system gaps.

| Task | Description | Effort |
|------|-------------|--------|
| T-056 | Implement daily notes (`memory/YYYY-MM-DD.md`) | S |
| T-057 | Implement dreaming (background memory consolidation) | L |
| T-058 | Implement memory wiki plugin | L |
| T-059 | Implement hybrid search (vector + keyword) | M |
| T-060 | Add Honcho/LanceDB memory backends | M |

**Rationale:** OpenClaw's memory system is more mature with dreaming and multiple backends.

---

### Milestone 5.3: Context Engine
**Goal:** Pluggable context management.

| Task | Description | Effort |
|------|-------------|--------|
| T-061 | Implement session pruning (tool result trimming) | M |
| T-062 | Implement pluggable compaction providers | M |
| T-063 | Implement identifier preservation policy | S |
| T-064 | Implement memory flush before compaction | M |

**Rationale:** OpenClaw's compaction is more sophisticated with identifier preservation and pluggable providers.

---

## Phase 6: External Harness Support (P2 — Flexibility)

### Milestone 6.1: ACP / External Harness Integration
**Goal:** Support external coding harnesses.

| Task | Description | Effort |
|------|-------------|--------|
| T-065 | Implement ACP protocol client | L |
| T-066 | Add Claude Code harness adapter | L |
| T-067 | Add Gemini CLI harness adapter | L |
| T-068 | Add Codex app-server harness adapter | L |

**Rationale:** OpenClaw can delegate to specialized coding harnesses. ragent's built-in loop is the only option.

---

## Phase 7: Ecosystem (P3 — Long-term)

### Milestone 7.1: Plugin System
**Goal:** Enable third-party extensions.

| Task | Description | Effort |
|------|-------------|--------|
| T-069 | Design plugin API and manifest format | L |
| T-070 | Implement plugin loader (WASM-based?) | L |
| T-071 | Implement plugin registry / discovery | L |
| T-072 | Implement runtime hooks (before/after tool call) | L |

**Rationale:** OpenClaw's plugin ecosystem is a major differentiator. A Rust plugin system would need careful design (WASM is promising).

---

### Milestone 7.2: Browser Automation
**Goal:** Add sandboxed browser tool.

| Task | Description | Effort |
|------|-------------|--------|
| T-073 | Integrate headless Chrome/CDP | L |
| T-074 | Implement browser tool (navigate, click, screenshot) | L |
| T-075 | Implement sandboxed browser container | L |

**Rationale:** Browser automation enables web-based research and testing.

---

### Milestone 7.3: Sandboxing Backend
**Goal:** Real container isolation for code execution.

| Task | Description | Effort |
|------|-------------|--------|
| T-076 | Implement Docker sandbox backend | L |
| T-077 | Implement per-agent container scope | M |
| T-078 | Implement workspace bind-mount logic | M |
| T-079 | Implement sandbox browser in container | L |

**Rationale:** Docker sandboxing provides real isolation for untrusted code.

---

## 5. Effort Summary

| Phase | Milestones | Tasks | Est. Effort |
|-------|------------|-------|-------------|
| Phase 1: Foundation | 3 | 18 | 3-4 months |
| Phase 2: Communication | 2 | 12 | 2-3 months |
| Phase 3: Automation | 3 | 13 | 2-3 months |
| Phase 4: Voice & Media | 2 | 7 | 1-2 months |
| Phase 5: Advanced Agent | 3 | 14 | 2-3 months |
| Phase 6: External Harness | 1 | 4 | 1-2 months |
| Phase 7: Ecosystem | 3 | 11 | 3-4 months |
| **Total** | **17** | **79** | **14-21 months** |

---

## 6. Recommended Priority Order

For maximum impact with minimum effort:

1. **Daemon mode** (Milestone 1.1) — Enables always-on operation
2. **Web Control UI** (Milestone 1.2) — Dramatically expands user base
3. **Discord channel** (Milestone 2.1) — First messaging channel (highest dev demand)
4. **Cron/scheduled tasks** (Milestone 3.1) — Enables automation
5. **OpenRouter provider** (Milestone 1.3) — Instant model access expansion
6. **Sub-agent enhancement** (Milestone 5.1) — Improves parallel work
7. **Heartbeat** (Milestone 3.2) — Proactive agent behavior
8. **TTS** (Milestone 4.1) — Accessibility and hands-free
9. **Memory enhancements** (Milestone 5.2) — Better long-term recall
10. **Plugin system** (Milestone 7.1) — Long-term ecosystem growth

---

## 7. Differentiation Strategy

Rather than pure feature parity, ragent should lean into its strengths while selectively closing high-value gaps:

**Keep as differentiators:**
- Single static binary, zero dependencies
- Rust performance and safety
- Deep code intelligence (tree-sitter + Tantivy)
- Native GitHub/GitLab tools
- Office/PDF document support
- 7-layer bash security
- Team coordination tools

**Close selectively:**
- Daemon mode (enables new use cases)
- Web UI (expands audience)
- One messaging channel (Discord, for community)
- Cron (enables automation)
- OpenRouter (instant model diversity)

**Defer or skip:**
- 20+ messaging channels (not ragent's core audience)
- Media generation (outside coding focus)
- Realtime voice (niche for coding)
- Full plugin ecosystem (MCP may suffice)
- External harness support (built-in loop is fine)

---

## 8. Conclusion

OpenClaw and ragent serve different primary use cases: OpenClaw is a personal AI assistant platform spanning all your devices and channels; ragent is a terminal-native coding agent. OpenClaw has substantially broader surface area (~100K+ lines, 20+ channels, media, voice, automation), while ragent is leaner and more focused (~65K lines, deep code tooling, zero dependencies).

Achieving full functional parity would require 14-21 months of focused development across 79 tasks. A more pragmatic approach is to selectively implement the highest-impact features (daemon mode, web UI, Discord, cron, OpenRouter) while maintaining ragent's core strengths as a fast, portable, code-focused agent.

The two projects can coexist: OpenClaw for users wanting a personal assistant everywhere, ragent for developers wanting a fast, portable coding agent in the terminal.
