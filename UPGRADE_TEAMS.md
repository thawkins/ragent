# UPGRADE_TEAMS.md — Agent Coordination & Communication Gap Analysis

## 1. Overview

This document compares how agents coordinate and communicate in three systems — **GitHub Copilot CLI `/swarm`**, **Claude Code Agent Teams**, and **ragent Teams** — then defines milestones to close the gaps.

---

## 2. System Comparison: Agent Coordination & Communication

### 2.1 Copilot CLI `/swarm`

**Architecture:** Orchestrator → Subagent (hub-and-spoke, one-way)

| Aspect | Detail |
|--------|--------|
| **Topology** | Single orchestrator decomposes a plan into subtasks and dispatches them to subagents |
| **Subagent creation** | Automatic — the orchestrator analyses the prompt/plan and decides how many subagents to spawn and what each does |
| **Communication direction** | One-way: orchestrator assigns task → subagent returns result. No peer-to-peer messaging between subagents |
| **Dependency handling** | Orchestrator sequences dependent subtasks and parallelises independent ones. Subagents never talk to each other about dependencies |
| **Agent specialisation** | Custom agent profiles can be routed via `@agent-name` in the prompt. Subagents can use different AI models (`Use GPT-5.3-Codex, to create …`) |
| **Context isolation** | Each subagent has its own context window; the orchestrator's full context is not shared |
| **Monitoring** | `/tasks` shows background subtask list with status, details, kill, remove |
| **Plan integration** | Designed as a pipeline: plan mode → `/swarm` → autopilot. The plan is the input, swarm is the executor |
| **User interaction** | Minimal — autopilot mode runs autonomously; interactive mode may ask questions during execution |

**Key insight:** Fleet is an *automatic decomposition engine*. The user provides a plan; the system figures out parallelism. There is no ongoing coordination between subagents — each runs in isolation and reports back.

---

### 2.2 Claude Code Agent Teams

**Architecture:** Lead + Teammates (mesh with shared state)

| Aspect | Detail |
|--------|--------|
| **Topology** | One lead agent coordinates N teammate agents. Teammates can message each other directly (not just through the lead) |
| **Team creation** | User requests a team or Claude proposes one. User confirms before spawning. Natural-language team specification |
| **Communication** | **Mailbox system** with automatic delivery. Message types: direct message, broadcast, plan request/approval/rejection, idle notify, shutdown request/ack |
| **Peer-to-peer** | ✅ Teammates message each other directly. Enables debate, hypothesis challenging, and collaborative convergence |
| **Shared task list** | File-locked atomic task claiming. Dependencies auto-unblock when prerequisites complete. Three states: pending → in progress → completed |
| **Task claiming** | Self-claim (teammate picks next unblocked task) or lead-assigned |
| **Plan approval** | Teammate works in read-only plan mode → submits plan → lead approves or rejects with feedback → teammate revises or implements |
| **Quality gate hooks** | `TeammateIdle` (exit 2 = send feedback, keep working), `TaskCreated` (exit 2 = reject), `TaskCompleted` (exit 2 = reject completion) |
| **Display modes** | In-process (Shift+Down to cycle teammates) or split-pane (tmux/iTerm2) |
| **User ↔ teammate** | User can message any teammate directly, not just through the lead |
| **Persistent memory** | Per-agent memory directory that survives across sessions (user, project, or local scope) |
| **Model per agent** | Each teammate/subagent can use a different model (`sonnet`, `opus`, `haiku`, or full model ID) |
| **Agent specialisation** | Subagent `.md` files with YAML frontmatter define tools, permissions, system prompts, MCP servers, skills, hooks |

**Key insight:** Claude Code teams are a *collaborative mesh* where teammates can debate, challenge each other, and self-organise around a shared task list. The lead orchestrates but doesn't monopolise communication.

---

### 2.3 ragent Teams (Current Implementation)

**Architecture:** Lead + Teammates (hub-spoke with shared task file)

