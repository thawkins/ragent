# Comparative Analysis: ragent vs Eve Agent V2 Unleashed

> Document: EAVECOMP.md  
> Date: 2025-01-17  
> Status: Draft for Review

---

## 1. Executive Summary

This document provides a head-to-head feature comparison between **ragent** (a Rust-based AI coding agent) and **Eve Agent V2 Unleashed** (a Python-based local-first coding agent). The analysis identifies gaps where ragent can borrow ideas from Eve to improve user experience, tool coverage, and agentic capabilities.

| Dimension | ragent | Eve V2 Unleashed |
|-----------|--------|------------------|
| **Language** | Rust (static binary) | Python (interpreter required) |
| **LLM Providers** | 8+ (multi-provider) | Ollama only (local + cloud) |
| **Tool Count** | ~111 tools (15 categories) | ~12 tools (basic set) |
| **Sub-agents** | Background task spawning | 112 specialized agents |
| **Slash Commands** | ~15 built-in | 111 custom commands |
| **Skills System** | YAML-based, bundled + custom | 273 progressive-load skills |
| **UI** | Terminal UI (ratatui) + Web API | Web UI (cyberpunk theme) |
| **Agentic Loop** | Session-based with permissions | 40-round autonomous loop |
| **Architecture** | 15-crate workspace, event-driven | Monolithic Python, FastAPI |

---

## 2. Feature Comparison Matrix

### 2.1 Core Agent Capabilities

| Feature | ragent | Eve | Gap Analysis |
|---------|--------|-----|--------------|
| Multi-provider support | ✅ 8+ providers | ❌ Ollama only | Eve is locked to Ollama; ragent wins on flexibility |
| Local-first operation | ✅ Via Ollama | ✅ Native Ollama | Both support local, ragent adds cloud options |
| Autonomous agentic loop | ✅ Session-based | ✅ 40-round loop | Eve has explicit round counting; ragent uses event-driven |
| Streaming responses | ✅ SSE + TUI | ✅ SSE token stream | Both real-time; Eve shows token-by-token |
| Context window management | ✅ Configurable per model | ✅ Auto-summarization | Eve auto-compacts; ragent has compaction but less aggressive |
| Conversation history | ✅ SQLite persistent | ✅ Session persistent | Both persistent; ragent has export/import |
| Permission system | ✅ Multi-layered (7 layers) | ✅ Basic blocked patterns | ragent has far more sophisticated security |
| YOLO mode | ✅ Available | ❌ Not mentioned | ragent has trusted-environment bypass |
| Image/vision support | ✅ Clipboard paste (Alt+V), base64 encoding, data-URI transport; Anthropic, OpenAI, Gemini, Copilot, Azure, HuggingFace, Ollama Cloud | ✅ Multimodal via Ollama | **Parity: both support vision** |
| Voice input / TTS | ❌ Not available | 🔄 Roadmap | Both lack; Eve plans it |

### 2.2 Tool System

| Tool Category | ragent | Eve | Notes |
|---------------|--------|-----|-------|
| **File Operations** | read, write, create, edit, multiedit, patch, rm, move, copy, mkdir, append, file_info, diff, glob, list | read_file, read_lines, write_file, insert_after_line, replace_lines, list_directory, glob | ragent has more granular tools (patch, multiedit, diff) |
| **Shell** | bash, bash_reset (7-layer security) | bash (basic blocked patterns) | ragent far ahead on shell security |
| **Search** | grep | grep | Comparable |
| **Web** | webfetch, websearch, http_request | web_search, web_fetch | Comparable; ragent has full http_request |
| **Code Intelligence** | codeindex_search, symbols, references, dependencies, status, reindex | ❌ None | **Major gap for Eve** |
| **Memory** | 8 memory tools (file + structured + semantic) | ❌ None | **Major gap for Eve** |
| **Git/VCS** | 29 GitHub + GitLab tools | git (basic wrapper) | **Major gap for Eve** |
| **Office/PDF** | office_read/write, libre_read/write, pdf_read/write | ❌ None | ragent has document tools |
| **Teams** | 20 team coordination tools | ❌ None | **Major gap for Eve** |
| **Sub-agents** | new_task, cancel_task, list_tasks, wait_tasks | SubagentManager (112 agents) | Eve has more pre-built agents; ragent has runtime spawning |
| **Planning** | plan_enter, plan_exit | ❌ None | ragent has planning hooks |
| **MCP** | mcp_tool (MCP client) | ❌ None | ragent supports MCP servers |
| **Interactive** | question, think, todo_read, todo_write | think | ragent has todo system |
| **Utility** | calculator, get_env | ❌ None | ragent has utility tools |
| **Prompt Optimization** | /opt (12 frameworks) | ❌ None | **Gap for Eve** |
| **Spec Management** | /spec commands | ❌ None | **Gap for Eve** |

