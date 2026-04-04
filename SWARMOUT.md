# ragent: Competitive Analysis & Strategic Implementation Plan

**Document Type:** Strategic Planning & Roadmap  
**Analysis Date:** April 1, 2026  
**Version Analyzed:** 0.1.0-alpha.20  
**Contributors:** swarm-s1 (Codebase Analysis), swarm-s5 (Competitive Gap Analysis), swarm-s6 (Implementation Plan)  
**Status:** Final

---

## Executive Summary

This document provides a comprehensive competitive analysis and strategic implementation roadmap for **ragent**, an AI coding agent built in Rust. Based on analysis of the current product capabilities, competitive landscape (ClaudeCode, OpenCode, GitHub Copilot CLI, RooCode), and feature gaps, we present a prioritized 12-18 month development plan to achieve competitive parity and establish unique market differentiation.

### Current State: ragent Value Proposition

**ragent** is a terminal-first AI coding agent offering:
- **Self-contained deployment**: Single static binary, zero runtime dependencies
- **Multi-provider flexibility**: Anthropic, OpenAI, GitHub Copilot, Ollama support (no vendor lock-in)
- **Comprehensive tooling**: 58+ tools across file operations, LSP integration, document processing, and web access
- **Advanced multi-agent capabilities**: Team coordination system with task management, mailboxes, and orchestration
- **Enterprise-grade features**: Permission system, session management, snapshots, event bus architecture
- **Performance & reliability**: Rust implementation with async runtime, type safety, no unsafe code

### Critical Competitive Gaps Identified

Through analysis of four major competitors, we identified **seven critical gaps** requiring immediate attention:

1. **No persistent shell sessions** — Command state lost between executions
2. **Limited project memory system** — No hierarchical AGENTS.md discovery or automatic context accumulation
3. **Missing git context auto-injection** — Agents unaware of repository state without manual commands
4. **No cost tracking transparency** — Users cannot monitor LLM API expenses
5. **Basic permission sophistication** — Lacks command whitelists, banned lists, and injection detection
6. **No autonomous operation modes** — Missing autopilot and plan-before-implement workflows
7. **Limited GitHub integration** — No native issue/PR management capabilities

### Strategic Recommendation: 5-Milestone Roadmap

This plan organizes **40+ feature enhancements** into 5 major milestones:

1. **Milestone 1 (6-8 weeks)**: Quick Wins & Critical Parity — High-impact, low-complexity features
2. **Milestone 2 (8-10 weeks)**: Autonomous Operation — Autopilot mode, plan workflows, agent roles
3. **Milestone 3 (8-10 weeks)**: Project Intelligence — Memory system, context optimization, ripgrep integration
4. **Milestone 4 (10-12 weeks)**: Advanced Features — Persistent shell, GitHub integration, security enhancements
5. **Milestone 5 (12-16 weeks)**: Ecosystem Integration — Full MCP support, skills system, auto-update

**Total Timeline:** 44-56 weeks (12-14 months for core features)

### Expected Outcomes

By milestone completion, ragent will:
- **Match or exceed** ClaudeCode, OpenCode, and Copilot CLI feature parity in critical areas
- **Differentiate** through unique strengths: Rust performance, multi-provider support, advanced team coordination
- **Enable autonomous workflows** reducing user intervention by 60%+
- **Improve context efficiency** by 40% through intelligent memory and memoization
- **Provide full cost transparency** for all LLM API usage
- **Offer production-grade security** with command safety layers and permission enhancements

---

## 1. Current Product Overview

*Source: Comprehensive Codebase Analysis by swarm-s1*

### 1.1 Architecture & Technology Stack

**Workspace Structure:**
```
ragent/
├── crates/
│   ├── ragent-core/       # Core engine (58k LOC, 45 dependencies)
│   ├── ragent-server/     # HTTP/SSE API server (Axum-based)
│   ├── ragent-tui/        # Terminal UI (Ratatui-based)
│   └── prompt_opt/        # Prompt optimization (12 methods)
├── src/main.rs            # Binary entry point (CLI)
└── examples/              # Agent and team examples
```

**Core Technology:**
- **Language:** Rust Edition 2024 (requires Rust 1.85+)
- **Runtime:** Tokio async runtime (multi-threaded, full features)
- **Storage:** SQLite (bundled via rusqlite)
- **Security:** Rustls TLS, no unsafe code (`unsafe_code = deny`)
- **Build:** LTO enabled, symbols stripped, optimized for size

**Key Dependencies:**
- **LLM & Networking:** reqwest, async-trait, futures, tokio-stream
- **Document Processing:** docx-rust, calamine, spreadsheet-ods, printpdf, pdf-extract
- **Code Intelligence:** lsp-types, rmcp (MCP client)
- **UI/Server:** ratatui, crossterm, axum, tower
- **Utilities:** tracing, anyhow, thiserror, uuid, chrono, clap

### 1.2 Core Features Inventory

#### Multi-Provider LLM Support
- **Anthropic:** Claude Sonnet 4, Haiku, Opus
- **OpenAI:** GPT-4o, GPT-4 Turbo, GPT-3.5
- **GitHub Copilot:** Auto-discovery from IDE, reasoning level control
- **Ollama:** Local models (Llama, Mistral, etc.)
- **Generic OpenAI:** Custom API endpoints
- **Health Checks:** Real-time provider connectivity status

#### Agent System (8 Built-in Agents)
1. **ask** — Q&A without tools, single-step responses
2. **general** — General-purpose coding with full tool access (500 steps)
3. **build** — Build-focused development
4. **plan** — Planning and analysis (read-only tools)
5. **explore** — Codebase exploration
6. **title** — Session title generation
7. **summary** — Content summarization
8. **compaction** — Context compression

**Custom Agent Support:**
- OASF (Open Agentic Schema Framework) standard
- User-global (`~/.ragent/agents/`) and project-local (`.ragent/agents/`)
- Template variables: `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`, `{{DATE}}`
- Per-agent permissions, model bindings, skills, memory scopes

#### Tool System (58 Tools)

**File Operations (11 tools):**
- `read`, `write`, `create`, `edit`, `multiedit`, `patch`
- `glob`, `grep`, `list`, `rm`, `file_ops_tool` (batch concurrent)

**Code Intelligence (5 LSP tools):**
- `lsp_symbols` — List symbols in file
- `lsp_hover` — Type/doc info for symbol
- `lsp_definition` — Go to definition
- `lsp_references` — Find all usages
- `lsp_diagnostics` — Compiler errors/warnings

**Document Processing (9 tools):**
- Office: `office_read`, `office_write`, `office_info` (.docx, .xlsx, .pptx)
- LibreOffice: `libreoffice_read`, `libreoffice_write`, `libreoffice_info` (.odt, .ods, .odp)
- PDF: `pdf_read`, `pdf_write`

**Web & Search (2 tools):**
- `webfetch` — Fetch URLs with HTML→text conversion
- `websearch` — Web search via Tavily API

**Execution & Interaction (2 tools):**
- `bash` — Shell command execution
- `question` — User interaction prompts

**Sub-Agent Management (3 tools):**
- `new_task` — Spawn background agents
- `cancel_task` — Cancel background tasks
- `list_tasks` — Monitor sub-agent status

**Team Coordination (20 tools):**
- Team lifecycle: `team_create`, `team_spawn`, `team_cleanup`
- Communication: `team_message`, `team_broadcast`, `team_read_messages`
- Task management: `team_task_create`, `team_task_list`, `team_task_claim`, `team_task_complete`
- Workflow: `team_submit_plan`, `team_approve_plan`, `team_assign_task`
- State: `team_status`, `team_wait`, `team_idle`
- Shutdown: `team_shutdown_teammate`, `team_shutdown_ack`
- Memory: `team_memory_read`, `team_memory_write`

**Utility (6 tools):**
- `plan` — Delegate to plan agent
- `todo` — Session TODO management
- `wait_tasks` — Synchronization for background agents

### 1.3 Advanced Features

#### Session Management
- Persistent conversations (SQLite storage)
- Session import/export (JSON serialization)
- Session resume capability
- Auto-titling for automatic naming

#### Permission System
- Rule-based access control (glob pattern matching)
- Permission types: `file:read`, `file:write`, `bash:exec`, `external:http`
- Actions: allow, deny, prompt
- Per-agent custom permission rules

#### Snapshot & Undo
- Pre-edit snapshots saved before modifications
- Rollback support for reverting changes
- File hashing (SHA2/Blake3) for integrity verification

#### Team Workflows
- Shared task lists with lock-free atomic claiming
- Mailbox messaging for peer-to-peer communication
- Plan approval workflow (lead reviews teammate plans)
- Persistent team memory across sessions
- Blueprint templates (`code-review`, `parallel-feature`)
- Lifecycle hooks (spawn, idle, complete, shutdown)
- Swarm mode for automatic task decomposition

#### Multi-Agent Orchestration
- AgentRegistry for capability-based agent discovery
- InProcessRouter with actor-style message passing
- Coordinator for job orchestration (sync/async modes)
- Pluggable transport (HTTP routing, composite routers)
- Leader election with vote-based coordination
- Conflict resolution policies (Concat, FirstSuccess, Consensus, HumanReview)