| Aspect | Detail |
|--------|--------|
| **Topology** | Lead session spawns teammate sessions. Communication is lead ↔ teammate via file-backed mailboxes |
| **Team creation** | Explicit via `/team create <blueprint>` or `team_create` tool. Blueprint system seeds tasks and spawn prompts |
| **Communication** | File-backed mailbox per agent (`mailbox/{agent-id}.json`). 8 message types: Message, Broadcast, PlanRequest, PlanApproved, PlanRejected, IdleNotify, ShutdownRequest, ShutdownAck |
| **Peer-to-peer** | ⚠️ Partial — the `team_message` tool accepts any `to` agent ID, so teammate→teammate is structurally possible, but teammates are not prompted to do it and there is no discovery mechanism for peer IDs |
| **Shared task list** | `tasks.json` with file-locking via `fs2`. Atomic `claim_next()`. Dependencies tracked and checked during claim |
| **Task claiming** | Self-claim via `team_task_claim` tool. Also `team_assign_task` for lead-directed assignment |
| **Plan approval** | ✅ Implemented — PlanRequest/PlanApproved/PlanRejected message types, `team_submit_plan`/`team_approve_plan` tools, PlanPending member status |
| **Quality gate hooks** | ❌ Not implemented |
| **Display modes** | ❌ Teammates are invisible background sessions. No TUI panes or cycling |
| **User ↔ teammate** | ❌ Only through lead via `/team message <name> <text>` |
| **Persistent memory** | ✅ Implemented — MemoryScope (none/user/project), memory injection at spawn, team_memory_read/write tools |
| **Model per agent** | ⚠️ Partial — teammates inherit the lead's active model. No per-teammate model override at spawn time |
| **Message delivery** | Polling-based (500ms interval). Not push/event-driven |
| **Agent specialisation** | Via `agent_type` at spawn and custom agent resolution, but no declarative agent profile files with tool/permission/hook config |
| **Auto-decomposition** | ❌ No swarm-style automatic task decomposition from a plan |

---

## 3. Detailed Gap Analysis: Coordination & Communication

### 3.1 Gaps vs Copilot `/fleet`

| # | Gap | Impact | Priority |
|---|-----|--------|----------|
| F1 | **Auto-decomposition** — No mechanism to analyse a plan and automatically split it into parallel subtasks | Users must manually create teams, blueprints, and tasks. No "give me a plan, I'll parallelise it" flow | P0 |
| F2 | **Orchestrator mode** — No lightweight "spawn N subagents for this plan, collect results, tear down" workflow | Teams require explicit creation/cleanup. Fleet is ephemeral by design | P0 |
| F3 | **Per-subagent model selection** — Teammates all use lead's model | Cannot route compute-heavy subtasks to stronger models or cost-optimise with cheaper models | P1 |
| F4 | **Plan → swarm pipeline** — No integration between plan mode and parallel execution | Users must manually transition from planning to parallel team execution | P1 |
| F5 | **`/tasks` monitoring for swarm subtasks** — `/team tasks` exists but shows the shared task list, not a live view of subagent background task progress | Less visibility into what each subagent is actually doing right now | P2 |

### 3.2 Gaps vs Claude Code Agent Teams

| # | Gap | Impact | Priority |
|---|-----|--------|----------|
| C1 | **Direct user ↔ teammate interaction** — User can only message teammates through lead; cannot cycle through or inspect teammate sessions | Lead becomes bottleneck. Cannot steer individual teammates without going through lead context | P0 |
| C2 | **Teammate visibility / display modes** — Teammates are invisible background sessions | No awareness of what teammates are doing. Cannot see their output in real-time | P0 |
| C3 | **Quality gate hooks** — ✅ `HookEvent` enum with `TeammateIdle`, `TaskCreated`, `TaskCompleted` events. Hooks configured via `TeamSettings.hooks`. Exit 0 = allow, Exit 2 = reject with feedback. Integrated into idle, task create, task complete tools and blueprint seeding | Automated quality checks enforced at lifecycle points | P1 |
| C4 | **Peer-to-peer messaging** — ✅ Teammates now have roster with agent IDs and peer collaboration guidance. P2P events tracked separately | Enables debate-style investigation, hypothesis challenging, collaborative convergence | P1 |
| C5 | **Persistent agent memory** — No per-agent memory that survives across sessions | Teammates cannot build institutional knowledge. Each team starts from scratch | P2 |
| C6 | **Push-based message delivery** — ✅ Global `MailboxNotifierRegistry` with `tokio::sync::Notify` per agent. `Mailbox::push()` signals instantly; 5s fallback poll for external writers. Avg delivery latency ~7ms | Event-driven delivery is responsive and efficient | P2 |
| C7 | **Declarative agent profiles** — ✅ `.md` files with JSON frontmatter in `.ragent/agents/`. Markdown body = system prompt. Supports `name`, `description`, `mode`, `model`, `permissions`, `max_steps`, `temperature`, `skills`. `/agents` command shows `[profile]` tag. Blueprints reference profiles via `"profile"` key | Agent specialisation via config files, no code changes needed | P2 |
| C8 | **Natural-language team creation** — Teams require explicit blueprint/command; Claude Code can propose teams from task analysis | Less accessible to users who don't know the blueprint system | P3 |

