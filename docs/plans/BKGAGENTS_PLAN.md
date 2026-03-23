# Background & Sub-Agent Plan (F13 + F14)

## Problem Statement

Ragent currently processes all work sequentially within a single agent loop.
While skill-based forking exists (`context: fork` → `invoke_forked_skill()`),
it blocks the parent session until the sub-agent completes and there is no
general-purpose sub-agent spawning mechanism outside the skill system.

**F13 — Sub-agent spawning**: Launch specialized sub-agents (e.g., explore,
code-review) from within a session for focused, isolated tasks.

**F14 — Background agents**: Run multiple agent instances concurrently for
parallel task execution without blocking the parent session.

---

## Current Architecture (What Exists)

| Component | Status | Location |
|-----------|--------|----------|
| `invoke_forked_skill()` | ✅ Working | `skill/invoke.rs:159-234` — creates isolated session, resolves agent, runs loop |
| `Session.parent_id` | ✅ Working | `session/mod.rs:28` — tracks parent→child relationships |
| `AgentMode::Subagent` | ✅ Defined | `agent/mod.rs:42` — marks agent as subagent |
| `EventBus` (broadcast) | ✅ Working | `event/mod.rs:226-295` — tokio broadcast channel, multiple subscribers |
| `PermissionRuleset` per agent | ✅ Working | `agent/mod.rs:73` — per-agent permission scoping |
| `cancel_flag: Arc<AtomicBool>` | ✅ Working | `session/processor.rs` — cooperative cancellation |
| `SessionProcessor` (shared) | ✅ Working | `session/processor.rs:27-500` — `Arc<SessionProcessor>` already shared |
| `Storage` (SQLite + Mutex) | ✅ Working | `storage/mod.rs` — thread-safe, single connection |
| Provider registry | ✅ Working | `provider/mod.rs` — stateless, concurrent-safe |
| Background task events | ❌ Missing | No `SubagentStart`/`SubagentComplete` events |
| Task queue / manager | ❌ Missing | No background task tracking |
| TUI multi-session view | ❌ Missing | Single `session_id` in `App` struct |
| Inter-agent messaging | ❌ Missing | Results only flow child→parent via formatted text |

---

## Proposed Approach

### Design Principles

1. **Reuse `invoke_forked_skill()` pattern** — the core mechanics work; generalise them
2. **`tokio::spawn` for concurrency** — background agents run as independent tokio tasks
3. **Event-driven status tracking** — new event variants signal agent lifecycle
4. **Parent conversation injection** — results flow back into the parent session as tool output
5. **Shared `SessionProcessor`** — already `Arc<>`, safe for concurrent use
6. **Per-agent permissions** — sub-agents inherit or restrict parent permissions

### Architecture Diagram

```text
┌──────────────────────────────────────────────────────┐
│                   Parent Session                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  AgentLoop (general agent)                     │  │
│  │                                                │  │
│  │  User: "Analyse auth module and fix tests"     │  │
│  │                                                │  │
│  │  → Tool: new_task(agent: "explore",            │  │
│  │          task: "Map the auth module")           │  │
│  │  → Tool: new_task(agent: "build",              │  │
│  │          task: "Run failing tests",             │  │
│  │          background: true)                      │  │
│  │                                                │  │
│  │  ← SubagentComplete { id, summary }            │  │
│  └────────────────────────────────────────────────┘  │
│         │                          │                  │
│    ┌────▼────┐              ┌──────▼──────┐          │
│    │ Task 1  │              │  Task 2     │          │
│    │ explore │              │  build      │          │
│    │ (sync)  │              │ (background)│          │
│    │ blocks  │              │ tokio::spawn│          │
│    │ parent  │              │ runs async  │          │
│    └─────────┘              └─────────────┘          │
└──────────────────────────────────────────────────────┘
```

---

## Phase 1: Core Sub-Agent Infrastructure

### Task 1.1 — `new_task` Tool

Add a new tool that the agent can invoke to spawn a sub-agent task.

**File**: `crates/ragent-core/src/tool/new_task.rs` (new)