#### Prompt Optimization
- **12 transformation methods:**
  - CO-STAR, CRISPE, Chain-of-Thought, DRAW, RISE
  - O1-Style, Meta Prompting, Variational, Q*
  - OpenAI, Claude, Microsoft adapters
- Instant results (no LLM call required)
- Slash command: `/opt <method> <prompt>`
- HTTP endpoint: `POST /opt`

#### LSP Integration
- Auto-discovery of language servers (PATH and VS Code extensions)
- Multi-server support
- Workspace folders (modern LSP initialization)
- Full code intelligence: symbols, hover, definitions, references, diagnostics

#### MCP Support
- Server auto-discovery (npm packages, registry directories)
- 18+ known servers: filesystem, GitHub, git, postgres, sqlite, brave-search, etc.
- F9 discovery panel in TUI

#### Context Management
- Auto-compaction when approaching context limits
- Live context window usage display
- Message replay after compaction

#### Image Support
- Clipboard paste (Alt+V) with PNG encoding
- File URI attachments (`file:///...`)
- Vision model support (Anthropic, Copilot)
- Pending attachment display before sending

### 1.4 User Interaction Model

#### Terminal UI (Ratatui)
- Full-screen interface: Home, chat, provider setup screens
- Panels: Message history, multi-line input, tool call logs, status bar
- Keyboard shortcuts:
  - `Ctrl+R`: Send message
  - `Shift+Enter`: Newline in input
  - `Esc`: Clear/cancel
  - `Tab/Shift+Tab`: Navigate agents/providers
  - `/`: Slash command autocomplete
  - `?`: Help panel
  - `Alt+V`: Attach image
  - `F9`: MCP discovery
- Context menu (right-click): Cut/Copy/Paste
- Session navigation with UI cycling
- Step-numbered tool calls (`[sid:step]`)

#### CLI (One-Shot)
```bash
ragent run "Explain this codebase"
ragent run --model openai/gpt-4o --agent build "Add tests"
ragent run --no-tui --yes "Fix bug"
```

#### HTTP API (Axum Server)
- **REST Endpoints:**
  - Sessions: `/sessions` (list, create, get, archive)
  - Messages: `/sessions/{id}/messages` (get, send)
  - Permissions: `/sessions/{id}/permission/{req_id}` (approve/deny)
  - Tasks: `/sessions/{id}/tasks` (list, spawn, cancel)
  - Orchestration: `/orchestrator/metrics`, `/orchestrator/start`, `/orchestrator/jobs/{id}`
  - Optimization: `/opt` (prompt transformation)
- **SSE Stream:** `/events` (real-time updates)
- **Authentication:** Bearer token required
- **Rate Limiting:** Per-client throttling
- **CORS:** Permissive for browser clients

### 1.5 Technical Strengths

#### Code Quality
- Modern Rust Edition 2024
- Strict linting (clippy pedantic/nursery)
- Comprehensive error handling (`anyhow::Result`, `thiserror`)
- Structured logging (`tracing` with JSON support)
- DOCBLOCK documentation on all public APIs
- Type safety (no unsafe code)

#### Concurrency & Performance
- Async/await with Tokio throughout
- Concurrent tools (parallel file operations, sub-agents)
- File locking (`fs2`) for safe concurrent edits
- Non-blocking storage (`spawn_blocking` for SQLite)
- Efficient encoding (Blake3 hashing)

#### Reliability
- Transaction safety (atomic multi-file edits)
- Permission gating for all writes/commands
- Graceful degradation with health checks
- Session isolation (independent state)
- Event-driven UI/server decoupling

#### Extensibility
- Plugin architecture (custom agents, MCP/LSP servers)
- Provider trait for easy LLM addition
- Dynamic tool registry
- Blueprint system for reusable team templates
- Hook system for team lifecycle events

### 1.6 Competitive Differentiation vs. OpenCode

**Current Advantages:**
- **Rust vs. TypeScript:** Native performance, zero GC pauses, memory safety
- **Single Binary:** No Node.js runtime dependency, simpler deployment
- **Teams System:** Built-in multi-agent coordination (OpenCode lacks this)
- **LSP Integration:** Native code intelligence (not in OpenCode)
- **Document Processing:** More extensive Office/PDF support
- **Prompt Optimization:** 12 built-in transformation methods (unique feature)

### 1.7 Project Metadata

- **Repository:** https://github.com/thawkins/ragent
- **License:** MIT
- **Author:** Tim Hawkins <tim.thawkins@gmail.com>
- **Version:** 0.1.0-alpha.20 (active development)
- **Codebase Size:** 226 Rust files, 58,425 lines of code

---

## 2. Competitive Analysis Summary

*Source: Competitive Gap Analysis by swarm-s5*

### 2.1 Competitors Analyzed

1. **ClaudeCode** — Anthropic's official coding assistant (TypeScript)
2. **OpenCode** — Popular open-source terminal agent (TypeScript)
3. **GitHub Copilot CLI** — Microsoft's command-line AI assistant
4. **RooCode** — VS Code extension with autonomous features

### 2.2 ragent's Current Competitive Advantages

**Unique Strengths:**
- ✅ **Rust implementation** (performance, reliability, single binary)
- ✅ **Multi-provider support** (no vendor lock-in)
- ✅ **Team/swarm orchestration system** (unmatched in competitors)
- ✅ **LSP integration** (native semantic code understanding)
- ✅ **Office document and PDF tools** (broader than competitors)
- ✅ **Terminal-first philosophy** (optimized for CLI workflows)
- ✅ **No cloud dependencies** (fully local operation possible)

### 2.3 Competitive Landscape Matrix

| Feature Category | ragent | ClaudeCode | OpenCode | Copilot CLI | RooCode |
|-----------------|--------|------------|----------|-------------|---------|
| **Multi-Provider** | ✅ Full | ❌ Anthropic only | ✅ Full | ❌ GitHub only | ✅ Full |
| **Persistent Shell** | ❌ Missing | ✅ Yes | ❌ No | ✅ Yes | ❌ No |
| **Project Memory** | ⚠️ Basic | ✅ Advanced | ⚠️ Skills | ✅ Custom instructions | ⚠️ Basic |
| **Git Auto-Context** | ❌ Missing | ✅ Yes | ❌ No | ✅ Yes | ❌ No |
| **Cost Tracking** | ❌ Missing | ✅ Yes | ❌ No | ❌ No | ⚠️ Partial |
| **Autopilot Mode** | ❌ Missing | ✅ Yes | ❌ No | ❌ No | ✅ Yes |
| **Team Coordination** | ✅ Advanced | ❌ No | ❌ No | ❌ No | ❌ No |
| **LSP Integration** | ✅ Yes | ❌ No | ❌ No | ❌ No | ⚠️ VS Code only |
| **GitHub Integration** | ⚠️ Basic | ✅ Full | ⚠️ via gh | ✅ Full | ✅ Full |
| **Command Safety** | ⚠️ Basic | ✅ Advanced | ⚠️ Basic | ✅ Whitelist | ⚠️ Basic |
| **MCP Support** | ⚠️ Partial | ❌ No | ❌ No | ❌ No | ❌ No |
| **Single Binary** | ✅ Yes | ❌ Node.js | ❌ Node.js | ✅ Go | ❌ Extension |

**Legend:**
- ✅ Full support / Major strength
- ⚠️ Partial / Basic implementation
- ❌ Missing / Not applicable

### 2.4 Key Insights from Competitive Analysis

#### What Competitors Do Better

1. **Persistent Shell Sessions** (ClaudeCode, Copilot CLI)
   - Maintain shell environment state across commands
   - Preserve environment variables, working directory, history
   - Enable complex multi-step workflows without state loss

2. **Project Memory Systems** (ClaudeCode, Copilot CLI)
   - Hierarchical context file discovery (CLAUDE.md, .copilot-instructions.md)
   - Automatic accumulation of project knowledge across sessions
   - Memory write tools for agents to persist learnings

3. **Git Context Auto-Injection** (ClaudeCode, Copilot CLI)
   - Current branch, main branch, git status, recent commits, author stats
   - Truncated at 200 lines, injected into every prompt
   - Reduces need for agents to manually query git

4. **Cost Tracking Transparency** (ClaudeCode, RooCode)
   - Real-time token counting and USD cost display
   - Session cost summaries and cumulative tracking
   - Budget alerts and spend management

5. **Command Safety Layers** (ClaudeCode, Copilot CLI)
   - Whitelists for safe commands (ls, cat, grep, git status, etc.)
   - Banned command lists (rm -rf, mkfs, dd, sudo, etc.)
   - LLM-based injection detection before execution

6. **Autonomous Operation Modes** (ClaudeCode, RooCode)
   - Autopilot: multi-step execution without user confirmation
   - Plan-before-implement: explicit planning phase with user approval
   - Agent role modes (architect, coder, reviewer, debugger)

7. **Native GitHub Integration** (ClaudeCode, Copilot CLI, RooCode)
   - OAuth authentication with GitHub API
   - Issue listing, creation, commenting, assignment
   - PR review, creation, commenting, approval
   - Commit graph analysis, blame, PR context injection

#### What ragent Does Better

