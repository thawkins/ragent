# Teams — Agent Team Coordination for ragent

> Specification, milestones and tasks for the "Teams" capability.
> Inspired by Anthropic's [Agent Teams](https://code.claude.com/docs/en/agent-teams) pattern,
> adapted for ragent's Rust TUI architecture.

---

## 1. Overview

**Teams** lets one ragent session act as a _team lead_, spawning and coordinating multiple
_teammate_ agent sessions that each run in their own context window. Unlike subagents (which
are ephemeral workers that report only to the lead), teammates have persistent named identities,
can message each other directly, and share a common task list to coordinate work without the
lead acting as a bottleneck.

### 1.1 When to use Teams vs Subagents

| Dimension | Subagents (`new_task`) | Teams |
|-----------|------------------------|-------|
| Context | Own context; result summarised back | Own context; fully independent |
| Communication | Reports to lead only | Teammates message each other directly |
| Coordination | Lead manages all work | Shared task list; self-coordinating |
| Persistence | Ephemeral; destroyed on completion | Named; persist until team cleanup |
| Best for | Focused tasks; result is all that matters | Complex work requiring collaboration |
| Token cost | Lower | Higher (scales with active teammates) |

**Use Teams when:**
- Research with multiple independent angles (parallel code review, competing hypotheses)
- New features/modules where teammates each own a different file set without overlap
- Debugging where multiple theories need simultaneous investigation
- Cross-layer changes (API, UI, tests) owned by dedicated specialist teammates

**Stick with subagents when:**
- Tasks are sequential or share files
- Only the final result matters, not the investigation path
- Token budget is constrained

---

## 2. Architecture

### 2.1 Components

```
┌─────────────────────────────────────────────────────────────────┐
│  TUI (Team Lead)                                                 │
│  ┌──────────────┐  ┌──────────────────┐  ┌──────────────────┐  │
│  │ Chat Panel   │  │ Teams Panel      │  │ Log Panel        │  │
│  │ (lead chat)  │  │ (team status)    │  │ (team events)    │  │
│  └──────────────┘  └──────────────────┘  └──────────────────┘  │
└────────────────────────────┬────────────────────────────────────┘
                             │ Tools
          ┌──────────────────┼──────────────────┐
          │                  │                  │
   ┌──────▼──────┐   ┌───────▼──────┐  ┌───────▼──────┐
   │ TeamManager  │   │ TaskStore    │  │ Mailbox      │
   │ (spawns,     │   │ (shared task │  │ (agent-to-   │
   │  tracks,     │   │  list with   │  │  agent msgs) │
   │  cleans up)  │   │  file locks) │  └──────────────┘
   └──────┬───────┘   └──────────────┘
          │
     ┌────┴────────────────────────┐
     │  Teammate Sessions           │
     │  ┌──────┐ ┌──────┐ ┌──────┐ │
     │  │ tm-A │ │ tm-B │ │ tm-C │ │
     │  │ (own │ │ (own │ │ (own │ │
     │  │ ctx) │ │ ctx) │ │ ctx) │ │
     │  └──────┘ └──────┘ └──────┘ │
     └─────────────────────────────┘
```

### 2.2 Storage Layout

Teams and tasks are stored locally so they survive process restarts:

```
~/.ragent/teams/{team-name}/
    config.json          # Team metadata and member list
    tasks.json           # Shared task list (file-locked on write)
    mailbox/
        {agent-id}.json  # Per-agent inbound message queue

[PROJECT]/.ragent/teams/{team-name}/   # Project-local teams (higher priority)
    (same structure)
```

### 2.3 Team Config Schema (`config.json`)

```json
{
  "name": "my-review-team",
  "lead_session_id": "sess-abc123",
  "created_at": "2026-03-19T05:32:47Z",
  "status": "active",
  "members": [
    {
      "name": "security-reviewer",
      "agent_id": "tm-001",
      "session_id": "sess-def456",
      "agent_type": "general",
      "status": "working",
      "current_task_id": "task-003"
    }
  ],
  "settings": {
    "max_teammates": 8,
    "require_plan_approval": false,
    "auto_claim_tasks": true
  }
}
```

