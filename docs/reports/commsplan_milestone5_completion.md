# COMMSPLAN Milestone 5 — Completion Report

**Date:** 2026-06-21
**Plan:** `COMMSPLAN.md` §2 Milestone 5
**Status:** ✅ COMPLETE
**Priority:** P1 (debuggability, future-proofing)

## Goal

Make the on-disk and event-layer representations consistent, versioned, and
self-describing.

## Deliverables

- [x] Consistent serialization for statuses and message types.
- [x] Schema versions on all root persisted types.
- [x] Input validation and `deny_unknown_fields` on key structs.
- [x] Correlation IDs for request/reply message pairs.
- [x] `updated_at` timestamps and clearer event payloads.
- [x] Dead event types wired or documented.

## Task-by-task summary

### M5-T1 — Unify `TaskStatus` string representation

**Files:** `crates/ragent-team/src/team/task.rs`, `crates/ragent-team/src/team/config.rs`,
`crates/ragent-team/src/tools/team_status.rs`, `crates/ragent-team/src/tools/team_task_list.rs`,
`crates/ragent-team/src/tools/team_wait.rs`, `crates/ragent-team/src/tools/team_assign_task.rs`,
`crates/ragent-team/src/tools/team_message.rs`, `crates/ragent-team/src/tools/team_create.rs`.

Added `as_str()` methods to `TaskStatus`, `MemberStatus`, `TeamStatus`,
`PlanStatus`, and `MemoryScope` returning canonical snake_case strings
(`"pending"`, `"in_progress"`, `"completed"`, `"cancelled"`, `"spawning"`,
`"working"`, `"idle"`, `"plan_pending"`, `"blocked"`, `"suspended"`,
`"shutting_down"`, `"stopped"`, `"failed"`, `"none"`, `"approved"`,
`"rejected"`, `"active"`, `"disbanded"`, `"user"`, `"project"`).

Replaced every `format!("{:?}", x).to_lowercase()` and `"in-progress"` (hyphen)
usage in the tools with `as_str()`, so serde, Debug-derived tool output, and
SSE all produce the same snake_case values. The previous `"in-progress"`
(hyphen) mismatch in `team_task_claim` and `team_task_complete` debug logs is
fixed.

**Findings addressed:** s4 Issue 3.

### M5-T2 — Add `schema_version` to root persisted types

**Files:** `crates/ragent-team/src/team/config.rs`, `crates/ragent-team/src/team/task.rs`,
`crates/ragent-team/src/team/store.rs`.

Added `#[serde(default)] schema_version: u32` to `TeamConfig` and `TaskList`,
with `TEAM_CONFIG_SCHEMA_VERSION` / `TASK_LIST_SCHEMA_VERSION` consts (both `1`).

Added `migrate()` methods to both `TeamConfig` and `TaskList`. Currently they
stamp `schema_version` to the current const when it is `0` and set
`updated_at = Some(Utc::now())` when it is `None`. Future breaking changes
should bump the const and perform field transforms in `migrate()`.

`TeamStore::load()` and `TaskStore::read()` call `migrate()` on the
deserialised value before returning it. `TeamStore::save()` and
`TaskStore::write_locked()` stamp `schema_version` (if `0`) and `updated_at` on
every write.

**Findings addressed:** s4 Issue 6.

### M5-T3 — Add `#[serde(deny_unknown_fields)]` and validation

**Files:** `crates/ragent-team/src/team/mailbox.rs`, `crates/ragent-team/src/team/task.rs`,
`crates/ragent-team/src/team/config.rs`.

Applied `#[serde(deny_unknown_fields)]` to `MailboxMessage`, `Task`,
`TeamConfig`, and `TaskList`. Unknown fields in manual edits now cause a
deserialisation error instead of being silently ignored.

Added `validate()` methods:

- `MailboxMessage::validate()` — `message_id`, `from`, `to` are non-empty.
- `Task::validate()` — `id` non-empty; `assigned_to` (when set) is `"lead"` or
  starts with `"tm-"`; `depends_on` does not contain the task's own id.
- `TaskList::validate()` — delegates to each task.
- `TeamConfig::validate()` — `name` and `lead_session_id` non-empty; member
  `agent_id`s are unique and non-empty.

**Findings addressed:** s4 Issues 7, 11.

### M5-T4 — Add `correlation_id` to `MailboxMessage`