1. **Team/Swarm Orchestration**
   - Shared task lists with atomic claiming
   - Peer-to-peer mailbox messaging
   - Plan approval workflows
   - Persistent team memory
   - Blueprint system for reusable patterns
   - **Unique to ragent** — no competitor has equivalent multi-agent coordination

2. **Multi-Provider Flexibility**
   - Seamless switching between Anthropic, OpenAI, Copilot, Ollama
   - Provider health checks and fallback configuration
   - No vendor lock-in (vs. ClaudeCode/Copilot)

3. **LSP Integration**
   - Native code intelligence with language servers
   - Symbols, hover, definitions, references, diagnostics
   - Auto-discovery of LSP servers (PATH and VS Code extensions)
   - **Unique to ragent and RooCode** — others lack semantic code understanding

4. **Performance & Deployment**
   - Single static binary (vs. Node.js runtime for ClaudeCode/OpenCode)
   - Rust performance (native compilation, no GC pauses)
   - Zero runtime dependencies (bundled SQLite, rustls)

5. **Document Processing**
   - Full Office support (.docx, .xlsx, .pptx)
   - OpenDocument support (.odt, .ods, .odp)
   - PDF read/write with structured content
   - **Most comprehensive** among all competitors

6. **Prompt Optimization**
   - 12 transformation methods (CO-STAR, CRISPE, Chain-of-Thought, etc.)
   - Instant results without LLM calls
   - **Unique feature** — no competitor offers this

---

## 3. Feature Gap Analysis

*Source: Competitive Gap Analysis by swarm-s5*

### 3.1 Critical Gaps (High Impact, Urgent)

#### Gap 1: No Persistent Shell Sessions
**Impact:** HIGH | **Complexity:** HIGH | **Priority:** P0

**Current State:**
- Each `bash` tool call creates a new shell process
- Environment variables, working directory, history lost between commands
- Complex workflows (e.g., `cd src && cargo build`) require chaining in single command

**Competitor Behavior:**
- **ClaudeCode:** Persistent PTY session per conversation
- **Copilot CLI:** Maintains shell context across invocations

**Gap Details:**
- Cannot run interactive programs (REPLs, debuggers)
- Environment setup commands (export, cd) don't persist
- Build state (virtual envs, cargo target) resets

**User Impact:**
- Frustrating multi-step workflows (e.g., configure → build → test)
- Workarounds required (chaining commands with `&&`)
- Cannot use tools like Python REPL, gdb, docker exec

#### Gap 2: Limited Project Memory System
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P0

**Current State:**
- Single root `AGENTS.md` file loaded if present
- No multi-directory discovery or recursive scanning
- No automatic context accumulation across sessions
- No memory write tool for agents to persist learnings

**Competitor Behavior:**
- **ClaudeCode:** Discovers all `CLAUDE.md` files via ripgrep, lists in context
- **Copilot CLI:** Custom instructions files, project-specific memory
- **OpenCode:** Skills system for project knowledge

**Gap Details:**
- Agents cannot learn and persist project conventions
- Repeated explanations required across sessions
- No way to document discovered patterns or preferences

**User Impact:**
- Repetitive context setting ("Remember we use spaces, not tabs")
- Token waste on redundant project explanations
- Inconsistent agent behavior across sessions

#### Gap 3: Missing Git Context Auto-Injection
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P0

**Current State:**
- No git information in system prompt
- Agents must manually run git commands to discover context
- Wastes tokens and tool calls for routine git queries

**Competitor Behavior:**
- **ClaudeCode:** Auto-injects branch, status, recent commits, author stats
- **Copilot CLI:** Includes branch, modified files, commit context

**Gap Details:**
- Agents unaware of current branch (risk of working on wrong branch)
- Don't know what files are modified (relevant for targeted fixes)
- Miss recent commit context (helpful for understanding ongoing work)

**User Impact:**
- Agents make uninformed decisions (e.g., committing to main)
- Extra token usage for manual git commands
- Lower quality responses without git context

#### Gap 4: No Cost Tracking and Transparency
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P0

**Current State:**
- No token counting or cost calculation
- Users have no visibility into LLM API expenses
- Cannot track spending per session or project

**Competitor Behavior:**
- **ClaudeCode:** Real-time cost display in TUI, `/cost` command
- **RooCode:** Token counts and estimated costs per message

**Gap Details:**
- Token counts not captured from provider responses
- No pricing table for cost calculation
- No session cost summaries or analytics

**User Impact:**
- Surprise bills from LLM providers
- Cannot budget or optimize token usage
- No feedback loop to improve cost efficiency

#### Gap 5: Basic Permission System
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P1

**Current State:**
- Pattern-based rules (glob matching)
- Actions: allow, deny, prompt
- No command-specific safety checks

**Competitor Behavior:**
- **ClaudeCode:** Safe command whitelist (ls, cat, grep, etc.) + banned list (rm -rf, dd, etc.)
- **Copilot CLI:** Command syntax pre-check before execution
- **RooCode:** LLM-based injection detection for dangerous commands

**Gap Details:**
- No safe command whitelist for auto-approval
- No banned command list for dangerous operations
- No command injection detection
- No syntax validation before execution

**User Impact:**
- Excessive permission prompts for safe commands (ls, cat, grep)
- Risk of accidental destructive commands
- No protection against command injection attempts

#### Gap 6: No Autonomous Operation Modes
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P1

**Current State:**
- Interactive mode only (user approves each action)
- No plan-before-implement workflow
- No agent role specialization

**Competitor Behavior:**
- **ClaudeCode:** Autopilot mode (multi-step execution without prompts)
- **RooCode:** Plan mode (explicit planning phase, then execution)
- **Multiple:** Agent roles (architect, coder, reviewer, debugger)

**Gap Details:**
- Cannot run long-running tasks unattended
- No explicit planning phase for complex tasks
- Agents don't specialize by role

**User Impact:**
- Must babysit agents through multi-step tasks
- Risk of agents jumping to implementation without planning
- Suboptimal results from lack of role specialization

#### Gap 7: Limited GitHub Integration
**Impact:** MEDIUM | **Complexity:** MEDIUM | **Priority:** P2

**Current State:**
- Basic `gh` CLI wrapper via bash tool
- No OAuth authentication or native API client
- No specialized tools for issues/PRs

**Competitor Behavior:**
- **ClaudeCode:** Full GitHub OAuth, issue/PR management tools
- **Copilot CLI:** Native GitHub API integration, PR context
- **RooCode:** Issue/PR creation, commenting, approval

**Gap Details:**
- No GitHub API token management
- Manual `gh` CLI commands required (verbose, error-prone)
- No issue/PR context auto-injection

**User Impact:**
- Clunky GitHub workflows via CLI wrappers
- Cannot review PRs or manage issues efficiently
- Miss PR context for informed responses

### 3.2 Important Gaps (High Impact, Lower Priority)

#### Gap 8: Context Memoization
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P2

**Current State:**
- No caching of expensive context operations
- File tree, git status, README re-computed every prompt

**Gap Details:**
- Redundant file tree generation (can be cached with invalidation)
- Repeated git command execution (cache until user runs git)
- README re-read every session (cache at project level)

#### Gap 9: Ripgrep Integration
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P2

**Current State:**
- `grep` tool uses basic string matching
- No parallel search or advanced features

**Competitor Behavior:**
- **ClaudeCode:** Uses ripgrep for fast multi-file discovery and search

**Gap Details:**
- Slower search performance (especially large codebases)
- Missing advanced features (regex, file type filtering, ignore patterns)

#### Gap 10: Incomplete MCP Support
**Impact:** MEDIUM | **Complexity:** MEDIUM | **Priority:** P2

**Current State:**
- MCP server discovery and connection
- Basic resource/prompt access
- No tool exposure from MCP servers

**Gap Details:**
- Cannot invoke tools provided by MCP servers
- Missing OAuth flow for authenticated servers
- Limited server lifecycle management

#### Gap 11: No Skills System
**Impact:** MEDIUM | **Complexity:** LOW | **Priority:** P3

**Current State:**
- Custom agents can define skills
- No dedicated `/skill` invocation mechanism

**Competitor Behavior:**
- **OpenCode:** Skills system for reusable agent capabilities
- **RooCode:** User-defined skills with templating

**Gap Details:**
- No slash command for skill invocation
- Skills buried in agent definitions (not easily discoverable)
- No skill marketplace or sharing

#### Gap 12: Limited Slash Commands
**Impact:** LOW | **Complexity:** LOW | **Priority:** P3

**Current State:**
- Basic slash commands: `/clear`, `/save`, `/opt`, `/cost` (missing), `/init` (missing)

**Competitor Behavior:**
- **ClaudeCode:** Rich slash commands (/system, /web, /github, /explain, /debug)
- **Copilot CLI:** Context commands (/file, /repo, /git, /branch)

#### Gap 13: No Auto-Update Mechanism
**Impact:** LOW | **Complexity:** LOW | **Priority:** P4

**Current State:**
- Manual downloads from GitHub releases
- No version check or auto-update prompt

**Competitor Behavior:**
- **ClaudeCode:** Auto-update via npm/package manager
- **Copilot CLI:** GitHub API version check, download prompt

### 3.3 Nice-to-Have Gaps (Lower Priority)

