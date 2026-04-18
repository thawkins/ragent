# Teams How-To Manual

This guide is a practical, end-to-end manual for using the Teams capability in `ragent`.

It covers:

- Team concepts and data layout
- Blueprints — pre-built team templates
- TUI command workflows
- Tool-driven workflows
- Work context delegation
- Example operating patterns
- Configuration and file examples
- Troubleshooting and safe cleanup

---

## 1) What Teams are for

Teams let one lead session coordinate multiple teammate agents with:

- Shared task queue (`tasks.json`)
- Per-member mailbox messaging
- Persistent team configuration (`config.json`)
- Blueprint-based team templates with auto-spawning
- Work context propagation — teammates know exactly what code to target

Use Teams when you want parallel execution (for example: API/UI/tests split, or security/perf/test review swarms) while keeping one lead in control.

---

## 2) Team architecture and storage

Teams are persisted on disk:

- Project-local (preferred): `[PROJECT]/.ragent/teams/<team-name>/`
- Global fallback: `~/.ragent/teams/<team-name>/`

Typical structure:

```text
.ragent/
  teams/
    feature-squad/
      config.json
      tasks.json
      README.md
      mailbox/
        tm-001.ndjson
        tm-002.ndjson
```

Key files:

- `config.json`: team metadata, members, settings, lifecycle state
- `tasks.json`: shared task list and status
- `README.md`: team documentation (copied from blueprint if used)
- `mailbox/*.ndjson`: teammate/lead messages

---

## 3) Blueprints

Blueprints are pre-built team templates that define which teammates to spawn and what seed tasks to create. When you create a team with a blueprint, all teammates are spawned automatically with their role-specific prompts.

### Blueprint storage locations

Blueprints are searched in priority order:

1. **Project-local:** `[PROJECT]/.ragent/blueprints/teams/<name>/`
2. **Global:** `~/.ragent/blueprints/teams/<name>/`

### Blueprint directory structure

```text
.ragent/blueprints/teams/code-review/
  README.md              # Team description (copied to team directory)
  spawn-prompts.json     # Teammate definitions with role-specific prompts
  task-seed.json         # Initial tasks seeded on team creation
```

### spawn-prompts.json format

Defines teammates to auto-spawn when the team is created:

```json
[
  {
    "tool_name": "team_spawn",
    "teammate_name": "security-reviewer",
    "agent_type": "general",
    "prompt": "Perform a focused security review of the codebase..."
  },
  {
    "tool_name": "team_spawn",
    "teammate_name": "test-reviewer",
    "agent_type": "general",
    "prompt": "Review test coverage and identify gaps..."
  }
]
```

### task-seed.json format

Defines initial tasks created when the team is set up:

```json
[
  {
    "tool": "team_task_create",
    "input": {
      "title": "Audit authentication boundaries",
      "description": "Confirm session/auth checks protect privileged actions."
    }
  }
]
```

Both `"input"` and `"args"` keys are supported for tool arguments.

### Browsing installed blueprints

Use the `/team blueprint` slash command:

```text
/team blueprint              # List all installed blueprints
/team blueprint code-review  # Show detailed summary of a specific blueprint
```

The list view shows a table with blueprint name, scope (project/global), teammate count, task count, and description. The detail view shows the full README, teammate table (name, type, prompt), seed tasks table (title, description), and usage instructions.

---

## 4) Quick start in the TUI

### Create a team from a blueprint (recommended)

```text
/team create code-review
```

This creates a team using the `code-review` blueprint, auto-spawns all defined teammates with their prompts, and seeds the initial task list. The blueprint parameter is required.

When the LLM calls `team_create`, it should **always pass a `context` parameter** with the user's specific request — which files/directories to review, what to produce, and where to write output. This context is prepended to every teammate's spawn prompt so they know exactly what to work on.

### Wait for teammates to finish

After creation, the lead should call `team_wait` to block until all teammates complete their initial work. Do **not** use `wait_tasks` for teammates — that only tracks `new_task` sub-agents.

### Inspect team state

```text
/team                 # Same as /team status
/team status          # Show active team with member table
/team tasks           # Show shared task list
/team show            # List all registered teams
/team show myteam     # Show details for a specific team
```

### View teammate output

In the Teams panel, click the `[T]` button next to a teammate to open their output view. This shows all tool calls, results, and messages in the same rich format as the primary Messages panel — including inline diffs, result summaries, and reasoning blocks.

### Message teammates

