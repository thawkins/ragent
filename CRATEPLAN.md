# Crate Reorganization Plan

## Executive Summary

**Current State:** 6 crates totaling ~119,462 lines of code
- `ragent-core`: 64,909 lines (54% of codebase) — **TOO LARGE**
- `ragent-tui`: 26,786 lines
- `ragent-codeindex`: 12,991 lines
- `ragent-aiwiki`: 11,522 lines
- `ragent-server`: 2,640 lines
- `ragent-prompt_opt`: 614 lines

**Problem:** `ragent-core` is a monolithic kitchen-sink crate containing:
- 100 tool implementations (24,630 lines, 38% of ragent-core)
- 8 LLM provider implementations (6,534 lines)
- 26 distinct subsystems mixed together
- High interdependency and difficult navigation

**Goal:** Split `ragent-core` into 10-12 focused crates organized by responsibility, reducing crate sizes to 3,000-8,000 lines each, improving modularity and maintainability.

---

## Current Crate Analysis

### ragent-core (64,909 lines) — NEEDS SPLIT
**26 subsystems mixed together:**

| Subsystem | Lines | Description |
|-----------|-------|-------------|
| `tool/` | 24,630 | 100 tool implementations |
| `provider/` | 6,534 | 8 LLM providers (Anthropic, OpenAI, Gemini, Ollama, HuggingFace, Copilot, Generic) |
| `memory/` | 6,558 | Memory blocks, embeddings, structured storage |
| `skill/` | 3,561 | Skill loading, argument substitution |
| `session/` | 3,443 | Session processor, conversation loop |
| `team/` | 2,762 | Team coordination, mailboxes, tasks |
| `storage/` | 2,490 | SQLite persistence layer |
| `agent/` | 2,141 | Agent definitions, OASF loader |
| `orchestrator/` | 1,573 | Multi-tool orchestration |
| `mcp/` | 1,391 | Model Context Protocol client |
| `config/` | 1,170 | Config loading, merging |
| `lsp/` | 1,145 | Language Server Protocol client |
| `reference/` | 1,111 | @ file reference parsing |
| `task/` | 783 | Sub-agent task management |
| `event/` | 746 | Event bus |
| `message/` | 437 | Message types |
| `hooks/` | 437 | Lifecycle hooks |
| `gitlab/` | 443 | GitLab API client |
| `permission/` | 381 | Permission rules |
| `file_ops/` | 362 | File operation utilities |
| `github/` | 356 | GitHub API client |
| `snapshot/` | 328 | File snapshotting |
| `llm/` | 202 | LLM traits |
| `updater/` | 186 | Auto-update logic |
| Root files | ~5,000 | bash_lists, dir_lists, error, id, resource, sanitize, yolo, predictive, intern |

### Other Crates (Analysis)

**ragent-tui (26,786 lines)** — OK size, but could benefit from extraction:
- TUI rendering, input handling, layout (~18,000 lines)
- Markdown rendering (~3,000 lines) — could extract
- App state management (~5,000 lines)

**ragent-codeindex (12,991 lines)** — Good size, well-focused
- Tree-sitter parsing, Tantivy indexing, file watching

**ragent-aiwiki (11,522 lines)** — Good size, well-focused
- Wiki knowledge base, extraction, web interface

**ragent-server (2,640 lines)** — Good size, well-focused
- HTTP/SSE API, REST endpoints

**ragent-prompt_opt (614 lines)** — Good size, well-focused
- Prompt optimization frameworks

---

## Proposed New Crate Structure

### Target: 13 crates (from 6)

```
crates/
├── ragent-types/          NEW — Core types and traits (2,500 lines)
├── ragent-config/         NEW — Configuration system (1,500 lines)
├── ragent-storage/        NEW — SQLite persistence (2,800 lines)
├── ragent-llm/            NEW — LLM providers (7,500 lines)
├── ragent-tools-core/     NEW — Essential file/shell tools (8,000 lines)
├── ragent-tools-extended/ NEW — Document/web/specialized tools (10,000 lines)
├── ragent-tools-vcs/      NEW — Git/GitHub/GitLab tools (3,000 lines)
├── ragent-agent/          NEW — Agent orchestration & session (12,000 lines)
├── ragent-team/           NEW — Team coordination (3,500 lines)
├── ragent-codeindex/           KEEP — Codebase indexing (12,991 lines)
├── ragent-server/         KEEP — HTTP/SSE server (2,640 lines)
├── ragent-tui/            KEEP — Terminal interface (26,786 lines)
├── ragent-aiwiki/       KEEP — Wiki knowledge base (11,522 lines)
└── ragent-prompt_opt/     KEEP — Prompt optimization (614 lines)
```

