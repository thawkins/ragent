# Message Schemas and Serialization Review

**Task ID:** s4  
**Reviewer:** swarm-s4  
**Date:** 2026-06-21  

## Scope

Reviewed all message types, schemas, data structures, and serialization/deserialization
logic used for inter-agent communication in the `ragent-team` and `ragent-agent` crates.

Files examined:
- `crates/ragent-team/src/team/mailbox.rs` — MailboxMessage, MessageType, Mailbox
- `crates/ragent-team/src/team/task.rs` — Task, TaskStatus, TaskList, TaskStore
- `crates/ragent-team/src/team/config.rs` — TeamConfig, TeamMember, MemberStatus, PlanStatus
- `crates/ragent-team/src/team/store.rs` — TeamStore
- `crates/ragent-team/src/team/swarm.rs` — SwarmSubtask, SwarmDecomposition, SwarmState
- `crates/ragent-team/src/team/manager.rs` — publish_message_event, shutdown_teammate
- `crates/ragent-team/src/tools/*.rs` — All 20 team tools
- `crates/ragent-agent/src/team/*.rs` — Duplicate team module
- `crates/ragent-types/src/event/mod.rs` — Event types for team communication
- `crates/ragent-server/src/sse.rs` — SSE event serialization
- `crates/ragent-tui/src/app.rs` — TUI event handling

---

## Issues Found

### ISSUE 1: CRITICAL — TOCTOU Race Condition in Mailbox and TaskStore writes (ragent-team version)

**Files:**  
- `crates/ragent-team/src/team/mailbox.rs` lines 207-209, 246-247, 282-283  
- `crates/ragent-team/src/team/task.rs` lines 238-239, 341-342, 397-398, 485-486, 527-528, 562-563  

**Description:**  
The `ragent-team` versions of `Mailbox::push()`, `Mailbox::drain_unread()`,
`Mailbox::mark_read()`, and all `TaskStore` mutating methods follow this pattern:

```rust
file.lock_exclusive()?;
// ... read and modify in-memory data ...
file.unlock()?;                         // ← Lock released HERE
Self::write_atomic(&self.path, &data)?; // ← Write happens AFTER unlock
```

Between `unlock()` and `write_atomic()`, another process can lock the file, read
the old content, make its own modifications, unlock, and write. The first process's
`write_atomic()` then overwrites the second process's changes, **silently losing
messages or task state updates**.

The `ragent-agent` versions of these same files fix this by using `write_locked()`,
which writes while still holding the lock:
```rust
Self::write_locked(&mut file, &messages)?;  // Write while locked
file.unlock()?;                              // Then unlock
```

**Impact:** Message loss and task state corruption under concurrent access from
multiple teammates. This is a data-loss bug.

**Fix:** Backport the `write_locked()` pattern from `ragent-agent` to `ragent-team`,
or eliminate the code duplication (see Issue 8).

---

### ISSUE 2: HIGH — Divergent Code Duplication Between ragent-team and ragent-agent

**Files:**  
- `crates/ragent-team/src/team/mailbox.rs` vs `crates/ragent-agent/src/team/mailbox.rs` (55 diff lines)  
- `crates/ragent-team/src/team/task.rs` vs `crates/ragent-agent/src/team/task.rs` (63 diff lines)  
- `crates/ragent-team/src/team/manager.rs` vs `crates/ragent-agent/src/team/manager.rs` (96 diff lines)  
- `crates/ragent-team/src/team/swarm.rs` vs `crates/ragent-agent/src/team/swarm.rs` (98 diff lines)  
- `crates/ragent-team/src/team/config.rs` vs `crates/ragent-agent/src/team/config.rs` (4 diff lines)  

**Description:**  
The entire team module is duplicated across two crates. `ragent-agent` does NOT
depend on `ragent-team` (confirmed in Cargo.toml); instead, both crates maintain
independent copies of the same source files. Only `store.rs` uses a `#[path = ...]`
include to share the implementation.

The copies have **diverged significantly**:
- `ragent-team/mailbox.rs` uses `write_atomic` (temp file + rename, after unlock) — has TOCTOU race
- `ragent-agent/mailbox.rs` uses `write_locked` (in-place write, before unlock) — race-free
- `ragent-team/task.rs` uses `write_atomic` — has TOCTOU race
- `ragent-agent/task.rs` uses `write_locked` — race-free
- `ragent-team/manager.rs` has inline helper functions (`is_token_overflow_error`, `is_permanent_api_error`)
- `ragent-agent/manager.rs` imports these from `session::processor`

**Impact:** Bug fixes in one copy may not be applied to the other. The TOCTOU race
(Issue 1) is a direct consequence. Any future schema changes must be applied twice.

**Fix:** Make `ragent-agent` depend on `ragent-team` as a library crate, or extract
the shared types into a common crate (e.g., `ragent-team-types`).