```text
/team message api-builder Please prioritize auth error handling.
```

### End-of-work actions

```text
/team clear                 # clear shared tasks list only
/team close                 # close active team in this session only
/team delete feature-squad  # delete persisted team from disk
/team cleanup               # cleanup active team (conservative guardrails)
```

---

## 5) Complete slash command reference

| Command | Arguments | Description |
|---|---|---|
| `/team help` | none | Show command reference table. |
| `/team status` | none | Show the currently active team in this session. |
| `/team show [name]` | optional `name` | Show one team in detail, or all registered teams when no name is given. |
| `/team create <blueprint> [name]` | required `blueprint`, optional `name` | Create a new project-local team (blueprint mandatory) and set it active. |
| `/team close` | none | Close the active team in this session (does not delete on disk). |
| `/team delete <name>` | required `name` | Delete a team from disk (also clears active state if it is active). |
| `/team blueprint [name]` | optional `name` | List all installed blueprints, or show details of a specific blueprint. |
| `/team message <teammate-name> <text>` | required `teammate-name`, `text` | Send a mailbox message from lead to a teammate. |
| `/team tasks` | none | Show the task table for the active team. |
| `/team clear` | none | Clear/remove the active team task list file. |
| `/team cleanup` | none | Clean up the active team (requires no working teammates). |

Alias: `/teams ...` routes to `/team ...` (for example `/teams help`, `/teams blueprint`).

---

## 6) Tool-based workflow (lead + teammates)

Teams are fully operable via tools. TUI commands are convenience wrappers.

### 6.1 Create team with context

```json
{
  "tool": "team_create",
  "input": {
    "blueprint": "code-review",
    "context": "Review the crates/ragent-server directory for security issues, test coverage gaps, and performance problems. Write findings to crates/ragent-server/COMPLIANCE.md",
    "project_local": true
  }
}
```

The `context` parameter is critical — it tells every auto-spawned teammate **exactly what code** to target. Without it, teammates only receive their generic role prompt from the blueprint.

If the team already exists, `team_create` gracefully recovers and re-applies the blueprint.

### 6.2 Wait for teammates

After `team_create` spawns all teammates, call `team_wait` immediately:

```json
{
  "tool": "team_wait",
  "input": {
    "team_name": "code-review-20260329-13-10-25",
    "timeout_secs": 300
  }
}
```

This blocks until all teammates become idle. **Do NOT use `wait_tasks`** — that only tracks `new_task` sub-agents, not team members.

### 6.3 Spawn additional teammates (manual)

```json
{
  "tool": "team_spawn",
  "input": {
    "team_name": "feature-squad",
    "teammate_name": "api-builder",
    "agent_type": "general",
    "prompt": "Implement backend endpoint + validation for the feature."
  }
}
```

Note: When using blueprints, teammates are spawned automatically — do NOT re-spawn them manually.

### 6.4 Create tasks

```json
{
  "tool": "team_task_create",
  "input": {
    "team_name": "feature-squad",
    "title": "Implement API endpoints",
    "description": "Create API routes and data access for feature flow.",
    "depends_on": []
  }
}
```

### 6.5 Claim and complete tasks (teammate sessions)

- Claim next available task: `team_task_claim`
- Complete claimed task: `team_task_complete`

### 6.6 Check status and communicate

- `team_status` — Full team report with member states and task summary
- `team_task_list` — List all tasks with status
- `team_message` — Send direct message to a teammate
- `team_read_messages` — Read inbox messages
- `team_broadcast` — Send message to all teammates

---

## 7) Work context delegation

When a team is created and assigned work, the specific details about what code needs to be addressed must flow through to every teammate. This is handled by the `context` parameter on `team_create`.

### How context flows

1. User asks: *"Review the crates/ragent-server directory for security issues"*
2. Lead calls `team_create` with `context: "Review the crates/ragent-server directory..."`
3. Each teammate's spawn prompt from the blueprint gets the context prepended:

```text
## Work Context
Review the crates/ragent-server directory for security issues, test coverage,
and performance problems. Write findings to COMPLIANCE.md.

## Your Role
Perform a focused security review of the assigned code...
```

4. Teammates immediately know which files to read and what to produce.

### Best practices for context

- Include **specific directories/files** to target
- Specify **output format** (e.g., "write to COMPLIANCE.md")
- Mention **what to look for** (security, performance, tests)
- Keep it concise but complete — this is the teammates' primary instruction

---