**Total: 13 crates, average ~7,900 lines per crate**

---

## Crate Details & Responsibilities

### 1. ragent-types (NEW, ~2,500 lines)
**Foundation crate — no dependencies on other ragent crates**

**Contents:**
- Core types: `Message`, `ContentPart`, `ToolCall`, `ToolResult`
- Error types: `RagentError`, `ToolError`, `ProviderError`
- ID generation: `ConversationId`, `MessageId`, `SessionId`
- Event bus: `Event`, `EventBus`, `EventSubscriber`
- Traits: `Tool`, `Provider`, `Storage`
- Resource limits: `ProcessResourceManager`
- Utility modules: `sanitize`, `intern`

**Dependencies:** tokio, serde, anyhow, thiserror, uuid

**Why:** Provides shared types that all other crates depend on. No circular dependencies.

---

### 2. ragent-config (NEW, ~1,500 lines)
**Configuration loading and merging**

**Contents:**
- Config loading from `ragent.json` / `ragent.jsonc`
- Config merging (global + project + CLI overrides)
- Permission rules: `PermissionRule`, `PermissionChecker`
- Runtime lists: `bash_lists`, `dir_lists`
- YOLO mode configuration

**Dependencies:** ragent-types, serde, globset, dirs

**Why:** Configuration is a cross-cutting concern used by all subsystems but is logically independent.

---

### 3. ragent-storage (NEW, ~2,800 lines)
**SQLite persistence layer**

**Contents:**
- Session storage: conversations, messages, history
- Memory storage: memory blocks, structured memories, embeddings
- Journal storage: journal entries, tags, FTS5
- Snapshot storage: file snapshots, diffs
- Team storage: team state, mailboxes, tasks

**Dependencies:** ragent-types, rusqlite, tokio, anyhow

**Why:** Database schema and queries are a distinct concern. All persistence logic in one place.

---

### 4. ragent-llm (NEW, ~7,500 lines)
**LLM provider implementations**

**Contents:**
- Provider trait: `LlmProvider` (from ragent-types)
- 8 provider implementations:
  - `anthropic.rs` (455 lines)
  - `openai.rs` (491 lines)
  - `gemini.rs` (503 lines)
  - `ollama.rs` (727 lines)
  - `ollama_cloud.rs` (775 lines)
  - `huggingface.rs` (1,171 lines)
  - `copilot.rs` (1,876 lines)
  - `generic_openai.rs` (56 lines)
- HTTP client utilities: `http_client.rs` (209 lines)
- Provider registry and discovery
- Model metadata and capabilities

**Dependencies:** ragent-types, ragent-config, reqwest, tokio, serde

**Why:** LLM providers are a self-contained subsystem. Can be independently versioned and tested.

---

### 5. ragent-tools-core (NEW, ~8,000 lines)
**Essential file, shell, and search tools**

**17 file operation tools (~5,000 lines):**
- `read.rs`, `write.rs`, `create.rs`, `append_to_file.rs`
- `edit.rs`, `multiedit.rs`, `patch.rs`, `str_replace_editor.rs`
- `copy_file.rs`, `move_file.rs`, `rm.rs`, `mkdir.rs`
- `file_info.rs`, `diff.rs`, `truncate.rs`
- `glob.rs`, `list.rs`

**Shell tools (~1,500 lines):**
- `bash.rs` (862 lines)
- `bash_reset.rs`

**Search tools (~500 lines):**
- `grep.rs` (312 lines)
- `search.rs`

**Other core tools (~1,000 lines):**
- `ask_user.rs` / `question.rs`
- `task_complete.rs`
- `think.rs`
- `get_env.rs`
- `calculator.rs`

**Dependencies:** ragent-types, ragent-config, ragent-storage, tokio, glob, grep-regex

**Why:** These are the most frequently used tools. Keeping them together reduces compilation cascades.

---

### 6. ragent-tools-extended (NEW, ~10,000 lines)
**Document, web, and specialized tools**

