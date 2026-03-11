# Project Index — Markdown Files

This document lists all markdown files in the project root with a summary of their contents.

---

## Files Summary

### AGENTS.md
**Purpose:** Agent guidelines and development standards for Rust projects
**Key Content:**
- Technology stack (Rust edition 2021+)
- Build, test, and lint commands with timeouts
- Test organization requirements (tests/ directories, naming conventions)
- Unit system specifications (UTC for dates, mm for dimensions, UTF8 for text)
- GitHub access procedures and push workflows
- Changelog and RELEASE.md management guidelines
- Documentation standards (docblocks, module documentation)
- Code style guidelines (4-space indent, 100 max width, snake_case/PascalCase naming)
- Logging requirements (tracing crate, no println!)
- Workflow procedures and task management

---

### AGENTS_FIX.md
**Purpose:** Comprehensive list of incomplete/stub code implementations
**Key Content:**
- Summary of 8 incomplete tasks (3 critical, 3 high, 2 medium priority)
- Detailed task breakdown:
  - **P0 (Critical):** MCP client methods (connect, list_tools, call_tool)
  - **P1 (High):** HTTP abort_session endpoint, TUI agent switching, slash command parsing
  - **P2 (Medium):** CLI session resume and import persistence
- Dependency graph showing task relationships
- Recommended implementation order
- Testing strategy for each task
- Code quality checklist before marking tasks complete
- Implementation notes for MCP, session management, and TUI features

---

### CHANGELOG.md
**Purpose:** Project changelog following Keep a Changelog format
**Key Content:**
- Current version: 0.1.0-alpha.4 (2026-03-11)
- Added features across 4 alpha versions:
  - 8 new tools (multiedit, patch, webfetch, websearch, plan_enter, plan_exit, todo_read, todo_write)
  - Agent delegation system with AgentSwitchRequested/AgentRestoreRequested events
  - Web tools and TODO persistence with SQLite storage
  - AGENTS.md auto-loading on session start
  - Office document and PDF read/write tools
  - TUI improvements (tool call display, home screen, provider setup)
- Fixed issues (compact command errors, line counts, tool result display)
- Changed behaviors (ToolCallArgs event, content_line_count computation)
- Comprehensive changelog since initial project scaffolding (v0.1.0-alpha.0)

---

### DOC_INVENTRY.md
**Purpose:** Inventory of public API functions and types requiring documentation
**Key Content:**
- Documentation coverage: 112 items, 100% documented ✅
- Breakdown by crate:
  - **ragent-core:** 87 documented items across 11 modules
  - **ragent-server:** 3 documented items
  - **ragent-tui:** 22 documented items
- Detailed tables showing each function/type with line numbers
- Status: COMPLETE — all items have documentation and examples

---

### O365_TOOL.md
**Purpose:** Specification for Office document read/write/info tools
**Key Content:**
- Overview of three new tools (office_read, office_write, office_info)
- Recommended Rust crates (docx-rust, calamine, rust_xlsxwriter, ooxmlsdk)
- Architecture with file structure for four new modules
- Tool schemas with parameter definitions for all three tools
- 11-task implementation plan:
  - Add dependencies and create module skeleton
  - Format detection and shared helpers
  - office_read for Word, Excel, PowerPoint
  - office_write for Word, Excel, PowerPoint
  - office_info metadata extraction
  - Tool registration and fixture files
  - Documentation updates
- Output format examples showing expected tool behavior
- Risk mitigation table
- Dependencies summary with version guidance

---

### QUICKSTART.md
**Purpose:** Quick start guide for installing and using ragent
**Key Content:**
- Prerequisites and installation from source
- Provider configuration (Anthropic, OpenAI, Copilot, Ollama)
- First run commands with examples
- Model listing across providers with cost information
- Configuration file format (ragent.json) with merge precedence
- Built-in agents (ask, general, build, plan, explore, title, summary, compaction)
- Available tools (8 built-in + 13 extended tools including office/PDF/web tools)
- Session management (list, resume, export, import)
- HTTP server mode with API endpoints
- Environment variables reference
- Permission system overview
- Common workflows and use cases
- TUI interaction (mouse support, slash commands, key bindings)
- Data storage locations
- Troubleshooting guide

---

### README.md
**Purpose:** Main project overview and introduction
**Key Content:**
- Project description: AI coding agent in Rust inspired by OpenCode
- Feature summary:
  - Multi-provider LLM support (Anthropic, OpenAI, Copilot, Ollama)
  - 21 tools (8 built-in + 13 extended)
  - Terminal UI with home screen and provider setup
  - HTTP server for any frontend
  - Session management with SQLite
  - Permission system with configurable rules
  - Agent presets with tailored prompts
  - AGENTS.md auto-loading for project guidelines
  - MCP client support
  - Snapshot & undo functionality
  - Event bus for real-time updates