## 8) Team panel (TUI)

The Teams panel displays the active team in a table format:

```text
Team: code-review-20260329-13-10-25  (lead + 3 teammates)
  id         name                                status      elapsed  steps  claimed  done
  b729b590   general                             active      6m41s    14     -        -
  tm-001     security-reviewer                   idle        6m27s    22     0        0    [T]
  tm-002     test-reviewer                       idle        6m27s    25     0        0    [T]
  tm-003     performance-reviewer                idle        6m27s    7      0        0    [T]
```

Columns:

- **id**: Short agent ID (last 8 chars of session UUID for lead, tm-NNN for teammates)
- **name**: Teammate name (35 chars max) or agent type for lead
- **status**: `active`, `working`, `idle`, `failed`, `spawning`
- **elapsed**: Time since teammate was created
- **steps**: Number of tool calls executed
- **claimed**: Number of tasks claimed from the shared task list
- **done**: Number of tasks completed
- **[T]**: Click to open teammate's output view

### Teammate output view

Clicking `[T]` opens a scrollable overlay showing the teammate's full execution history — all tool calls, read results, write operations, and assistant messages. The output uses the same rich format as the primary Messages panel:

- Colored status dots (green = success, red = error)
- Inline diff stats for edit/patch operations
- Line counts for read/write operations
- File tree rendering for list operations

The output view updates incrementally during execution — you can watch a teammate's progress in real-time without waiting for completion.

---

## 9) Example operation playbooks

### 9.1 Blueprint-based code review (recommended)

1. Browse available blueprints:

```text
/team blueprint
```

2. Inspect the code-review blueprint:

```text
/team blueprint code-review
```

3. Ask ragent to create and run the team:

```text
Create a code-review team and have it review the crates/ragent-server directory
for security issues, test gaps, and performance problems. Write the output to
COMPLIANCE.md in the crates/ragent-server folder.
```

The LLM will:
- Call `team_create` with `blueprint="code-review"` and `context="Review the crates/ragent-server directory..."`
- All 3 teammates auto-spawn with the context prepended to their prompts
- Call `team_wait` to block until all teammates finish
- Call `team_status` to collect findings
- Aggregate results into the requested output file

### 9.2 Parallel feature delivery (API/UI/Tests)

1. Lead creates team:

```text
/team create parallel-feature
```

2. Lead spawns 3 teammates:

- `api-builder`
- `ui-builder`
- `test-owner`

3. Lead seeds tasks with dependency chain:

- API implementation
- UI implementation (depends on API)
- Regression tests (depends on API + UI)

4. Track in TUI:

```text
/team status
/team tasks
```

5. Team completes work, lead finalizes:

```text
/team clear
/team close
```

---

## 10) Configuration and file examples

This section shows representative file formats used by Teams.

### 10.1 Example team `config.json`

```json
{
  "name": "feature-squad",
  "lead_session_id": "sess-lead-123",
  "created_at": "2026-03-19T10:00:00Z",
  "status": "active",
  "members": [
    {
      "name": "api-builder",
      "agent_id": "tm-001",
      "session_id": "sess-tm-001",
      "agent_type": "general",
      "status": "working",
      "current_task_id": "task-001",
      "plan_status": "none",
      "created_at": "2026-03-19T10:01:00Z"
    }
  ],
  "settings": {
    "max_teammates": 8,
    "require_plan_approval": false,
    "auto_claim_tasks": true
  }
}
```

### 10.2 Example `tasks.json`

```json
{
  "team_name": "feature-squad",
  "tasks": [
    {
      "id": "task-001",
      "title": "Implement API endpoints",
      "description": "Add backend endpoint + data wiring.",
      "status": "completed",
      "assigned_to": "tm-001",
      "depends_on": [],
      "created_at": "2026-03-19T10:02:00Z",
      "claimed_at": "2026-03-19T10:03:00Z",
      "completed_at": "2026-03-19T10:20:00Z"
    },
    {
      "id": "task-002",
      "title": "Implement UI flow",
      "description": "Build screens and interactions.",
      "status": "inprogress",
      "assigned_to": "tm-002",
      "depends_on": ["task-001"],
      "created_at": "2026-03-19T10:04:00Z",
      "claimed_at": "2026-03-19T10:21:00Z",
      "completed_at": null
    }
  ]
}
```

### 10.3 Example blueprint spawn-prompts.json