**Document tools (~5,500 lines):**
- `pdf_read.rs` (314 lines), `pdf_write.rs` (605 lines)
- `office_read.rs` (738 lines), `office_write.rs` (1,009 lines), `office_info.rs` (307 lines)
- `libreoffice_read.rs` (307 lines), `libreoffice_write.rs` (653 lines)

**Web tools (~800 lines):**
- `webfetch.rs` (212 lines)
- `websearch.rs`
- `http_request.rs`

**Memory & journal tools (~1,500 lines):**
- `memory_write.rs` (581 lines), `memory_read.rs`, `memory_search.rs`, `memory_store.rs`, `memory_recall.rs`, `memory_forget.rs`, `memory_replace.rs`, `memory_migrate.rs`
- `journal.rs` (361 lines)

**Todo tools:**
- `todo.rs` (373 lines)

**AIWiki tools (~800 lines):**
- `aiwiki_search.rs`, `aiwiki_status.rs` (315 lines), `aiwiki_ingest.rs`, `aiwiki_export.rs`, `aiwiki_import.rs`

**Codeindex tools (~1,200 lines):**
- `codeindex_search.rs` (270 lines), `codeindex_status.rs`, `codeindex_symbols.rs`, `codeindex_references.rs`, `codeindex_dependencies.rs`, `codeindex_reindex.rs`

**LSP tools (~600 lines):**
- `lsp_hover.rs`, `lsp_definition.rs`, `lsp_references.rs`, `lsp_symbols.rs`, `lsp_diagnostics.rs`

**Dependencies:** ragent-types, ragent-config, ragent-storage, ragent-codeindex, ragent-aiwiki, docx-rust, pdf-extract, lopdf, calamine, rust_xlsxwriter, spreadsheet-ods, html2text, reqwest

**Why:** These tools have heavy external dependencies (PDF, Office formats, web). Isolating them speeds up builds when core tools change.

---

### 7. ragent-tools-vcs (NEW, ~3,000 lines)
**Version control system integration**

**GitHub tools (~1,500 lines):**
- `github_issues.rs` (457 lines)
- `github_prs.rs` (421 lines)
- GitHub API client (356 lines)

**GitLab tools (~1,500 lines):**
- `gitlab_issues.rs` (445 lines)
- `gitlab_mrs.rs` (425 lines)
- `gitlab_pipelines.rs` (713 lines)
- GitLab API client (443 lines)

**Dependencies:** ragent-types, ragent-config, reqwest, serde

**Why:** VCS tools are optional and rarely used. Splitting them out reduces dependency bloat.

---

### 8. ragent-agent (NEW, ~12,000 lines)
**Agent orchestration, session management, and execution**

**Contents:**
- Agent definitions: `agent/mod.rs` (2,141 lines)
  - Built-in agents: coder, task, architect, debug, code-review
  - OASF loader for custom agents
- Session processor: `session/mod.rs` (3,443 lines)
  - Conversation loop
  - Tool call execution
  - Permission checking
  - Streaming response handling
- Orchestrator: `orchestrator/mod.rs` (1,573 lines)
  - Multi-tool call coordination
  - Parallel execution
  - Result aggregation
- Task management: `task/mod.rs` (783 lines)
  - Sub-agent spawning
  - Background task tracking
- Skill system: `skill/mod.rs` (3,561 lines)
  - Skill loading and argument substitution
- Reference resolution: `reference/mod.rs` (1,111 lines)
  - @ file reference parsing and fuzzy matching
- Hooks: `hooks/mod.rs` (437 lines)
  - Lifecycle hook execution
- MCP client: `mcp/mod.rs` (1,391 lines)
  - Model Context Protocol integration
- LSP client: `lsp/mod.rs` (1,145 lines)
  - Language Server Protocol integration
- Tool registry: `tool/mod.rs` (650 lines)
  - Tool discovery and dispatch

**Dependencies:** ragent-types, ragent-config, ragent-storage, ragent-llm, ragent-tools-core, ragent-tools-extended, ragent-tools-vcs, tokio, dashmap

**Why:** This is the "brain" of the agent — orchestration, execution flow, and integration. It depends on all tool crates but doesn't contain tool implementations.

---

### 9. ragent-team (NEW, ~3,500 lines)
**Team coordination and collaboration**

