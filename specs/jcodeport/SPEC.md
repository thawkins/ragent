---
status: in_progress
audit:
  - { time: 1784720000, from: "draft", to: "in_progress", actor: "ragent" }
---
# Jcode → ragent Capability Port Plan

## Context

`jcode` (https://github.com/1jehuang/jcode) is a Rust coding-agent harness with a
large set of first-class built-in tools and ambient/background execution. `ragent`
already has a broad tool surface; this spec tracks porting the most valuable
`jcode`-unique capabilities into `ragent` without duplicating existing functionality.

## Goal

Close the highest-value gaps between `jcode` and `ragent` by porting selected
agent-facing tools and supporting runtime features, without duplicating existing
`ragent` functionality (VCS, codeindex, office/PDF, masterfetch, teams, specs, MCP)
or building UI-only features.

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
| Structure-aware search         | `codeindex_*` tools                                     | Implemented. `codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, and `codeindex_status` provide symbol-aware code navigation. | High — reduces context use.                       |
| Codex-style patches            | `apply_patch`                                           | Implemented. `apply_patch` (`ragent-tools-core/src/apply_patch.rs`); `patch` + `multi_edit` also exist. | Medium — model often emits Codex patches.         |
| Parallel batched calls         | `batch`                                                 | Not present. Sub-agents/teams exist, but no in-turn batch wrapper.                                                           | Medium — throughput for independent reads.        |
| Background task manager        | `bg`                                                    | Implemented. `bg` (`ragent-agent/src/tool/bg.rs`) manages background tasks; `new_agent`/`wait_agents`/`cancel_agent` cover sub-agents. | High — long-running builds/tests.                 |
| Browser automation             | `browser`                                               | Not present.                                                                                                                 | High — web actions beyond fetch.                  |
| Conversation search            | `conversation_search`                                   | Implemented. `conversation_search` (`ragent-agent/src/tool/conversation_search.rs`). | Medium — self-recall within session.              |
| Cross-session RAG              | `session_search`                                        | Implemented. `session_search` (`ragent-agent/src/tool/session_search.rs`). | High — long-term memory.                          |
| Ambient scheduling             | `schedule`, `schedule_ambient`, `end_ambient_cycle` | Partial. `cron_add`/`cron_list`/`cron_remove`/`cron_enable`/`cron_disable` schedule work; no ambient-cycle tools. | Medium — background agent cycles.                 |
| Structured permission requests | `request_permission`                                    | Partial. Permission system exists, but no tool that lets the agent itself request approval with review context.              | Medium — safer ambient/agent workflows.           |
| Gmail integration              | `gmail`                                                 | Not present.                                                                                                                 | Medium — email as I/O channel.                    |
| Channel messaging              | `send_channel_message`                                  | Not present.                                                                                                                 | Low — depends on external chat config.            |
| Durable initiatives/goals      | `initiative`                                            | Implemented. `initiative` (`ragent-agent/src/tool/initiative.rs`). | Medium — long-lived project goals.                |
| Skill management tool          | `skill_manage`                                          | Implemented. The `Skill` tool (`ragent-agent/src/tool/skill_manage.rs`) manages the skill registry at runtime. | Medium — hot-load skill packs.                    |
| Open/reveal files              | `open`                                                  | Implemented. `open` (`ragent-tools-core`, registered as `open::OpenTool`). | Low — desktop convenience.                        |
| Swarm plan orchestration       | `communicate`                                           | Partial.`team_*` tools provide coordination, but no single plan-graph tool.                                                | Low — teams already cover most use.               |
| Discoverable tool directory    | `discover_tools`                                        | Not present.                                                                                                                 | Low — depends on hosted directory + partnerships. |
| Debug socket introspection     | `debug_socket`                                          | Not present.                                                                                                                 | Low — internal tooling.                           |

## Requirements

### M1 — Structure-aware code search (`agentgrep`)

FR-M1-001: The `agentgrep` tool **shall** support `mode` values `grep`,
`outline`, `smart`, and `find`.

FR-M1-002: The `agentgrep` tool **shall** return ranked matches with surrounding
symbol boundaries when requested.

FR-M1-003: The `agentgrep` tool **shall** omit file regions that the current
session has already read, when session history is available.

FR-M1-004: The `agentgrep` tool **shall** be implemented in
`crates/ragent-tools-extended/src/agentgrep.rs` with tests in
`crates/ragent-tools-extended/tests/test_agentgrep.rs`.

FR-M1-005: The `agentgrep` tool **shall** be registered in
`ragent-agent/src/tool/mod.rs` as `agentgrep`.

### M2 — Codex-style patch + batched tool calls

FR-M2-001: The `apply_patch` tool **shall** parse Codex-style
`*** Begin Patch` / `*** End Patch` blocks supporting add, delete, update, and
move hunks.

FR-M2-002: The `apply_patch` tool **shall** be implemented in
`crates/ragent-tools-core/src/apply_patch.rs` with tests in
`crates/ragent-tools-core/tests/test_apply_patch.rs`.

FR-M2-003: The `batch` tool **shall** accept an array of `{tool, parameters}`
calls and execute independent calls concurrently with `tokio::try_join!`.

FR-M2-004: The `batch` tool **shall** enforce the permission rules of each
subcall individually.

FR-M2-005: The `batch` tool **shall** publish subcall progress events on the
`EventBus`.

FR-M2-006: The `apply_patch` and `batch` tools **shall** be registered in the
default tool registry.

### M3 — Background task manager (`bg`)

FR-M3-001: The `bg` tool **shall** support actions `spawn`, `list`, `status`,
`output`, `tail`, `cancel`, `wait`, and `cleanup`.

FR-M3-002: Background tasks **shall** run arbitrary shell commands in a
subprocess and be persisted to SQLite via `ragent-storage`.

FR-M3-003: The `bg` tool **shall** parse `JCODE_PROGRESS`-style progress lines
from stdout/stderr.

FR-M3-004: The `bg` subsystem **shall** provide wake/notify hooks so a
long-running task can resume the parent session when it completes.

FR-M3-005: The `bg` tool **shall** be implemented in
`crates/ragent-tools-core/src/bg.rs` with tests in
`crates/ragent-tools-core/tests/test_bg.rs`, backed by a small service in
`ragent-agent/src/background/`.

### M4 — Browser automation (`browser`)

FR-M4-001: The `browser` tool **shall** support actions `open`, `snapshot`,
`click`, `type`, `fill_form`, `select`, `wait`, `eval`, `scroll`, `upload`,
`press`, `screenshot`, `status`, and `setup`.

FR-M4-002: The `browser` tool **shall** implement a CDP (Chrome DevTools
Protocol) backend as the default portable backend.

FR-M4-003: The `browser` tool **shall** be implemented in
`crates/ragent-tools-extended/src/browser.rs` with tests using a local test HTTP
server and headless Chrome when available.

### M5 — Conversation and cross-session search

FR-M5-001: The `conversation_search` tool **shall** provide keyword, turn-range,
and stats search modes over the current session messages.

FR-M5-002: The `session_search` tool **shall** provide full-text (FTS5) search
across all stored sessions, with filters for date, working_dir, provider,
source, include_tools/system, and max_per_session.

FR-M5-003: Session-message indexes **shall** be warmed in the background on
startup.

FR-M5-004: Both tools **shall** be implemented in `ragent-agent/src/tool/` with
storage/indexing support in `ragent-storage`.

### M6 — Ambient scheduling and structured permission requests

FR-M6-001: The `schedule` tool **shall** support create/list/cancel actions and
persist scheduled entries to SQLite.

FR-M6-002: The `schedule_ambient` tool **shall** queue autonomous ambient-agent
cycles.

FR-M6-003: The `end_ambient_cycle` tool **shall** report cycle results and
schedule the next wake.

FR-M6-004: The `request_permission` tool **shall** surface structured review
context with urgency and wait/queue semantics, wired into the existing permission
system.

FR-M6-005: Ambient execution **shall** be disableable via configuration.

### M7 — External integrations: Gmail and channel messaging

FR-M7-001: The `gmail` tool **shall** support search/read/draft/send actions via
OAuth2 or a managed backend, with tokens stored encrypted in `ragent-storage`.

FR-M7-002: The `send_channel_message` tool **shall** support a registry for
Telegram/Discord webhooks, configured in `ragent.json`.

### M8 — Durable initiatives and skill management

FR-M8-001: The `initiative` tool **shall** support `create`, `update`,
`checkpoint`, `list`, and `close` actions for durable goals with milestones,
stored in SQLite.

FR-M8-002: The `skill_manage` tool **shall** support `load`, `list`, `reload`,
and `read` actions to dynamically manage skill packs at runtime.

### M10 — Open/reveal and remaining UX tools

FR-M10-001: The `open` tool **shall** support `open` and `reveal` actions across
platforms using `xdg-open` / `open` / `start`, with URL scheme validation.

Implemented in `crates/ragent-tools-core/src/open.rs` and registered via
`create_core_registry()`, with tests in `crates/ragent-tools-core/tests/test_open.rs`; URL schemes are allowlisted to `http`, `https`,
`mailto` and `file`, and unknown schemes are rejected with a clear error.

## Task Index and Acceptance

Task ids are cited by source comments (for example `T-070`, `T-071`). `done` marks tasks the original plan checked off; current implementation state is in the Capability Gap Matrix.

| Task | Milestone | Description | Status |
| ---- | --------- | ----------- | ------ |
| T-010 | M2 | Implement `apply_patch` parser supporting add/delete/update with moves. | |
| T-011 | M2 | Hook into `ragent-tools-core::replace` fuzzy matching for update hunks. | |
| T-012 | M2 | Add `batch` tool that accepts an array of `{tool, parameters}` and runs independent calls in parallel using `tokio::try_join!`, respecting permissions for each subcall. | |
| T-013 | M2 | Publish `BusEvent` subcall progress for TUI/server consumers. | |
| T-014 | M2 | Add tests for patch parsing and batch execution. | |
| T-015 | M2 | Register both tools. | |
| - | M2 | **Acceptance:** A single model response can call `batch` with ten `read` calls, and a Codex-style patch edits three files without falling back to `edit`. | |
| T-020 | M3 | Design `bg` actions: `spawn` (alias run via `bash`), `list`, `status`, `output`, `tail`, `cancel`, `wait`, `cleanup`. | |
| T-021 | M3 | Persist background tasks to SQLite via `ragent-storage`. | |
| T-022 | M3 | Parse `JCODE_PROGRESS`-style progress lines from stdout; reuse existing bash output parsing where possible. | |
| T-023 | M3 | Add wake/notify hooks so a long-running task can resume the session. | |
| T-024 | M3 | Add tests in `crates/ragent-tools-core/tests/test_bg.rs`. | |
| T-025 | M3 | Register as `bg`. | |
| - | M3 | **Acceptance:** `bg action="spawn" command="cargo test"` returns a task id; `bg action="wait" task_id="..."` returns the exit code and tail output. | |
| T-030 | M4 | Define `browser` action schema (`open`, `snapshot`, `click`, `type`, `fill_form`, `select`, `wait`, `eval`, `scroll`, `upload`, `press`, `screenshot`, `status`, `setup`). | |
| T-031 | M4 | Implement a CDP (Chrome DevTools Protocol) backend as the most portable starting point; gate macOS-specific computer-use separately. | |
| T-032 | M4 | Add setup/status slash commands and configuration in `ragent.json`. | |
| T-033 | M4 | Add tests using a local test HTTP server + headless Chrome if available. | |
| T-034 | M4 | Register as `browser`. | |
| - | M4 | **Acceptance:** The model can open a page, click a button, and return the updated page text as markdown. | |
| T-040 | M5 | Add full-text + embedding index over session messages in SQLite. | |
| T-041 | M5 | Implement `conversation_search` with keyword, turn-range, and stats modes; integrate with the compaction manager. | |
| T-042 | M5 | Implement `session_search` with filters (date, working_dir, provider, source, include_tools/system, max_per_session). | |
| T-043 | M5 | Warm indexes in the background on startup. | |
| T-044 | M5 | Add tests for both tools. | |
| T-045 | M5 | Register both tools. | |
| - | M5 | **Acceptance:** `session_search query="database migration"` returns ranked messages from prior sessions with surrounding context. | |
| T-050 | M6 | Implement a persistent scheduled queue in `ragent-storage`. | |
| T-051 | M6 | Build an ambient runner that wakes sessions/ambient agents based on schedule entries, with nudge/backoff logic. | |
| T-052 | M6 | Implement `schedule` (create/list/cancel) and `schedule_ambient`. | |
| T-053 | M6 | Implement `end_ambient_cycle` to report cycle results and schedule the next wake. | |
| T-054 | M6 | Implement `request_permission` with structured review context, urgency, and wait/queue semantics; wire into the existing permission system. | |
| T-055 | M6 | Add tests; ensure ambient sessions can be disabled via config. | |
| T-056 | M6 | Register tools. | |
| - | M6 | **Acceptance:** `schedule task="review open PRs" wake_in_minutes=60` queues a future resume; `request_permission` surfaces an actionable prompt in the TUI. | |
| T-060 | M7 | Implement Gmail search/read/draft/send via OAuth2 or a managed backend (e.g. Composio-style). Store tokens in `ragent-storage` encrypted table. | done |
| T-061 | M7 | Implement channel registry for Telegram/Discord webhooks; add config block in `ragent.json`. | done |
| T-062 | M7 | Add tests with mocked backends. | done |
| T-063 | M7 | Register tools. | done |
| - | M7 | **Acceptance:** `gmail action="search" query="from:ci@example.com"` returns messages; `send_channel_message message="deployed"` sends to configured channels. | |
| T-070 | M8 | Store durable goals in SQLite with milestones, progress, and status; surface them in system prompt or via tool. | done |
| T-070 | M8 | Implement `initiative` actions (`create`, `update`, `checkpoint`, `list`, `close`). | done |
| T-071 | M8 | Extend the existing skills system with a `skill_manage` tool that can load/list/reload skills on demand and read a skill's prompt. | done |
| T-072 | M8 | Add tests. | done |
| T-073 | M8 | Register tools. | done |
| - | M8 | **Acceptance:** `initiative action="checkpoint" id="api-v2"` updates progress; `skill_manage action="load" name="rust-error-handling"` injects the skill. | |
| T-090 | M10 | Cross-platform `xdg-open` / `open` / `start` wrapper. | done |
| T-091 | M10 | `reveal` action that opens the parent directory. | done |
| T-092 | M10 | URL scheme validation. | done |
| T-093 | M10 | Add tests. | done |
| T-094 | M10 | Register tool. | done |
| - | M10 | **Acceptance:** `open target="target/release/ragent" action="reveal"` opens the file manager. | |

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
2. Does `ragent` want to support ambient/autonomous execution, or should M6 be
   deferred until permission/audit logging is stronger?
3. Which browser backend should be implemented first: CDP (Chrome), Firefox
   Agent Bridge, or Playwright-style remote?

## Success Criteria

- All new tools are registered in `create_default_registry()`.
- Each tool has unit tests under the relevant `tests/` directory.
- `cargo test` and `cargo clippy` pass with no new warnings.
- User-facing tools are documented in `SPEC.md` and `QUICKSTART.md`.
- `CHANGELOG.md` is updated with each merged milestone.

## References

- `jcode`: https://github.com/1jehuang/jcode
- `crates/ragent-agent/src/tool/mod.rs`
- `crates/ragent-tools-core/src/lib.rs`
- `crates/ragent-tools-extended/src/lib.rs`
- `crates/ragent-storage/src/storage.rs`
- `crates/ragent-types/src/event/mod.rs`