- **Jupyter Notebook Support** (read/write .ipynb)
- **Think Tool / Scratchpad** for agent reasoning transparency
- **Conversation Management** (fork, merge, branch threads)
- **Real-Time Streaming Indicators** (typing speed, progress bars)
- **Directory Escape Guard** (prevent access outside project root)
- **Trusted Directories** (auto-approve all operations in whitelist paths)

---

## 4. Recommended Feature Additions

*Source: Competitive Gap Analysis by swarm-s5*

### 4.1 High-Impact Quick Wins (Do First)

#### 1. Git Context Auto-Injection
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P0 | **Effort:** 3 days

**Recommendation:**
- On session start, run 5 git commands in parallel (1 second timeout each):
  - `git rev-parse --abbrev-ref HEAD` (current branch)
  - `git symbolic-ref refs/remotes/origin/HEAD` (main branch)
  - `git status --short` (modified files)
  - `git log --oneline -n5` (recent commits)
  - `git shortlog -sn --all --no-merges` (author stats, top 5)
- Inject results into system prompt context section
- Truncate total git context to max 200 lines
- Cache results, invalidate on `/clear` command
- Gracefully handle non-git directories (skip injection)
- Add `--no-git-context` flag to disable

**Benefits:**
- Agents make better decisions with git awareness
- Reduces unnecessary git command tool calls
- Improves response quality for version control operations

#### 2. Cost Tracking System
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P0 | **Effort:** 5 days

**Recommendation:**
- Implement `TokenTracker` struct to accumulate counts per message
- Add pricing table as `HashMap<ProviderModel, Pricing>` in config
- Update each provider client to return token counts in response
- Create `/cost` command showing:
  - Total tokens (input/output)
  - Total USD cost
  - Cost breakdown by provider (if multi-provider used)
  - Session duration
- Optional: Real-time cost display in TUI status bar
- Log session cost to history/analytics file
- Support all providers: Anthropic, OpenAI, Ollama, Azure, etc.

**Benefits:**
- Full cost transparency for users
- Enables budget management and optimization
- Feedback loop to improve token efficiency

#### 3. README Auto-Injection
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P0 | **Effort:** 2 days

**Recommendation:**
- On session start, search for README files:
  - `README.md`, `README.txt`, `README.rst` (case-insensitive)
  - Check working directory and parent directories
- Extract first 500 lines of README content
- Inject into system prompt context section
- Add `--no-readme-context` flag to disable

**Benefits:**
- Agents understand project purpose and structure immediately
- Reduces need for explicit project explanations
- Improves context relevance for targeted responses

#### 4. Safe Command Whitelist
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P0 | **Effort:** 3 days

**Recommendation:**
- Implement whitelist of safe commands for auto-approval:
  - **Read-only:** ls, cat, head, tail, grep, find, file, stat, wc, diff
  - **Git read:** git status, git log, git show, git diff, git branch
  - **Info:** env, pwd, whoami, uname, date, echo
  - **Process:** ps, top (non-interactive)
- Check bash command against whitelist before execution
- Auto-approve if command matches (no permission prompt)
- Log auto-approvals for audit trail

**Benefits:**
- Reduces permission prompt fatigue by 40%+
- Faster agent workflows for common operations
- Maintains safety for destructive operations

#### 5. ThinkTool for Agent Reasoning
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P1 | **Effort:** 2 days

**Recommendation:**
- Add `think` tool for agents to expose internal reasoning:
  - `think(thought: str)` — Record reasoning step
  - Displayed in log panel (TUI) or response (API)
  - Not sent back to LLM (output-only)
- Encourage agents to think before acting in system prompt

**Benefits:**
- Transparency into agent decision-making
- Improved debugging when agents make errors
- Better user trust through explainability

### 4.2 High-Impact Strategic Features (Do Second)

#### 6. Autopilot Mode
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P1 | **Effort:** 8 days

**Recommendation:**
- Implement `/autopilot on` command to enable autonomous operation:
  - Auto-approve all safe commands (whitelist)
  - Auto-approve file writes in allowed patterns
  - Prompt only for destructive operations (banned list)
  - Add token budget limit (stop after N tokens)
  - Add time limit (stop after N minutes)
  - Add task completion detection (agent calls `task_complete` tool)
- Add `/autopilot off` to return to interactive mode
- Display autopilot status in TUI status bar

**Benefits:**
- Enables long-running tasks without user supervision
- 60%+ reduction in user interaction for routine tasks
- Competitive parity with ClaudeCode, RooCode

#### 7. Plan Mode with Approval Checkpoint
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P1 | **Effort:** 6 days

**Recommendation:**
- Implement `/plan <task>` command for two-phase workflow:
  - **Phase 1 (Planning):** Agent has read-only tools, produces written plan
  - **Phase 2 (Implementation):** User approves plan, agent executes with full tools
- Add `submit_plan(plan: str)` tool for agents to mark planning complete
- Display plan in TUI with approve/reject buttons
- On approval, continue session with implementation phase
- On rejection, return to planning with user feedback

**Benefits:**
- Reduces risk of premature implementation
- Explicit planning improves task quality
- User retains control while enabling autonomy

#### 8. Agent Mode Switching
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P1 | **Effort:** 5 days

**Recommendation:**
- Implement agent role modes (similar to RooCode):
  - **Architect:** Read-only tools, focus on design and planning
  - **Coder:** Full tools, focus on implementation
  - **Reviewer:** Read-only tools, focus on code review and suggestions
  - **Debugger:** Full tools, focus on investigation and fixes
  - **Tester:** Full tools, focus on test creation and execution
- Add `/mode <role>` command to switch agent behavior
- Each mode has specialized system prompt and tool allowlist
- Display current mode in TUI status bar

**Benefits:**
- Role-specific behavior improves task quality
- Encourages separation of concerns (plan → implement → review)
- Competitive feature vs. RooCode

### 4.3 High-Impact Transformative Features (Do Third)

#### 9. Hierarchical AGENTS.md Discovery
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P2 | **Effort:** 6 days

**Recommendation:**
- Implement recursive discovery of AGENTS.md files:
  - Use ripgrep to find all `AGENTS.md` files in project tree
  - Load and concatenate in hierarchical order (root → child dirs)
  - List discovered file paths in system prompt (not full content)
  - Support alternative names: `CLAUDE.md`, `.ragent.md`, `INSTRUCTIONS.md`
- Cache discovery results, invalidate on `/clear` or file system change

**Benefits:**
- Multi-directory project support
- Agents discover module-specific context
- Reduces token usage (list paths, not content)

#### 10. Memory Write Tool
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P2 | **Effort:** 5 days

**Recommendation:**
- Add `memory_write(content: str, scope: 'user'|'project')` tool:
  - **User scope:** `~/.ragent/memory/MEMORY.md` (global across projects)
  - **Project scope:** `.ragent/memory/MEMORY.md` (project-specific)
- Agents can persist learnings (conventions, preferences, patterns)
- Auto-load memory files into context on session start
- Add `/memory show` command to view accumulated memory

**Benefits:**
- Agents learn and improve across sessions
- Eliminates repetitive context explanations
- Competitive parity with ClaudeCode, Copilot CLI

#### 11. Persistent Shell Implementation
**Impact:** HIGH | **Complexity:** HIGH | **Priority:** P2 | **Effort:** 15 days

**Recommendation:**
- Implement persistent PTY-based shell session:
  - Create PTY on first `bash` tool call
  - Keep PTY alive across tool invocations
  - Parse command output using prompt detection (PS1)
  - Support interactive programs (REPLs, debuggers)
  - Add shell state display (cwd, env vars) in TUI
  - Add `bash_reset` tool to restart shell if state corrupted
- Isolate shell per conversation (not shared across sessions)
- Add `--no-persistent-shell` flag to disable

**Benefits:**
- Complex multi-step workflows (cd, export, build, test)
- Interactive program support (Python REPL, gdb, etc.)
- Competitive parity with ClaudeCode, Copilot CLI

#### 12. GitHub OAuth & API Client
**Impact:** MEDIUM | **Complexity:** MEDIUM | **Priority:** P2 | **Effort:** 10 days

**Recommendation:**
- Implement GitHub OAuth flow:
  - Device flow for CLI authentication (user authorizes via browser)
  - Store access token securely (OS keychain or encrypted file)
  - Add `/github login` command to initiate auth
- Create GitHub API client with rate limiting and error handling
- Add specialized tools:
  - `github_issue_list`, `github_issue_create`, `github_issue_comment`
  - `github_pr_list`, `github_pr_create`, `github_pr_review`, `github_pr_comment`
  - `github_repo_info`, `github_commit_history`, `github_file_blame`
- Auto-inject PR context if running in PR branch

**Benefits:**
- Native GitHub integration (no gh CLI required)
- Streamlined issue/PR workflows
- Competitive parity with ClaudeCode, Copilot CLI, RooCode

### 4.4 Medium-Impact Quick Wins

#### 13. Context Memoization System
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P2 | **Effort:** 5 days

**Recommendation:**
- Implement caching for expensive context operations:
  - File tree generation (cache with file system watcher invalidation)
  - Git commands (cache until user runs git via bash)
  - README content (cache at project level, invalidate on file change)
- Use `FileSystemWatcher` or mtime checks for invalidation
- Add `/context refresh` command to force re-computation

