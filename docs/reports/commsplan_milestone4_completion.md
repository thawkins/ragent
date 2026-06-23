# COMMSPLAN Milestone 4 — Completion Report

**Date:** 2026-06-21
**Plan:** `COMMSPLAN.md` §2 Milestone 4
**Status:** ✅ COMPLETE
**Priority:** P1 (correctness, UX)

## Goal

Reduce the chance that messages are lost after being written to the mailbox,
and give senders visibility into delivery.

## Deliverables

- [x] `drain_unread` does not permanently mark messages read until downstream
      processing succeeds (peek + acknowledge path added; `team_read_messages`
      switched to it).
- [x] `team_assign_task` notifies the assigned teammate.
- [x] `team_broadcast` reports per-recipient success/failure.
- [x] `team_message` validates the recipient is active.
- [x] Messages carry enough context for the LLM to understand them
      (`to`, `read`, snake_case `type`).
- [~] Delivery-status outbox (M4-T6) — **deferred** as the plan marks it
      "optional, post-MVP".

## Task-by-task summary

### M4-T1 — Separate "read" from "processed" in mailbox consumption

**Files:** `crates/ragent-team/src/team/mailbox.rs`,
`crates/ragent-team/src/tools/team_read_messages.rs` (+ mirrored copy in
`crates/ragent-agent/src/tool/team_read_messages.rs`).

`Mailbox` gained two methods:

- `peek_unread(&self) -> Result<Vec<MailboxMessage>>` — returns unread
  messages **without** marking them read (acquires a shared lock). This is
  the "peek" half.
- `acknowledge(&self, message_id: &str) -> Result<bool>` — the explicit "I
  processed this message" ack; semantically `mark_read` but named to make the
  peek → process → acknowledge flow explicit. Idempotent: a second ack of an
  already-read message reports `false`.

`drain_unread` is kept as the legacy "read and mark" path for the mailbox poll
loop in `TeamManager`, which treats publishing `Event::TeammateMessage` as the
processing step.

`team_read_messages` was rewritten to: `peek_unread` → build output → on
success, `acknowledge` each message. If anything above returns early via `?`,
the messages stay unread and are redelivered on the next call (at-least-once
delivery).

As part of this, `mark_read` / `acknowledge` now return `changed` instead of
`pos.is_some()`, so acknowledging an already-read message is idempotent and
reports `false` (matching the documented contract — a pre-existing bug where
the second ack reported `true` is fixed).

**Findings addressed:** s3 §5.1/5.2, s4 §3.8.

### M4-T2 — Notify assigned teammate on `team_assign_task`

**File:** `crates/ragent-team/src/tools/team_assign_task.rs` (+ mirrored copy).

After `task_store.update_task` marks the task `InProgress` and sets
`assigned_to`, the tool pushes a `MailboxMessage { type: Message, from: "lead",
to: agent_id, content: "Task '{id}' has been assigned to you by the lead.
Title: {title}. …" }` to the assignee's mailbox. The notification outcome
(`delivered` / `failed: …` / `failed to open mailbox: …`) is recorded in the
tool output so the lead has visibility. A failure to notify does **not** roll
back the assignment (the task is already `InProgress` on disk); the message
is best-effort.

The tool also now rejects assignment to `Stopped` / `Failed` teammates up
front (previously only checked membership).

**Findings addressed:** s2 Issue 8, s4 Issue 9, s2 Issue 21.

### M4-T3 — Return per-recipient results from `team_broadcast`

**File:** `crates/ragent-team/src/tools/team_broadcast.rs` (+ mirrored copy).

The `?` early-return on the first `Mailbox::open` / `push` failure is replaced
with a loop that collects `Result` per recipient. A failure on one teammate
no longer aborts delivery to the rest. The tool output now includes
`succeeded: [agent_id, …]` and `failed: [{ agent_id, name, error }, …]`
arrays in the JSON metadata, and the human-readable content notes the failure
count when non-zero.

**Findings addressed:** s2 Issue 7.

### M4-T4 — Validate recipient state in `team_message`

**File:** `crates/ragent-team/src/tools/team_message.rs` (+ mirrored copy).

Before pushing, the tool loads `TeamStore` and:
- rejects messages to unknown agent IDs (`not a member of team …`),
- rejects messages to `Stopped` / `Failed` teammates (`… is stopped/failed in
  team … and cannot receive messages`),