**Contents:**
- Team management: `team/mod.rs` (2,762 lines)
  - Team creation, status, cleanup
  - Mailbox messaging
  - Task list (claim/complete/create)
  - Teammate spawning and lifecycle
- Team tools (~700 lines):
  - `team_spawn.rs` (376 lines)
  - `team_create.rs` (502 lines)
  - `team_message.rs`, `team_broadcast.rs`, `team_read_messages.rs`
  - `team_task_claim.rs`, `team_task_complete.rs`, `team_task_create.rs`, `team_task_list.rs`
  - `team_status.rs`, `team_wait.rs`, `team_cleanup.rs`
  - `team_memory_read.rs`, `team_memory_write.rs`

**Dependencies:** ragent-types, ragent-config, ragent-storage, ragent-agent, tokio

**Why:** Teams are a self-contained feature. Separating them allows teams to evolve independently.

---

### 10-13. KEEP Existing Crates

**ragent-codeindex (12,991 lines)** — No changes
- Codebase indexing, tree-sitter parsing, Tantivy FTS

**ragent-server (2,640 lines)** — No changes
- HTTP/SSE API, REST endpoints

**ragent-tui (26,786 lines)** — No changes
- Terminal UI, input handling, rendering

**ragent-aiwiki (11,522 lines)** — No changes
- Wiki knowledge base, extraction, web interface

**ragent-prompt_opt (614 lines)** — No changes
- Prompt optimization frameworks

---

## Dependency Graph (Proposed)

```
ragent-types (foundation, no ragent deps)
    ↓
ragent-config → ragent-types
    ↓
ragent-storage → ragent-types
    ↓
ragent-llm → ragent-types, ragent-config
    ↓
ragent-tools-core → ragent-types, ragent-config, ragent-storage
ragent-tools-extended → ragent-types, ragent-config, ragent-storage, ragent-codeindex, aiwiki
ragent-tools-vcs → ragent-types, ragent-config
    ↓
ragent-agent → ALL of the above (orchestrator/brain)
    ↓
ragent-team → ragent-types, ragent-config, ragent-storage, ragent-agent
    ↓
ragent-server → ragent-agent, ragent-team, ragent-prompt_opt
ragent-tui → ragent-agent, ragent-team, ragent-server, ragent-codeindex, aiwiki
```

**Key properties:**
- **No circular dependencies**
- **Clear layering:** types → config → storage → providers/tools → orchestration → UI/API
- **Tool crates are peers:** tools-core, tools-extended, and tools-vcs don't depend on each other

---

## Migration Strategy

### Milestone 1: Foundation Layer (1-2 days)
**Create type and config crates**

**Tasks:**
1. Create `ragent-types` crate with Cargo.toml
2. Move core types from ragent-core to ragent-types:
   - `message/mod.rs` → `ragent-types/src/message.rs`
   - `error.rs` → `ragent-types/src/error.rs`
   - `id.rs` → `ragent-types/src/id.rs`
   - `event/mod.rs` → `ragent-types/src/event.rs`
   - `resource.rs` → `ragent-types/src/resource.rs`
   - `sanitize.rs` → `ragent-types/src/sanitize.rs`
   - `intern.rs` → `ragent-types/src/intern.rs`
   - `llm/mod.rs` (traits only) → `ragent-types/src/llm.rs`
3. Create `ragent-config` crate
4. Move config modules from ragent-core to ragent-config:
   - `config/mod.rs` → `ragent-config/src/config.rs`
   - `permission/mod.rs` → `ragent-config/src/permission.rs`
   - `bash_lists.rs` → `ragent-config/src/bash_lists.rs`
   - `dir_lists.rs` → `ragent-config/src/dir_lists.rs`
   - `yolo.rs` → `ragent-config/src/yolo.rs`
5. Update ragent-core dependencies: add `ragent-types` and `ragent-config`
6. Fix imports across ragent-core
7. Test: `cargo test -p ragent-types -p ragent-config`

**Success Criteria:**
- ✅ ragent-types and ragent-config compile independently
- ✅ ragent-core compiles with new dependencies
- ✅ All tests pass

---

### Milestone 2: Storage Layer (1 day)
**Extract persistence logic**

**Tasks:**
1. Create `ragent-storage` crate with Cargo.toml
2. Move storage module from ragent-core to ragent-storage:
   - `storage/mod.rs` → `ragent-storage/src/lib.rs`
   - Split into submodules: `session.rs`, `memory.rs`, `journal.rs`, `snapshot.rs`, `team.rs`