---

### ISSUE 3: HIGH — Inconsistent TaskStatus Representation Across Output Paths

**Files:**  
- `crates/ragent-team/src/team/task.rs` line 19 — `#[serde(rename_all = "lowercase")]`  
- `crates/ragent-team/src/tools/team_task_list.rs` line 89 — `format!("{:?}", t.status).to_lowercase()`  
- `crates/ragent-team/src/tools/team_task_claim.rs` lines 72-77 — manual match with hyphenated strings  
- `crates/ragent-team/src/tools/team_task_complete.rs` lines 86-91 — manual match with hyphenated strings  

**Description:**  
`TaskStatus::InProgress` is serialized three different ways:

| Path | Format | Example |
|------|--------|---------|
| serde on-disk (`tasks.json`) | `lowercase` with underscore | `"in_progress"` |
| `team_task_list` JSON metadata | Debug format + `to_lowercase()` | `"inprogress"` (no separator) |
| `team_task_claim` / `team_task_complete` debug logging | manual match | `"in-progress"` (hyphen) |

**Impact:** Any consumer that compares status strings across these paths will fail.
The on-disk format (`"in_progress"`) differs from the tool output format
(`"inprogress"`), which differs from the debug log format (`"in-progress"`).
An LLM or external client parsing these would get inconsistent values.

**Fix:** Use `serde_json::to_value(&t.status)` or a shared helper function for all
status-to-string conversions.

---

### ISSUE 4: HIGH — Inconsistent MessageType Serialization (snake_case vs PascalCase)

**Files:**  
- `crates/ragent-team/src/team/mailbox.rs` line 28 — `#[serde(rename_all = "snake_case")]`  
- `crates/ragent-team/src/tools/team_read_messages.rs` line 88 — `format!("{:?}", m.message_type)`  

**Description:**  
`MessageType` uses `#[serde(rename_all = "snake_case")]`, so on-disk serialization
produces `"message"`, `"broadcast"`, `"plan_request"`, `"plan_approved"`, etc.

But `team_read_messages.rs` serializes the type for tool output as:
```rust
"type": format!("{:?}", m.message_type)
```
This produces `"Message"`, `"Broadcast"`, `"PlanRequest"`, `"PlanApproved"`, etc.
(PascalCase Debug format).

The LLM receives PascalCase from `team_read_messages` but the on-disk format is
snake_case. Any logic that compares or round-trips these values will mismatch.

**Fix:** Use `serde_json::to_value(&m.message_type)` instead of `format!("{:?}", ...)`.

---

### ISSUE 5: HIGH — Missing Fields in team_read_messages JSON Output

**File:** `crates/ragent-team/src/tools/team_read_messages.rs` lines 82-93  

**Description:**  
The JSON metadata for each message includes:
```json
{
  "message_id": "...",
  "from": "...",
  "type": "...",
  "content": "...",
  "sent_at": "..."
}
```

But `MailboxMessage` has 7 fields. Missing from the output:
- `"to"` — recipient agent ID (critical for P2P message context)
- `"read"` — read status (always false here since these are unread, but omitted)

The human-readable text output (lines 72-79) also omits the "To" field, showing only
"From", "Type", timestamp, and content. For peer-to-peer messages, the recipient is
essential context for the agent to understand who the message was for.

**Fix:** Add `"to": m.to` and `"read": m.read` to the JSON metadata, and include
the recipient in the text output.

---

### ISSUE 6: MEDIUM — No Schema Versioning on Any Serialized Structure

**Files:** All serialized types in `crates/ragent-team/src/team/`  