**Files:** `crates/ragent-team/src/team/mailbox.rs`, `crates/ragent-team/src/team/config.rs`,
`crates/ragent-team/src/team/manager.rs`, `crates/ragent-team/src/tools/team_submit_plan.rs`,
`crates/ragent-team/src/tools/team_approve_plan.rs`,
`crates/ragent-team/src/tools/team_shutdown_ack.rs`,
`crates/ragent-team/src/tools/team_shutdown_teammate.rs`.

Added `#[serde(default, skip_serializing_if = "Option::is_none")]
correlation_id: Option<String>` to `MailboxMessage` and a
`MailboxMessage::new_correlated()` constructor.

Added `plan_request_id: Option<String>` and `shutdown_request_id:
Option<String>` to `TeamMember` as the bridge between request and reply.

- `team_submit_plan` generates a UUID, stores it on `member.plan_request_id`,
  and puts it on the `PlanRequest` message.
- `team_approve_plan` copies `member.plan_request_id` into the
  `PlanApproved`/`PlanRejected` reply's `correlation_id` and clears the member
  field.
- `TeamManager::shutdown_teammate` (unified path) and the disk-only fallback in
  `team_shutdown_teammate` both stamp a correlation id on the `ShutdownRequest`
  and record it on `member.shutdown_request_id`.
- `team_shutdown_ack` copies `member.shutdown_request_id` into the `ShutdownAck`
  reply's `correlation_id` and clears the member field.

**Findings addressed:** s4 Issue 8, s2 Issue 11.

### M5-T5 — Add `updated_at` to `TeamConfig` and `TaskList`

**Files:** `crates/ragent-team/src/team/config.rs`, `crates/ragent-team/src/team/task.rs`,
`crates/ragent-team/src/team/store.rs`.

Added `#[serde(default)] updated_at: Option<DateTime<Utc>>` to `TeamConfig`
and `TaskList`. `TeamStore::save()` and `TaskStore::write_locked()` stamp
`updated_at = Some(Utc::now())` on every write. `migrate()` sets it when
`None`. `TeamConfig::new()` and `TaskList::new()` initialise it to
`Some(Utc::now())`.

**Findings addressed:** s4 Issue 12.

### M5-T6 — Distinguish message types in events

**Files:** `crates/ragent-types/src/event/mod.rs`, `crates/ragent-server/src/sse.rs`,
`crates/ragent-team/src/team/manager.rs`, `crates/ragent-tui/src/app.rs`,
`crates/ragent-server/tests/test_event_to_sse.rs`, `crates/ragent-server/benches/bench_sse.rs`,
`crates/ragent-tui/tests/test_teams_tui.rs`.

Extended `Event::TeammateMessage` and `Event::TeammateP2PMessage` with a
`message_type: String` field (snake_case, e.g. `"message"`, `"plan_approved"`,
`"broadcast"`, `"shutdown_request"`).

`publish_message_event` in `manager.rs` serialises the `MessageType` via
`serde_json::to_value` (honouring `#[serde(rename_all = "snake_case")]` on
`MessageType`) and passes the resulting string into the event.

Updated the SSE `TeammateMessageP` payload struct (added `message_type: &str`),
the `to_data` match arms for both variants, the TUI event handlers (log line now
includes `(message_type)`), and all test/bench constructors that build these
events.

**Findings addressed:** s2 Issue 20, s3 §6.2.

### M5-T7 — Remove or publish dead event types

**Files:** `crates/ragent-team/src/tools/team_task_claim.rs`,
`crates/ragent-team/src/tools/team_task_complete.rs`,
`crates/ragent-team/src/tools/team_cleanup.rs`.

Wired the previously-defined-but-never-published event variants:

- `team_task_claim` publishes `Event::TeamTaskClaimed` on both the specific-ID
  and next-available claim paths (lead session id derived from the on-disk
  config).
- `team_task_complete` publishes `Event::TeamTaskCompleted` after a successful
  completion (and after the hook does not reject).
- `team_cleanup` publishes `Event::TeamCleanedUp` after the team directory is
  removed (lead session id captured from the config before deletion).

`MessageType::IdleNotify` is kept — the mailbox poll loop in `manager.rs`
already maps it to `Event::TeammateIdle`, so it is not dead code.

The TUI and SSE already had handlers for all three event variants; they were
just never receiving them. No TUI/SSE changes were needed.

**Findings addressed:** s3 §4.2, s4 Issues 13/15, s2 §4.9/4.10.

## Files modified

