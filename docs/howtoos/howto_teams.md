# Teams How-To Manual

This guide is a practical, end-to-end manual for using the Teams capability in `ragent`.

It covers:

- Team concepts and data layout
- TUI command workflows
- Tool-driven workflows
- Example operating patterns
- Configuration and file examples
- Troubleshooting and safe cleanup

---

## 1) What Teams are for

Teams let one lead session coordinate multiple teammate agents with:

- Shared task queue (`tasks.json`)
- Per-member mailbox messaging
- Persistent team configuration (`config.json`)

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
      mailbox/
        tm-001.ndjson
        tm-002.ndjson
```

Key files:

- `config.json`: team metadata, members, settings, lifecycle state
- `tasks.json`: shared task list and status
- `mailbox/*.ndjson`: teammate/lead messages

---

## 3) Quick start in the TUI

### Create or open a team

```text
/team create feature-squad
/team open feature-squad
```

`/team create` only creates new teams. If `feature-squad` already exists, use `/team open feature-squad` to reuse it.

### Inspect team state

```text
/team
/team status
/team tasks
```

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

## 4) Complete slash command reference

Supported `/team` commands:

- `/team` (defaults to status)
- `/team status`
- `/team create <name>`
- `/team open <name>`
- `/team close`
- `/team delete <name>`
- `/team message <teammate-name> <text>`
- `/team tasks`
- `/team clear`
- `/team cleanup`

Behavior notes:

- `/team close` clears team context for the current TUI session, but keeps files.
- `/team clear` removes the current active team task list (`tasks.json`).
- `/team delete <name>` removes the team directory from disk.
- `/team tasks` renders a table with `ID`, `Title`, `Status`, `Assignee`.

---

## 5) Tool-based workflow (lead + teammates)

Teams are fully operable via tools. TUI commands are convenience wrappers.

### 5.1 Create team

```json
{
  "tool": "team_create",
  "input": {
    "name": "feature-squad",
    "project_local": true
  }
}
```

If the team already exists, `team_create` returns an error; use `/team open <name>` for reopening.

### 5.2 Spawn teammates

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

Repeat for additional teammates (for example `ui-builder`, `test-owner`).

### 5.3 Create tasks

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

### 5.4 Claim and complete tasks (teammate sessions)

- Claim next available task: `team_task_claim`
- Complete claimed task: `team_task_complete`

### 5.5 Check status and communicate

- `team_status`
- `team_task_list`
- `team_message`
- `team_read_messages`
- `team_broadcast`

---

## 6) Example operation playbooks

## 6.1 Parallel feature delivery (API/UI/Tests)

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

## 6.2 Multi-discipline code review swarm

1. Create:

```text
/team create code-review
```

2. Spawn:

- `security-reviewer`
- `performance-reviewer`
- `test-reviewer`

3. Create review tasks by subsystem and severity.

4. Teammates report findings through mailbox messages.

5. Lead aggregates, triages, and assigns follow-up tasks.

---

## 7) Configuration and file examples

This section shows representative file formats used by Teams.

## 7.1 Example team `config.json`

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

## 7.2 Example `tasks.json`

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

## 7.3 Example teammate spawn request

```json
{
  "tool": "team_spawn",
  "input": {
    "team_name": "code-review",
    "teammate_name": "security-reviewer",
    "agent_type": "general",
    "prompt": "Focus on auth, permissions, and command-execution boundaries."
  }
}
```

## 7.4 Example task seed file

See bundled examples:

- `examples/teams/code-review/task-seed.json`
- `examples/teams/parallel-feature/task-seed.json`

You can apply those entries by invoking each `team_task_create` payload.

---

## 8) Reading `/team tasks` output

The command shows a table like:

```text
ID            Title                               Status        Assignee
------------  ----------------------------------  ------------  --------
task-001      Implement API endpoints             completed     tm-001
task-002      Implement UI flow                   in-progress   tm-002
task-003      Add feature regression tests        pending       —
```

Status meanings:

- `pending`: waiting to be claimed
- `in-progress`: currently being worked
- `completed`: finished
- `cancelled`: intentionally stopped

---

## 9) Operational guidance and best practices

- Keep teammate prompts role-specific and narrow.
- Seed tasks before heavy teammate execution.
- Use `depends_on` to control sequencing across streams.
- Ask teammates to send short, frequent progress messages.
- Use `/team clear` when you want to reset task planning without deleting the team.
- Use `/team close` to exit team context safely in your current session.
- Use `/team delete <name>` only when you are done and want full removal.

---

## 10) Troubleshooting

## “No active team”

Cause: no current team selected in this session.

Fix:

```text
/team open <name>
```

or create one:

```text
/team create <name>
```

## “Failed to open team”

Cause: team name not found in project-local/global team directories.

Fix:

- Verify team name spelling
- Confirm the team exists under `.ragent/teams/` or `~/.ragent/teams/`

## “teammate(s) still active — shut them down first”

Cause: destructive operation requested while teammates still in `working` state.

Fix:

- Request teammate shutdown (`team_shutdown_teammate` + `team_shutdown_ack`)
- Retry cleanup/delete after teammates are idle/stopped

---

## 11) Safety and lifecycle recommendations

- Prefer `close` before `delete`.
- Use `clear` to reset work queue while preserving team composition/history.
- Keep cleanup conservative unless you are sure no teammate is running.
- Avoid manual edits in team files while sessions are active.

---

## 12) Related docs and examples

- Team guide: `docs/teams.md`
- Quickstart: `QUICKSTART.md` (Teams section)
- Example bundles:
  - `examples/teams/code-review/`
  - `examples/teams/parallel-feature/`
  - `examples/teams/hooks/`