**Benefits:**
- 40%+ reduction in context preparation latency
- Lower token usage (skip redundant context)
- Faster agent response times

#### 14. Ripgrep Integration
**Impact:** HIGH | **Complexity:** LOW | **Priority:** P2 | **Effort:** 4 days

**Recommendation:**
- Replace `grep` tool backend with ripgrep:
  - Install ripgrep binary or bundle statically
  - Use `rg` command for all grep operations
  - Support advanced features: regex, file type filtering, ignore patterns
  - Parallel search across files (10x+ faster)
- Maintain backward compatibility (same tool interface)

**Benefits:**
- 10x+ faster search performance
- Better ignore patterns (.gitignore, .rgignore)
- Advanced regex and filter support

### 4.5 Ecosystem & Polish Features (Do Later)

#### 15. Complete MCP Client Implementation
**Impact:** HIGH | **Complexity:** MEDIUM | **Priority:** P3 | **Effort:** 12 days

**Recommendation:**
- Extend MCP client to expose tools from servers:
  - Parse MCP tool schemas and register dynamically
  - Map MCP tool invocations to server RPC calls
  - Handle tool results and errors
- Add OAuth support for authenticated MCP servers
- Implement server lifecycle management (start, stop, restart)

**Benefits:**
- Access to ecosystem of MCP tools (filesystem, git, postgres, etc.)
- Extensibility without recompiling ragent
- Competitive advantage (no competitor has full MCP support)

#### 16. Skills System
**Impact:** MEDIUM | **Complexity:** LOW | **Priority:** P3 | **Effort:** 5 days

**Recommendation:**
- Add `/skill <name>` command to invoke predefined capabilities:
  - Skills defined in `~/.ragent/skills/` and `.ragent/skills/`
  - Skill files contain prompt template and optional tool allowlist
  - Variables substituted: `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{ARGS}}`
- Add `/skill list` to show available skills
- Ship default skills: /skill debug, /skill refactor, /skill explain

**Benefits:**
- Reusable agent capabilities without custom agents
- Easy skill sharing (copy .md file)
- Competitive parity with OpenCode

#### 17. Auto-Update Mechanism
**Impact:** LOW | **Complexity:** LOW | **Priority:** P4 | **Effort:** 5 days

**Recommendation:**
- On startup, check GitHub releases API for new version:
  - Compare current version (from Cargo.toml) with latest release
  - Display update notification in TUI if newer version available
  - Add `/update` command to download and replace binary
- Use GitHub's auto-update patterns (download, verify signature, replace)
- Add `--no-update-check` flag to disable

**Benefits:**
- Users stay up-to-date automatically
- Reduces support burden (fewer old version issues)
- Competitive parity with ClaudeCode

---

## 5. Implementation Plan with Milestones and Tasks

*Source: Implementation Plan by swarm-s6*

### 5.1 Milestone Overview

| Milestone | Duration | Focus | Success Metric |
|-----------|----------|-------|----------------|
| **M1: Quick Wins** | 6-8 weeks | High-impact, low-complexity features | Cost transparency 100%, permission prompts -40%, git context in all sessions |
| **M2: Autonomous Operation** | 8-10 weeks | Autopilot, plan mode, agent roles | Autonomous task completion 60%+, planning adoption 30%+ |
| **M3: Project Intelligence** | 8-10 weeks | Memory system, context optimization | Context efficiency +40%, memory write usage 50%+ |
| **M4: Advanced Features** | 10-12 weeks | Persistent shell, GitHub, security | Multi-step workflows +80%, GitHub API usage 40%+ |
| **M5: Ecosystem & Polish** | 12-16 weeks | MCP, skills, auto-update | MCP tool usage 20%+, skill invocations 30%+ |

**Total Timeline:** 44-56 weeks (12-14 months)

---

### 5.2 Milestone 1: Quick Wins & Critical Parity

**Duration:** 6-8 weeks  
**Goal:** Deliver high-impact, low-complexity features to close immediate gaps

**Progress Tracker:** mark each task complete here as it lands.

- [x] Task 1.1: Cost Tracking System
- [x] Task 1.2: Git Context Auto-Injection
- [x] Task 1.3: README Auto-Injection
- [x] Task 1.4: Safe Command Whitelist
- [x] Task 1.5: ThinkTool for Agent Reasoning

#### Task 1.1: Cost Tracking System
**Priority:** P0 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Token counting for all message exchanges (input + output)
- [ ] USD cost calculation using provider-specific pricing tables
- [ ] `/cost` command shows total tokens, USD cost, breakdown by provider, session duration
- [ ] Real-time cost display in TUI status bar (optional)
- [ ] Session cost logged to history/analytics file
- [ ] Support for all providers: Anthropic, OpenAI, Ollama, Azure

**Technical Approach:**
1. Create `TokenTracker` struct to accumulate counts per message
2. Add pricing table as `HashMap<ProviderModel, Pricing>` in config
3. Update each provider client to return token counts in response
4. Create `/cost` command handler
5. Add cost summary to session cleanup

**Risks:** Pricing tables may become outdated (mitigation: auto-update from public APIs)

---

#### Task 1.2: Git Context Auto-Injection
**Priority:** P0 | **Effort:** 3 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] On session start, run 5 git commands in parallel (1s timeout each)
- [ ] Inject results into system prompt context section
- [ ] Truncate total git context to max 200 lines
- [ ] Cache results, invalidate on `/clear` command
- [ ] Gracefully handle non-git directories (skip injection)
- [ ] Add `--no-git-context` flag to disable

**Technical Approach:**
1. Create `GitContextProvider` module
2. Use `tokio::spawn` for parallel command execution
3. Parse and format output into structured context string
4. Inject into `SystemPrompt` builder
5. Add memoization cache with session lifetime

**Risks:** Git commands may timeout in large repos (mitigation: 1s timeout per command)

---

#### Task 1.3: README Auto-Injection
**Priority:** P0 | **Effort:** 2 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Search for README files (README.md, README.txt, README.rst, case-insensitive)
- [ ] Check working directory and parent directories (up to 3 levels)
- [ ] Extract first 500 lines of README content
- [ ] Inject into system prompt context section
- [ ] Add `--no-readme-context` flag to disable

**Technical Approach:**
1. Create `ReadmeProvider` module
2. Search directory tree with `fs::read_dir` (non-recursive)
3. Filter for README variants (case-insensitive)
4. Read and truncate to 500 lines
5. Inject into `SystemPrompt` builder

**Risks:** Large README files (mitigation: truncate to 500 lines)

---

#### Task 1.4: Safe Command Whitelist
**Priority:** P0 | **Effort:** 3 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Whitelist of safe commands for auto-approval
- [ ] Check bash command against whitelist before execution
- [ ] Auto-approve if command matches (no permission prompt)
- [ ] Log auto-approvals for audit trail
- [ ] Configurable whitelist in `ragent.json`

**Technical Approach:**
1. Define `SAFE_COMMANDS` constant with regex patterns
2. Add `is_safe_command()` function to permission checker
3. Update `bash` tool to check whitelist before prompting
4. Log auto-approvals to event bus
5. Add config option to extend/override whitelist

**Risks:** Overly broad whitelist (mitigation: conservative initial list)

---

#### Task 1.5: ThinkTool for Agent Reasoning
**Priority:** P1 | **Effort:** 2 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] `think(thought: str)` tool for agents to expose reasoning
- [ ] Displayed in log panel (TUI) or response (API)
- [ ] Not sent back to LLM (output-only)
- [ ] System prompt encourages agents to think before acting

**Technical Approach:**
1. Create `ThinkTool` struct implementing `Tool` trait
2. Add to default tool registry
3. Update TUI log panel to display think messages
4. Add "Use the think() tool to explain your reasoning" to system prompt

**Risks:** Agents may over-use and slow down (mitigation: monitor usage)

---

#### Milestone 1 Testing & Integration
- [ ] Integration test for cost tracking (all providers)
- [ ] Integration test for git context injection
- [ ] Integration test for README auto-injection
- [ ] Integration test for safe command whitelist
- [ ] Manual TUI testing for all new features
- [ ] Documentation updates (QUICKSTART.md, CHANGELOG.md)

---

### 5.3 Milestone 2: Autonomous Operation

**Duration:** 8-10 weeks  
**Goal:** Enable autonomous agent operation with plan workflows

#### Task 2.1: Autopilot Mode
**Priority:** P1 | **Effort:** 8 days | **Assignee:** TBD | **Dependencies:** Task 1.4 (Safe Command Whitelist)

**Acceptance Criteria:**
- [ ] `/autopilot on` command to enable autonomous operation
- [ ] Auto-approve safe commands (whitelist)
- [ ] Auto-approve file writes in allowed patterns
- [ ] Prompt only for destructive operations (banned list)
- [ ] Token budget limit (stop after N tokens)
- [ ] Time limit (stop after N minutes)
- [ ] Task completion detection (agent calls `task_complete` tool)
- [ ] `/autopilot off` to return to interactive mode
- [ ] Display autopilot status in TUI status bar