### 2.4 Task List Schema (`tasks.json`)

```json
{
  "team_name": "my-review-team",
  "tasks": [
    {
      "id": "task-001",
      "title": "Review authentication module for security issues",
      "description": "...",
      "status": "completed",
      "assigned_to": "tm-001",
      "depends_on": [],
      "created_at": "...",
      "claimed_at": "...",
      "completed_at": "..."
    },
    {
      "id": "task-002",
      "title": "Review performance of database queries",
      "status": "pending",
      "assigned_to": null,
      "depends_on": ["task-001"]
    }
  ]
}
```

### 2.5 Mailbox Message Schema

```json
{
  "message_id": "msg-uuid",
  "from": "tm-001",
  "to": "lead",
  "type": "message|broadcast|plan_request|plan_approved|plan_rejected|idle_notify|shutdown_request|shutdown_ack",
  "content": "...",
  "sent_at": "2026-03-19T05:32:47Z",
  "read": false
}
```

---

## 3. LLM-Visible Tools

The lead session receives these tools; teammates receive a smaller subset.

### 3.1 Lead-only tools

| Tool | Description |
|------|-------------|
| `team_create` | Create a new named team; returns team config |
| `team_spawn` | Spawn one or more named teammates with a prompt |
| `team_assign_task` | Assign a specific task to a specific teammate |
| `team_broadcast` | Send a message to all active teammates |
| `team_approve_plan` | Approve or reject a teammate's plan submission |
| `team_shutdown_teammate` | Request graceful shutdown of one teammate |
| `team_cleanup` | Tear down team resources (requires all teammates stopped) |
| `team_status` | List team members, their status, and task progress |

### 3.2 Lead + Teammate tools

| Tool | Description |
|------|-------------|
| `team_message` | Send a direct message to one team member by name |
| `team_task_list` | Read all tasks and their status |
| `team_task_claim` | Atomically claim the next available task (file-locked) |
| `team_task_complete` | Mark a task as completed and unblock dependents |
| `team_task_update` | Update task description or status |
| `team_read_messages` | Read unread inbound messages from the mailbox |
| `team_submit_plan` | (Teammate) Submit a plan to the lead for approval |
| `team_idle` | (Teammate) Notify lead of idle state; triggers TeammateIdle hook |
| `team_shutdown_ack` | (Teammate) Acknowledge shutdown request |

---

## 4. TUI Integration

### 4.1 Teams Panel

A new panel in the TUI (when a team is active) shows the full team tree:

```
 Teams ─────────────────────────────────────────────
 ● lead (you)              primary      steps:12
 ├○ security-reviewer      working  2m3s steps:8
 ├○ perf-reviewer          idle     4m1s steps:15   [done]
 └○ coverage-reviewer      planning 1m2s steps:3    [plan?]
 3 teammates | 2/5 tasks done
```

### 4.2 `/team` Slash Commands

| Command | Description |
|---------|-------------|
| `/team` | Show team status (same as `/team status`) |
| `/team create <name>` | Create a new team and set lead |
| `/team status` | Show all teammates and task progress |
| `/team message <name> <text>` | Send a direct message to a teammate |
| `/team tasks` | List all tasks with status and assignments |
| `/team cleanup` | Clean up the current team |

### 4.3 Agents Window Integration

- Teammates appear in the existing **Agents** window below sub-agents
- Badge **`[T]`** in bold blue (distinct from custom agent **`[C]`** in magenta)
- Shows teammate name, status (working/idle/planning), elapsed time, step count

### 4.4 Log Panel Events

New log-level events surfaced in the log panel:
- `[team] teammate <name> spawned`
- `[team] teammate <name> claimed task <id>`
- `[team] teammate <name> completed task <id>`
- `[team] teammate <name> went idle`
- `[team] teammate <name> submitted plan for approval`
- `[team] lead approved/rejected plan from <name>`
- `[team] message from <name> to <target>: <preview>`