**Tool Count Summary:**
- ragent: ~111 tools across 15 categories
- Eve: ~12 tools in basic categories

### 2.3 UI / UX

| Feature | ragent | Eve | Gap Analysis |
|---------|--------|-----|--------------|
| Terminal UI | ✅ ratatui (full-screen) | ❌ Web only | ragent has native TUI; Eve requires browser |
| Web UI | ✅ HTTP API (any frontend) | ✅ Built-in cyberpunk UI | Eve has themed UI; ragent is API-first |
| Slash commands | ~15 built-in | 111 custom commands | **Major gap for ragent** |
| Autocomplete | ✅ Provider + command | ❌ Not mentioned | ragent has command completion |
| Streaming display | ✅ Step-numbered JSON | ✅ Token-by-token terminal | Different approaches |
| Permission countdown | ✅ Live 120s timer | ❌ Not mentioned | ragent has countdown |
| Workspace picker | ❌ CLI only | ✅ UI button | **Gap: ragent lacks UI workspace switching** |
| Animated avatar | ❌ None | ✅ Robot avatar + Eve face | **Gap: ragent lacks personality UI** |
| Theme/styling | ✅ Standard terminal | ✅ Cyberpunk theme | Eve has stronger visual identity |

### 2.4 Agent Ecosystem

| Feature | ragent | Eve | Gap Analysis |
|---------|--------|-----|--------------|
| Built-in agents | 8 presets (coder, debug, etc.) | 112 sub-agents | **Major gap for ragent** |
| Custom agent format | JSON (OASF) + Markdown | YAML definitions | Both extensible; ragent uses standard |
| Agent loading | ~/.ragent/agents/ + project | ~/.eve_unleashed/agents/ | Similar patterns |
| Agent tool restrictions | ✅ Allowed tools per agent | ✅ allowed_tools list | Comparable |
| Agent temperature | ✅ Configurable | ✅ Configurable | Comparable |
| Progressive skill loading | ✅ Keyword-based | ✅ 273 skills | Eve has more skills pre-built |
| Skill format | YAML (skill.yaml + SKILL.md) | YAML (skill.yaml + SKILL.md) | Same pattern! |
| Auto-load skills | ✅ On keyword match | ✅ On keyword match | Comparable |

### 2.5 Configuration & Extensibility

| Feature | ragent | Eve | Gap Analysis |
|---------|--------|-----|--------------|
| Config file format | JSON/JSONC (ragent.json) | .env + Python dict | ragent has richer config |
| Config locations | ~/.config/ragent/ + project | ~/.eve_unleashed/ + project | Similar hierarchy |
| Model switching | Runtime via /model | Runtime via UI dropdown | Comparable |
| Cloud model support | ✅ Multiple clouds | ✅ Ollama cloud only | ragent more flexible |
| Plugin system | ✅ MCP tools | ❌ Not mentioned | ragent has MCP extensibility |
| Hook system | ✅ Shell-command lifecycle hooks (on_session_start, on_error, pre_tool_use, post_tool_use, on_permission_denied); async with timeout; can allow/deny/modify tools | ✅ HookManager (event-driven) | **Parity: both support hooks** |
| Custom system prompt | ✅ Via config | ✅ Via env var | Comparable |

### 2.6 Performance & Deployment

| Feature | ragent | Eve | Gap Analysis |
|---------|--------|-----|--------------|
| Deployment | Single static binary | Python + venv + pip | ragent wins on deployment ease |
| Zero dependencies | ✅ Yes | ❌ Many pip packages | ragent is self-contained |
| Cross-platform | ✅ Linux, macOS, Windows | ✅ All (Python) | Both cross-platform |
| GPU acceleration | ✅ Via provider | ✅ Via Ollama | Comparable |
| Memory usage | Lower (Rust) | Higher (Python) | ragent more efficient |
| Startup time | Faster | Slower (Python import) | ragent faster |