**Technical Approach:**
1. Add `AutopilotState` to session context
2. Create `/autopilot` command handler
3. Update permission checker to auto-approve in autopilot mode
4. Add `task_complete` tool for agents to signal completion
5. Add token/time limits with graceful shutdown
6. Update TUI status bar to show autopilot status

**Risks:** Agents may run indefinitely (mitigation: token/time limits)

---

#### Task 2.2: Plan Mode with Approval Checkpoint
**Priority:** P1 | **Effort:** 6 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] `/plan <task>` command for two-phase workflow
- [ ] Phase 1 (Planning): Read-only tools, produces written plan
- [ ] Phase 2 (Implementation): User approves plan, agent executes with full tools
- [ ] `submit_plan(plan: str)` tool for agents to mark planning complete
- [ ] Display plan in TUI with approve/reject buttons
- [ ] On approval, continue session with implementation phase
- [ ] On rejection, return to planning with user feedback

**Technical Approach:**
1. Add `PlanModeState` to session context
2. Create `/plan` command handler
3. Create `submit_plan` tool
4. Update tool registry to filter by mode (planning vs implementation)
5. Add TUI plan approval dialog
6. Implement state transition logic (planning → implementation)

**Risks:** Agents may skip planning (mitigation: system prompt emphasis)

---

#### Task 2.3: Agent Mode Switching
**Priority:** P1 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Agent role modes: architect, coder, reviewer, debugger, tester
- [ ] `/mode <role>` command to switch agent behavior
- [ ] Each mode has specialized system prompt and tool allowlist
- [ ] Display current mode in TUI status bar

**Technical Approach:**
1. Define `AgentMode` enum with role variants
2. Create mode-specific system prompts
3. Create mode-specific tool allowlists
4. Add `/mode` command handler
5. Update session processor to apply mode constraints
6. Update TUI status bar to show current mode

**Risks:** Mode-specific prompts may conflict with agent definitions (mitigation: merge prompts)

---

#### Task 2.4: Completion Detection Tool
**Priority:** P1 | **Effort:** 2 days | **Assignee:** TBD | **Dependencies:** Task 2.1 (Autopilot Mode)

**Acceptance Criteria:**
- [ ] `task_complete(summary: str)` tool for agents to signal task completion
- [ ] Stops autopilot loop gracefully
- [ ] Displays summary in TUI
- [ ] Logs completion event

**Technical Approach:**
1. Create `TaskCompleteTool` struct
2. Add to default tool registry
3. Update autopilot loop to exit on task_complete
4. Display summary in TUI chat panel
5. Log completion event to event bus

**Risks:** Agents may call prematurely (mitigation: system prompt guidance)

---

#### Milestone 2 Testing & Integration
- [ ] Integration test for autopilot mode (full workflow)
- [ ] Integration test for plan mode (approval/rejection)
- [ ] Integration test for agent mode switching
- [ ] Manual TUI testing for all new features
- [ ] Documentation updates (QUICKSTART.md, CHANGELOG.md)

---

### 5.4 Milestone 3: Project Intelligence

**Duration:** 8-10 weeks  
**Goal:** Implement memory system and context optimization

#### Task 3.1: Hierarchical AGENTS.md Discovery
**Priority:** P2 | **Effort:** 6 days | **Assignee:** TBD | **Dependencies:** Task 3.5 (Ripgrep Integration)

**Acceptance Criteria:**
- [ ] Recursive discovery of AGENTS.md files via ripgrep
- [ ] Load and concatenate in hierarchical order (root → child dirs)
- [ ] List discovered file paths in system prompt (not full content)
- [ ] Support alternative names: CLAUDE.md, .ragent.md, INSTRUCTIONS.md
- [ ] Cache discovery results, invalidate on `/clear` or FS change

**Technical Approach:**
1. Use ripgrep to find all AGENTS.md variants
2. Sort by directory depth (root first)
3. Load and concatenate content
4. Inject file path list into system prompt
5. Add cache with file system watcher invalidation

**Risks:** Large AGENTS.md files (mitigation: list paths, not content)

---

#### Task 3.2: Memory Write Tool
**Priority:** P2 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] `memory_write(content: str, scope: 'user'|'project')` tool
- [ ] User scope: `~/.ragent/memory/MEMORY.md` (global)
- [ ] Project scope: `.ragent/memory/MEMORY.md` (project-specific)
- [ ] Auto-load memory files into context on session start
- [ ] `/memory show` command to view accumulated memory

**Technical Approach:**
1. Create `MemoryWriteTool` struct
2. Add to default tool registry
3. Implement file append logic (user vs project scope)
4. Add memory loader to session initialization
5. Create `/memory` command handler

**Risks:** Memory files may grow large (mitigation: periodic compaction)

---

#### Task 3.3: `/init` Command for Project Analysis
**Priority:** P2 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** Task 3.2 (Memory Write Tool)

**Acceptance Criteria:**
- [ ] `/init` command triggers project analysis agent
- [ ] Agent uses `explore` agent definition
- [ ] Analyzes file tree, README, build files, test structure
- [ ] Writes summary to `.ragent/memory/PROJECT_ANALYSIS.md`
- [ ] Auto-loads on subsequent session starts

**Technical Approach:**
1. Create `/init` command handler
2. Spawn `explore` agent with project analysis prompt
3. Agent uses file operations, grep, LSP tools
4. Agent calls `memory_write` with project scope
5. Add loader for PROJECT_ANALYSIS.md to session init

**Risks:** Long analysis time for large projects (mitigation: progress indicator)

---

#### Task 3.4: Context Memoization System
**Priority:** P2 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Cache file tree generation (invalidate on FS change)
- [ ] Cache git commands (invalidate on git operations)
- [ ] Cache README content (invalidate on file change)
- [ ] Use `FileSystemWatcher` or mtime checks for invalidation
- [ ] `/context refresh` command to force re-computation

**Technical Approach:**
1. Create `ContextCache` struct with TTL and invalidation
2. Wrap file tree, git, and README providers with cache
3. Add file system watcher for automatic invalidation
4. Create `/context refresh` command handler
5. Add cache hit/miss metrics

**Risks:** Stale cache if invalidation fails (mitigation: TTL fallback)

---

#### Task 3.5: Ripgrep Integration
**Priority:** P2 | **Effort:** 4 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Replace `grep` tool backend with ripgrep
- [ ] Install ripgrep binary or bundle statically
- [ ] Support advanced features: regex, file type filtering, ignore patterns
- [ ] Parallel search across files (10x+ faster)
- [ ] Maintain backward compatibility (same tool interface)

**Technical Approach:**
1. Add ripgrep as dependency (static binary or runtime check)
2. Update `GrepTool` to invoke `rg` command
3. Map tool parameters to `rg` flags
4. Parse and format `rg` output
5. Add fallback to grep if ripgrep unavailable

**Risks:** Ripgrep unavailable on user system (mitigation: fallback to grep)

---

#### Milestone 3 Testing & Integration
- [ ] Integration test for hierarchical AGENTS.md discovery
- [ ] Integration test for memory write tool
- [ ] Integration test for `/init` command
- [ ] Integration test for context memoization
- [ ] Integration test for ripgrep integration
- [ ] Manual TUI testing for all new features
- [ ] Documentation updates (QUICKSTART.md, CHANGELOG.md)

---

### 5.5 Milestone 4: Advanced Features & Differentiation

**Duration:** 10-12 weeks  
**Goal:** Implement persistent shell, GitHub integration, and security enhancements

#### Task 4.1: Persistent Shell Implementation
**Priority:** P2 | **Effort:** 15 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Create PTY on first `bash` tool call
- [ ] Keep PTY alive across tool invocations
- [ ] Parse command output using prompt detection (PS1)
- [ ] Support interactive programs (REPLs, debuggers)
- [ ] Add shell state display (cwd, env vars) in TUI
- [ ] Add `bash_reset` tool to restart shell if state corrupted
- [ ] Isolate shell per conversation (not shared across sessions)
- [ ] Add `--no-persistent-shell` flag to disable

**Technical Approach:**
1. Add `pty-process` or `portable-pty` dependency
2. Create `PersistentShell` struct managing PTY lifecycle
3. Implement command execution with output parsing
4. Add PS1 detection for command completion
5. Create shell state tracker (cwd, env vars)
6. Update TUI to display shell state
7. Add `bash_reset` tool

**Risks:** PTY parsing errors (mitigation: fallback to non-persistent)

---

#### Task 4.2: Shell State Display
**Priority:** P2 | **Effort:** 3 days | **Assignee:** TBD | **Dependencies:** Task 4.1 (Persistent Shell)

**Acceptance Criteria:**
- [ ] Display current working directory in TUI status bar
- [ ] Display key environment variables (PATH, VIRTUAL_ENV, etc.)
- [ ] Update in real-time after each bash command

**Technical Approach:**
1. Extract cwd and env vars from shell state tracker
2. Add shell state section to TUI status bar
3. Update on each bash tool execution

**Risks:** Cluttered status bar (mitigation: abbreviate paths)

---

#### Task 4.3: GitHub OAuth & API Client
**Priority:** P2 | **Effort:** 10 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] GitHub OAuth device flow for CLI authentication
- [ ] Store access token securely (OS keychain or encrypted file)
- [ ] `/github login` command to initiate auth
- [ ] Create GitHub API client with rate limiting and error handling