---

## 4. What ragent Already Does Well

These are areas where ragent is at parity or ahead:

| Feature | ragent | Copilot Fleet | Claude Code |
|---------|--------|---------------|-------------|
| Blueprint system (team templates) | ✅ Rich | ❌ None | ❌ None |
| File-locked atomic task claiming | ✅ | N/A (orchestrator) | ✅ |
| Task dependencies | ✅ | Orchestrator-managed | ✅ |
| Plan approval workflow | ✅ | ❌ | ✅ |
| Broadcast messaging | ✅ | ❌ | ✅ |
| Reconciliation on restart | ✅ | ❌ | ❌ |
| 18 dedicated team tools | ✅ | ❌ | ✅ (fewer) |
| Explicit shutdown protocol | ✅ | ❌ | ✅ |
| Spawn retry with backoff | ✅ | ❌ | ❌ |

---

## 5. Milestones & Tasks

### Milestone T1 — Teammate Visibility & Direct Interaction (P0) ✅ COMPLETE

Make teammates visible in the TUI and allow users to interact with them directly.

| Task | Description | Status |
|------|-------------|--------|
| T1.1 | **Teammate status bar** — Persistent status strip below the status bar showing teammate names, statuses (colored icons), with focused highlight. Uses `render_teammate_strip()`. | ✅ Done |
| T1.2 | **Teammate output panel** — Reuses existing `OutputViewTarget::TeamMember` overlay. Cycle with Alt+↑/↓ keybinds (`FocusNextTeammate`/`FocusPrevTeammate` InputActions). | ✅ Done |
| T1.3 | **Direct user → teammate input** — When a teammate is focused, `SendMessage` routes to their mailbox via `send_teammate_message()`. Input box title changes to "→ name (focused)". | ✅ Done |
| T1.4 | **`/team focus <name>`** — Slash command with partial name/id matching. No args clears focus. Added to `/team help` table. | ✅ Done |

### Milestone T2 — Per-Teammate Model Selection ✅ COMPLETE

Allow each teammate to use a different AI model, with inheritance from parent when unspecified.

| Task | Status | Description |
|------|--------|-------------|
| T2.1 | ✅ | **`model_override` field on `TeamMember`** — `Option<ModelRef>` stored in config, serde-defaulted |
| T2.2 | ✅ | **`team_spawn` tool `model` param** — Optional `provider_id/model_id` string parsed and passed through |
| T2.3 | ✅ | **Model inheritance chain** — Priority: teammate override → lead session model → agent default → "general" fallback. `apply_teammate_model_override()` refactored to 2-param (teammate_model, lead_model) |
| T2.4 | ✅ | **Blueprint model support** — `spawn-prompts.json` supports `"model"` flattened key, persisted to `TeamMember.model_override` for reconcile pickup |
| T2.5 | ✅ | **`/team status` & teams panel show model** — Status output shows model per teammate; teams panel has 18-char model column (magenta if overridden, dim "(inherited)" otherwise) |
| T2.6 | ✅ | **Subagent inheritance verified** — `new_task` tool already falls back to `ctx.active_model`, no changes needed |

### Milestone T3 — Fleet-Style Auto-Decomposition ✅ COMPLETE