---

## 3. Unique Strengths by Project

### 3.1 ragent Unique Strengths

1. **Multi-provider flexibility** — Not locked to Ollama; works with Anthropic, OpenAI, Gemini, Azure, etc.
2. **Massive tool ecosystem** — 111 tools vs Eve's ~12; especially strong in VCS (29 tools), code intelligence (6 tools), and memory (8 tools)
3. **Security depth** — 7-layer bash security with safe commands, banned commands, denied patterns, directory escape prevention, syntax validation, obfuscation detection, and user lists
4. **Code intelligence** — Tree-sitter parsing + Tantivy FTS + file watcher; Eve has none
5. **Memory system** — Three-tier memory (file blocks, structured SQLite, semantic embeddings); Eve has none
6. **Teams & swarms** — Multi-agent coordination with task lists and messaging; Eve has basic subagents
7. **MCP support** — Model Context Protocol client for external tool servers
8. **Snapshot & undo** — File snapshots before edits; Eve lacks this
9. **Spec management** — Full spec lifecycle with validation and tracking
10. **Static binary** — Single executable, zero runtime dependencies

### 3.2 Eve Agent V2 Unleashed Unique Strengths

1. **Pre-built agent ecosystem** — 112 specialized sub-agents vs ragent's 8 presets
2. **Rich slash commands** — 111 custom commands vs ragent's ~15
3. **Skills volume** — 273 skills pre-built vs ragent's bundled set
4. **Agentic loop depth** — Explicit 40-round loop with self-correction
5. **Web-native UI** — Built-in cyberpunk-themed web terminal
6. **Windows-native** — PowerShell-aware, .bat launchers
7. **Image/vision support** — Multimodal model support (ragent also has this via clipboard paste, Alt+V, and data-URI transport across Anthropic, OpenAI, Gemini, Azure, Copilot, HuggingFace, and Ollama providers)
8. **Context auto-compaction** — Automatic summarization when context overflows
9. **Hook system** — Event hooks for extensibility (ragent also has this via shell-command lifecycle hooks with pre/post tool use, allow/deny/modify decisions, and async execution)
10. **Persona system** — Animated avatar with Eve face panel

---

## 4. Gap Analysis: What ragent Should Borrow from Eve

### 4.1 High Priority (Quick Wins)

| Gap | Eve Implementation | ragent Implementation Path |
|-----|-------------------|---------------------------|
| **More built-in agents** | 112 YAML-defined agents | Expand agent presets beyond 8; create domain-specific agents (Rust, Python, DevOps, security, etc.) |
| **Richer slash commands** | 111 commands in markdown+YAML | Expand /commands; add /fix, /review, /refactor, /test, /docs like Eve |
| **Larger skill library** | 273 progressive skills | Bundle more default skills (Rust, Python, web dev, DevOps, testing) |
| **Image/vision support** | Multimodal via Ollama | Add TUI image preview (ASCII art), vision-specific tools (`analyze_image`, `compare_images`), and screenshot capture |
| **Context auto-compaction** | Auto-summarize when full | Implement automatic context compaction with summarization |

### 4.2 Medium Priority (Architecture Work)

| Gap | Eve Implementation | ragent Implementation Path |
|-----|-------------------|---------------------------|
| **Web UI** | Cyberpunk terminal in browser | Create optional web UI crate (or document API for frontends) |
| **Workspace picker UI** | Button to change directory | Add /workspace command + TUI dialog for directory switching |
| **Hook system depth** | Shell commands + team hooks with stdin/stdout | Rich event hooks; add webhooks, hook chaining, and marketplace |
| **Persona/visual identity** | Avatar + Eve face | Optional ASCII art or theming system |
| **Voice/TTS** | Roadmap item | Design voice input/output trait for future implementation |

### 4.3 Low Priority / Differentiation

| Feature | Notes |
|---------|-------|
| Cyberpunk theme | Not aligned with ragent's terminal-first philosophy |
| Ollama-only focus | ragent intentionally multi-provider; this is a feature |
| Python plugin system | MCP is ragent's plugin story |

---

## 5. Implementation Plan: Achieving Parity

### Milestone 1: Expand Agent Ecosystem (Week 1-2)
**Goal:** Reach 50+ built-in agents

