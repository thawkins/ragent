# Jcode → ragent Capability Port Plan

> A comparison-driven roadmap for bringing `jcode`-unique agent capabilities into
> `ragent` while preserving `ragent`'s existing architecture and constraints.

## Context

- `jcode` (https://github.com/1jehuang/jcode) is a Rust coding-agent harness with
  a large set of first-class built-in tools and ambient/background execution.
- `ragent` (this repo) already has a broad tool surface: file/shell/search, VCS,
  office/PDF, web, codeindex, memory, teams/swarm, sub-agents, specs, MCP, etc.
- This plan focuses on **built-in tool and runtime capabilities** that `jcode`
  provides and `ragent` currently lacks, grouped into milestones that can be
  implemented independently.

## Goal

Close the most valuable gaps between `jcode` and `ragent` by porting selected
agent-facing tools and supporting runtime features, without duplicating
existing `ragent` functionality (VCS, codeindex, office/PDF, masterfetch, teams,
specs, MCP) or building UI-only features that are out of scope.

## Approach

1. Reuse `ragent`'s existing extracted tool crates (`ragent-tools-core`,
   `ragent-tools-extended`, `ragent-tools-vcs`) and `ragent-agent/src/tool/`
   for new tool implementations.
2. Add new modules in `ragent-tools-extended` or `ragent-agent` depending on
   whether the capability is generically useful (extended) or session/agent
   specific (agent).
3. Keep each milestone self-contained: design the schema, implement the tool,
   add tests in `tests/`, register it in `create_default_registry()`, and
   document in `SPEC.md`/`QUICKSTART.md` if user-facing.
4. Where a capability needs a new background subsystem (ambient runner,
   background task manager), implement the subsystem in `ragent-agent` and
   expose it through one or more tools.

## Capability Gap Matrix

| jcode capability               | jcode tool(s)                                             | ragent status                                                                                                                | relevance                                          |
| ------------------------------ | --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Structure-aware search         | `agentgrep`                                             | Not present. Core`grep` exists, but does not return symbols/outline/displacement info.                                     | High — reduces context use.                       |
| Codex-style patches            | `apply_patch`                                           | Not present.`patch` + `multi_edit` exist.                                                                                | Medium — model often emits Codex patches.         |
| Parallel batched calls         | `batch`                                                 | Not present. Sub-agents/teams exist, but no in-turn batch wrapper.                                                           | Medium — throughput for independent reads.        |
| Background task manager        | `bg`                                                    | Partial.`new_task`/`wait_tasks`/`cancel_task` exist for sub-agents, but no generic background bash task introspection. | High — long-running builds/tests.                 |
| Browser automation             | `browser`                                               | Not present.                                                                                                                 | High — web actions beyond fetch.                  |
| Conversation search            | `conversation_search`                                   | Not present.                                                                                                                 | Medium — self-recall within session.              |
| Cross-session RAG              | `session_search`                                        | Not present. Sessions are stored but not searchable.                                                                         | High — long-term memory.                          |
| Ambient scheduling             | `schedule`, `schedule_ambient`, `end_ambient_cycle` | Not present.                                                                                                                 | Medium — background agent cycles.                 |
| Structured permission requests | `request_permission`                                    | Partial. Permission system exists, but no tool that lets the agent itself request approval with review context.              | Medium — safer ambient/agent workflows.           |
| Gmail integration              | `gmail`                                                 | Not present.                                                                                                                 | Medium — email as I/O channel.                    |
| Channel messaging              | `send_channel_message`                                  | Not present.                                                                                                                 | Low — depends on external chat config.            |
| Durable initiatives/goals      | `initiative`                                            | Not present.`todo_read`/`todo_write` exist but are lightweight.                                                          | Medium — long-lived project goals.                |
| Skill management tool          | `skill_manage`                                          | Not present. Skills are auto-loaded/bundled but not dynamically managed.                                                     | Medium — hot-load skill packs.                    |
| Open/reveal files              | `open`                                                  | Not present.                                                                                                                 | Low — desktop convenience.                        |
| Swarm plan orchestration       | `communicate`                                           | Partial.`team_*` tools provide coordination, but no single plan-graph tool.                                                | Low — teams already cover most use.               |
| Discoverable tool directory    | `discover_tools`                                        | Not present.                                                                                                                 | Low — depends on hosted directory + partnerships. |
| Debug socket introspection     | `debug_socket`                                          | Not present.                                                                                                                 | Low — internal tooling.                           |

## Milestones

### M1 — Structure-aware code search (`agentgrep`)

**What:** Port the `agentgrep` concept: a `grep`-like tool that also returns file
structure metadata (function list, line ranges, symbol displacement) and can
truncate results based on what the session has already read.

**Where:** `crates/ragent-tools-extended/src/agentgrep.rs`

**Tasks:**

- T-001 Design `agentgrep` JSON schema (`mode`: `grep`/`outline`/`smart`/`find`,
  `query`, `path`, `glob`, `max_regions`, `max_files`, `full_region`, etc.).
