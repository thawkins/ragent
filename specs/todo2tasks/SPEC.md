---
status: draft
audit:
  - { time: 1786893477, from: "none", to: "draft", actor: "system" }
---
# Todo-to-Tasks Migration Specification

## Background

ragent currently provides a lightweight **Todo** system: two LLM-callable
tools (`todo_read`, `todo_write`) backed by a `todos` SQLite table, and a
TUI side panel (Alt+T) that renders items as `[PENDING] / [IN_PROGRESS] /
[DONE] / [BLOCKED]` status lines. The model is flat: each todo has an
`id`, `title`, `status`, `description`, `session_id`, and timestamps —
no dependency relationships, no ownership, no structured metadata, and no
progressive display form.

The article ["From Todos to Tasks"](https://pub.spillwave.com/claude-code-todos-to-tasks-5a1b0e351a1c)
describes Claude Code's migration from an equivalent Todos system to a
richer **Tasks** architecture. Tasks are still **session-scoped** (they
do not persist across sessions by design — persistence is handled
out-of-band via spec/hydration files), but they add:

| Concept | Old Todo | New Task |
|---|---|---|
| Title | `title` (imperative) | `subject` (imperative) + `active_form` (present-continuous, shown in progress spinners) |
| Description | free-text | free-text, carries acceptance criteria |
| Status | pending / in_progress / done / blocked | pending / in_progress / completed |
| Dependencies | none | `blocked_by` + `blocks` — a directed acyclic graph (DAG) |
| Ownership | none | `owner` — a string label naming the agent/worker responsible |
| Metadata | none | arbitrary `metadata` key-value pairs (feature, phase, priority, …) |
| Tool surface | `todo_read` + `todo_write` (one read, one multi-action write) | `task_create` + `task_update` + `task_get` + `task_list` (four single-purpose tools) |
| Blocking | a status value | a derived state: a task is "blocked" when `blocked_by` is non-empty and not all blockers are `completed` |

**This spec covers ONLY the session-scoped Todo→Task replacement.**
It does **not** touch the sub-agent task system (`AgentManager` /
`TaskEntry` / `new_agent` / `cancel_agent`), the team task system
(`ragent_agent::team::task::Task` / `TaskStore` / `team_task_*` tools),
or the spec-hydration pattern described in the article. Those are
separate concerns. The migration is a pure in-place upgrade of the
existing Todo data model, tool surface, and TUI panel.

## Scope

### In Scope

1. **Data model** — extend the per-session work-item schema to support
   `active_form`, `owner`, `metadata` (JSON), and `blocked_by` /
   `blocks` dependency lists.
2. **Storage** — migrate the SQLite `todos` table (additive ALTER TABLE
   columns with safe defaults) and add a join table or JSON column for
   dependencies. Existing rows must continue to work.
3. **Tool surface** — introduce four new LLM tools — `task_create`,
   `task_update`, `task_get`, `task_list` — mirroring the article's
   `TaskCreate` / `TaskUpdate` / `TaskGet` / `TaskList`. There is no need 
   `todo_read` / `todo_write` as **backward-compatible aliases** that
   delegate to the new tools .
4. **Dependency DAG** — when a task is marked `completed`, every task
   that listed it in `blocked_by` is re-evaluated; if all its blockers
   are now `completed`, it becomes "available" (still `pending`, but
   no longer blocked).
5. **TUI panel** — migrate the Alt+T side panel from "TODO" to "TASKS".
   Render each task with its status colour, subject, owner (if any),
   and a `[blocked by #2, #5]` annotation when applicable. Keep the
   existing scroll, text-selection, and mutual-exclusion behaviour.
6. **Slash commands** — `/todo` (toggle) and `/todo add` etc. continue
   to work; add `/task` and `/tasks` as aliases. The `/todo_list`
   textual command is re-routed to the new list path.

### Out of Scope

- Cross-session task persistence / hydration from spec files (future
  work; the article describes it as a pattern, not a built-in).
- Sub-agent or team task systems — untouched.
- Gantt charts, time tracking, notifications — explicitly excluded by
  the article.
- HTTP REST endpoints for tasks (the current Todo system has none;
  adding them is a separate spec).
- Changing the session-scoped lifetime — tasks remain per-session,
  matching the current `todos.session_id` scoping.

## Definitions

- **Task** — a session-scoped work item with a subject, description,
  active form, status, owner, metadata, and dependency edges. The
  successor to a Todo.
- **blocked_by** — a list of task IDs that must reach `completed`
  before this task is considered "available" for work.
- **blocks** — the inverse: task IDs that cannot start until *this*
  task completes. Derived from the union of all `blocked_by` lists.
- **available** — a task whose `status` is `pending`, `owner` is empty,
  and every ID in its `blocked_by` list is `completed`.
- **active_form** — a present-continuous phrase ("Implementing JWT
  auth") shown in progress indicators, distinct from the imperative
  `subject` ("Implement JWT auth").

## Requirements

### FR-001 (Ubiquitous) — Session scoping

The Tasks system SHALL store every task scoped to a single session via
the existing `session_id` foreign key, so that tasks are isolated per
session and are destroyed when the session is deleted.

### FR-002 (Ubiquitous) — Backward compatibility of existing todos

The system SHALL preserve all existing `todos` rows across the schema
migration. Rows that lack the new columns MUST be readable and
listable, with missing fields populated by safe defaults (`active_form`
= empty, `owner` = `None`, `metadata` = `{}`, `blocked_by` = `[]`).

### FR-003 (Event-driven) — Auto-unblock on completion

WHEN a task's status is set to `completed`, the system SHALL evaluate
every other task in the same session that lists the completed task's ID
in its `blocked_by` list and, for each such dependent task, recompute
whether it is still blocked. A dependent task whose `blocked_by` list is
now fully `completed` SHALL remain `pending` but be flagged as
"available" (unblocked) in the `task_list` output and TUI panel.

### FR-004 (State-driven) — Cycle prevention

WHEN a `task_update` call attempts to add a `blocked_by` or `blocks`
edge that would create a dependency cycle, the system SHALL reject the
update with an error describing the cycle and SHALL NOT persist the
edge.

### FR-005 (State-driven) — Blocked status derivation

The system SHALL NOT store a `blocked` status value. A task's
"blocked-ness" SHALL be derived at read time from the `blocked_by`
list: a task is blocked if and only if its `status` is `pending` and at
least one ID in its `blocked_by` list is not `completed`. The `task_list`
output and TUI panel SHALL annotate blocked tasks with `[blocked by #id,
…]`.

### FR-006 (Optional) — Owner assignment

The system MAY accept an `owner` string on `task_create` and
`task_update`. When `owner` is set, the task SHALL be displayed with
the owner label in the TUI panel and included in `task_get` /
`task_list` output. The `owner` field is a free-form label; it does
not bind to the sub-agent or team runtime.

### FR-007 (Optional) — Active form

The system MAY accept an `active_form` string on `task_create`. When
provided, the TUI panel SHALL display it in a progress indicator line
beneath the subject for tasks whose status is `in_progress`. When
omitted, the panel SHALL fall back to displaying the subject.

### FR-008 (Optional) — Metadata key-value pairs

The system MAY accept a `metadata` JSON object on `task_create` and
`task_update`. The object SHALL be persisted as a JSON blob and
returned verbatim by `task_get`. Unknown keys SHALL be accepted
silently (the schema is open). The TUI panel SHALL NOT render metadata
by default (it is machine-readable only), but `task_list` output SHALL
include it in the structured response.

### FR-009 (Unwanted) — No cross-session leakage

The system SHALL NOT allow a task's `blocked_by` list to reference a
task ID that does not exist in the same session. A reference to a
foreign or non-existent task ID SHALL be rejected with an error.

### FR-010 (Unwanted) — No persistence beyond session lifetime

The system SHALL NOT persist tasks beyond the session that created
them. Tasks SHALL NOT be written to a global or project-scoped store;
the existing `session_id` scoping of the `todos` table SHALL be
preserved. (Cross-session hydration is explicitly out of scope.)

### FR-011 (Ubiquitous) — Four-tool surface

The system SHALL provide four LLM-callable tools: `task_create`,
`task_update`, `task_get`, `task_list`. Each tool SHALL be
auto-approved (hardwired allowed), mirroring the existing
`todo_read` / `todo_write` permission policy. The tools SHALL be
registered in the same tool category the TUI already recognises for
panel-refresh events.

### FR-012 (State-driven) — TaskCreate parameters

WHEN `task_create` is called, the system SHALL require a `subject`
string and a `description` string, and SHALL accept optional
`active_form`, `owner`, `metadata`, and `blocked_by` (array of task
IDs). The system SHALL generate a unique task ID (preserving the
existing `generate_todo_id` scheme), set `status` to `pending`, and
return the created task.

### FR-013 (State-driven) — TaskUpdate parameters

WHEN `task_update` is called, the system SHALL require a `task_id` and
SHALL accept optional `status` (`pending` | `in_progress` |
`completed`), `subject`, `description`, `active_form`, `owner`,
`metadata`, `add_blocked_by` (array), and `add_blocks` (array). The
system SHALL apply each provided field, validate dependencies
(FR-004, FR-009), and — if `status` transitions to `completed` —
trigger the auto-unblock evaluation (FR-003).

### FR-014 (State-driven) — TaskGet output

WHEN `task_get` is called with a `task_id`, the system SHALL return the
full task record: `id`, `subject`, `description`, `active_form`,
`status`, `owner`, `metadata`, `blocked_by` (array of IDs), `blocks`
(derived array of IDs that list this task in their `blocked_by`),
`created_at`, and `updated_at`.

### FR-015 (State-driven) — TaskList output

WHEN `task_list` is called, the system SHALL return all tasks for the
current session, ordered by `created_at`. Each entry SHALL include
`id`, `subject`, `status`, `owner` (if any), and `blocked_by` (array).
A `status` filter parameter (`pending` | `in_progress` | `completed` |
`all`) SHALL be accepted, defaulting to `all`.

### FR-016 (Ubiquitous) — Backward-compatible aliases

The existing `todo_read` tool SHALL remain registered and SHALL
delegate to the `task_list` implementation, producing the same
human-readable text output format. The existing `todo_write` tool
SHALL remain registered and SHALL map its `action` parameter
(`add` → `task_create`, `update`/`complete` → `task_update`,
`remove` → a delete path, `clear` → bulk delete) onto the new
storage layer. Both alias tools SHALL emit a deprecation notice in
their output directing callers to the new tool names.

### FR-017 (Event-driven) — TUI panel refresh on task mutation

WHEN a `task_create`, `task_update`, or delete operation succeeds, the
system SHALL publish the same event-bus signal that the current
`todo_write` publishes, so the TUI panel re-queries and re-renders
without requiring a manual Alt+T toggle.

### FR-018 (State-driven) — TUI panel title and layout

The TUI side panel toggled by Alt+T SHALL be retitled from "TODO" to
"TASKS". The panel SHALL render each task as a status-coloured line
(`pending` = yellow, `in_progress` = cyan, `completed` = green,
blocked = red) followed by the subject, an optional `(owner)` suffix,
and — when the task is blocked — a ` [blocked by #id, …]` annotation.
The existing scroll, scrollbar, text-selection, and mutual-exclusion
behaviour (only one of log / profile / tasks / memory / telemetry
visible at a time) SHALL be preserved.

### FR-019 (Optional) — `/task` slash aliases

The system MAY register `/task` and `/tasks` slash aliases that toggle
the panel (when invoked with no arguments) and delegate to the new
tools for subcommands (`/task add`, `/task list`, etc.). The existing
`/todo` and `/todo_list` slash commands SHALL continue to function as
aliases.

### FR-020 (Unwanted) — No removal of existing tests

The migration SHALL NOT delete or disable the existing
`test_todo_panel`, `test_todo_lifecycle`, `test_todo_status_change`, or
`test_todo_demo` test suites. These tests SHALL be updated to exercise
the new tool names (or the alias shim) and SHALL continue to pass.

## Non-Functional Requirements

### NFR-001 — Performance

The dependency DAG evaluation (FR-003, FR-005) SHALL complete in O(N +
E) time per completion event, where N is the number of tasks in the
session and E is the number of `blocked_by` edges. For a typical
session (≤ 100 tasks) this SHALL be sub-millisecond.

### NFR-002 — Migration safety

The SQLite schema migration SHALL be additive-only (ALTER TABLE ADD
COLUMN with defaults). No destructive DROP or RENAME shall occur at
startup. A user upgrading ragent SHALL not lose existing todos.

### NFR-003 — No new dependencies

The migration SHALL NOT introduce new third-party crate dependencies.
DAG cycle detection SHALL use the standard library (`HashMap`,
`HashSet`) or a small inline routine.

## Open Questions

1. **`blocked` status removal** — the current Todo system has a
   `blocked` *status*. The new model derives blocked-ness from
   dependencies (FR-005). Should `task_update status=blocked` be
   rejected, or silently mapped to "set `blocked_by` to a sentinel"?
   *Recommendation: reject it with a clear error, since blocked is now
   derived.*

2. **`done` vs `completed`** — the old system uses `done`; the article
   uses `completed`. Should the alias layer accept both? *Recommendation:
   yes — `todo_write action=complete` already maps to `done`; the new
   tools standardise on `completed` and the alias accepts both.*

3. **Delete path** — the article's four tools do not include a delete.
   The current `todo_write action=remove` / `clear` does. Should
   `task_update` support a delete, or should a separate `task_delete`
   be added? *Recommendation: keep delete on the alias only; do not add
   a fifth tool, matching the article's four-tool surface.*