# Ragent Documentation Index

This index provides an overview of all markdown files in the ragent project. Documentation is organized into two categories: project-root files (configuration and release information) and documentation-folder files (detailed guides and specifications).

---

## Project Root Documentation

Root-level markdown files contain critical project configuration, guidelines, and status information.

### AGENTS.md

**Purpose:** Defines guidelines and standards for AI agent implementations and Rust development practices.

**Contents:**
- Technology stack requirements (Rust edition 2021+)
- Build, test, lint, and format commands with timeout specifications
- Test organization requirements and naming conventions
- Unit specifications (DateTime UTC, dimensional units in mm, UTF-8 encoding)
- GitHub access patterns and changelog management
- Documentation standards (DOCBLOCK comments, folder organization)
- Code style guidelines (4 spaces, max 100 width, snake_case/PascalCase)
- Workflow methodology for task management and debugging
- Versioning with alpha suffix convention
- Priority levels (0: critical, 1: high, 2: medium, 3: low, 4: backlog)

**Key Note:** "Hi im Rust Agent and I have read Agents.md" — startup greeting requirement.

---

### AGENTS_FIX.md

**Purpose:** Comprehensive task list for implementing missing and incomplete code features.

**Contents:**
- Summary table of 8 incomplete tasks (3 critical, 3 high, 2 medium priority)
- Critical Priority (P0):
  - TASK-001: MCP Client `connect()` method implementation
  - TASK-002: MCP Client `list_tools()` method implementation
  - TASK-003: MCP Client `call_tool()` method implementation
- High Priority (P1):
  - TASK-004: HTTP Server `abort_session()` endpoint
  - TASK-005: TUI agent switching functionality
  - TASK-006: TUI slash command parsing and dispatch
- Medium Priority (P2):
  - TASK-007: CLI session resume implementation
  - TASK-008: CLI session import persistence
- Dependency graph showing blocked relationships
- Recommended implementation order
- Testing strategy for each task category
- Code quality checklist for completion verification

---

### CHANGELOG.md

**Purpose:** Maintains a semantic versioning changelog following Keep a Changelog format.

**Contents:**
- Current version: 0.1.0-alpha.2 (2025-07-25)
- Added features: `/provider_reset` slash command, clipboard copy support, storage methods, robust API base resolution
- Fixed bugs: Copilot "Unknown model" error, API base URL resolution, provider reset persistence
- Previous releases (0.1.0-alpha.1 and 0.1.0-alpha.0) with detailed change history
- Initial scaffolding through alpha development phases

---

### DOC_INVENTRY.md

**Purpose:** Tracks all public API functions and types requiring documentation.

**Contents:**
- Summary: 112 items tracked, all with documentation and examples (100% coverage)
- Organized by crate:
  - ragent-core: 87 items (agent, config, event, id, mcp, message, permission, provider, sanitize, session, snapshot, storage, tool modules)
  - ragent-server: 3 items (HTTP routes and SSE)
  - ragent-tui: 22 items (app, input, layout, lib, tips, widgets)
- Documentation status tracked for each function type and module
- Verification that all items have example code blocks

---

### O365_TOOL.md

**Purpose:** Specification and implementation plan for Office document read/write support.

**Contents:**
- Overview of adding office_read, office_write, and office_info tools
- Recommended Rust crates: docx-rust, calamine, rust_xlsxwriter, ooxmlsdk
- Three new tools: office_read (file:read), office_write (file:write), office_info (file:read)
- Detailed JSON schemas for each tool's parameters
- 11-task implementation plan:
  - TASK-001: Add dependencies and module skeleton
  - TASK-002: Format detection and shared helpers
  - TASK-003 to TASK-005: office_read implementations (Word, Excel, PowerPoint)
  - TASK-006 to TASK-008: office_write implementations (Word, Excel, PowerPoint)
  - TASK-009: office_info metadata extraction
  - TASK-010: Tool registration and integration tests
  - TASK-011: Documentation updates
- Output format examples showing expected LLM interaction patterns
- Risk mitigation strategies for complexity and file handling
- Dependencies summary with version guidance

---

### QUICKSTART.md

**Purpose:** User-facing quick start guide for installation, configuration, and common workflows.