```json
[
  {
    "tool_name": "team_spawn",
    "teammate_name": "security-reviewer",
    "agent_type": "general",
    "prompt": "Perform a focused security review. Check for auth gaps, injection vectors, unsafe shell usage, and unvalidated input."
  },
  {
    "tool_name": "team_spawn",
    "teammate_name": "test-reviewer",
    "agent_type": "general",
    "prompt": "Review test coverage. Identify untested paths, missing edge cases, and fragile assertions."
  },
  {
    "tool_name": "team_spawn",
    "teammate_name": "performance-reviewer",
    "agent_type": "general",
    "prompt": "Analyze performance and resource usage. Focus on expensive loops, repeated I/O, unnecessary allocations, and blocking calls."
  }
]
```

### 10.4 Example task-seed.json

```json
[
  {
    "tool": "team_task_create",
    "input": {
      "title": "Audit authentication boundaries",
      "description": "Confirm session/auth checks protect privileged actions."
    }
  },
  {
    "tool": "team_task_create",
    "input": {
      "title": "Audit command execution safety",
      "description": "Review shell/tool invocation paths for injection risks."
    }
  }
]
```

---

## 11) Reading `/team tasks` output

The command shows a table like:

```text
ID            Title                               Status        Assignee
------------  ----------------------------------  ------------  --------
task-001      Implement API endpoints             completed     tm-001
task-002      Implement UI flow                   in-progress   tm-002
task-003      Add feature regression tests        pending       ---
```

Status meanings:

- `pending`: waiting to be claimed
- `in-progress`: currently being worked
- `completed`: finished
- `cancelled`: intentionally stopped

---

## 12) Operational guidance and best practices

- **Always use blueprints** for repeatable team patterns — they save time and reduce errors.
- **Always pass `context`** when creating teams — teammates can't read your mind about which files to target.
- **Call `team_wait`** immediately after `team_create` — don't try to read results before teammates finish.
- **Don't re-spawn blueprint teammates** — they are spawned automatically by `team_create`.
- Keep teammate prompts role-specific and narrow.
- Seed tasks before heavy teammate execution.
- Use `depends_on` to control sequencing across streams.
- Ask teammates to send short, frequent progress messages.
- Use `/team clear` when you want to reset task planning without deleting the team.
- Use `/team close` to exit team context safely in your current session.
- Use `/team delete <name>` only when you are done and want full removal.

---

## 13) Troubleshooting

### "No active team"

Cause: no current team selected in this session.

Fix:

```text
/team show          # list available teams
/team create <blueprint>   # create new team
```

### "Failed to open team"

Cause: team name not found in project-local/global team directories.

Fix:

- Verify team name spelling
- Confirm the team exists under `.ragent/teams/` or `~/.ragent/teams/`

### "teammate(s) still active --- shut them down first"

Cause: destructive operation requested while teammates still in `working` state.

Fix:

- Request teammate shutdown (`team_shutdown_teammate` + `team_shutdown_ack`)
- Retry cleanup/delete after teammates are idle/stopped

### "parse tasks.json: EOF while parsing"

Cause: `tasks.json` exists but is empty (0 bytes). This can happen if a concurrent operation created the file but didn't write content.

Fix: The system now handles empty `tasks.json` files gracefully. If you encounter this with an older version, delete the empty file and retry:

```text
/team clear
```

### Teammates resolving wrong paths

Cause: In older versions, teammates could resolve relative paths against the team directory instead of the project root.

Fix: Ensure you are running the latest version. Teammate sessions now inherit the lead session's working directory (the project root).

### "0 lines read" in teammate output

Cause: In older versions, tool metadata (line counts, file counts) was not persisted to the message store, so the teammate output view couldn't display result summaries.

Fix: Ensure you are running the latest version. Tool metadata is now merged into the persisted message, and the output view displays correct line counts, diff stats, and summaries.

---

## 14) Safety and lifecycle recommendations

- Prefer `close` before `delete`.
- Use `clear` to reset work queue while preserving team composition/history.
- Keep cleanup conservative unless you are sure no teammate is running.
- Avoid manual edits in team files while sessions are active.

---

## 15) Related docs and examples

- Team guide: `docs/teams.md`
- Quickstart: `QUICKSTART.md` (Teams section)
- Blueprint location: `[PROJECT]/.ragent/blueprints/teams/` or `~/.ragent/blueprints/teams/`
- Example bundles:
  - `examples/teams/code-review/`
  - `examples/teams/parallel-feature/`
  - `examples/teams/hooks/`