| File | Change |
|------|--------|
| `crates/ragent-team/src/team/config.rs` | `as_str()` for `MemberStatus`/`PlanStatus`/`TeamStatus`/`MemoryScope`; `schema_version` + `updated_at` on `TeamConfig`; `deny_unknown_fields`; `validate()`; `migrate()`. |
| `crates/ragent-team/src/team/task.rs` | `as_str()` for `TaskStatus`; `schema_version` + `updated_at` on `TaskList`; `deny_unknown_fields` on `Task`/`TaskList`; `validate()` on `Task`/`TaskList`; `migrate()`; `write_locked` stamps version + `updated_at`. |
| `crates/ragent-team/src/team/mailbox.rs` | `correlation_id` on `MailboxMessage`; `new_correlated()`; `deny_unknown_fields`; `validate()`. |
| `crates/ragent-team/src/team/store.rs` | `TeamStore::load()` calls `migrate()`; `save()` stamps `schema_version` + `updated_at`. |
| `crates/ragent-team/src/team/manager.rs` | `publish_message_event` includes `message_type`; `shutdown_teammate` stamps + records `correlation_id`. |
| `crates/ragent-team/src/tools/team_status.rs` | Use `as_str()` for status/team_status in text + JSON. |
| `crates/ragent-team/src/tools/team_task_list.rs` | Use `as_str()` for task status in JSON. |
| `crates/ragent-team/src/tools/team_wait.rs` | Use `as_str()` for member status. |
| `crates/ragent-team/src/tools/team_assign_task.rs` | Use `as_str()` for dead-assignee error. |
| `crates/ragent-team/src/tools/team_message.rs` | Use `as_str()` for dead-recipient error. |
| `crates/ragent-team/src/tools/team_create.rs` | Use `as_str()` for member status. |
| `crates/ragent-team/src/tools/team_task_claim.rs` | `"in_progress"` (underscore) in debug log; publish `TeamTaskClaimed` on both paths. |
| `crates/ragent-team/src/tools/team_task_complete.rs` | `"in_progress"` (underscore) in debug log; publish `TeamTaskCompleted`. |
| `crates/ragent-team/src/tools/team_cleanup.rs` | Publish `TeamCleanedUp`; capture `lead_sid` before deletion. |
| `crates/ragent-team/src/tools/team_submit_plan.rs` | Generate + store `correlation_id` on `PlanRequest`. |
| `crates/ragent-team/src/tools/team_approve_plan.rs` | Copy `correlation_id` into reply; clear `member.plan_request_id`. |
| `crates/ragent-team/src/tools/team_shutdown_ack.rs` | Copy `correlation_id` into `ShutdownAck`; clear `member.shutdown_request_id`. |
| `crates/ragent-team/src/tools/team_shutdown_teammate.rs` | Disk-only fallback stamps + records `correlation_id`. |
| `crates/ragent-types/src/event/mod.rs` | `message_type: String` on `TeammateMessage` + `TeammateP2PMessage`. |
| `crates/ragent-server/src/sse.rs` | `TeammateMessageP` gains `message_type`; `to_data` match arms updated. |
| `crates/ragent-tui/src/app.rs` | TUI handlers destructure + display `message_type`. |
| `crates/ragent-server/tests/test_event_to_sse.rs` | Test constructors updated with `message_type`. |
| `crates/ragent-server/benches/bench_sse.rs` | Bench event constructor updated with `message_type`. |
| `crates/ragent-tui/tests/test_teams_tui.rs` | Test event constructors updated with `message_type`. |

## Verification

- `cargo build --workspace` — ✅
- `cargo fmt` — applied
- `cargo test -p ragent-team` — 53 tests pass (16 lib + 6 + 7 M3 + 12 M4 + 8 + 4)
- `cargo test -p ragent-agent --lib` — 352 pass
- `cargo test -p ragent-tui --lib` — 44 pass
- `cargo test -p ragent-tui --test test_teams_tui` — 47 pass
- `cargo test -p ragent-server` — 71+ pass

## Notes / caveats

- The tool source files are single-source via `#[path]` includes (M2
  unification), so all tool edits apply to both `ragent-agent` and
  `ragent-team` automatically.
- `validate()` methods are available but not yet called automatically on every
  load. A follow-up could call `validate()` in `TeamStore::load()` /
  `TaskStore::read()` and surface validation errors; for now they are available
  for callers that want to check invariants.
- `migrate()` is called on every load so existing team directories are
  transparently upgraded to `schema_version = 1` with `updated_at` populated.
  No explicit migration step is needed.
- The `correlation_id` on `MailboxMessage` is `#[serde(default,
  skip_serializing_if = "Option::is_none")]`, so existing mailbox files
  deserialise without it and new messages only serialise it when set.