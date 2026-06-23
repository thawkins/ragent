# Observability Review: Inter-Agent Messages

**Task:** s5 — Review observability of inter-agent messages  
**Scope:** Logging, tracing, monitoring, and debugging support for messages exchanged between the leader and subagents/teammates.  
**Date:** 2026-06-21  
**Reviewer:** swarm-s5

---

## Executive Summary

The project already has a solid **event-based observability foundation**: most inter-agent interactions produce strongly-typed `Event` variants, and the TUI log panel plus the `/events` SSE stream expose them in real time. Mailbox messages carry UUIDs, sender/recipient, type, content, timestamp, and read state.

However, several **gaps remain** that would make production debugging and operational monitoring harder than necessary:

1. **No correlation IDs** linking request/response message pairs (e.g. `PlanRequest` → `PlanApproved`/`PlanRejected`).
2. **No structured send/receive logs** for individual mailbox messages.
3. **No metrics or dashboards** for message volume, latency, failures, or queue depth.
4. **No HTTP API surface** for team mailboxes or team member status.
5. **Two team lifecycle events are silently ignored by the TUI** (`TeammateSuspended`, `TeammateResumed`).
6. **Tracing instrumentation is sparse** around message flows; no spans on mailbox push/drain.
7. **Unused caching hook** for team guidance hints at a planned but unimplemented observability/performance path.
8. **No delivery/read-receipt events** beyond the internal `read` flag.

None of these are security-critical, but collectively they reduce the ability to answer "what happened to message X?" in a running system.

---

## 1. What is observable today

### 1.1 Mailbox message schema (`crates/ragent-team/src/team/mailbox.rs`)

`MailboxMessage` already carries rich context:

```rust
pub struct MailboxMessage {
    pub message_id: String,      // UUID v4
    pub from: String,            // "lead" or agent id
    pub to: String,              // recipient agent id
    pub message_type: MessageType, // Message, Broadcast, PlanRequest, ...
    pub content: String,
    pub sent_at: DateTime<Utc>,
    pub read: bool,
}
```

This gives every message a unique identity, sender/recipient, type, timestamp, and read status.

### 1.2 Team lifecycle events (`crates/ragent-types/src/event/mod.rs`)

The event bus covers the major team transitions:

| Event | Context included |
|-------|------------------|
| `TeammateSpawned` | session_id, team_name, teammate_name, agent_id |
| `TeammateMessage` | session_id, team_name, from, to, preview |
| `TeammateP2PMessage` | session_id, team_name, from, to, preview |
| `TeammateIdle` | session_id, team_name, agent_id |
| `TeammateFailed` | session_id, team_name, agent_id, error |
| `TeammateSuspended` | session_id, team_name, agent_id |
| `TeammateResumed` | session_id, team_name, agent_id |
| `TeamTaskClaimed` | session_id, team_name, agent_id, task_id |
| `TeamTaskCompleted` | session_id, team_name, agent_id, task_id |
| `TeamCleanedUp` | session_id, team_name |

### 1.3 Sub-agent lifecycle events

| Event | Context included |
|-------|------------------|
| `SubagentStart` | session_id, task_id, child_session_id, agent, task, background |
| `SubagentComplete` | session_id, task_id, child_session_id, summary, success, duration_ms |
| `SubagentCancelled` | session_id, task_id |
| `SubagentSuspended` | session_id, task_id, child_session_id |
| `SubagentResumed` | session_id, task_id, child_session_id |
| `SubagentKilled` | session_id, task_id, child_session_id, force |

### 1.4 TUI rendering

`crates/ragent-tui/src/app.rs` handles the above events:

- `SubagentStart/Complete/Cancelled/Suspended/Resumed/Killed` update `active_tasks` and log step-tagged lines to the log panel.
- `TeammateSpawned/TeammateMessage/TeammateP2PMessage/TeammateIdle/TeammateFailed/TeamTaskClaimed/TeamTaskCompleted/TeamCleanedUp` update `team_members` and log to the log panel.
- `team_message_counts: HashMap<String, (u32, u32)>` tracks sent/received message counts per agent.
- `layout_teams.rs` renders a table with `sent`/`recv` columns derived from those counts, plus claimed/done task counts from the team task store.