- [ ] **A1.1** Create agent templates for common domains:
  - `rust-coder` — Rust-specific coding agent
  - `python-coder` — Python-specific coding agent
  - `typescript-coder` — TS/JS-specific coding agent
  - `fastapi-agent` — FastAPI project specialist
  - `security-auditor` — Security review agent
  - `test-writer` — Test generation agent
  - `documenter` — Documentation agent
  - `devops-agent` — Docker/k8s specialist
  - `database-agent` — SQL/migration specialist
  - `frontend-agent` — React/Vue/CSS specialist

- [ ] **A1.2** Add agent discovery from registries:
  - Load agents from remote URL (GitHub raw)
  - Agent marketplace JSON index

- [ ] **A1.3** Create agent composition system:
  - Agents can delegate to other agents
  - Parent/child agent relationships

**Success Criteria:** 50+ agents available via `/agents` command

---

### Milestone 1: Expand Agent Ecosystem — A1.1 COMPLETE ✅
**Status:** 10 new domain-specific agents added to `create_builtin_agents()` in `crates/ragent-agent/src/agent/mod.rs`.

**Agents added:**
| Agent | Mode | Description | Permissions |
|-------|------|-------------|-------------|
| `rust-coder` | Primary | Rust coding specialist — idiomatic code, error handling, async | default |
| `python-coder` | Primary | Python coding specialist — type hints, testing, packaging | default |
| `typescript-coder` | Primary | TS/JS coding specialist — type safety, React, modern JS | default |
| `fastapi-agent` | Primary | FastAPI project specialist — API design, Pydantic, async | default |
| `security-auditor` | Primary | Security reviewer — OWASP Top 10, CWE, mitigations | read-only |
| `test-writer` | Primary | Test generation — unit, integration, e2e coverage | default |
| `documenter` | Primary | Documentation specialist — docstrings, READMEs, API docs | default |
| `devops-agent` | Primary | DevOps specialist — Docker, Kubernetes, CI/CD | default |
| `database-agent` | Primary | Database specialist — SQL, migrations, performance | default |
| `frontend-agent` | Primary | Frontend specialist — React, Vue, CSS, accessibility | default |

**Tests added:**
- `test_domain_agents_exist` — verifies all 18 built-in agents are present
- `test_domain_agents_are_primary` — verifies new agents have Primary mode
- `test_security_auditor_is_read_only` — verifies security-auditor has read-only permissions

**Progress: 18/50+ agents created (36%)**
**Next: A1.2 (agent discovery from registries) and A1.3 (agent composition)**

---

### Milestone 2: Slash Command Expansion (Week 2-3)
**Goal:** Reach 50+ slash commands

- [ ] **A2.1** Add task-oriented slash commands:
  - `/fix` — Diagnose and fix bugs
  - `/review` — Code review with prioritized feedback
  - `/refactor` — Refactor for clarity and performance
  - `/test` — Write or improve test coverage
  - `/docs` — Generate docstrings and documentation
  - `/plan` — Step-by-step implementation plan
  - `/explain` — Explain code or concepts
  - `/optimize` — Performance optimization
  - `/security` — Security audit
  - `/lint` — Run and fix lint errors

- [ ] **A2.2** Add workflow slash commands:
  - `/commit` — Generate commit message from diff
  - `/pr` — Generate PR description
  - `/release` — Generate release notes
  - `/changelog` — Update CHANGELOG.md
  - `/readme` — Update README.md

- [ ] **A2.3** Add utility slash commands:
  - `/workspace` — Change working directory
  - `/model` — Switch model with picker
  - `/provider` — Switch provider
  - `/history` — Show conversation history
  - `/export` — Export session to file

**Success Criteria:** 50+ slash commands with completions

---

### Milestone 3: Skills Library Expansion (Week 3-4)
**Goal:** Reach 100+ bundled skills

- [ ] **A3.1** Create domain skill packs:
  - `rust-testing` — Rust test patterns (cargo test, mockall)
  - `rust-async` — Async Rust patterns (tokio, futures)
  - `python-testing` — pytest patterns
  - `python-async` — asyncio patterns
  - `web-frontend` — React/Vue/Angular patterns
  - `web-backend` — FastAPI/Flask/Django patterns
  - `devops-docker` — Dockerfile best practices
  - `devops-k8s` — Kubernetes patterns
  - `database-sql` — SQL optimization
  - `database-nosql` — MongoDB/Redis patterns

