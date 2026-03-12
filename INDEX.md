# Documentation Index

An overview of all documentation files in the ragent project.

---

## Root Level Documentation

### [AGENTS.md](AGENTS.md)
Guidelines for AI agents and developers working with the ragent project. Covers:
- Technology stack and Rust edition requirements
- Build and test commands with timeout specifications
- Test organization and naming conventions  
- Code style guidelines (4 spaces, max 100 width, snake_case naming)
- Linting and formatting with clippy and rustfmt
- Unit standards (UTC dates, millimeters for dimensions, UTF8 encoding)
- GitHub workflow and version control practices
- Changelog management using Keep a Changelog format
- Semantic versioning with alpha suffix during development
- Documentation standards for functions, modules, and markdown files
- Workflow for handling "whats next" task lists
- Priority levels for issues (P0-P4)

**Status:** Primary reference document for the project

---

### [AGENTS_FIX.md](AGENTS_FIX.md)
Comprehensive task list of incomplete and stub implementations across the codebase. Contains:
- Summary table of tasks by priority (3 critical, 3 high, 2 medium)
- Detailed breakdown of 8 unimplemented tasks:
  - **TASK-001 to TASK-003**: MCP Client implementation (connect, list_tools, call_tool)
  - **TASK-004**: HTTP Server abort_session endpoint
  - **TASK-005**: TUI agent switching functionality
  - **TASK-006**: Slash command parsing and dispatch
  - **TASK-007**: Session resume functionality
  - **TASK-008**: Session import persistence
- Dependency graph showing task relationships
- Recommended implementation order
- Testing strategy for each task type
- Code quality checklist
- Implementation notes for complex features (MCP, session management, TUI)
- Related documentation references

**Status:** Active tracking document for incomplete features

---

### [CHANGELOG.md](CHANGELOG.md)
Complete history of project releases and changes. Includes:
- Version 0.1.0-alpha.7 (latest): rm tool, /tools command enhancements, SPEC.md expansion
- Version 0.1.0-alpha.6: Office document tool display summaries, UTF-8 bug fix
- Version 0.1.0-alpha.5: create tool, slash command headers, wrapped-line fixes
- Version 0.1.0-alpha.4: multiedit, patch, webfetch, websearch, plan agent delegation, todo management
- Version 0.1.0-alpha.3: AGENTS.md auto-loading, init exchange, tool display improvements, INDEX.md
- Version 0.1.0-alpha.2: provider reset, Copilot token persistence, device flow auth
- Version 0.1.0-alpha.1: TUI home screen, provider setup, agent cycling, ask agent, settings persistence
- Version 0.1.0-alpha.0: Initial project scaffolding with workspace structure

**Format:** Keep a Changelog compatible with semantic versioning

---

### [DOC_INVENTRY.md](DOC_INVENTRY.md)
Documentation audit and inventory of public API items:
- Summary: 112 total items, all documented (100% coverage)
- Detailed table tracking all public functions and types across crates:
  - ragent-core: 87 items (agent, config, event, mcp, message, permission, provider, session, snapshot, storage, tool modules)
  - ragent-server: 3 items (HTTP server routes and SSE)
  - ragent-tui: 22 items (app, input, layout, widgets, tips)
- Each item shows: name, line number, item type, documentation status, examples status
- All items marked with ✅ for complete documentation

**Status:** Complete — all public APIs documented

---

### [O365_TOOL.md](O365_TOOL.md)
Specification for Microsoft Office document read/write tools. Covers:
- Goals: read/write Word (.docx), Excel (.xlsx), PowerPoint (.pptx) files
- Recommended Rust crates (docx-rust, calamine, rust_xlsxwriter, ooxmlsdk)
- Architecture: three new tools (office_read, office_write, office_info)
- Tool schemas for parameters and return values
- Implementation plan with 11 tasks (TASK-001 to TASK-011):
  - Module setup and dependencies
  - Format detection helpers
  - Per-format read/write implementations
  - Integration testing with fixtures
- Output format examples for Word, Excel, and PowerPoint
- Risk assessment and mitigation strategies
- Dependency specifications with versions

**Status:** Implementation specification (tools already partially implemented)