### 1.5 HTTP/SSE server observability

`crates/ragent-server/src/sse.rs` serializes **all** the events above into named SSE events on `GET /events`.

Sub-agent tasks are exposed via REST:

- `GET/POST /sessions/{id}/tasks` — list/spawn
- `GET/DELETE /sessions/{id}/tasks/{tid}` — get/cancel

The orchestrator exposes:

- `GET /orchestrator/metrics` — active/completed jobs, timeouts, errors
- `POST /orchestrator/start` — start a multi-agent job
- `GET /orchestrator/jobs/{id}` — poll status/result

### 1.6 Tracing already present

`TeamManager` uses `tracing::{info, debug, warn, error}` for:

- teammate spawn/reconciliation lifecycle
- retry backoffs
- token-overflow / permanent API error detection
- mailbox polling failures
- suspend/resume/shutdown operations

`EventBus::publish` warns when events are dropped due to no subscribers or a full channel.

### 1.7 Permission auto-approval

`check_permission_with_prompt` in `processor.rs` hardwires auto-approval for:

- all `team_*` tools
- all `*_task` tools
- `list_tasks`, `wait_tasks`, `task_complete`, `ask_user`, `todo_*`

This prevents interactive permission prompts from interrupting message flows.

---

## 2. Deficiencies

### 2.1 No correlation ID linking message pairs

**Problem:** `MailboxMessage` has a `message_id`, but there is **no `in_reply_to` or `correlation_id`** field. When a teammate sends a `PlanRequest` and the lead later replies with `PlanApproved`/`PlanRejected`, the two mailbox messages are independent. Operators cannot easily trace a reply back to the original request in logs or the mailbox JSON.

**Evidence:**

- `MailboxMessage` definition in `crates/ragent-team/src/team/mailbox.rs:52-68` contains only `message_id`.
- `publish_message_event` in `crates/ragent-team/src/team/manager.rs:1071-1107` translates a single inbound message into an event; no linkage is attempted.
- `team_approve_plan.rs` and `team_submit_plan.rs` create new `MailboxMessage::new(...)` calls without referencing the original plan request id.

**Impact:** Debugging plan-approval deadlocks or slow replies requires manual content matching; no single query can show "all replies to request X".

### 2.2 Mailbox send/receive is not logged

**Problem:** `Mailbox::push` and `Mailbox::drain_unread` perform file I/O under `fs2` locks, but neither routine emits a structured log line at INFO/DEBUG. The only mailbox-related logs are warnings when polling fails.

**Evidence:**

- `crates/ragent-team/src/team/mailbox.rs:188-215` (`push`) unlocks the file and calls `signal_notifier`, but does not log the message id, from/to, or type.
- `crates/ragent-team/src/team/mailbox.rs:218-252` (`drain_unread`) similarly does not log how many unread messages were drained.
- `crates/ragent-team/src/team/manager.rs:855-880` only logs `warn!(..., "Cannot open mailbox for polling")` and `warn!(..., "Cannot drain mailbox")`.

**Impact:** There is no audit trail of when a message was written, by whom, and when it was consumed. Reconstructing message flow requires reading the on-disk `mailbox/{agent_id}.json` files.

### 2.3 No message metrics or dashboard

**Problem:** Message activity is visible only in the TUI, and only while the session is alive. There is no aggregate metric or dashboard for inter-agent messaging.

**Evidence:**

- `team_message_counts` lives only in `crates/ragent-tui/src/app/state.rs:1240` (in-memory `HashMap`). It is cleared when the team is cleaned up.
- `crates/ragent-server/src/routes/mod.rs:529-539` `/orchestrator/metrics` returns `active_jobs`, `completed_jobs`, `timeouts`, `errors` only.
- No `/metrics` or `/teams/{name}/messages` endpoint exists.
- No Prometheus-style counters for messages sent/received/failed, queue depth, or delivery latency.