**Description:**  
None of the on-disk serialized types include a version field:
- `TeamConfig` (config.json) — no `schema_version` or `version` field
- `TaskList` (tasks.json) — no version field
- `MailboxMessage` (mailbox/*.json) — no version field
- `SwarmState` — no version field
- `TeamMember` — no version field

If fields are added, renamed, or removed in future versions, old on-disk files will
fail to deserialize with a generic serde error, providing no migration path.

**Impact:** Backward compatibility breaks on any schema change. No way to detect
or migrate old formats.

**Fix:** Add a `#[serde(default)] schema_version: u32` field to each root type
(`TeamConfig`, `TaskList`) and bump it on breaking changes.

---

### ISSUE 7: MEDIUM — No Validation of Incoming Messages on Deserialization

**Files:**  
- `crates/ragent-team/src/team/mailbox.rs` lines 173, 204, 239, 276 — `serde_json::from_str`  
- `crates/ragent-team/src/team/task.rs` lines 172, 211, 271, 364, 510, 552 — `serde_json::from_str`  
- `crates/ragent-team/src/team/store.rs` line 158 — `serde_json::from_str`  

**Description:**  
All deserialization uses `serde_json::from_str` directly, with no post-deserialization
validation. There are no checks for:
- `from` field is a valid agent ID (starts with "tm-" or equals "lead")
- `to` field is a valid agent ID or "lead"
- `message_id` is a valid UUID format
- `sent_at` is a reasonable timestamp (not future-dated, not epoch zero)
- `task.id` matches the expected "task-NNN" format
- `assigned_to` references a valid agent ID

A corrupted or manually-edited file could contain arbitrary values that pass
deserialization but cause logic errors downstream.

**Fix:** Add a `validate()` method to `MailboxMessage`, `Task`, and `TeamConfig`
that checks field formats and cross-references.

---

### ISSUE 8: MEDIUM — No Correlation ID for Request/Reply Message Pairs

**File:** `crates/ragent-team/src/team/mailbox.rs` lines 52-68  

**Description:**  
`MailboxMessage` has no `correlation_id` or `reply_to` field. The plan approval
flow involves multiple messages:
1. Teammate sends `PlanRequest` to lead
2. Lead sends `PlanApproved` or `PlanRejected` to teammate

There is no way to correlate the approval/rejection with the specific plan request.
If a teammate submits multiple plans (or multiple teammates submit plans), the
approval message has no reference to which plan it approves.

Similarly, `ShutdownRequest` and `ShutdownAck` have no correlation, though this is
less critical since shutdown is a terminal operation.

**Impact:** Ambiguity in plan approval tracking. A teammate receiving `PlanApproved`
cannot determine which of its submitted plans was approved.

**Fix:** Add an optional `correlation_id: Option<String>` field to `MailboxMessage`.
Set it on request messages and copy it to reply messages.

---

### ISSUE 9: MEDIUM — team_assign_task Silently Ignores Non-Pending Tasks

**File:** `crates/ragent-team/src/tools/team_assign_task.rs` lines 79-85  

**Description:**  
```rust
let task = task_store.update_task(task_id, |t| {
    if t.status == TaskStatus::Pending {
        t.status = TaskStatus::InProgress;
        t.assigned_to = Some(agent_id.clone());
        t.claimed_at = Some(chrono::Utc::now());
    }
})?;
```

The closure only modifies the task if it's `Pending`. If the task is already
`InProgress` or `Completed`, the closure silently does nothing. The tool then
returns success with the task's current (unchanged) state, giving the lead no
indication that the assignment was skipped.

**Impact:** The lead believes a task was assigned when it wasn't. The teammate
won't see the task as theirs, leading to confusion.

**Fix:** Return an error or a distinct metadata field (`"assigned": false`) when
the task is not in a claimable state.

---

### ISSUE 10: MEDIUM — Task.complete() Auto-Claims Unclaimed Tasks

**File:** `crates/ragent-team/src/team/task.rs` lines 379-392  

**Description:**  
```rust
if task.assigned_to.as_deref() != Some(agent_id) {
    if task.status == TaskStatus::Pending || task.assigned_to.is_none() {
        task.assigned_to = Some(agent_id.to_owned());
        task.claimed_at = Some(Utc::now());
        task.status = TaskStatus::InProgress;
    } else {
        // ... error ...
    }
}
```

Any agent can complete any Pending/unassigned task, even if it was meant for
another agent. The auto-claim silently reassigns the task to the completing agent.

**Impact:** Task stealing. Agent A could complete a task that was intended for
Agent B (e.g., pre-assigned via `team_assign_task` but still in Pending state).

**Fix:** Only auto-claim if `assigned_to.is_none()`. If `assigned_to` is `Some(other)`,
reject the completion.

---

### ISSUE 11: MEDIUM — No `deny_unknown_fields` on Any Serialized Struct

**Files:** All structs with `#[derive(Serialize, Deserialize)]` in `crates/ragent-team/src/team/`  

**Description:**  
None of the serialized structs use `#[serde(deny_unknown_fields)]`. This means:
- Typos in field names are silently ignored (the field is just absent)
- Extra fields from future versions or manual edits are silently dropped
- A misspelled `"form"` instead of `"from"` in a mailbox file would deserialize
  successfully with `from` as the default (empty string), producing a message
  with no sender

**Fix:** Add `#[serde(deny_unknown_fields)]` to at least `MailboxMessage`, `Task`,
and `TeamConfig`, or add `#[serde(default)]` to all optional fields and validate
required fields post-deserialization.

---

### ISSUE 12: MEDIUM — TeamConfig Has No `updated_at` Timestamp

**File:** `crates/ragent-team/src/team/config.rs` lines 222-236  

**Description:**  
`TeamConfig` has `created_at` but no `updated_at`. Members are constantly modified
(status changes, task assignments, plan status updates), but there's no record of
when the config was last modified.

**Impact:** Impossible to detect stale configs or debug race conditions where two
processes modify the config concurrently. The file's mtime is the only indicator,
but that's not part of the schema.

**Fix:** Add `#[serde(default)] updated_at: Option<DateTime<Utc>>` and update it
on every `save()`.

---

### ISSUE 13: LOW — MessageType::IdleNotify Is Dead Code

**Files:**  
- `crates/ragent-team/src/team/mailbox.rs` line 41 — enum variant defined  
- `crates/ragent-team/src/team/manager.rs` line 1025 — handled in `publish_message_event`  
- `crates/ragent-team/src/tools/team_idle.rs` — does NOT push an IdleNotify message  

**Description:**  
`MessageType::IdleNotify` is defined and handled in `publish_message_event`, but
no code ever pushes a message with this type. The `team_idle` tool only:
1. Updates `TeamMember.status` to `Idle` in config
2. Fires the `TeammateIdle` hook
3. Returns success

It does NOT send an `IdleNotify` message to the lead's mailbox. The lead discovers
idle state through the `Event::TeammateIdle` event bus publication (from the poll
loop reading config changes), not through the mailbox.

The `IdleNotify` branch in `publish_message_event` is dead code — it can never
be reached because no code creates a message with that type.

**Fix:** Either remove `MessageType::IdleNotify` and its handling, or have
`team_idle` push an `IdleNotify` message to the lead's mailbox for consistency
with other message types.

---

### ISSUE 14: LOW — SwarmSubtask ID Format vs Task ID Format Mismatch

**Files:**  
- `crates/ragent-team/src/team/swarm.rs` line 15 — IDs like `"s1"`, `"s2"`  
- `crates/ragent-team/src/team/store.rs` line 271 — IDs like `"task-001"`  
- `crates/ragent-team/src/team/swarm.rs` line 22 — `depends_on` references `"s1"` style IDs  

**Description:**  
Swarm decomposition uses short IDs (`"s1"`, `"s2"`) for subtasks and their
dependencies. When these are converted to team tasks, IDs become `"task-001"`,
`"task-002"`, etc. The `depends_on` references must be translated from `"s1"`
to `"task-001"` during this conversion.

If the translation is incorrect or incomplete, dependency chains break silently
— a task's `depends_on` might reference `"s1"` which doesn't match any task ID
in the task store.

**Fix:** Ensure the swarm-to-task conversion translates all `depends_on` references,
and add validation that all `depends_on` IDs exist in the task list after conversion.

---

### ISSUE 15: LOW — TeammateMessage and TeammateP2PMessage Produce Identical SSE Payloads

**File:** `crates/ragent-server/src/sse.rs` lines 633-645, 714-726  

**Description:**  
Both `Event::TeammateMessage` and `Event::TeammateP2PMessage` serialize using the
same `TeammateMessageP` struct, producing identical JSON payloads:
```json
{
  "session_id": "...",
  "team_name": "...",
  "from": "...",
  "to": "...",
  "preview": "..."
}
```

They differ only in the SSE event type name (`"teammate_message"` vs
`"teammate_p2p_message"`). Clients that parse the JSON payload without checking
the event type cannot distinguish between lead-directed and peer-to-peer messages.

**Impact:** Low — clients should check the event type name. But the identical
payload structure may cause confusion.

**Fix:** Consider adding a `"message_type": "p2p"` or `"message_type": "lead"` field
to the payload, or accept this as intentional design.

---

## Summary

| # | Severity | Issue |
|---|----------|-------|
| 1 | CRITICAL | TOCTOU race in mailbox/task writes (ragent-team version) |
| 2 | HIGH | Divergent code duplication between ragent-team and ragent-agent |
| 3 | HIGH | Inconsistent TaskStatus string representation (3 different formats) |
| 4 | HIGH | Inconsistent MessageType serialization (snake_case vs PascalCase) |
| 5 | HIGH | Missing "to" and "read" fields in team_read_messages JSON output |
| 6 | MEDIUM | No schema versioning on any serialized structure |
| 7 | MEDIUM | No validation of incoming messages on deserialization |
| 8 | MEDIUM | No correlation ID for request/reply message pairs |
| 9 | MEDIUM | team_assign_task silently ignores non-pending tasks |
| 10 | MEDIUM | Task.complete() auto-claims unclaimed tasks (task stealing risk) |
| 11 | MEDIUM | No deny_unknown_fields on any serialized struct |
| 12 | MEDIUM | TeamConfig has no updated_at timestamp |
| 13 | LOW | MessageType::IdleNotify is dead code |
| 14 | LOW | SwarmSubtask ID format vs Task ID format mismatch |
| 15 | LOW | TeammateMessage and TeammateP2PMessage produce identical SSE payloads |

**Total: 15 issues** — 1 Critical, 4 High, 7 Medium, 3 Low