**Contents:**
- Prerequisites: Rust 1.85+ (edition 2024), LLM provider
- Installation from source with build instructions
- 13 numbered sections:
  1. Provider configuration (Anthropic, OpenAI, Copilot, Ollama)
  2. First run with example commands
  3. Listing available models
  4. Configuration file format with example ragent.json
  5. Built-in agents (ask, general, build, plan, explore, internal)
  6. Available tools with permission categories
  7. Session management (list, resume, export, import)
  8. HTTP server mode and API endpoints
  9. Environment variables reference
  10. Permission system with rule examples
  11. Common workflows (code review, refactoring, local privacy, project setup)
  12. TUI interaction (mouse support, slash commands, key bindings)
  13. Data storage locations and troubleshooting
- End note: "For full details, see README.md and SPEC.md"

---

### README.md

**Purpose:** High-level project overview and quick introduction.

**Contents:**
- Project tagline: "An AI coding agent for the terminal, built in Rust"
- Inspiration from OpenCode; implemented as learning exercise in Rust
- Key features: multi-provider LLM support, 8 built-in tools, TUI, HTTP server, session management, permission system, agent presets, MCP client, snapshot/undo, event bus
- Installation instructions (from source, requires Rust 1.85+)
- Quick start examples (configure API key, launch TUI, run one-shot, serve HTTP)
- Usage overview with command list
- Configuration file format and example
- Architecture diagram showing data flow (TUI/Server → Event Bus → Session Processor → Providers/Tools/Storage)
- Project status: v0.1.0-alpha.2 with detailed feature list
- MIT License reference

---

### RELEASE.md

**Purpose:** Tracks current release version and recent changelog entry for GitHub releases.

**Contents:**
- Current version: 0.1.0-alpha.2
- Recent additions (since 0.1.0-alpha.1):
  - `/provider_reset` slash command with interactive UI
  - Clipboard copy support on Copilot device code screen
  - Storage methods: `delete_provider_auth()`, `delete_setting()`
  - Robust Copilot API base resolution with multi-source discovery
  - VS Code-compatible headers for Copilot chat API
- Recent fixes:
  - Copilot "Unknown model" error — device flow token prioritized
  - Copilot API uses plan-specific endpoints
  - Provider reset persistence across restarts

---

### SPEC.md

**Purpose:** Complete technical specification and architecture reference (comprehensive).

**Contents:**
- 10-section specification with table of contents:
  1. **Goals & Non-Goals** — 10 goals (feature parity, single binary, cross-platform, sub-second startup, etc.) and 5 non-goals (no desktop GUI, no cloud service, no plugin system, etc.)
  2. **Architecture Overview** — ASCII diagram showing CLI, HTTP Server, TUI, Session Manager, Agent Loop, LLM Stream, Tools, Permissions, MCP, Providers, Storage
  3. **Core Modules** (16 subsections):
     - CLI & Entry Point — clap subcommands (run, serve, session, auth, models, config, mcp, upgrade, uninstall)
     - Configuration — file format, load precedence, schema, deep-merge semantics
     - Provider System — supported providers (Anthropic, Copilot, OpenAI, Ollama, Google, Azure, Bedrock, etc.), streaming interface, StreamEvent enum
     - Agent System — agent definitions, built-in agents (ask, general, build, plan, explore, title, summary, compaction)
     - Session Management — session lifecycle (create, chat, continue, compact, archive, resume, export, import)
     - Message Model — parts-based message structure, Role and MessagePart enums
     - Tool System — Tool trait, ToolRegistry, built-in tools (read, write, edit, bash, grep, glob, list, etc.), execution flow
     - Permission System — rule structure, evaluation order, special permissions, ask flow
     - HTTP Server — axum routes (GET/POST/PUT/DELETE), SSE event types
     - Terminal UI (TUI) — home screen, provider setup dialog, chat layout, log panel, key bindings, mouse support, scrollbars, auto-expanding input, slash commands
     - MCP Client — server configuration, lifecycle (start, initialize, list tools, execute, reconnect, shutdown)
     - LSP Integration — supported language servers (Rust, TypeScript, Python, Go, C/C++), LSP capabilities
     - Event Bus — event types (SessionCreated, TextDelta, ToolCallStart, PermissionRequested, etc.), implementation via tokio broadcast
     - Storage & Database — SQLite via rusqlite, schema (sessions, messages, provider_auth, mcp_servers, snapshots tables)
     - Shell Execution — bash tool execution model, safety features (kill_on_drop, timeout, output truncation, permission gating)
     - Snapshot & Undo — snapshot flow, undo granularity (per-tool-call, per-message, per-session)
  4. **Data Flow** — request-to-response flow diagram with permission checking and tool execution
  5. **Configuration File Format** — minimal and full example configs with jsonc syntax
  6. **Rust Crate Map** — dependency mapping for all major functionality
  7. **Project Layout** — complete directory structure of workspace
  8. **Build & Distribution** — build commands, binary optimization, distribution channels (GitHub, Homebrew, Cargo, AUR, Nix, Docker)
  9. **Testing Strategy** — test layers (unit, integration, E2E, TUI, fuzzing), mock LLM server
  10. **Future / Stretch Goals** — 10 future features (Web UI, mobile client, plugin system, git worktree isolation, OpenTelemetry, multi-agent, benchmarks, enterprise, voice, vision)