---

## 5. Teammate Execution

### 5.1 Spawn Sequence

1. Lead calls `team_spawn` tool with teammate name, agent type, and spawn prompt
2. `TeamManager` creates a new named session for the teammate
3. System prompt is augmented with team context:
   - Team name and lead identity
   - Other member names (for messaging)
   - Task list URL (path to `tasks.json`)
   - Mailbox path
   - Available team tools subset
4. Teammate starts processing its spawn prompt
5. `Event::TeammateSpawned` is published to TUI

### 5.2 Teammate System Prompt Additions

```
## Team Context

You are a teammate in team "{{TEAM_NAME}}". Your name is "{{TEAMMATE_NAME}}".
The team lead is "lead". Other teammates: {{TEAMMATE_LIST}}.

Use `team_read_messages` at the start of each turn to check for new messages.
Use `team_task_claim` to claim your next task when you finish one.
Use `team_message` to communicate findings to other teammates or the lead.
Use `team_idle` when you have no more tasks to work on.
```

### 5.3 Plan Approval Workflow

1. Teammate calls `team_submit_plan` with its planned approach
2. Teammate enters read-only mode (no write/bash tools active)
3. Lead receives a `plan_request` mailbox message
4. Lead calls `team_approve_plan` (approve or reject with feedback)
5. Teammate receives result:
   - **Approved**: exits plan mode, begins implementation
   - **Rejected**: revises plan and resubmits

### 5.4 Idle & Shutdown

- **Idle**: Teammate calls `team_idle` → `TeammateIdle` hook runs → if hook exits 2, feedback is sent back and teammate keeps working → otherwise teammate waits
- **Shutdown**: Lead calls `team_shutdown_teammate` → teammate receives `shutdown_request` mailbox message → teammate calls `team_shutdown_ack` → session terminates

### 5.5 Task Claiming (Race-Free)

`team_task_claim` uses `flock`-based file locking on `tasks.json`:
1. Acquire exclusive file lock
2. Find first pending task with no unresolved dependencies
3. Mark it `in_progress`, set `assigned_to` to caller's agent ID
4. Write file, release lock
5. Return task details (or "no tasks available")

---

## 6. Quality Gates (Hooks)

Hooks are shell scripts or executables configured in `ragent.json`:

```json
{
  "hooks": {
    "TeammateIdle": {
      "command": ".ragent/hooks/teammate-idle.sh",
      "args": ["{{TEAM_NAME}}", "{{TEAMMATE_NAME}}"]
    },
    "TaskCompleted": {
      "command": ".ragent/hooks/task-completed.sh",
      "args": ["{{TASK_ID}}", "{{TEAMMATE_NAME}}"]
    }
  }
}
```

- Exit `0`: allow idle/completion
- Exit `2`: send hook stdout as feedback; keep working / block completion
- Any other exit: log warning; allow idle/completion

---

## 7. Configuration

Settings in `ragent.json`:

```json
{
  "teams": {
    "max_teammates": 8,
    "default_require_plan_approval": false,
    "auto_claim_tasks": true,
    "mailbox_poll_interval_ms": 500,
    "task_claim_lock_timeout_ms": 5000
  }
}
```

---

## 8. Limitations (Initial Release)

- No session resumption for active teammates (known limitation; document clearly)
- One active team per lead session
- Teammates cannot spawn sub-teams (no nested teams)
- Split-pane display (tmux/iTerm2) is out of scope for V1; in-process only
- Per-teammate permission modes cannot be set at spawn time (inherits lead permissions)
- Teammate context windows are independent; no shared memory beyond tasks/mailbox

---

## 9. Milestones and Tasks

---

### M1 — Core Data Structures & Storage

**Goal**: Establish the on-disk data model for teams, tasks, and mailboxes. No LLM integration yet; just the Rust types and I/O layer.