- [ ] **A3.2** Create language skill packs:
  - `lang-rust` — Rust idioms and patterns
  - `lang-python` — Python idioms and patterns
  - `lang-typescript` — TypeScript patterns
  - `lang-go` — Go patterns
  - `lang-c` — C/C++ patterns

- [ ] **A3.3** Create framework skill packs:
  - `framework-axum` — Axum web framework
  - `framework-tokio` — Tokio runtime
  - `framework-sqlx` — SQLx database
  - `framework-serde` — Serde serialization

**Success Criteria:** 100+ skills in `~/.ragent/skills/` after first run

---

### Milestone 4: Vision & Multimodal Enhancement (Week 4-5)
**Goal:** Build on existing vision support (clipboard paste, Alt+V, base64 encoding, data-URI transport across 8 providers) with richer TUI tooling

**Already Implemented in ragent:**
- ✅ `MessagePart::Image` with MIME type and file path
- ✅ Clipboard paste via Alt+V with temp-file encoding (50 MB limit, 16K×16K max dimensions)
- ✅ Base64 encoding and data-URI transport in LLM layer
- ✅ `ContentPart::ImageUrl` for provider API payloads
- ✅ `vision: bool` model capability flag (used in model picker UI)
- ✅ Provider support: Anthropic, OpenAI, Gemini, Copilot, Azure Foundry, Azure Resource, HuggingFace, Ollama Cloud

**Remaining Work:**
- [ ] **A4.1** TUI image display:
  - ASCII art rendering for terminal preview
  - Image metadata overlay (dimensions, size, MIME type)
  - Sixel/kitty graphics protocol support (optional)

- [ ] **A4.2** Add vision-specific tools:
  - `analyze_image` — Describe image content using vision model
  - `compare_images` — Compare two images and highlight differences
  - `screenshot` — Capture screen region (if platform supported)

- [ ] **A4.3** Image ingestion from file paths:
  - `@image:path/to/file.png` mention syntax in chat input
  - Drag-and-drop file path detection (if terminal supports it)
  - Batch image upload (multiple images in one message)

- [ ] **A4.4** Vision model awareness:
  - Auto-detect vision capability when model is selected
  - Graceful fallback to text-only when model lacks vision
  - Vision token counting for context management

**Success Criteria:** Users can paste, preview, and analyze images end-to-end in the TUI

---

### Milestone 5: Context Management Enhancement (Week 5-6)
**Goal:** Automatic context compaction with summarization

- [ ] **A5.1** Implement context compaction strategy:
  - Token counting per message
  - Context window tracking per model
  - Threshold-based compaction trigger

- [ ] **A5.2** Add summarization:
  - Summarize older conversation turns
  - Preserve tool call results in summary
  - Configurable compaction depth

- [ ] **A5.3** Add context analytics:
  - `/context` command to show usage
  - Warnings when approaching limit
  - Suggest compaction when needed

**Success Criteria:** Automatic compaction works without user intervention

---

### Milestone 6: Web UI (Optional) (Week 6-8)
**Goal:** Browser-based alternative to TUI

- [ ] **A6.1** Create web UI crate:
  - `ragent-web` — New crate for web interface
  - Reuse existing SSE API from ragent-server
  - Static file serving

- [ ] **A6.2** Implement chat interface:
  - Message history display
  - Streaming message rendering
  - Tool call visualization
  - Permission dialog modal

- [ ] **A6.3** Implement tool panels:
  - File browser sidebar
  - Tool call log panel
  - Workspace picker
  - Model selector

- [ ] **A6.4** Theming:
  - Light/dark mode
  - Cyberpunk theme option
  - Custom CSS support

**Success Criteria:** Can run `ragent serve --web-ui` and use browser

---

### Milestone 7: Hook System Enhancement (Week 7-8)
**Goal:** Expand existing hook system with additional trigger points and richer capabilities