**Impact:** Operators cannot alert on a teammate that stops reading messages, measure message throughput, or detect a backing-up mailbox.

### 2.4 No HTTP API for team mailboxes or team status

**Problem:** Sub-agents have REST endpoints (`/sessions/{id}/tasks`), but team coordination is only accessible through tool execution.

**Evidence:**

- `crates/ragent-server/src/routes/mod.rs:100-124` defines `/sessions/*`, `/events`, `/memory`, `/research`, and `/orchestrator/*`. There are no `/teams/*` routes.
- `crates/ragent-server/src/sse.rs` streams team events, but clients cannot query current team members, unread message counts, or historical messages via HTTP.

**Impact:** External dashboards or CLI scripts cannot inspect team state without impersonating an agent and calling tools.

### 2.5 TUI ignores `TeammateSuspended` and `TeammateResumed` events

**Problem:** The TUI handles most team events in `handle_event`, but not the explicit suspend/resume events.

**Evidence:**

- `crates/ragent-tui/src/app.rs` matches `TeammateSpawned`, `TeammateMessage`, `TeammateP2PMessage`, `TeammateIdle`, `TeammateFailed`, `TeamTaskClaimed`, `TeamTaskCompleted`, and `TeamCleanedUp`, but **no arms for `TeammateSuspended`/`TeammateResumed`**.
- `layout_teams.rs:334` does render a suspend toggle button, and `app.rs:10630-10632` calls `tm.resume_teammate` / `tm.suspend_teammate`, but the event-driven UI update relies on `refresh_team_member_session_ids()` reading from disk rather than reacting to the event.

**Impact:** If the event arrives before the next render, the UI may briefly show a stale status until the disk refresh catches up. More importantly, the log panel does not record the suspend/resume transitions.

### 2.6 Sparse tracing spans around message flows

**Problem:** While there are many `tracing::info!` lines, there are no `#[tracing::instrument]` spans wrapping the message send/receive or tool-execution paths.

**Evidence:**

- `crates/ragent-team/src/tools/team_message.rs`, `team_broadcast.rs`, `team_read_messages.rs` have no `tracing::` calls.
- `crates/ragent-agent/src/tool/new_task.rs` has no `tracing::` calls for spawn/sync paths.
- `crates/ragent-team/src/team/mailbox.rs` has no tracing.
- `crates/ragent-agent/src/task/mod.rs` logs lifecycle points but does not create spans around `run_subagent`.

**Impact:** Distributed-tracing-style debugging is impossible. A slow message cannot be traced from tool invocation → mailbox write → poll loop → event publication → TUI log without manually stitching logs by timestamp.

### 2.7 Unused `get_team_guidance` cache

**Problem:** `SystemPromptCache` has a `get_team_guidance` method that is documented as part of the cached system-prompt components, but it is **never called**. Instead, the full team-lead/teammate guidance text is concatenated inline in `processor.rs` on every turn.

**Evidence:**

- `crates/ragent-agent/src/session/cache.rs:280-308` defines `get_team_guidance`.
- `crates/ragent-agent/src/session/processor.rs:1136-1199` builds the guidance string directly and calls `get_tool_reference` and `get_codeindex_guidance`, but never `get_team_guidance`.
- `grep` for `get_team_guidance` returns only the definition.

**Impact:** This is not directly an observability bug, but it is a missed optimization that also complicates adding structured team-context instrumentation (e.g. logging which guidance variant was served).

### 2.8 No delivery/read-receipt events

**Problem:** A message is marked `read: true` in the mailbox JSON, but the event bus does not emit an explicit event when a recipient drains or reads a message.

**Evidence:**

- `drain_unread` in `mailbox.rs` mutates `read` flags but publishes nothing.
- `publish_message_event` only fires on poll-loop discovery; it does not distinguish first-time delivery from a message that was already delivered on a previous poll.

**Impact:** There is no reliable way for the TUI or external consumers to know when a message was actually seen by the recipient.