**Deliverables**:
- `crates/ragent-core/src/team/` module (new crate module)
- Team config, task list, and mailbox types with serde
- File-locked task claiming
- Discovery of team directories (project-local and global)

**Tasks**:

| ID | Task | Notes |
|----|------|-------|
| M1-T1 | Create `crates/ragent-core/src/team/mod.rs` — public module with re-exports | Add `pub mod team;` in `lib.rs` |
| M1-T2 | Define `TeamConfig`, `TeamMember`, `TeamStatus`, `MemberStatus` structs with serde | In `team/config.rs` |
| M1-T3 | Define `Task`, `TaskStatus`, `TaskList` structs with serde | In `team/task.rs` |
| M1-T4 | Define `MailboxMessage`, `MessageType` structs with serde | In `team/mailbox.rs` |
| M1-T5 | Implement `TaskStore::claim_next()` with `flock`-based exclusive file lock | Use `fs2` crate for portable locking |
| M1-T6 | Implement `find_team_dir(working_dir, name)` — project-local then global | Mirrors custom agent discovery |
| M1-T7 | Implement `TeamStore::create()`, `load()`, `save()`, `list_teams()` | In `team/store.rs` |
| M1-T8 | Implement `Mailbox::push()`, `drain_unread()`, `mark_read()` | JSON append-and-rewrite pattern |
| M1-T9 | Unit tests: task claim races, dependency blocking, mailbox drain | `tests/test_teams.rs` |
| M1-T10 | Add `fs2` dependency for file locking | `cargo add fs2 -p ragent-core` |

---

### M2 — Team Management Tools (LLM-visible)

**Goal**: Expose all team tools to the LLM via the tool registry. Lead tools and teammate tools are registered with the correct permission gating.

**Deliverables**:
- All tools in `crates/ragent-core/src/tool/team_*.rs`
- `ToolContext` gains optional `team_context: Option<TeamContext>` field
- Tool registry updated; tool count test updated

**Tasks**:

| ID | Task | Notes |
|----|------|-------|
| M2-T1 | `tool/team_create.rs` — `team_create` tool | Creates team dir, config; sets lead session |
| M2-T2 | `tool/team_spawn.rs` — `team_spawn` tool | Calls `TeamManager::spawn_teammate()` |
| M2-T3 | `tool/team_message.rs` — `team_message` tool | Writes to recipient mailbox |
| M2-T4 | `tool/team_broadcast.rs` — `team_broadcast` tool | Writes to all active member mailboxes |
| M2-T5 | `tool/team_task_list.rs` — `team_task_list` tool | Returns task list (read-only) |
| M2-T6 | `tool/team_task_claim.rs` — `team_task_claim` tool | Calls `TaskStore::claim_next()` |
| M2-T7 | `tool/team_task_complete.rs` — `team_task_complete` tool | Marks done; unblocks dependents |
| M2-T8 | `tool/team_task_create.rs` — `team_task_create` tool | Lead-only; adds tasks |
| M2-T9 | `tool/team_assign_task.rs` — `team_assign_task` tool | Lead assigns task to specific teammate |
| M2-T10 | `tool/team_status.rs` — `team_status` tool | Returns human-readable team status |
| M2-T11 | `tool/team_read_messages.rs` — `team_read_messages` tool | Drains unread mailbox for caller |
| M2-T12 | `tool/team_submit_plan.rs` — `team_submit_plan` tool | Teammate submits plan; sets plan-pending flag |
| M2-T13 | `tool/team_approve_plan.rs` — `team_approve_plan` tool | Lead approves/rejects; sends mailbox reply |
| M2-T14 | `tool/team_idle.rs` — `team_idle` tool | Fires TeammateIdle hook; marks member idle |
| M2-T15 | `tool/team_shutdown_teammate.rs` — `team_shutdown_teammate` tool | Sends shutdown_request to mailbox |
| M2-T16 | `tool/team_shutdown_ack.rs` — `team_shutdown_ack` tool | Teammate confirms; session terminates |
| M2-T17 | `tool/team_cleanup.rs` — `team_cleanup` tool | Removes team dir (fails if teammates active) |
| M2-T18 | Register all new tools in `tool/mod.rs`; update tool count test | Update `test_registry_total_tool_count` |
| M2-T19 | Add `TeamContext` to `ToolContext`; propagate through session processor | `Option<Arc<TeamContext>>` |