- Installation instructions
- Quick start examples
- Usage and command overview
- Configuration file format (OpenCode-compatible)
- Architecture diagram and module overview
- Current project status (v0.1.0-alpha.2)
- License (MIT)

---

### RELEASE.md
**Purpose:** Current release information and version tracking
**Key Content:**
- Current version: 0.1.0-alpha.4
- Added features (since 0.1.0-alpha.3):
  - 8 new tools with full descriptions
  - Agent delegation system components
  - Web tools (webfetch, websearch)
  - TODO persistence with SQLite
  - Storage layer extensions with todos table
  - ToolContext storage handle
  - Event variants for agent switching
  - TUI agent stack management
  - Tool display summaries
  - SSE serialization for new events
  - todo permission rule
- Fixed issues in processor and event matching

---

### SPEC.md
**Purpose:** Comprehensive technical specification and architecture document
**Key Content:**
- Project goals and non-goals (10 goals, 5 non-goals)
- Detailed architecture overview with data flow diagrams
- 16 core modules documented:
  1. CLI & entry point (clap, subcommands, global flags)
  2. Configuration system (file format, load precedence, schema)
  3. Provider system (7 implemented, 4 planned providers with examples)
  4. Agent system (agent definitions, built-in agents, resolution, AGENTS.md)
  5. Session management (lifecycle, persistence, resume/export/import)
  6. Message model (parts-based structure, tool calls, reasoning)
  7. Tool system (trait definition, 21 tools total)
  8. Permission system (rules, evaluation, ask flow)
  9. HTTP server (axum, SSE, route map)
  10. Terminal UI (ratatui, home screen, chat layout, key bindings, mouse)
  11. MCP client (server lifecycle, configuration, tool discovery)
  12. LSP integration (supported languages, capabilities)
  13. Event bus (event types, tokio broadcast)
  14. Storage & database (SQLite schema with 6 tables)
  15. Shell execution (safety features, timeout, kill_on_drop)
  16. Snapshot & undo (per-tool, per-message, per-session levels)
- Data flow documentation with doom loop protection
- Full configuration file format with examples
- Rust crate map (dependencies, purposes)
- Project layout directory structure
- Build & distribution instructions
- Testing strategy (unit, integration, E2E, TUI, fuzzing)
- Future/stretch goals (web UI, mobile, plugins, etc.)

---

### STATS.md
**Purpose:** Project statistics and metrics
**Key Content:**
- Total Rust lines of code: 27,515
- Rust file count: 90 files
- Tests passed: 538
- Tests failed: 0
- Tools implemented: 21

---

### TOOLS_UPDATE.md
**Purpose:** Implementation plan for 8 extended tools
**Key Content:**
- Current state: 13 implemented tools, 8 missing tools
- 8 detailed task specifications:
  - **TASK-T01:** multiedit — atomic multi-file edits ✅ Done
  - **TASK-T02:** patch — unified diff application ✅ Done
  - **TASK-T03:** webfetch — URL content fetching with HTML-to-text ✅ Done
  - **TASK-T04:** websearch — web search with Tavily API ✅ Done
  - **TASK-T07:** todo_read — read session TODOs ✅ Done
  - **TASK-T08:** todo_write — update session TODOs ✅ Done
  - **TASK-T05:** plan_enter — delegate to plan agent ✅ Done
  - **TASK-T06:** plan_exit — return from plan agent ✅ Done
- Implementation order with priority and dependencies
- Per-tool checklist (trait implementation, registration, tests, TUI display)
- All 8 tools marked as completed ✅

---

### UPDATE_DOCS.md
**Purpose:** Documentation update requirements for public functions
**Key Content:**
- Documentation coverage status: 1 missing docblock out of 23 functions (95.7%)
- Single remediation task:
  - Add docblock to `redact_secrets()` in crates/ragent-core/src/sanitize.rs
  - Includes suggested docblock template with explanation and examples
- Current status: Not started
- Priority: Low (only 1 function)
- All other public functions documented

---

## Quick Navigation

| File | Purpose | Audience |
|------|---------|----------|
| **AGENTS.md** | Development guidelines | Developers, CI/CD |
| **AGENTS_FIX.md** | Incomplete tasks | Project manager, developers |
| **CHANGELOG.md** | Version history | Users, release manager |
| **DOC_INVENTRY.md** | API documentation status | Documentation team |
| **O365_TOOL.md** | Office tools specification | Developers implementing office tools |
| **QUICKSTART.md** | User guide | New users |
| **README.md** | Project overview | Everyone |
| **RELEASE.md** | Current release info | Release manager, users |
| **SPEC.md** | Technical specification | Architects, developers |
| **STATS.md** | Code metrics | Project manager |
| **TOOLS_UPDATE.md** | Tool implementation plan | Tool developers |
| **UPDATE_DOCS.md** | Documentation gaps | Documentation team |

---

**Last Updated:** 2026-03-11  
**Total Files:** 12 markdown files in project root