3. Move snapshot module:
   - `snapshot/mod.rs` → `ragent-storage/src/snapshot.rs`
4. Update ragent-core dependencies: add `ragent-storage`
5. Fix imports across ragent-core
6. Test: `cargo test -p ragent-storage`

**Success Criteria:**
- ✅ ragent-storage compiles independently
- ✅ All storage tests pass

---

### Milestone 3: LLM Provider Layer (1-2 days)
**Extract provider implementations**

**Tasks:**
1. Create `ragent-llm` crate with Cargo.toml
2. Move provider modules from ragent-core to ragent-llm:
   - `provider/*.rs` → `ragent-llm/src/providers/`
   - Keep `mod.rs` as registry
3. Update ragent-core dependencies: add `ragent-llm`
4. Fix imports across ragent-core
5. Test: `cargo test -p ragent-llm`

**Success Criteria:**
- ✅ ragent-llm compiles independently
- ✅ All provider tests pass
- ✅ All providers accessible from ragent-agent

---

### Milestone 4: Core Tools Layer (2-3 days)
**Extract essential file and shell tools**

**Tasks:**
1. Create `ragent-tools-core` crate with Cargo.toml
2. Move core tool implementations from ragent-core/src/tool/ to ragent-tools-core/src/:
   - File operations: `read.rs`, `write.rs`, `create.rs`, `edit.rs`, `multiedit.rs`, `patch.rs`, `str_replace_editor.rs`, `copy_file.rs`, `move_file.rs`, `rm.rs`, `mkdir.rs`, `append_to_file.rs`, `file_info.rs`, `diff.rs`, `truncate.rs`
   - Search: `glob.rs`, `list.rs`, `grep.rs`, `search.rs`
   - Shell: `bash.rs`, `bash_reset.rs`
   - Misc: `ask_user.rs`, `question.rs`, `task_complete.rs`, `think.rs`, `get_env.rs`, `calculator.rs`
3. Create tool registry in ragent-tools-core: `registry.rs`
4. Update ragent-core dependencies: add `ragent-tools-core`
5. Update tool/mod.rs in ragent-core to import from ragent-tools-core
6. Test: `cargo test -p ragent-tools-core`

**Success Criteria:**
- ✅ ragent-tools-core compiles independently
- ✅ All core tool tests pass
- ✅ Tools accessible from ragent-agent

---

### Milestone 5: Extended Tools Layer (2-3 days)
**Extract document, web, and specialized tools**

**Tasks:**
1. Create `ragent-tools-extended` crate with Cargo.toml
2. Move extended tool implementations from ragent-core/src/tool/ to ragent-tools-extended/src/:
   - Documents: `pdf_read.rs`, `pdf_write.rs`, `office_read.rs`, `office_write.rs`, `office_info.rs`, `libreoffice_read.rs`, `libreoffice_write.rs`
   - Web: `webfetch.rs`, `websearch.rs`, `http_request.rs`
   - Memory: `memory_write.rs`, `memory_read.rs`, `memory_search.rs`, `memory_store.rs`, `memory_recall.rs`, `memory_forget.rs`, `memory_replace.rs`, `memory_migrate.rs`
   - Journal: `journal.rs` (journal_read, journal_write, journal_search)
   - Todo: `todo.rs` (todo_read, todo_write)
   - AIWiki: `aiwiki_search.rs`, `aiwiki_status.rs`, `aiwiki_ingest.rs`, `aiwiki_export.rs`, `aiwiki_import.rs`
   - Codeindex: `codeindex_search.rs`, `codeindex_status.rs`, `codeindex_symbols.rs`, `codeindex_references.rs`, `codeindex_dependencies.rs`, `codeindex_reindex.rs`
   - LSP: `lsp_hover.rs`, `lsp_definition.rs`, `lsp_references.rs`, `lsp_symbols.rs`, `lsp_diagnostics.rs`
3. Create tool registry in ragent-tools-extended: `registry.rs`
4. Update ragent-core dependencies: add `ragent-tools-extended`
5. Test: `cargo test -p ragent-tools-extended`

**Success Criteria:**
- ✅ ragent-tools-extended compiles independently
- ✅ All extended tool tests pass

---