```rust
pub struct NewTaskTool;

/// Tool parameters
struct NewTaskInput {
    /// Agent to use: "explore", "build", "plan", "general", or custom
    agent: String,
    /// The task/prompt to send to the sub-agent
    task: String,
    /// If true, run in background (F14); if false, block and return result (F13)
    background: Option<bool>,
    /// Model override (e.g. "anthropic:claude-haiku")
    model: Option<String>,
}
```

**Behaviour:**
- **Sync mode** (`background: false`, default): Creates forked session, runs
  sub-agent to completion, returns result as tool output. Identical to current
  `invoke_forked_skill()` but triggered as a tool rather than a skill.
- **Background mode** (`background: true`): Spawns via `tokio::spawn`, returns
  immediately with a task ID. Parent continues processing. Result injected
  via event when complete.

**Registration**: Add to `create_default_registry()` in `tool/mod.rs`.

### Task 1.2 — Task Manager

Central registry for tracking spawned sub-agent tasks.

**File**: `crates/ragent-core/src/task/mod.rs` (new module)

```rust
pub struct TaskManager {
    tasks: Arc<RwLock<HashMap<String, TaskEntry>>>,
    event_bus: Arc<EventBus>,
}

pub struct TaskEntry {
    pub id: String,                          // ULID
    pub parent_session_id: String,
    pub child_session_id: String,
    pub agent_name: String,
    pub task_prompt: String,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancel_flag: Arc<AtomicBool>,
}

pub enum TaskStatus {
    Running,
    Completed,
    Failed(String),
    Cancelled,
}
```

**API:**
- `spawn_task(parent_sid, agent, prompt, background) → TaskEntry`
- `get_task(id) → Option<TaskEntry>`
- `list_tasks(parent_sid) → Vec<TaskEntry>`
- `cancel_task(id)`
- `wait_task(id) → TaskResult` (async, for sync mode)

### Task 1.3 — Sub-Agent Events

Extend the `Event` enum for sub-agent lifecycle.

**File**: `crates/ragent-core/src/event/mod.rs`

```rust
// New event variants:
SubagentStart {
    session_id: String,        // parent session
    task_id: String,
    child_session_id: String,
    agent: String,
    task: String,
    background: bool,
},
SubagentProgress {
    session_id: String,        // parent session
    task_id: String,
    step: u32,                 // current step number
    tool: Option<String>,      // tool being used (if any)
},
SubagentComplete {
    session_id: String,        // parent session
    task_id: String,
    child_session_id: String,
    summary: String,           // brief result summary
    success: bool,
    duration_ms: u64,
},
SubagentCancelled {
    session_id: String,
    task_id: String,
},
```

### Task 1.4 — Result Injection

When a background task completes, inject the result into the parent session.

**Mechanism:**
1. `TaskManager` receives completion from `tokio::spawn` join handle
2. Publishes `SubagentComplete` event with summary
3. Parent's agent loop checks for pending completions between steps
4. Injects result as a system message: `[Background Task Complete: {agent} — {task_id}]\n\n{summary}`
5. Agent can act on the result in its next iteration

**File**: `crates/ragent-core/src/session/processor.rs` — add completion
check in the agent loop between tool-call rounds.

---

## Phase 2: Background Agent Execution (F14)

### Task 2.1 — Background Task Spawning

In `new_task` tool, when `background: true`:

```rust
let task_id = ulid::Ulid::new().to_string();
let cancel_flag = Arc::new(AtomicBool::new(false));

// Clone shared resources for the background task
let processor = Arc::clone(&self.processor);
let event_bus = Arc::clone(&self.event_bus);

event_bus.publish(Event::SubagentStart { ... });

tokio::spawn(async move {
    let result = invoke_subagent(
        &processor, &child_session, &agent, &prompt, cancel_flag
    ).await;

    match result {
        Ok(response) => {
            event_bus.publish(Event::SubagentComplete {
                session_id: parent_sid,
                task_id: task_id.clone(),
                summary: truncate(&response, 2000),
                success: true,
                duration_ms: elapsed,
            });
        }
        Err(e) => {
            event_bus.publish(Event::SubagentComplete {
                success: false,
                summary: format!("Error: {e}"),
                ..
            });
        }
    }
});

// Return immediately to parent
ToolOutput {
    content: format!("Background task started: {task_id}\nAgent: {agent}\nTask: {prompt}"),
    metadata: json!({ "task_id": task_id }),
}
```

