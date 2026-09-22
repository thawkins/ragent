# Implementation Plan — Todo-to-Tasks Migration

**Spec:** `specs/todo2tasks/SPEC.md`

This plan decomposes the migration into ordered, verifiable tasks. Each
task links to one or more requirements (FR-### / NFR-###) from the spec.
Effort is sized S / M / L. Priority follows the project's 0–4 scale
mapped to Critical / High / Medium / Low.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Extend `TodoRow` / storage types with Task fields (`active_form`, `owner`, `metadata`, `blocked_by`) and add serde defaults for legacy rows | FR-001, FR-002, FR-006, FR-007, FR-008 | M | Critical | completed | — |
| T-002 | Additive SQLite migration: `ALTER TABLE todos ADD COLUMN` for `active_form`, `owner`, `metadata` (TEXT JSON), and `blocked_by` (TEXT JSON array); safe defaults on existing rows | FR-002, NFR-002 | S | Critical | completed | T-001 |
| T-003 | Add storage CRUD methods for the new Task columns: `create_task`, `get_task`, `list_tasks`, `update_task` (extend or wrap existing `create_todo` / `get_todos` / `update_todo` to read/write new columns) | FR-001, FR-010, FR-014, FR-015 | M | Critical | completed | T-002 |
| T-004 | Implement dependency DAG: persist `blocked_by` edges as JSON array, derive `blocks` (inverse edges) on read, compute "available" and "blocked" flags at read time | FR-005, NFR-001 | M | High | completed | T-003 |
| T-005 | Implement cycle detection on `add_blocked_by` / `add_blocks` — DFS over the session's task graph using stdlib `HashMap` / `HashSet`; reject cycles with descriptive error | FR-004, NFR-003 | M | High | completed | T-004 |
| T-006 | Implement auto-unblock evaluation: when a task transitions to `completed`, re-evaluate all tasks in the session that list it in `blocked_by`; flag unblocked dependents as "available" in list/get output | FR-003 | S | High | completed | T-004, T-005 |
| T-007 | Create `task_create` tool (`TaskCreateTool`) — required `subject` + `description`; optional `active_form`, `owner`, `metadata` (JSON object), `blocked_by` (array of task IDs); validates `blocked_by` refs exist in session (FR-009); generates ID via existing scheme; sets `status` = `pending`; returns created task | FR-009, FR-011, FR-012 | M | Critical | completed | T-003, T-005 |
| T-008 | Create `task_update` tool (`TaskUpdateTool`) — required `task_id`; optional `status` (`pending` / `in_progress` / `completed`), `subject`, `description`, `active_form`, `owner`, `metadata`, `add_blocked_by` (array), `add_blocks` (array); validates deps (FR-004, FR-009); rejects `status=blocked`; triggers auto-unblock (FR-003) on `completed` transition | FR-004, FR-009, FR-011, FR-013 | M | Critical | completed | T-006, T-007 |
| T-009 | Create `task_get` tool (`TaskGetTool`) — required `task_id`; returns full record: `id`, `subject`, `description`, `active_form`, `status`, `owner`, `metadata`, `blocked_by`, `blocks` (derived), `created_at`, `updated_at` | FR-011, FR-014 | S | High | completed | T-004 |
| T-010 | Create `task_list` tool (`TaskListTool`) — optional `status` filter (`pending` / `in_progress` / `completed` / `all`, default `all`); returns all session tasks ordered by `created_at`; each entry includes `id`, `subject`, `status`, `owner`, `blocked_by` | FR-011, FR-015 | S | High | completed | T-004 |
| T-011 | Register all four new tools in `ragent_tools_extended::register_tools` and bridge through `CoreStorageAdapter` trait; hardwire auto-approve (same permission category as `todo_read` / `todo_write`); wire into the tool category that triggers TUI panel-refresh events | FR-011, FR-017 | S | Critical | completed | T-007, T-008, T-009, T-010 |
| T-012 | Convert `TodoReadTool` into a thin alias delegating to `task_list` implementation; produce same human-readable text format; emit deprecation notice directing callers to `task_list` | FR-016 | S | High | completed | T-010, T-011 |
| T-013 | Convert `TodoWriteTool` into a thin alias: map `action=add` → `task_create`, `action=update`/`complete` → `task_update` (accept both `done` and `completed`), `action=remove` → delete path, `action=clear` → bulk delete; emit deprecation notice directing callers to `task_create` / `task_update` | FR-016 | M | High | completed | T-007, T-008, T-011 |
| T-014 | Update `message_widget.rs` tool-display pretty-printer to recognise `task_create` / `task_update` / `task_get` / `task_list` tool names and route them through the same panel-refresh path as `todo_write` | FR-017 | S | Medium | completed | T-011 |
| T-015 | Rework `render_todo_panel` → `render_tasks_panel`: retitle panel header "TODO" → "TASKS"; render each task as a status-coloured line (`pending` = yellow, `in_progress` = cyan, `completed` = green, blocked = red); append `(owner)` suffix when owner is set; append `[blocked by #id, …]` annotation when derived blocked; render `active_form` as indented sub-line beneath subject when task is `in_progress`; preserve existing scroll, scrollbar, text-selection, and mutual-exclusion behaviour | FR-005, FR-007, FR-018 | M | High | completed | T-004 |
| T-016 | Update `InputAction::ToggleTodo` status-bar message to "tasks panel visible" / "tasks panel hidden"; update `/todo` and `/todo_list` slash command descriptions; add `/task` and `/tasks` slash aliases that toggle the panel (no args) and delegate subcommands (`/task add`, `/task list`, etc.) to the new tools | FR-018, FR-019 | S | Medium | completed | T-015 |
| T-017 | Reject `status=blocked` at the `task_update` boundary with a clear error listing valid statuses (`pending`, `in_progress`, `completed`); reject foreign/non-existent `blocked_by` references at `task_create` / `task_update` boundary with a clear error (per FR-009) | FR-004, FR-005, FR-009, FR-013 | S | High | completed | T-005 |
| T-018 | Migrate and update existing test suites: `test_todo_panel`, `test_todo_lifecycle`, `test_todo_status_change`, `test_todo_demo` — exercise new `task_*` tool names and alias shim; add new test coverage for DAG cycle detection, auto-unblock on completion, derived blocked-annotation rendering in the panel, and owner/active_form display | FR-020, NFR-001 | L | High | completed | T-015, T-013 |
| T-019 | Update project documentation: AGENTS.md, SPEC.md, QUICKSTART.md, README.md, and TUI-QUICKSTART.md tool listings to describe the four `task_*` tools, the "TASKS" panel, and the deprecated `todo_*` aliases | FR-018, FR-019 | S | Low | completed | T-011, T-016 |
| T-020 | Run `cargo fmt`, `cargo clippy`, `cargo test` across the workspace; resolve any warnings or formatting issues introduced by the migration | NFR-002, NFR-003 | S | Critical | completed | T-018 |
## Execution Order

The tasks form a mostly-linear dependency chain with parallel branches
after the storage layer lands:

```
T-001 → T-002 → T-003 → T-004 → T-005 → T-006
                                   │
                                   ├── T-007 → T-008 ──┐
                                   ├── T-009           ├── T-011 → T-012 / T-013 → T-014
                                   └── T-010           │
                                                       ▼
                                          T-015 → T-016 → T-017 → T-018 → T-019 → T-020
```

- **Phase 1 (storage & model):** T-001 → T-002 → T-003 → T-004 →
  T-005 → T-006. Verifiable by `cargo check` after each step and a
  unit test that creates a task with dependencies and reads it back.
- **Phase 2 (tools):** T-007 / T-008 / T-009 / T-010 in parallel,
  then T-011 to wire them in. Verifiable by the existing
  `test_hardwired_todo_*` pattern adapted to `task_*` names.
- **Phase 3 (aliases & display):** T-012 / T-013 / T-014 / T-015 /
  T-016. Verifiable by the migrated TUI panel tests and slash-command
  tests.
- **Phase 4 (hardening & docs):** T-017 / T-018 / T-019 / T-020.
  Verifiable by `cargo clippy` + `cargo test` green and a manual
  Alt+T smoke test.

## Verification Milestones

| Milestone | Success criterion |
|-----------|-------------------|
| M1 — Storage ready | `cargo test -p ragent-storage` passes with a task containing `blocked_by`; reading it back yields the same edges; legacy rows with missing columns return safe defaults. |
| M2 — Tools wired | `task_create` + `task_update` + `task_get` + `task_list` round-trip through the tool registry; `cargo test -p ragent-tools-extended` green; tools are hardwired auto-approved. |
| M3 — Aliases pass | Existing `test_todo_*` suites pass unchanged against the alias shim; deprecation notice appears in alias output. |
| M4 — Panel renders | `test_todo_panel` (renamed / updated) asserts the "TASKS" title, blocked annotation, owner suffix, and `active_form` sub-line. |
| M5 — Full green | `cargo fmt --check && cargo clippy && cargo test` all clean across the workspace. |