### Milestone 6: VCS Tools Layer (1 day)
**Extract GitHub and GitLab tools**

**Tasks:**
1. Create `ragent-tools-vcs` crate with Cargo.toml
2. Move VCS modules from ragent-core to ragent-tools-vcs:
   - `github/mod.rs` → `ragent-tools-vcs/src/github/mod.rs`
   - `gitlab/mod.rs` → `ragent-tools-vcs/src/gitlab/mod.rs`
   - `tool/github_issues.rs`, `tool/github_prs.rs` → `ragent-tools-vcs/src/github/`
   - `tool/gitlab_issues.rs`, `tool/gitlab_mrs.rs`, `tool/gitlab_pipelines.rs` → `ragent-tools-vcs/src/gitlab/`
3. Create tool registry in ragent-tools-vcs: `registry.rs`
4. Update ragent-core dependencies: add `ragent-tools-vcs`
5. Test: `cargo test -p ragent-tools-vcs`

**Success Criteria:**
- ✅ ragent-tools-vcs compiles independently
- ✅ All VCS tool tests pass

---

### Milestone 7: Agent Orchestration Layer (2-3 days)
**Create unified agent crate**

**Tasks:**
1. Create `ragent-agent` crate with Cargo.toml
2. Move orchestration modules from ragent-core to ragent-agent:
   - `agent/` → `ragent-agent/src/agent/`
   - `session/` → `ragent-agent/src/session/`
   - `orchestrator/` → `ragent-agent/src/orchestrator/`
   - `task/` → `ragent-agent/src/task/`
   - `skill/` → `ragent-agent/src/skill/`
   - `reference/` → `ragent-agent/src/reference/`
   - `hooks/` → `ragent-agent/src/hooks/`
   - `mcp/` → `ragent-agent/src/mcp/`
   - `lsp/` → `ragent-agent/src/lsp/`
   - `tool/mod.rs` (registry only) → `ragent-agent/src/tool/mod.rs`
   - `updater/` → `ragent-agent/src/updater/`
   - `file_ops/` → `ragent-agent/src/file_ops/`
   - `predictive.rs` → `ragent-agent/src/predictive.rs`
3. Update ragent-agent dependencies: ALL tool crates, ragent-llm, ragent-storage, ragent-config, ragent-types
4. Test: `cargo test -p ragent-agent`

**Success Criteria:**
- ✅ ragent-agent compiles independently
- ✅ All orchestration tests pass
- ✅ Session processor works end-to-end

---

### Milestone 8: Team Layer (1 day)
**Extract team coordination**

**Tasks:**
1. Create `ragent-team` crate with Cargo.toml
2. Move team modules from ragent-core to ragent-team:
   - `team/` → `ragent-team/src/team/`
   - Team tools from `tool/team_*.rs` → `ragent-team/src/tools/`
3. Update ragent-team dependencies: ragent-types, ragent-config, ragent-storage, ragent-agent
4. Test: `cargo test -p ragent-team`

**Success Criteria:**
- ✅ ragent-team compiles independently
- ✅ All team tests pass

---

### Milestone 9: Update Dependent Crates (2 days)
**Update server and TUI to use new crate structure**

**Tasks:**
1. Update `ragent-server/Cargo.toml` dependencies:
   - Remove: ragent-core
   - Add: ragent-types, ragent-config, ragent-agent, ragent-team
2. Fix imports in ragent-server
3. Test: `cargo test -p ragent-server`
4. Update `ragent-tui/Cargo.toml` dependencies:
   - Remove: ragent-core
   - Add: ragent-types, ragent-config, ragent-agent, ragent-team, ragent-storage
5. Fix imports in ragent-tui
6. Test: `cargo test -p ragent-tui`
7. Update root `Cargo.toml` workspace members

**Success Criteria:**
- ✅ ragent-server compiles and all tests pass
- ✅ ragent-tui compiles and all tests pass
- ✅ Full workspace builds: `cargo build`

---

### Milestone 10: Delete ragent-core (1 day)
**Remove the old monolith**

**Tasks:**
1. Verify no code remains in ragent-core/src/
2. Update root `Cargo.toml`: remove ragent-core from workspace
3. Delete `crates/ragent-core/` directory
4. Update `src/main.rs` imports
5. Test full build: `cargo build --release`
6. Run all tests: `cargo test --workspace`
7. Update documentation: SPEC.md, README.md