### Task 2.2 — Task Cancellation

Add a `cancel_task` tool or honour abort signals:

```rust
pub struct CancelTaskTool;

struct CancelTaskInput {
    task_id: String,
}
```

Sets `cancel_flag` to `true` → sub-agent's loop checks flag and exits
gracefully → publishes `SubagentCancelled` event.

### Task 2.3 — Task Status Tool

Allow the agent (or user) to query running/completed tasks:

```rust
pub struct ListTasksTool;

// Returns:
// | ID | Agent | Status | Duration | Summary (truncated) |
// |------------|---------|-----------|----------|----------------------|
// | 01J...ABC  | explore | completed | 12s      | Found 3 auth modules |
// | 01J...DEF  | build   | running   | 45s      | —                    |
```

### Task 2.4 — Concurrency Limits

Add configuration to cap concurrent background agents:

```rust
// In Config:
pub struct ExperimentalFlags {
    pub max_background_agents: u32,   // Default: 4
    pub background_agent_timeout: u64, // Seconds, default: 600
}
```

TaskManager enforces these limits — queues tasks if at capacity.

---

## Phase 3: TUI Integration

### Task 3.1 — Background Task Status Bar

Show running background tasks in the TUI status bar:

```text
┌─ ragent ── general ── session: abc123 ─────────────────────┐
│                                                             │
│ [messages...]                                               │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ ⚙ Tasks: explore (12s) │ build (45s) │ 1 completed         │
│ > _                                                         │
└─────────────────────────────────────────────────────────────┘
```

**File**: `crates/ragent-tui/src/app.rs`

- Add `active_tasks: Vec<TaskEntry>` to `App` struct
- Subscribe to `SubagentStart`/`SubagentComplete` events
- Render task summary in status bar area

### Task 3.2 — `/tasks` Slash Command

User-facing command to view and manage background tasks:

```
/tasks              → List all tasks (running + recent completed)
/tasks cancel <id>  → Cancel a running task
/tasks show <id>    → Show full result of a completed task
```

### Task 3.3 — Background Completion Notification

When a background task completes:
1. Show transient notification in TUI: `✓ Task {id} ({agent}) completed in {duration}`
2. If result is important, inject into conversation for agent to act on
3. Play terminal bell (optional, configurable)

---

## Phase 4: Server/API Integration

### Task 4.1 — Task REST Endpoints

**File**: `crates/ragent-server/src/routes/mod.rs`

```
POST   /sessions/{id}/tasks         → Spawn a sub-agent task
GET    /sessions/{id}/tasks         → List tasks for session
GET    /sessions/{id}/tasks/{tid}   → Get task details & result
DELETE /sessions/{id}/tasks/{tid}   → Cancel a running task
```

### Task 4.2 — SSE Task Events

Stream `SubagentStart`, `SubagentProgress`, `SubagentComplete` events
via the existing SSE endpoint at `/events/sessions/{id}`.

---

## Phase 5: Documentation & Spec Update

### Task 5.1 — Update SPEC.md

- Mark F13 and F14 as ✅
- Document `new_task` tool schema
- Document `TaskManager` API
- Document new event types
- Add examples for sync and background sub-agent usage

### Task 5.2 — Update README / QUICKSTART

- Add section on sub-agent capabilities
- Show example of `/tasks` command
- Document `max_background_agents` config option

### Task 5.3 — Update IMPL.md

- Mark sub-agent spawning as implemented
- Mark background agents as implemented
- Note any limitations or known issues

---

## Implementation Order & Dependencies