Add a `/swarm` command that analyses a plan and automatically creates a temporary team with parallel subtasks.

| Task | Status | Description |
|------|--------|-------------|
| T3.1 | ✅ | **`/swarm <prompt>` slash command** — Registered in SLASH_COMMANDS, match arm with help/status/cancel subcommands. Sends meta-prompt to LLM via RagentCompleter pattern |
| T3.2 | ✅ | **Fleet decomposition prompt** — System prompt in `swarm.rs` instructs LLM to output JSON `{ tasks: [{ id, title, description, depends_on, agent_type?, model? }] }`. Handles markdown fences and trailing commas |
| T3.3 | ✅ | **Ephemeral team creation** — Creates `swarm-{timestamp}` team, seeds tasks.json with dependencies, records Spawning members. Manager reconcile loop handles actual spawning |
| T3.4 | ✅ | **Orchestrator loop** — `poll_swarm_completion()` runs every tick, checks task status. Auto-summarises when all tasks complete/cancel. Shows completion table |
| T3.5 | ✅ | **Plan mode integration** — After `execute_plan_restore`, a hint is shown: "You can execute this plan in parallel with `/swarm <goal>`" |
| T3.6 | ✅ | **`/swarm status`** — Progress bar, task table (✅🔄⏳❌ icons), teammate list, dependency display. `/swarm cancel` tears down the ephemeral team |

### Milestone T4 — Peer-to-Peer Communication (P1) ✅ COMPLETE

Enable teammates to discover and message each other directly.

| Task | Status | Description |
|------|--------|-------------|
| T4.1 | ✅ | **Teammate roster in system prompt** — `build_team_prompt_addition()` now receives `(name, agent_id)` pairs and displays them as `"name (agent_id)"` so teammates can address peers |
| T4.2 | ✅ | **Peer messaging guidance** — Added `### Peer collaboration` section to team system prompt instructing teammates to use `team_message` for direct peer coordination |
| T4.3 | ✅ | **Message routing events** — Added `Event::TeammateP2PMessage` published when neither sender nor recipient is `"lead"`. TUI shows 🔀 icon for P2P messages. SSE serializes as `teammate_p2p_message` |
| T4.4 | ✅ | **Debate mode blueprint** — Created `.ragent/blueprints/teams/debate/` with advocate/critic/synthesizer teammates that use adversarial peer messaging to converge on well-tested conclusions |

### Milestone T5 — Quality Gate Hooks (P1) ✅

Add lifecycle hooks that run shell commands at key team coordination points.

| Task | Status | Description |
|------|--------|-------------|
| T5.1 | ✅ | **Hook configuration** — Added `HookEvent` enum (`TeammateIdle`, `TaskCreated`, `TaskCompleted`), `HookEntry` struct, and `hooks: Vec<HookEntry>` field to `TeamSettings` (serde-defaulted for backwards compat) |
| T5.2 | ✅ | **Hook runner integration** — Extended `run_hook()` with optional `stdin_data` parameter for piping JSON. Added `run_team_hook()` helper that loads team settings and finds matching hook |
| T5.3 | ✅ | **TeammateIdle hook** — When a teammate goes idle, runs the hook. Exit 0 = allow idle. Exit 2 = send stdout as feedback message, revert to Working |
| T5.4 | ✅ | **TaskCompleted hook** — When a task is marked complete, runs the hook with task metadata as JSON on stdin. Exit 0 = accept. Exit 2 = revert to InProgress, send feedback |
| T5.5 | ✅ | **TaskCreated hook** — When a task is added (via `team_task_create` tool or blueprint seeding), runs the hook. Exit 2 = remove the created task |

### Milestone T6 — Push-Based Message Delivery (P2) ✅

Replace polling with event-driven message delivery.