- T-002 Integrate the existing `ragent-codeindex` index as a symbol source, or
  build a lightweight tree-sitter/outline fallback.
- T-003 Implement adaptive truncation using session read history.
- T-004 Add tests in `crates/ragent-tools-extended/tests/test_agentgrep.rs`.
- T-005 Register as `agentgrep` in `ragent-agent/src/tool/mod.rs`.
- T-006 Optionally alias legacy `grep` calls to `agentgrep` when the model asks
  for structure info.

**Acceptance:** `agentgrep pattern="pub fn" glob="*.rs"` returns ranked matches
with surrounding symbol boundaries and omits already-read regions.

### M2 — Codex-style patch + batched tool calls

**What:** Add `apply_patch` (Codex `*** Begin Patch` / `*** End Patch` blocks)
and `batch` (parallel subcalls up to 10).

**Where:** `ragent-tools-core` for `apply_patch`; `ragent-tools-extended` or
`ragent-agent` for `batch`.

**Tasks:**

- T-010 Implement `apply_patch` parser supporting add/delete/update with moves.
- T-011 Hook into `ragent-tools-core::replace` fuzzy matching for update hunks.
- T-012 Add `batch` tool that accepts an array of `{tool, parameters}` and runs
  independent calls in parallel using `tokio::try_join!`, respecting permissions
  for each subcall.
- T-013 Publish `BusEvent` subcall progress for TUI/server consumers.
- T-014 Add tests for patch parsing and batch execution.
- T-015 Register both tools.

**Acceptance:** A single model response can call `batch` with ten `read` calls,
  and a Codex-style patch edits three files without falling back to `edit`.

### M3 — Background task manager (`bg`)

**What:** A `bg` tool that runs arbitrary shell commands in the background and
lets the agent list, tail, wait on, cancel, and inspect them.

**Where:** `crates/ragent-tools-core/src/bg.rs` plus a small background task
service in `ragent-agent/src/background/`.

**Tasks:**

- T-020 Design `bg` actions: `spawn` (alias run via `bash`), `list`, `status`,
  `output`, `tail`, `cancel`, `wait`, `cleanup`.
- T-021 Persist background tasks to SQLite via `ragent-storage`.
- T-022 Parse `JCODE_PROGRESS`-style progress lines from stdout; reuse existing
  bash output parsing where possible.
- T-023 Add wake/notify hooks so a long-running task can resume the session.
- T-024 Add tests in `crates/ragent-tools-core/tests/test_bg.rs`.
- T-025 Register as `bg`.

**Acceptance:** `bg action="spawn" command="cargo test"` returns a task id;
`bg action="wait" task_id="..."` returns the exit code and tail output.

### M4 — Browser automation (`browser`)

**What:** Implement a `browser` tool that can open URLs, snapshot pages,
interact with elements, evaluate JS, and upload files. Follow the jcode browser
provider protocol design (Firefox Agent Bridge or CDP).

**Where:** `crates/ragent-tools-extended/src/browser.rs`

**Tasks:**

- T-030 Define `browser` action schema (`open`, `snapshot`, `click`, `type`,
  `fill_form`, `select`, `wait`, `eval`, `scroll`, `upload`, `press`,
  `screenshot`, `status`, `setup`).
- T-031 Implement a CDP (Chrome DevTools Protocol) backend as the most portable
  starting point; gate macOS-specific computer-use separately.
- T-032 Add setup/status slash commands and configuration in `ragent.json`.
- T-033 Add tests using a local test HTTP server + headless Chrome if available.
- T-034 Register as `browser`.

**Acceptance:** The model can open a page, click a button, and return the updated
page text as markdown.

### M5 — Conversation and cross-session search

**What:** Port `conversation_search` (RAG over current session history) and
`session_search` (RAG over all past sessions).

**Where:** `ragent-agent/src/tool/conversation_search.rs` and
`ragent-agent/src/tool/session_search.rs`; indexing in `ragent-storage`.

**Tasks:**

- T-040 Add full-text + embedding index over session messages in SQLite.
- T-041 Implement `conversation_search` with keyword, turn-range, and stats
  modes; integrate with the compaction manager.
- T-042 Implement `session_search` with filters (date, working_dir, provider,
  source, include_tools/system, max_per_session).
- T-043 Warm indexes in the background on startup.
- T-044 Add tests for both tools.
- T-045 Register both tools.

**Acceptance:** `session_search query="database migration"` returns ranked
messages from prior sessions with surrounding context.

### M6 — Ambient scheduling and structured permission requests

**What:** Add an ambient runner and the `schedule`, `schedule_ambient`,
`end_ambient_cycle`, and `request_permission` tools.

**Where:** `ragent-agent/src/ambient/` and `ragent-agent/src/tool/ambient.rs`.

**Tasks:**

- T-050 Implement a persistent scheduled queue in `ragent-storage`.
- T-051 Build an ambient runner that wakes sessions/ambient agents based on
  schedule entries, with nudge/backoff logic.