---

### STATS.md

**Purpose:** Project metrics and statistics at a glance.

**Contents:**
- Total Rust lines: 11,419
- Rust file count: 49
- Tests passed: 195
- Tests failed: 0
- Release binary size: 5.6M

---

### UPDATE_DOCS.md

**Purpose:** Tracks missing docblocks and documentation status.

**Contents:**
- Overview: 1 public function without proper documentation
- Remediation task for `redact_secrets()` in `crates/ragent-core/src/sanitize.rs` (line 11)
- Suggested docblock with parameter and return value documentation
- Example code block for usage
- Status: Not started, Priority: Low
- Summary: 22 documented functions, 1 without docs, 95.7% coverage

---

## Documentation Folder Files

Detailed documentation files located in the `docs/` directory per architectural guidelines.

### docs/CODE_CLEANUP.md

**Purpose:** Tracks code quality improvements and cleanup tasks separate from feature implementation.

**Current Status:** Referenced in AGENTS_FIX.md as related but different scope from missing features.

---

### docs/INDEX.md

**Purpose:** Provides navigation and overview of all documentation (this file).

---

### docs/TODO.md

**Purpose:** List of additional unimplemented functions and maintenance tasks.

**Current Status:** Referenced in AGENTS_FIX.md as related but different scope.

---

## File Organization Summary

| Category | Location | Purpose |
|----------|----------|---------|
| Agent Guidelines | Root | Development standards and workflow |
| Task Tracking | Root | Missing features and implementation checklist |
| Changelog | Root | Version history and release notes |
| Quick Start | Root | User-facing getting started guide |
| Specification | Root | Complete technical reference (1,593 lines) |
| Statistics | Root | Project metrics |
| Release Info | Root | Current version and recent changes |
| README | Root | High-level project overview |
| Documentation Index | docs/ | Navigation guide (this file) |
| Code Cleanup | docs/ | Code quality tasks |
| TODO | docs/ | Additional maintenance tasks |
| Feature Spec | Root | Office tool specification (O365_TOOL.md) |
| Doc Inventory | Root | Public API documentation tracking |
| Update Tracking | Root | Documentation gap tracking |

---

## Quick Navigation

- **Getting Started:** Start with [QUICKSTART.md](../QUICKSTART.md)
- **Development Guidelines:** See [AGENTS.md](../AGENTS.md)
- **Complete Architecture:** Refer to [SPEC.md](../SPEC.md)
- **Implementation Tasks:** Check [AGENTS_FIX.md](../AGENTS_FIX.md)
- **Project Status:** Read [README.md](../README.md)
- **What's New:** See [RELEASE.md](../RELEASE.md) and [CHANGELOG.md](../CHANGELOG.md)
- **Unfinished Work:** Review [docs/TODO.md](TODO.md) and [docs/CODE_CLEANUP.md](CODE_CLEANUP.md)

---

## Statistics

- **Total Root Documentation Files:** 11 markdown files
- **Documentation Folder Files:** 3 markdown files
- **Total Documentation:** 14 markdown files
- **Specification Completeness:** 1,593 lines (SPEC.md alone)
- **API Documentation Coverage:** 100% (112/112 items documented)
- **Code Documentation Gap:** 1 function (95.7% coverage)

---

*Last updated: 2025-03-10*  
*Version: 0.1.0-alpha.2*
