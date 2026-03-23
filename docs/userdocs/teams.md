# Teams

Teams let a lead session coordinate multiple teammate agents using shared on-disk state.

This feature is useful when you want parallel work streams (for example security/perf/tests reviewers, or API/UI/test implementation split) while keeping one lead conversation in control.

## Concepts

- Lead: your primary session that creates and manages the team.
- Teammates: named member sessions (for example `tm-001`) that can receive messages and work on tasks.
- Team store: persisted under `.ragent/teams/{name}/` (project-local) or `~/.ragent/teams/{name}/` (global).
- Shared task list: `tasks.json` with claim/complete semantics and dependency support.
- Mailboxes: per-agent mailbox files used for teammate ↔ lead communication.

Team status model:

- Team status: `active`, `completed`, `disbanded`
- Member status: `spawning`, `working`, `idle`, `planpending`, `shuttingdown`, `stopped`

## Getting Started

### 1) Create a team

From the TUI:

```text
/team create code-review
```

Or via tool:

```json
{
  "tool": "team_create",
  "input": { "name": "code-review", "project_local": true }
}
```

Note: `team_create` creates a new team and fails if that name already exists. To reuse an existing team, use `/team open <name>`.

### 2) Spawn teammates

Use `team_spawn` for each teammate:

```json
{
  "tool": "team_spawn",
  "input": {
    "team_name": "code-review",
    "teammate_name": "security-reviewer",
    "agent_type": "general",
    "prompt": "Review auth and permission boundaries. Report critical findings first."
  }
}
```

Repeat for additional teammates.

### 3) Create and assign tasks

Create shared tasks:

```json
{
  "tool": "team_task_create",
  "input": {
    "team_name": "code-review",
    "title": "Audit token handling",
    "description": "Trace token storage, rotation, and revocation paths.",
    "depends_on": []
  }
}
```

Teammates claim and complete:

- `team_task_claim`
- `team_task_complete`

### 4) Check status and communicate

- `/team status` or `team_status`
- `/team tasks` or `team_task_list`
- `/team message <teammate-name> <text>` or `team_message`
- `team_read_messages` from teammate sessions

### 5) Shutdown and cleanup

- `team_shutdown_teammate` (request graceful teammate shutdown)
- `team_shutdown_ack` (teammate confirms stop)
- `/team cleanup` or `team_cleanup` when done

## Slash commands

The TUI supports:

- `/team` (defaults to `status`)
- `/team status`
- `/team create <name>`
- `/team open <name>`
- `/team close`
- `/team delete <name>`
- `/team message <teammate-name> <text>`
- `/team tasks`
- `/team clear`
- `/team cleanup`

## Tool reference

Core lifecycle:

- `team_create`: create team and config
- `team_spawn`: spawn a teammate in an existing team
- `team_status`: summarize members + task progress
- `team_cleanup`: remove team resources (fails if members still active unless `force`)

Task management:

- `team_task_create`: add shared task (supports `depends_on`)
- `team_task_list`: list all tasks
- `team_task_claim`: atomically claim next available task
- `team_task_complete`: complete claimed task
- `team_assign_task`: explicit assignment to a teammate

Communication:

- `team_message`: direct message to teammate/lead
- `team_broadcast`: send to all members
- `team_read_messages`: drain unread mailbox messages
- `team_idle`: teammate reports idle state

Plan approval workflow:

- `team_submit_plan`: teammate submits plan
- `team_approve_plan`: lead approves/rejects plan

Shutdown workflow:

- `team_shutdown_teammate`: lead requests teammate shutdown
- `team_shutdown_ack`: teammate confirms shutdown

## Hooks

The team workflow can include hook-based quality gates. A common pattern is a `TeammateIdle` hook that checks whether standards are met before allowing final idle transition.

Example hook script:

- `examples/teams/hooks/teammate-idle.sh`

Typical behavior:

- Exit `0`: pass (idle accepted)
- Exit non-zero: fail with feedback

## Suggested operating pattern

1. Lead creates team and initial task list.
2. Lead spawns specialized teammates.
3. Teammates claim tasks, deliver progress via mailbox messages.
4. Lead reviews outputs and either assigns follow-ups or requests revision.
5. Lead shuts down teammates and cleans up team directory.

## Limitations

- `team_spawn` requires TeamManager wiring; behavior depends on runtime initialization.
- Team state is file-backed; external manual edits to team files can cause inconsistent state.
- `/team` in TUI is a convenience layer; advanced flows still rely on team tools.
- Cleanup is intentionally conservative and may require explicit shutdown first.

## Examples

See ready-to-run bundles:

- `examples/teams/code-review/`
- `examples/teams/parallel-feature/`
- `examples/teams/hooks/`