---

### [QUICKSTART.md](QUICKSTART.md)
User-facing quick start guide for installing and using ragent:
- Prerequisites and installation from source
- Configuration via interactive TUI or environment variables
- Provider setup options (Anthropic, OpenAI, Copilot, Ollama)
- Default models and context windows for each provider
- Configuration file format and load precedence
- Built-in agents (ask, general, build, plan, explore, internal utilities)
- Project guidelines via AGENTS.md auto-loading
- Available tools with permission categories
- Session management (list, resume, export, import)
- HTTP server mode and API endpoints
- Environment variables reference
- Permission system and rule configuration
- Common workflows (code review, refactoring, local Ollama, project setup)
- TUI interaction (mouse support, slash commands, key bindings)
- Data storage locations and troubleshooting guide

**Status:** Primary user guide

---

### [README.md](README.md)
Project overview and introduction:
- Brief description: AI coding agent for the terminal, built in Rust
- Reimplementation of OpenCode as a learning exercise
- Key features: multi-provider LLM, 23 built-in/extended tools, TUI, HTTP server, sessions, permissions, agents, project guidelines, MCP, snapshot/undo, event bus
- Installation from source with Rust 1.85+ requirement
- Quick start examples (configure key, launch TUI, one-shot prompt, HTTP server)
- Usage summary with subcommands
- Configuration file format and examples
- Architecture diagram showing component relationships
- Project status: v0.1.0-alpha.2 (early development)
- License: MIT

**Status:** Primary project introduction

---

### [RELEASE.md](RELEASE.md)
Latest release notes summary:
- Current Version: 0.1.0-alpha.7
- Summary of changes since 0.1.0-alpha.6:
  - `rm` tool for file deletion
  - `/tools` command enhancements
  - SPEC.md expansion with new sections
  - Feature comparison with competitor tools
- Tool count: 23
- SPEC.md line count: ~2168

**Status:** Used for GitHub Releases page descriptions

---

### [SPEC.md](SPEC.md)
Comprehensive system specification document (2168+ lines). Includes:
- Goals & non-goals (G1-G10 goals, N1-N5 non-goals)
- Architecture overview with component diagram
- Core modules (10 major sections):
  1. CLI & entry point (subcommands, global flags)
  2. Configuration (file format, load precedence, schema)
  3. Provider system (supported providers, streaming interface, Ollama, Copilot)
  4. Agent system (definition, built-in agents, tool groups, orchestrator, AGENTS.md loading)
  5. Session management (lifecycle, resumption, archiving)
  6. Message model (parts-based structure)
  7. Tool system (registry, 23 built-in tools, execution flow)
  8. Permission system (rules, evaluation order, ask flow)
  9. HTTP server (REST + SSE routes, event types)
  10. Terminal UI (home screen, provider setup, chat layout, tool display, slash commands, context compaction)
- Advanced features (sections 3.11-3.28):
  - MCP client, LSP integration, event bus
  - Storage & database schema
  - Shell execution and safety
  - Snapshot & undo with shadow git
  - Hooks, custom agents, skills
  - Persistent memory, trusted directories
  - Codebase indexing & semantic search
  - Post-edit diagnostics, task todos
  - Prompt enhancement, hierarchical instructions
  - File ignore patterns (.ragentignore)
  - Suggested responses
- Data flow diagram
- Configuration file format (minimal and full examples)
- Rust crate map
- Project layout
- Build & distribution
- Testing strategy
- Future goals (F1-F20)

**Status:** Living specification document, frequently updated

---

### [STATS.md](STATS.md)
Project statistics snapshot:
- Total Rust lines: 27,812
- Rust file count: 92
- Tests passed: 538
- Tests failed: 0
- Tools: 23

**Status:** Summary metrics file

---

### [TOOLS_UPDATE.md](TOOLS_UPDATE.md)
Implementation plan for 8 originally-missing tools (now complete):
- Current state: 13 implemented tools (mostly complete now)
- Missing tools tracker (all marked as done):
  - TASK-T01: `multiedit` ✅
  - TASK-T02: `patch` ✅
  - TASK-T03: `webfetch` ✅
  - TASK-T04: `websearch` ✅
  - TASK-T05: `plan_enter` ✅
  - TASK-T06: `plan_exit` ✅
  - TASK-T07: `todo_read` ✅
  - TASK-T08: `todo_write` ✅