```
Phase 1 (Core):
  1.2 TaskManager       ─┐
  1.3 SubagentEvents     ├→ 1.1 new_task tool → 1.4 Result Injection
                         ─┘

Phase 2 (Background):
  1.1 ──→ 2.1 Background Spawning
  1.2 ──→ 2.2 Task Cancellation
  1.2 ──→ 2.3 Task Status Tool
  Config → 2.4 Concurrency Limits

Phase 3 (TUI):
  1.3 ──→ 3.1 Status Bar
  1.2 ──→ 3.2 /tasks Command
  1.3 ──→ 3.3 Completion Notification

Phase 4 (Server):
  1.2 ──→ 4.1 REST Endpoints
  1.3 ──→ 4.2 SSE Events

Phase 5 (Docs):
  All ──→ 5.1-5.3 Documentation
```

---

## Key Design Decisions

### 1. Tool-based spawning vs. implicit spawning

**Decision**: Tool-based (`new_task` tool).

The agent explicitly decides to spawn sub-agents via tool calls. This is
transparent, auditable, and consistent with the existing tool execution
model. The model can reason about when to parallelise vs. serialise.

### 2. Result injection mechanism

**Decision**: Event-driven injection between agent loop iterations.

Background results arrive via `SubagentComplete` events. The processor
checks for pending completions between steps and injects them as system
messages. This avoids race conditions and maintains conversation coherence.

### 3. Permission inheritance

**Decision**: Sub-agents use their own agent-specific permissions.

When spawning `explore`, it gets the `explore` agent's read-only ruleset.
When spawning `general`, it gets full permissions. The parent's permissions
do not propagate — agent identity determines access. Skills can additionally
grant tools via `allowed_tools`.

### 4. Storage contention

**Decision**: Acceptable for now; connection pooling is a future optimisation.

The existing `Mutex<Connection>` on SQLite handles concurrent sessions.
For typical workloads (1-4 background agents), contention is negligible.
If we support 10+ concurrent agents, introduce a connection pool (e.g.,
`r2d2` or `deadpool-sqlite`).

### 5. Max concurrent agents

**Decision**: Default 4, configurable via `experimental.max_background_agents`.

Prevents runaway API costs and system resource exhaustion. Tasks beyond the
limit are queued and started as slots become available.

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| API rate limiting with many concurrent agents | High | Default cap of 4; configurable limit; exponential backoff |
| SQLite lock contention | Medium | Mutex is sufficient for 4 agents; pool if needed |
| Runaway cost from background agents | High | `max_steps` per agent; configurable timeout; cancel tool |
| Context window overflow from injected results | Medium | Truncate results to configurable max length (default 2000 chars) |
| Race conditions in result injection | Low | Event-driven, single-threaded processing in parent loop |
| Orphaned background tasks on session close | Medium | Cancel all child tasks when parent session is archived |

---

## Files to Create / Modify

### New Files
| File | Purpose |
|------|---------|
| `crates/ragent-core/src/task/mod.rs` | TaskManager, TaskEntry, TaskStatus |
| `crates/ragent-core/src/tool/new_task.rs` | `new_task` tool implementation |
| `crates/ragent-core/src/tool/cancel_task.rs` | `cancel_task` tool |
| `crates/ragent-core/src/tool/list_tasks.rs` | `list_tasks` tool |
| `crates/ragent-core/tests/test_task_manager.rs` | TaskManager unit tests |
| `crates/ragent-core/tests/test_subagent.rs` | Sub-agent integration tests |

### Modified Files
| File | Changes |
|------|---------|
| `crates/ragent-core/src/event/mod.rs` | Add SubagentStart/Progress/Complete/Cancelled events |
| `crates/ragent-core/src/tool/mod.rs` | Register new_task, cancel_task, list_tasks tools |
| `crates/ragent-core/src/session/processor.rs` | Check for background completions between steps |
| `crates/ragent-core/src/config/mod.rs` | Add max_background_agents, background_agent_timeout |
| `crates/ragent-core/src/lib.rs` | Export task module |
| `crates/ragent-tui/src/app.rs` | Add active_tasks, subscribe to SubagentStart/Complete |
| `crates/ragent-server/src/routes/mod.rs` | Add /tasks endpoints |
| `crates/ragent-server/src/sse.rs` | Handle Subagent* event types |
| `SPEC.md` | Mark F13/F14 ✅, document new tools and events |
| `IMPL.md` | Update implementation status |
| `README.md` | Add sub-agent / background agent section |