---

### M3 — Session & Execution Layer

**Goal**: Implement `TeamManager` — the runtime that spawns teammate sessions, polls their mailboxes, and delivers messages. Hook execution integrated.

**Deliverables**:
- `crates/ragent-core/src/team/manager.rs` — `TeamManager` struct
- Mailbox polling loop (async background task)
- Plan approval state machine
- Hook runner for `TeammateIdle` and `TaskCompleted`
- New `Event` variants for team lifecycle

**Tasks**:

| ID | Task | Notes |
|----|------|-------|
| M3-T1 | Define `TeamManager` struct; arc-wrap in `SessionProcessor` | Analogous to `TaskManager` |
| M3-T2 | Implement `TeamManager::spawn_teammate()` — creates session, injects team system-prompt additions | Augments `build_system_prompt()` |
| M3-T3 | Implement mailbox polling loop — background tokio task per teammate | `tokio::spawn` per member |
| M3-T4 | Implement message delivery: teammate message → lead EventBus event | `Event::TeammateMessage` |
| M3-T5 | Implement idle notification: `team_idle` → `Event::TeammateIdle` | |
| M3-T6 | Implement plan approval state machine (`PlanPending`, `PlanApproved`, `PlanRejected`) | Stored in `TeamMember.plan_status` |
| M3-T7 | Implement read-only plan mode: block write/bash tools while `PlanPending` | In session processor tool dispatch |
| M3-T8 | Implement hook runner for `TeammateIdle` and `TaskCompleted` | Exec subprocess; interpret exit code |
| M3-T9 | Implement `TeamManager::shutdown_teammate()` and cleanup | |
| M3-T10 | Add `Event` variants: `TeammateSpawned`, `TeammateMessage`, `TeammateIdle`, `TeamTaskClaimed`, `TeamTaskCompleted`, `TeamCleanedUp` | In `event/mod.rs` |
| M3-T11 | System prompt injection: add team context block when teammate session starts | Template vars: `{{TEAM_NAME}}`, `{{TEAMMATE_NAME}}`, etc. |
| M3-T12 | Tests: spawn lifecycle, plan approval round-trip, hook exit-2 feedback | In `tests/test_teams.rs` |

---

### M4 — TUI Integration

**Goal**: Expose teams in the TUI via the Teams panel, Agents window badges, `/team` slash commands, and log panel events.

**Deliverables**:
- `crates/ragent-tui/src/layout_teams.rs` — Teams panel renderer
- `/team` slash command with subcommands
- `[T]` badge in Agents window for teammates
- Log panel events for all team lifecycle moments
- `App` state extended with `active_team` and `team_members`

**Tasks**:

| ID | Task | Notes |
|----|------|-------|
| M4-T1 | Add `active_team: Option<TeamConfig>` and `team_members: Vec<TeamMember>` to `App` state | `app/state.rs` |
| M4-T2 | Handle `Event::TeammateSpawned`, `TeammateMessage`, `TeammateIdle`, `TeamTaskClaimed`, `TeamTaskCompleted` in `App::handle_event()` | `app.rs` |
| M4-T3 | Create `layout_teams.rs` — `render_teams_subpanel()` showing team tree (lead + teammates) | Modelled on `layout_active_agents.rs` |
| M4-T4 | Integrate Teams panel into main layout (shown when `active_team.is_some()`) | `layout.rs` |
| M4-T5 | Add `[T]` (bold blue) badge to Agents window for teammate agent entries | `layout_active_agents.rs` |
| M4-T6 | Add `/team` to `SLASH_COMMANDS` in `state.rs` | |
| M4-T7 | Implement `/team` handler in `app.rs` with subcommands: `status`, `create`, `message`, `tasks`, `cleanup` | |
| M4-T8 | Log panel: surface all `Event::Team*` variants with appropriate level and message | `app.rs handle_event()` |
| M4-T9 | TUI tests: `/team status` output, badge rendering, event handling | `tests/test_teams_tui.rs` |