| Task | Status | Description |
|------|--------|-------------|
| T6.1 | ✅ | **Tokio Notify per agent** — Global `MailboxNotifierRegistry` in `mailbox.rs` maps `(team_dir, agent_id)` → `Arc<Notify>`. `Mailbox::push()` signals the recipient's notifier after writing. Registered on spawn, deregistered on shutdown |
| T6.2 | ✅ | **Fallback polling** — `poll_interval` changed from 500ms to 5s. `start_poll_loop()` uses `tokio::select!` between `notify.notified()` (instant push wakeup) and `sleep(5s)` (fallback for external writers) |
| T6.3 | ✅ | **Benchmark** — 8 agents × 50 msgs = 400 deliveries; avg latency ~7ms, p99 ~22ms. Sub-10ms average confirmed. Tests in `test_mailbox_notify.rs` |

### Milestone T7 — Declarative Agent Profiles (P2) ✅

Allow agent types to be defined as `.md` files with JSON frontmatter — the markdown body becomes the system prompt.

| Task | Status | Description |
|------|--------|-------------|
| T7.1 | ✅ | **Agent profile format** — `.md` files with JSON frontmatter between `---` delimiters. Fields: `name`, `description`, `mode`, `model`, `max_steps`, `temperature`, `top_p`, `hidden`, `permissions`, `skills`, `options`. Markdown body = `system_prompt` |
| T7.2 | ✅ | **Profile discovery** — `scan_dir()` in `custom.rs` extended to load `.md` alongside `.json`. Same directories: `.ragent/agents/` (project) and `~/.ragent/agents/` (global). Project-level overrides global |
| T7.3 | ✅ | **Profile application** — Profiles produce the same `AgentInfo` as OASF `.json` files. `resolve_agent_with_customs()` resolves them transparently. `spawn_teammate_internal()` works unchanged |
| T7.4 | ✅ | **`/agents` command** — Shows custom agents with `[scope/format]` tag: `project/profile` for `.md`, `project/oasf` for `.json`. Updated hint text to mention `.md` files |
| T7.5 | ✅ | **Blueprint agent references** — `spawn-prompts.json` now accepts `"profile"` key as an alias for `"agent_type"`, so blueprints can reference profiles by name |

### Milestone T8 — Persistent Agent Memory (P2) ✅

Give agents a memory directory that persists across sessions.

| Task | Description | Status |
|------|-------------|--------|
| T8.1 | **Memory directory convention** — `~/.ragent/agent-memory/<agent-name>/` (user scope) or `.ragent/agent-memory/<agent-name>/` (project scope). Implemented `MemoryScope` enum and `resolve_memory_dir()` in `team/config.rs`. | ✅ Done |
| T8.2 | **Memory injection** — At teammate spawn, if memory is enabled, read `MEMORY.md` from the memory directory and inject into system prompt (first 200 lines or 25KB). Implemented `load_memory_block()` in `team/manager.rs`. | ✅ Done |
| T8.3 | **Memory tools** — `team_memory_read` and `team_memory_write` tools allow teammates to read/write files in their memory directory. Path escape validation prevents access outside the memory dir. | ✅ Done |
| T8.4 | **Memory scope config** — `memory` field added to agent profiles (`.md` frontmatter), OASF payloads, and blueprint spawn configs. Values: `none`, `user`, `project`. Scope resolves: TeamMember → AgentInfo → None. | ✅ Done |

---

## 6. Milestone Dependency Graph

```
T1 (Visibility)          T2 (Model Selection)      T4 (Peer Messaging)
    │                         │                         │
    └──────────┬──────────────┘                         │
               │                                        │
          T3 (Fleet Auto-Decomposition) ◄───────────────┘
               │
               ▼
          T5 (Quality Hooks)
               │
               ▼
          T6 (Push Delivery)     T7 (Agent Profiles)    T8 (Memory)
```

- **T1** and **T2** are prerequisites for T3 (swarm needs visible teammates and model routing)
- **T4** (peer messaging) enhances T3 (swarm teammates can coordinate)
- **T5–T8** are independent improvements that build on the foundation

---

## 7. Priority Ordering

| Phase | Milestones | Rationale |
|-------|-----------|-----------|
| **Phase 1** | T1, T2 | Foundation: make teammates visible and allow model control |
| **Phase 2** | T3 | Core differentiator: swarm-style auto-decomposition |
| **Phase 3** | T4, T5 | Collaboration quality: peer messaging and quality gates |
| **Phase 4** | T6, T7, T8 | Polish: performance, declarative config, institutional memory |