**Already Implemented in ragent:**
- ✅ `HookTrigger` enum with `OnSessionStart`, `OnSessionEnd`, `OnError`, `OnPermissionDenied`, `PreToolUse`, `PostToolUse`
- ✅ `HookConfig` with shell command, trigger, and timeout
- ✅ Pre-tool-use hooks that can return `{"decision": "allow"}`, `{"decision": "deny", "reason": "..."}`, or `{"modified_input": {...}}`
- ✅ Post-tool-use hooks that can return `{"modified_output": {...}}`
- ✅ Async execution via spawned tokio tasks with configurable timeout (default 30s)
- ✅ Environment variables passed to hooks: `RAGENT_TRIGGER`, `RAGENT_WORKING_DIR`, `RAGENT_ERROR`, `RAGENT_TOOL_NAME`, `RAGENT_TOOL_INPUT`, `RAGENT_TOOL_OUTPUT`, `RAGENT_TOOL_SUCCESS`
- ✅ `parse_hook_configs()` for loading from `ragent.json`
- ✅ Memory extraction hooks (`post_tool_use`, `session_end`)
- ✅ Team hooks: `TaskCreated`, `TaskCompleted`, `TeammateIdle` (with stdin JSON and `HookOutcome::Feedback`)

**Remaining Work:**
- [ ] **A7.1** Add hook trigger points:
  - `OnModelSelected` — When user switches model/provider
  - `OnAgentSwitched` — When agent preset changes
  - `OnPermissionGranted` — When user approves a permission
  - `OnPermissionDenied` — Already exists; add reason code
  - `OnFileModified` — When any file is written/edited/patched

- [ ] **A7.2** Richer hook payloads:
  - JSON schema for each trigger type documented in SPEC.md
  - Structured stdin for all hooks (not just team hooks)
  - Hook chaining (output of one hook becomes input to next)

- [ ] **A7.3** Webhook-style remote hooks:
  - HTTP POST hooks (not just shell commands)
  - Configurable endpoint URL, headers, retry policy
  - Async webhook delivery with backoff

- [ ] **A7.4** Hook marketplace/examples:
  - Desktop notification hook (`notify-send` / `osascript` / `toast`)
  - Audit logging hook (append to structured log file)
  - Metrics export hook (prometheus-compatible)
  - Slack/Discord notification hook

**Success Criteria:** Hooks cover full agent lifecycle; remote webhooks supported; example gallery available

---

### Milestone 8: Polish & Integration (Week 8-10)
**Goal:** Smooth integration of all new features

- [ ] **A8.1** Integration testing:
  - End-to-end tests for new agents
  - Slash command regression tests
  - Skill loading tests

- [ ] **A8.2** Documentation:
  - Update SPEC.md with new features
  - Update QUICKSTART.md
  - Create agent authoring guide
  - Create skill authoring guide

- [ ] **A8.3** Performance optimization:
  - Lazy-load agents (don't load all 50+ at startup)
  - Lazy-load skills (only on keyword match)
  - Cache compiled agent definitions

- [ ] **A8.4** Release preparation:
  - Bump version
  - Update CHANGELOG.md
  - Create release notes

**Success Criteria:** All tests pass, docs updated, version bumped

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Scope creep** | High | Medium | Strict milestone boundaries; defer non-essential features |
| **Performance regression** | Medium | High | Lazy loading; benchmark before/after |
| **TUI complexity** | Medium | Medium | Keep web UI optional; don't bloat TUI |
| **Skill quality** | Medium | Medium | Community contribution process; curated bundles |
| **Vision API fragmentation** | Low | Low | Already abstracted via `ContentPart::ImageUrl`; minor provider format differences handled |

---

## 7. Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Built-in agents | 8 | 50+ |
| Slash commands | ~15 | 50+ |
| Bundled skills | ~20 | 100+ |
| Tool categories | 15 | 16 (add vision tools: analyze_image, compare_images, screenshot) |
| Lines of code | ~57,500 | <65,000 (controlled growth) |
| Startup time | ~2s | <3s (with lazy loading) |
| Test coverage | TBD | Maintain or improve |

---

## 8. Conclusion

**ragent** is architecturally superior in terms of tool diversity, security depth, deployment simplicity, and multi-provider flexibility. **Eve Agent V2 Unleashed** excels in pre-built content (agents, commands, skills), visual personality, and local-first simplicity.

The recommended strategy is **not** to copy Eve's architecture (Python/monolithic) but to **absorb its content strengths** (agents, commands, skills) into ragent's superior architecture. This creates a best-of-both-worlds product: Rust performance + security with Eve's rich ecosystem of pre-built content.

The 8-milestone plan above, executed over ~10 weeks, would bring ragent to parity with Eve's content ecosystem while maintaining its architectural advantages.

---

*Document generated for comparative analysis. See SPEC.md for ragent's current feature specification.*