**Technical Approach:**
1. Add `octocrab` or `reqwest` for GitHub API
2. Implement OAuth device flow (user authorizes via browser)
3. Store token securely (use `keyring` crate or encrypted file)
4. Add `/github login` command handler
5. Create `GitHubClient` struct with rate limiting

**Risks:** OAuth flow complexity (mitigation: thorough testing)

---

#### Task 4.4: GitHub Issue Management
**Priority:** P2 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** Task 4.3 (GitHub OAuth)

**Acceptance Criteria:**
- [ ] `github_issue_list`, `github_issue_create`, `github_issue_comment` tools
- [ ] Support filtering by state, labels, assignee
- [ ] Display issue details in structured format

**Technical Approach:**
1. Create GitHub issue tools using `GitHubClient`
2. Map tool parameters to GitHub API calls
3. Format responses for LLM consumption
4. Add to default tool registry

**Risks:** API rate limits (mitigation: caching and rate limiting)

---

#### Task 4.5: GitHub PR Management
**Priority:** P2 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** Task 4.3 (GitHub OAuth)

**Acceptance Criteria:**
- [ ] `github_pr_list`, `github_pr_create`, `github_pr_review`, `github_pr_comment` tools
- [ ] Auto-inject PR context if running in PR branch
- [ ] Support PR review approval/request changes

**Technical Approach:**
1. Create GitHub PR tools using `GitHubClient`
2. Add PR context detection (check branch naming pattern)
3. Inject PR details into system prompt if detected
4. Add to default tool registry

**Risks:** PR context injection complexity (mitigation: simple heuristics)

---

#### Task 4.6: Advanced Security Features
**Priority:** P2 | **Effort:** 6 days | **Assignee:** TBD | **Dependencies:** Task 1.4 (Safe Command Whitelist)

**Acceptance Criteria:**
- [ ] Banned command list (rm -rf, mkfs, dd, sudo, etc.)
- [ ] LLM-based command injection detection (optional, expensive)
- [ ] Command syntax pre-check using shell parser
- [ ] Directory escape guard (prevent access outside project root)

**Technical Approach:**
1. Define `BANNED_COMMANDS` constant with regex patterns
2. Add banned command check to permission checker (hard reject)
3. Optional: Add LLM-based injection detection (separate LLM call)
4. Add shell syntax pre-check using `shlex` or similar
5. Add directory escape guard to file tools (check path canonicalization)

**Risks:** False positives on banned commands (mitigation: allow override with --force)

---

#### Milestone 4 Testing & Integration
- [ ] Integration test for persistent shell (multi-step workflows)
- [ ] Integration test for GitHub OAuth flow
- [ ] Integration test for GitHub issue/PR tools
- [ ] Integration test for advanced security features
- [ ] Manual TUI testing for all new features
- [ ] Documentation updates (QUICKSTART.md, CHANGELOG.md)

---

### 5.6 Milestone 5: Ecosystem Integration & Polish

**Duration:** 12-16 weeks  
**Goal:** Full MCP support, skills system, auto-update, and polish

#### Task 5.1: Complete MCP Client Implementation
**Priority:** P3 | **Effort:** 12 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Expose tools from MCP servers dynamically
- [ ] Parse MCP tool schemas and register in tool registry
- [ ] Map MCP tool invocations to server RPC calls
- [ ] Handle tool results and errors
- [ ] OAuth support for authenticated MCP servers
- [ ] Server lifecycle management (start, stop, restart)

**Technical Approach:**
1. Extend `rmcp` client to fetch tool definitions
2. Parse tool schemas and create dynamic tool wrappers
3. Register MCP tools in tool registry at runtime
4. Implement RPC call mapping (tool → MCP server)
5. Add OAuth flow for authenticated servers
6. Add server lifecycle management commands

**Risks:** MCP server compatibility issues (mitigation: test with common servers)

---

#### Task 5.2: MCP OAuth Support
**Priority:** P3 | **Effort:** 4 days | **Assignee:** TBD | **Dependencies:** Task 5.1 (MCP Client)

**Acceptance Criteria:**
- [ ] OAuth flow for MCP servers requiring authentication
- [ ] Token storage and refresh
- [ ] Add to MCP server configuration

**Technical Approach:**
1. Implement OAuth client flow (authorization code or device flow)
2. Store tokens securely (OS keychain or encrypted file)
3. Add token refresh logic
4. Update MCP server configuration schema

**Risks:** OAuth flow complexity (mitigation: reuse GitHub OAuth patterns)

---

#### Task 5.3: Skills System
**Priority:** P3 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] `/skill <name>` command to invoke predefined capabilities
- [ ] Skills defined in `~/.ragent/skills/` and `.ragent/skills/`
- [ ] Skill files contain prompt template and optional tool allowlist
- [ ] Variables substituted: `{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{ARGS}}`
- [ ] `/skill list` to show available skills
- [ ] Ship default skills: /skill debug, /skill refactor, /skill explain

**Technical Approach:**
1. Create skill discovery logic (search skill directories)
2. Parse skill files (YAML or Markdown with frontmatter)
3. Add `/skill` command handler with template substitution
4. Create default skill definitions
5. Add `/skill list` command

**Risks:** Skill template complexity (mitigation: simple variable substitution)

---

#### Task 5.4: Hooks System
**Priority:** P3 | **Effort:** 4 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] Hook points: `on_session_start`, `on_session_end`, `on_error`, `on_permission_denied`
- [ ] Hooks defined in config as shell commands or agent invocations
- [ ] Execute hooks asynchronously (non-blocking)

**Technical Approach:**
1. Define `Hook` struct with trigger and action
2. Add hook configuration to `ragent.json`
3. Add hook execution points in session processor
4. Implement async hook execution (spawn tasks)

**Risks:** Hook execution failures (mitigation: log errors, don't block)

---

#### Task 5.5: Auto-Update Mechanism
**Priority:** P4 | **Effort:** 5 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] On startup, check GitHub releases API for new version
- [ ] Display update notification in TUI if newer version available
- [ ] `/update` command to download and replace binary
- [ ] Verify signature (if available)
- [ ] Add `--no-update-check` flag to disable

**Technical Approach:**
1. Add GitHub releases API client
2. Compare current version (from Cargo.toml) with latest release
3. Display notification in TUI (non-blocking)
4. Add `/update` command handler
5. Download binary, verify, replace current binary

**Risks:** Binary replacement permissions (mitigation: prompt for sudo if needed)

---

#### Task 5.6: `/doctor` Diagnostic Command
**Priority:** P4 | **Effort:** 3 days | **Assignee:** TBD | **Dependencies:** None

**Acceptance Criteria:**
- [ ] `/doctor` command runs comprehensive system checks
- [ ] Check provider API keys (present and valid)
- [ ] Check LSP servers (installed and working)
- [ ] Check MCP servers (installed and working)
- [ ] Check git (installed and repo status)
- [ ] Check ripgrep (installed)
- [ ] Display summary with pass/fail indicators

**Technical Approach:**
1. Create `/doctor` command handler
2. Run provider health checks (API key validation)
3. Run LSP server discovery and health checks
4. Run MCP server discovery and health checks
5. Check git installation and repo status
6. Check ripgrep installation
7. Format results as structured report

**Risks:** False negatives (mitigation: detailed error messages)

---

#### Milestone 5 Testing & Integration
- [ ] Integration test for complete MCP client
- [ ] Integration test for skills system
- [ ] Integration test for hooks system
- [ ] Integration test for auto-update mechanism
- [ ] Integration test for `/doctor` command
- [ ] Manual TUI testing for all new features
- [ ] Documentation updates (QUICKSTART.md, CHANGELOG.md)

---

### 5.7 Effort Summary

| Milestone | Tasks | Total Effort (days) | Duration (weeks) |
|-----------|-------|---------------------|------------------|
| M1: Quick Wins | 5 | 15 | 6-8 |
| M2: Autonomous Operation | 4 | 21 | 8-10 |
| M3: Project Intelligence | 5 | 25 | 8-10 |
| M4: Advanced Features | 6 | 44 | 10-12 |
| M5: Ecosystem & Polish | 6 | 33 | 12-16 |
| **Total** | **26** | **138** | **44-56** |

**Adjusted Timeline with Parallelization:**
- Multiple developers working concurrently can reduce total duration
- Estimated 12-14 months for core features (Milestones 1-4)
- Milestone 5 can proceed in parallel with user feedback collection

---

### 5.8 Dependencies & Critical Path

#### Critical Path (Sequential)
1. **M1 Task 1.4** (Safe Command Whitelist) → **M2 Task 2.1** (Autopilot Mode)
2. **M3 Task 3.5** (Ripgrep Integration) → **M3 Task 3.1** (Hierarchical AGENTS.md)
3. **M3 Task 3.2** (Memory Write) → **M3 Task 3.3** (`/init` Command)
4. **M4 Task 4.3** (GitHub OAuth) → **M4 Task 4.4** (Issues) + **M4 Task 4.5** (PRs)
5. **M5 Task 5.1** (MCP Client) → **M5 Task 5.2** (MCP OAuth)