- allows messages to `lead` and active teammates.

Previously a typo like `tm-999` or a message to a dead teammate would succeed
and sit unread forever while the sender got a false success.

**Findings addressed:** s2 Issue 9, M8-T5 (partial — `resolve_agent_id` still
accepts any `tm-…` string, but `team_message` now validates against the
member list before pushing).

### M4-T5 — Fix `team_read_messages` output schema

**File:** `crates/ragent-team/src/tools/team_read_messages.rs` (+ mirrored copy).

- `type` is now serialised via `serde_json::to_value(&m.message_type)`, which
  honours `#[serde(rename_all = "snake_case")]` and produces `plan_request`
  (matching the on-disk format) instead of `format!("{:?}", …)` which produced
  `PlanRequest` (PascalCase). LLM round-trips no longer mismatch.
- The JSON metadata now includes `to` and `read` fields.
- The human-readable text now shows `To: {to}` and the snake_case type, so P2P
  messages carry recipient context.

**Findings addressed:** s4 Issues 4–5.

### M4-T6 — Delivery-status outbox (optional, post-MVP)

Deferred. The plan explicitly marks this "optional, post-MVP". The peek +
acknowledge path (M4-T1) provides the foundation for future retries /
dead-letter handling without requiring the outbox file.

## Files modified

| File | Change |
|------|--------|
| `crates/ragent-team/src/team/mailbox.rs` | Added `peek_unread`, `acknowledge`; fixed `mark_read` to return `changed` (idempotent ack). |
| `crates/ragent-team/src/tools/team_read_messages.rs` | Peek + ack flow; snake_case `type`; `to`/`read` fields. |
| `crates/ragent-team/src/tools/team_assign_task.rs` | Notify assignee via mailbox; reject dead assignees. |
| `crates/ragent-team/src/tools/team_broadcast.rs` | Per-recipient result collection; `succeeded`/`failed` metadata. |
| `crates/ragent-team/src/tools/team_message.rs` | Recipient state validation (reject Stopped/Failed/unknown). |
| `crates/ragent-agent/src/tool/team_read_messages.rs` | Mirrored copy. |
| `crates/ragent-agent/src/tool/team_assign_task.rs` | Mirrored copy. |
| `crates/ragent-agent/src/tool/team_broadcast.rs` | Mirrored copy. |
| `crates/ragent-agent/src/tool/team_message.rs` | Mirrored copy. |
| `crates/ragent-team/tests/test_m4_delivery.rs` | New integration test suite (12 tests). |
| `CHANGELOG.md` | New "COMMSPLAN Milestone 4" section under 0.1.0-alpha.114. |

## Verification

- `cargo build --workspace` — ✅
- `cargo clippy -p ragent-team -p ragent-agent` — ✅ no new warnings
- `cargo fmt` — applied
- `cargo test -p ragent-team` — 53 tests pass (16 lib + 6 + 7 M3 + **12 M4** + 8 + 4)
- `cargo test -p ragent-agent --lib` — 352 pass

## Notes / caveats

- The tool source files (`team_read_messages.rs`, `team_assign_task.rs`,
  `team_broadcast.rs`, `team_message.rs`) are duplicated between
  `ragent-agent/src/tool/` and `ragent-team/src/tools/`. M4 edits both copies
  identically (verified with `diff -q` after each copy). Eliminating the
  duplication is **Milestone 2's** scope (`COMMSPLAN.md` §2 M2-T1).
- `team_assign_task`'s notification is best-effort: a mailbox push failure
  does not roll back the `InProgress` assignment. The outcome is reported in
  the tool output so the lead can retry with `team_message` if needed.
- `team_message` still accepts any `tm-…` string in `resolve_agent_id` (for
  backward compat with callers that pass agent IDs directly), but now
  validates the resolved ID against the member list and rejects dead/unknown
  recipients **before** pushing. M8-T5 will tighten `resolve_agent_id` itself.
- M4-T6 (delivery-status outbox) is deferred as the plan marks it optional;
  the peek + acknowledge foundation is in place for a future outbox.

## Next milestones

Per `COMMSPLAN.md` §3, Milestones 5–8 can proceed largely in parallel. The
M5-T6 task (distinguish message types in events) should align with M4-T2's
task-assignment notification and M3-T4's `TeammateIdle` event so the TUI/SSE
display the new events correctly.