- T-052 Implement `schedule` (create/list/cancel) and `schedule_ambient`.
- T-053 Implement `end_ambient_cycle` to report cycle results and schedule the
  next wake.
- T-054 Implement `request_permission` with structured review context,
  urgency, and wait/queue semantics; wire into the existing permission system.
- T-055 Add tests; ensure ambient sessions can be disabled via config.
- T-056 Register tools.

**Acceptance:** `schedule task="review open PRs" wake_in_minutes=60` queues a
future resume; `request_permission` surfaces an actionable prompt in the TUI.

### M7 — External integrations: Gmail and channel messaging

**What:** Add `gmail` and `send_channel_message` tools.

**Where:** `ragent-tools-extended/src/gmail.rs` and
`ragent-tools-extended/src/channels.rs`.

**Tasks:**

- ✅ T-060 Implement Gmail search/read/draft/send via OAuth2 or a managed backend
  (e.g. Composio-style). Store tokens in `ragent-storage` encrypted table.
- ✅ T-061 Implement channel registry for Telegram/Discord webhooks; add config
  block in `ragent.json`.
- ✅ T-062 Add tests with mocked backends.
- ✅ T-063 Register tools.

**Acceptance:** `gmail action="search" query="from:ci@example.com"` returns
messages; `send_channel_message message="deployed"` sends to configured
channels.

### M8 — Durable initiatives and skill management

**What:** Port `initiative` (durable goals with milestones) and `skill_manage`
(skill load/list/reload/read at runtime).

**Where:** `ragent-agent/src/tool/initiative.rs` and
`ragent-agent/src/tool/skill_manage.rs`; storage in `ragent-storage`.

**Tasks:**

- ✅ T-070 Store durable goals in SQLite with milestones, progress, and
  status; surface them in system prompt or via tool.
- ✅ T-070 Implement `initiative` actions (`create`, `update`, `checkpoint`,
  `list`, `close`).
- ✅ T-071 Extend the existing skills system with a `skill_manage` tool that can
  load/list/reload skills on demand and read a skill's prompt.
- ✅ T-072 Add tests.
- ✅ T-073 Register tools.

**Acceptance:** `initiative action="checkpoint" id="api-v2"` updates progress;
`skill_manage action="load" name="rust-error-handling"` injects the skill.

### M10 — Open/reveal and remaining UX tools

**What:** Add `open` (open/reveal files, folders, URLs) and any remaining
low-effort aliases.

**Where:** `ragent-tools-core/src/open.rs`.

**Tasks:**

- ✅ T-090 Cross-platform `xdg-open` / `open` / `start` wrapper.
- ✅ T-091 `reveal` action that opens the parent directory.
- ✅ T-092 URL scheme validation.
- ✅ T-093 Add tests.
- ✅ T-094 Register tool.

**Acceptance:** `open target="target/release/ragent" action="reveal"` opens
   the file manager.

**Status:** Implemented. The `OpenTool` lives in `crates/ragent-tools-core/src/open.rs`,
is registered via `create_core_registry()` (and therefore surfaced automatically in the
agent default registry), and has integration tests in
`crates/ragent-tools-core/tests/test_open.rs`. URL schemes are allowlisted to
`http`, `https`, `mailto`, and `file`; unknown schemes are rejected with a clear
error.

## Out of Scope

The following `jcode` capabilities are intentionally not planned because they
are UI-only, platform-specific, already covered by `ragent`, or require external
infrastructure that is not part of this effort:

- Desktop macOS computer-use (`computer` / `macos_computer_use`) — UI/platform
  specific; revisit only if a headless equivalent is needed.
- UI rendering optimizations, mermaid diagrams, info widgets, custom scrollback,
  FPS targets — TUI/visual only.
- `discover_tools` sponsored partner directory — depends on a hosted directory
  and partnership agreements.
- `debug_socket` server introspection — internal diagnostic surface; can be
  added later.
- `invalid` placeholder tool — not useful.
- Direct provider OAuth flows and named profiles — valuable but orthogonal to
  the tool surface; track separately.
- iOS / native OpenClaw client.

## Open Questions

1. Should `apply_patch` replace or coexist with the existing `patch` tool?
2. Should `agentgrep` supersede `grep` for all model calls, or be an opt-in
   advanced tool?
3. Does `ragent` want to support ambient/autonomous execution, or should M6 be
   deferred until permission/audit logging is stronger?
4. Which browser backend should be implemented first: CDP (Chrome), Firefox
   Agent Bridge, or Playwright-style remote?

## Success Criteria

After the milestones above:

- All new tools are registered in `create_default_registry()`.
- Each tool has unit tests under the relevant `tests/` directory.
- `cargo test` and `cargo clippy` pass with no new warnings.
- User-facing tools are documented in `SPEC.md` and `QUICKSTART.md`.
- `CHANGELOG.md` is updated with each merged milestone.