---

### M5 — Documentation & Examples

**Goal**: User-facing documentation and example team configurations/hooks.

**Deliverables**:
- `docs/teams.md` — full user documentation
- `examples/teams/` — example team configs and hooks
- Updates to `README.md` and `QUICKSTART.md`

**Tasks**:

| ID | Task | Notes |
|----|------|-------|
| M5-T1 | Write `docs/teams.md` covering: concepts, getting started, slash commands, tool reference, hooks, limitations | |
| M5-T2 | Create `examples/teams/code-review/` — 3-reviewer team (security/perf/tests) | Config + spawn prompts |
| M5-T3 | Create `examples/teams/parallel-feature/` — 3-teammate feature build (API/UI/tests) | |
| M5-T4 | Create `examples/teams/hooks/teammate-idle.sh` — example quality gate hook | |
| M5-T5 | Update `README.md` — add "Teams" section after "Custom Agents" | |
| M5-T6 | Update `QUICKSTART.md` — add section 5c on Teams | |

---

### M6 — Testing

**Goal**: Comprehensive test coverage for teams across all layers.

**Deliverables**:
- `crates/ragent-core/tests/test_teams.rs` — core layer tests
- `crates/ragent-tui/tests/test_teams_tui.rs` — TUI layer tests
- All existing tests still pass

**Tasks**:

| ID | Task | Notes |
|----|------|-------|
| M6-T1 | Unit: `TeamConfig` serde round-trip | |
| M6-T2 | Unit: `TaskList` dependency blocking — task with unresolved deps cannot be claimed | |
| M6-T3 | Unit: `TaskStore::claim_next()` — concurrent claims produce no duplicates (multi-thread test) | |
| M6-T4 | Unit: `Mailbox::push()` and `drain_unread()` | |
| M6-T5 | Unit: plan approval state machine transitions | |
| M6-T6 | Integration: team create → spawn → task claim → complete → cleanup lifecycle | |
| M6-T7 | Integration: hook exit-2 feedback blocks idle | |
| M6-T8 | TUI: `/team status` with active team shows correct output | |
| M6-T9 | TUI: `[T]` badge appears on teammate task entries | |
| M6-T10 | TUI: team events update `active_team` state correctly | |

---

## 10. Implementation Order

```
M1 (Storage) ──► M2 (Tools) ──► M3 (Execution) ──► M4 (TUI) ──► M5 (Docs) ──► M6 (Tests)
```

M6 tests are written incrementally alongside each milestone, not only at the end.
M1 and M2 have no TUI dependencies and can be reviewed/merged independently.
M3 depends on M1 and M2. M4 depends on M3.

---

## 11. Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Storage format | JSON files (not SQLite) | Matches existing team config pattern; human-readable; simpler for hooks to read |
| File locking | `fs2::FileExt::lock_exclusive()` | Cross-platform; same crate used elsewhere in ragent |
| Teammate sessions | Separate `SessionProcessor` calls (not threads) | Reuses existing session/processor architecture |
| In-process only (V1) | No tmux/iTerm2 split panes | TUI is ratatui-based; split panes require external terminal support not yet available |
| Plan mode | Block write/bash tools at tool-dispatch level | Cleanest enforcement; no per-tool awareness needed |
| Mailbox polling | Background tokio task per teammate | Keeps messaging responsive without blocking the main loop |
| Hook runner | Subprocess exec (same as skill hooks) | Consistent with existing hook pattern; shell scripts work naturally |