### 2.9 No observability of mailbox size or content volume

**Problem:** There is no tracking of mailbox file size, message byte/char count, or queue depth.

**Evidence:**

- `MailboxMessage` has no `len` or `size` field.
- `Mailbox` has no `len()` or `unread_count()` method exposed.
- No log warns when a mailbox file grows large.

**Impact:** A teammate that is slow to consume messages could accumulate a large mailbox undetected, leading to parse/lock contention.

### 2.10 Dropped-event path is only a log warning

**Problem:** `EventBus::publish` logs a warning when no subscribers are present or the channel is full, but it does not increment a metric or write to a durable dead-letter channel.

**Evidence:**

- `crates/ragent-types/src/event/mod.rs:872-905` only calls `tracing::warn!(...)`.

**Impact:** In a headless/server deployment, dropped events can be lost silently if the log level is set above `warn` or if the log is not actively monitored.

---

## 3. Recommendations (no code changes requested)

1. **Add `in_reply_to` / `correlation_id` to `MailboxMessage`** and populate it for request/reply pairs (`PlanRequest` → `PlanApproved`/`PlanRejected`, `ShutdownRequest` → `ShutdownAck`).
2. **Instrument `Mailbox::push` and `drain_unread`** with structured `tracing::debug!`/`info!` lines including `message_id`, `from`, `to`, `type`, and result status.
3. **Expose team message metrics** either via `/orchestrator/metrics` or a new `/teams/{name}/metrics` endpoint: messages sent/received, unread counts per agent, average delivery latency, dropped events.
4. **Add `/teams/{name}` and `/teams/{name}/members` HTTP endpoints** for external observability, plus an endpoint to peek unread message counts.
5. **Handle `TeammateSuspended` and `TeammateResumed` in the TUI** so the log panel records transitions and the status updates immediately from events rather than disk polling.
6. **Add `#[tracing::instrument]` spans** to `Mailbox::push`, `Mailbox::drain_unread`, `TeamMessageTool::execute`, `NewTaskTool::execute`, and `TaskManager::run_subagent`.
7. **Wire `get_team_guidance` into the system-prompt cache** and use it to serve/cache the team guidance block, which also creates a natural place to log which variant was used.
8. **Emit explicit read/delivery events** (e.g. `TeammateMessageRead`) when a recipient drains a message, or at least include `delivered_at`/`read_at` timestamps in the mailbox model.
9. **Add periodic mailbox health checks** in the poll loop: log mailbox file size, message count, and age of oldest unread message.
10. **Convert dropped-event warnings into a metric counter** and consider a small on-disk event journal for critical team/sub-agent lifecycle events.

---

## 4. Files reviewed

- `crates/ragent-team/src/team/mailbox.rs`
- `crates/ragent-team/src/team/manager.rs`
- `crates/ragent-team/src/tools/team_message.rs`
- `crates/ragent-team/src/tools/team_broadcast.rs`
- `crates/ragent-team/src/tools/team_read_messages.rs`
- `crates/ragent-agent/src/task/mod.rs`
- `crates/ragent-agent/src/tool/new_task.rs`
- `crates/ragent-agent/src/tool/cancel_task.rs`
- `crates/ragent-agent/src/tool/wait_tasks.rs`
- `crates/ragent-agent/src/session/processor.rs`
- `crates/ragent-agent/src/session/cache.rs`
- `crates/ragent-types/src/event/mod.rs`
- `crates/ragent-tui/src/app.rs`
- `crates/ragent-tui/src/app/state.rs`
- `crates/ragent-tui/src/layout_teams.rs`
- `crates/ragent-tui/src/tracing_layer.rs`
- `crates/ragent-tui/src/widgets/message_widget.rs`
- `crates/ragent-server/src/routes/mod.rs`
- `crates/ragent-server/src/sse.rs`
- `crates/ragent-agent/src/orchestrator/coordinator.rs`
- `crates/ragent-config/src/config.rs`
- `crates/ragent-config/src/permission.rs`