- Per-tool implementation checklist
- Tool registration and TUI display requirements
- SPEC.md and CHANGELOG.md update requirements

**Status:** Completed — all 8 tools now implemented

---

### [UPDATE_DOCS.md](UPDATE_DOCS.md)
Documentation remediation tracking:
- Audit of public API documentation coverage
- Found 1 function without proper docblock:
  - `redact_secrets()` in `crates/ragent-core/src/sanitize.rs`
- Suggested docblock template
- Overall documentation coverage: 95.7% (112 functions documented)

**Status:** Minimal remediation needed

---

### [WORD_SUM.md](WORD_SUM.md)
Test summary of a sample Word document (testword1.docx):
- Document topic: Cloudflare Browser Rendering crawl endpoint
- Main features: URL submission, site crawl, multiple output formats
- How it works: 4-step process with async job handling
- Key capabilities: multiple output formats, crawl scope controls, page discovery, incremental crawling, bot etiquette
- Use cases: ML training, RAG pipelines, website monitoring
- Availability: Free and Paid plans
- API example: curl commands for crawl initiation and result retrieval

**Status:** Sample documentation test file

---

## Documentation in Subdirectories

The following documentation files reside in the `docs/` folder (per AGENTS.md guidelines):

- **docs/INDEX.md** — Documentation index for the docs folder
- **docs/TODO.md** — Unimplemented functions and feature tracking
- **docs/CODE_CLEANUP.md** — Code quality improvements (separate from missing features)

---

## How to Use This Index

1. **Getting Started?** → Read [QUICKSTART.md](QUICKSTART.md) first
2. **Understanding the Architecture?** → Read [SPEC.md](SPEC.md) (or sections 1–3)
3. **Contributing Code?** → Read [AGENTS.md](AGENTS.md) for style and workflow
4. **Implementing a Feature?** → Check [AGENTS_FIX.md](AGENTS_FIX.md) for pending tasks
5. **Building Tools?** → Consult [TOOLS_UPDATE.md](TOOLS_UPDATE.md) for tool patterns
6. **Writing Docs?** → Check [UPDATE_DOCS.md](UPDATE_DOCS.md) and [DOC_INVENTRY.md](DOC_INVENTRY.md)
7. **Tracking Changes?** → See [CHANGELOG.md](CHANGELOG.md) and [RELEASE.md](RELEASE.md)
8. **Project Metrics?** → View [STATS.md](STATS.md)

---

## File Summary Table

| File | Purpose | Audience | Status |
|------|---------|----------|--------|
| [AGENTS.md](AGENTS.md) | Developer guidelines & standards | Developers | Reference |
| [AGENTS_FIX.md](AGENTS_FIX.md) | Unimplemented tasks & stubs | Developers | Active |
| [CHANGELOG.md](CHANGELOG.md) | Release history | All | Maintained |
| [DOC_INVENTRY.md](DOC_INVENTRY.md) | API documentation audit | Developers | Complete |
| [O365_TOOL.md](O365_TOOL.md) | Office tools specification | Developers | Specification |
| [QUICKSTART.md](QUICKSTART.md) | Installation & usage guide | Users | Primary |
| [README.md](README.md) | Project introduction | All | Primary |
| [RELEASE.md](RELEASE.md) | Latest release notes | Users | Current |
| [SPEC.md](SPEC.md) | System specification | Developers | Living |
| [STATS.md](STATS.md) | Project metrics | All | Summary |
| [TOOLS_UPDATE.md](TOOLS_UPDATE.md) | Tool implementation tracker | Developers | Completed |
| [UPDATE_DOCS.md](UPDATE_DOCS.md) | Documentation gaps | Developers | Near-complete |
| [WORD_SUM.md](WORD_SUM.md) | Test document summary | Testing | Sample |

---

**Last Updated:** 2026-03-11  
**Total Root-Level Markdown Files:** 14  
**Total Documentation Coverage:** ~8500 lines across all files