**Success Criteria:**
- ✅ ragent-core directory deleted
- ✅ Full workspace builds successfully
- ✅ All tests pass
- ✅ Binary runs correctly

---

## Benefits of New Structure

### 1. **Reduced Crate Sizes**
- **Before:** ragent-core 64,909 lines (54% of codebase)
- **After:** Largest crate is ragent-tui at 26,786 lines (22%)
- **New crates:** Average ~7,900 lines, max ~12,000 lines

### 2. **Improved Modularity**
- **Clear separation of concerns:** types, config, storage, providers, tools, orchestration
- **Tool categories separated:** core, extended, VCS
- **No circular dependencies**

### 3. **Faster Compilation**
- **Parallel builds:** Tool crates can compile in parallel
- **Incremental compilation:** Changes to tools don't trigger provider recompilation
- **Smaller compile units:** Average crate size reduced by 70%

### 4. **Better Navigation**
- **Logical organization:** Crate name tells you what it does
- **Reduced cognitive load:** Each crate has a single responsibility
- **Easier onboarding:** New contributors can focus on one crate

### 5. **Independent Versioning**
- **Stable core:** ragent-types rarely changes
- **Fast iteration:** Tools can evolve independently
- **Backward compatibility:** Breaking changes isolated to specific crates

### 6. **Reduced Dependency Bloat**
- **Optional features:** VCS tools only included if needed
- **Lighter builds:** Core tools don't pull in PDF/Office dependencies
- **Faster CI:** Test only changed crates

### 7. **Easier Testing**
- **Focused test suites:** Each crate tests one concern
- **Parallel test execution:** Crates can test independently
- **Faster test runs:** Only test changed crates

---

## Risks & Mitigation

### Risk 1: Breaking Changes
**Mitigation:**
- Migrate incrementally (10 milestones)
- Keep ragent-core compiling until all crates extracted
- Extensive testing at each milestone

### Risk 2: Import Churn
**Mitigation:**
- Use workspace dependencies in root Cargo.toml
- Create re-export modules in ragent-agent for common types
- Document import paths in SPEC.md

### Risk 3: Increased Build Time (More Crates)
**Mitigation:**
- Smaller crates compile faster than one large crate
- Parallel compilation offsets overhead
- Incremental compilation more effective with smaller units

### Risk 4: Dependency Management Complexity
**Mitigation:**
- Clear dependency graph documented in CRATEPLAN.md
- Use workspace inheritance for common dependencies
- Deny circular dependencies in deny.toml

---

## Timeline Estimate

**Total: 15-20 working days (3-4 weeks)**

| Milestone | Duration | Cumulative |
|-----------|----------|------------|
| M1: Foundation (types, config) | 2 days | 2 days |
| M2: Storage | 1 day | 3 days |
| M3: LLM Providers | 2 days | 5 days |
| M4: Core Tools | 3 days | 8 days |
| M5: Extended Tools | 3 days | 11 days |
| M6: VCS Tools | 1 day | 12 days |
| M7: Agent Orchestration | 3 days | 15 days |
| M8: Team Layer | 1 day | 16 days |
| M9: Update Server/TUI | 2 days | 18 days |
| M10: Delete ragent-core | 1 day | 19 days |
| **Buffer for issues** | 1-3 days | **20-22 days** |

---

## Success Metrics

**Quantitative:**
- ✅ ragent-core deleted
- ✅ No crate over 15,000 lines
- ✅ Average crate size < 10,000 lines
- ✅ Dependency depth ≤ 4 levels
- ✅ No circular dependencies
- ✅ All tests pass (100%)
- ✅ Build time improvement ≥ 10%

**Qualitative:**
- ✅ Code navigation feels easier
- ✅ New contributors understand structure quickly
- ✅ Changes to tools don't trigger full recompilation
- ✅ Documentation clearly explains crate boundaries

---

## Next Steps

1. **Review this plan** — Get approval from maintainers
2. **Create tracking issue** — GitHub issue for the reorganization
3. **Start with M1** — Begin with foundation layer (ragent-types, ragent-config)
4. **Iterate milestone by milestone** — Test thoroughly at each step
5. **Document as you go** — Update SPEC.md and README.md
6. **Celebrate completion** — Ship v0.2.0 with new crate structure 🎉
