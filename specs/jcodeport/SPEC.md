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

FR-M5-002: The `session_search` tool **shall** provide full-text + embedding
search across all stored sessions, with filters for date, working_dir, provider,
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

## Success Criteria

- All new tools are registered in `create_default_registry()`.
- Each tool has unit tests under the relevant `tests/` directory.
- `cargo test` and `cargo clippy` pass with no new warnings.
- User-facing tools are documented in `SPEC.md` and `QUICKSTART.md`.
- `CHANGELOG.md` is updated with each merged milestone.

## References

- `docs/JCODEPLAN.md`
- `crates/ragent-agent/src/tool/mod.rs`
- `crates/ragent-tools-core/src/lib.rs`
- `crates/ragent-tools-extended/src/lib.rs`
- `crates/ragent-storage/src/storage.rs`
- `crates/ragent-types/src/event/mod.rs`