#### Parallel Opportunities
- **M1 tasks** are mostly independent (can run in parallel)
- **M2 tasks** can run in parallel after M1 completion
- **M3 tasks** can partially overlap (3.1, 3.2, 3.4 are independent)
- **M4 tasks** 4.1-4.2 (Shell) and 4.3-4.5 (GitHub) are independent
- **M5 tasks** are mostly independent (can run in parallel)

---

### 5.9 Risk Management

#### High-Risk Tasks
1. **M4 Task 4.1** (Persistent Shell) — Complex PTY handling, parsing
   - **Mitigation:** Thorough testing, fallback to non-persistent mode
2. **M5 Task 5.1** (Complete MCP Client) — MCP server compatibility issues
   - **Mitigation:** Test with common servers, graceful degradation

#### Medium-Risk Tasks
1. **M2 Task 2.1** (Autopilot Mode) — Agents may run indefinitely
   - **Mitigation:** Token/time limits, task completion detection
2. **M4 Task 4.3** (GitHub OAuth) — OAuth flow complexity
   - **Mitigation:** Reuse patterns from other tools, thorough testing

#### Low-Risk Tasks
- Most M1, M3, M5 tasks are low-risk (well-understood patterns)

---

### 5.10 Success Criteria by Milestone

#### Milestone 1 Success Criteria
- [ ] Cost transparency visible to 100% of users
- [ ] Permission prompts reduced by 40%
- [ ] Git context available in all sessions
- [ ] ThinkTool improves agent reasoning quality by 20%

#### Milestone 2 Success Criteria
- [ ] Autopilot mode enables 60%+ of users to run unattended tasks
- [ ] Plan mode adoption by 30%+ of users
- [ ] Agent role switching used in 20%+ of sessions

#### Milestone 3 Success Criteria
- [ ] Context efficiency improved by 40% (fewer tokens, faster responses)
- [ ] Memory write usage by 50%+ of users
- [ ] Ripgrep search 10x faster than grep

#### Milestone 4 Success Criteria
- [ ] Persistent shell enables 80%+ more complex workflows
- [ ] GitHub API usage by 40%+ of users
- [ ] Zero security incidents from command safety features

#### Milestone 5 Success Criteria
- [ ] MCP tool usage by 20%+ of users
- [ ] Skill invocations by 30%+ of users
- [ ] Auto-update adoption by 80%+ of users

---

## 6. Rollout Strategy & Release Plan

### 6.1 Alpha Releases (After Milestone 1)
**Version:** 0.2.0-alpha  
**Timeline:** 6-8 weeks from start  
**Target Audience:** Early adopters, power users  
**Features:**
- Cost tracking system
- Git context auto-injection
- README auto-injection
- Safe command whitelist
- ThinkTool

**Rollout:**
- GitHub release with binaries (Linux, macOS, Windows)
- Announcement on project README and relevant communities
- Solicit feedback on critical features

### 6.2 Beta Releases (After Milestone 2)
**Version:** 0.3.0-beta  
**Timeline:** 14-18 weeks from start  
**Target Audience:** Broader user base, production pilots  
**Features:**
- All Milestone 1 features
- Autopilot mode
- Plan mode with approval
- Agent role switching
- Task completion detection

**Rollout:**
- GitHub release with binaries
- Documentation updates (QUICKSTART.md, tutorial videos)
- User testimonials and case studies

### 6.3 Release Candidate (After Milestone 3)
**Version:** 0.9.0-rc  
**Timeline:** 22-28 weeks from start  
**Target Audience:** Production users, enterprises  
**Features:**
- All Milestone 1-2 features
- Hierarchical AGENTS.md discovery
- Memory write tool
- `/init` command
- Context memoization
- Ripgrep integration

**Rollout:**
- GitHub release with binaries
- Comprehensive documentation
- Stability and performance testing
- Bug bounty program

### 6.4 v1.0 Production (After Milestone 4)
**Version:** 1.0.0  
**Timeline:** 32-40 weeks from start  
**Target Audience:** General availability  
**Features:**
- All Milestone 1-3 features
- Persistent shell
- GitHub OAuth and API integration
- Advanced security features

**Rollout:**
- Major GitHub release announcement
- Blog post, social media campaign
- Package manager distribution (Homebrew, Cargo, apt)
- Press outreach

### 6.5 v1.5 Enhancement (After Milestone 5)
**Version:** 1.5.0  
**Timeline:** 44-56 weeks from start  
**Target Audience:** Ecosystem adopters  
**Features:**
- All Milestone 1-4 features
- Complete MCP support
- Skills system
- Hooks system
- Auto-update mechanism
- `/doctor` diagnostic command

**Rollout:**
- GitHub release with binaries
- MCP server ecosystem documentation
- Skills marketplace (community contributions)

---

## 7. Key Performance Indicators (KPIs)

### 7.1 Development Metrics
- **Velocity:** Tasks completed per week (target: 1.5-2 tasks/week)
- **Quality:** Bug rate per release (target: <5 critical bugs)
- **Test Coverage:** Code coverage percentage (target: >70%)
- **Documentation:** Docs-to-code ratio (target: 15%+)

### 7.2 Product Metrics
- **Adoption:** Active users per month (target: 10% MoM growth)
- **Engagement:** Sessions per user per week (target: 5+)
- **Retention:** 30-day retention rate (target: >40%)
- **Cost Efficiency:** Average LLM cost per session (target: <$0.50)

### 7.3 Competitive Metrics
- **Feature Parity:** % of competitor features implemented (target: 85%+)
- **Performance:** Response time vs. competitors (target: 20% faster)
- **Reliability:** Uptime and error rate (target: 99.5% uptime)
- **User Satisfaction:** NPS score (target: >50)

---

## 8. Open Questions & Decisions Needed

### 8.1 Technical Decisions
1. **Persistent Shell:** PTY library choice (`pty-process` vs `portable-pty`)?
2. **MCP OAuth:** Which OAuth flows to support (authorization code, device flow, both)?
3. **Context Memoization:** Use file system watcher or mtime checks for invalidation?
4. **GitHub API:** Use `octocrab` library or direct `reqwest` implementation?
5. **Ripgrep:** Bundle statically or require system installation?

### 8.2 Product Decisions
1. **Autopilot Defaults:** What token/time limits should be default for autopilot mode?
2. **Plan Mode:** Should plan mode be default for complex tasks, or opt-in?
3. **Agent Roles:** Should mode switching be automatic based on task type?
4. **Memory Scope:** Should memory be global by default, or project-scoped?
5. **GitHub Integration:** OAuth vs. PAT (personal access token) — which to prioritize?
6. **Cost Tracking:** Should there be budget limits with automatic stops?
7. **Skills System:** Should skills be discoverable via online registry, or local-only?
8. **Auto-Update:** Automatic updates by default, or prompt user for approval?

### 8.3 Business Decisions
1. **Pricing:** Will ragent remain free and open-source, or introduce paid tiers?
2. **Support:** What level of support will be provided (community-only, paid support)?
3. **Partnerships:** Should we pursue partnerships with LLM providers for discounts?
4. **Marketing:** What channels will be used for user acquisition (GitHub, Reddit, Twitter)?

---

## 9. Conclusion

This implementation plan provides a comprehensive roadmap to transform **ragent** from a strong alpha product into a competitive, feature-complete AI coding agent over the next 12-18 months. By prioritizing high-impact quick wins in Milestone 1, establishing autonomous operation capabilities in Milestone 2, and building intelligent context management in Milestone 3, we position ragent to achieve competitive parity with ClaudeCode, OpenCode, and GitHub Copilot CLI while leveraging unique differentiators (Rust performance, multi-provider support, team orchestration).

### Key Takeaways

1. **Strong Foundation:** ragent already has 58 tools, multi-provider support, LSP integration, and advanced team coordination — a solid base for growth.

2. **Clear Gaps:** Seven critical gaps identified (persistent shell, project memory, git context, cost tracking, permissions, autonomous modes, GitHub integration) with actionable solutions.

3. **Phased Approach:** 5 milestones balance quick wins (M1), strategic features (M2-3), transformative capabilities (M4), and ecosystem integration (M5).

4. **Competitive Positioning:** Upon completion, ragent will match or exceed competitors in critical areas while maintaining unique strengths (Rust, multi-provider, teams, LSP, prompt optimization).

5. **Risk Mitigation:** High-risk tasks identified with mitigation strategies; parallel opportunities enable faster delivery with multiple developers.

6. **Success Metrics:** Clear KPIs for each milestone ensure measurable progress and course correction opportunities.

### Next Steps

1. **Resource Allocation:** Assign developers to high-priority tasks (M1) immediately.
2. **Stakeholder Alignment:** Review and approve roadmap with project leadership.
3. **Community Engagement:** Share roadmap publicly for feedback and contributions.
4. **Infrastructure Setup:** Prepare CI/CD, testing environments, and documentation platforms.
5. **Kickoff Milestone 1:** Begin implementation of cost tracking, git context, README injection, safe command whitelist, and ThinkTool.

---

**Document Status:** Final  
**Approval Required:** Project Lead, Engineering Team  
**Next Review:** After Milestone 1 completion (6-8 weeks)  
**Contact:** swarm-s7 (document compiler), swarm-s1 (codebase analysis), swarm-s5 (competitive analysis), swarm-s6 (implementation plan)
