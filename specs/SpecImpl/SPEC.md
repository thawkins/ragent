---
status: draft
audit:
  - { time: 1780420837, from: "none", to: "draft", actor: "system" }
---
# SpecImpl — `/spec impl` Slash Command

## Overview

This specification defines the `/spec impl [specname]` slash command, which reads
the `PLAN.md` from a spec directory, parses its task table, and drives the agent
through implementing each task in dependency order. The command transforms a
static implementation plan into an orchestrated, step-by-step agent workflow
with progress tracking and status updates.

## Requirements

### Command Parsing & Invocation

**FR-001** (Event-driven) When the user enters `/spec impl [specname]` in the
TUI input, the system shall parse the command, extract the spec name, and
validate that the spec directory exists under `specs/[specname]/` with both
`SPEC.md` and `PLAN.md` present.

**FR-002** (Event-driven) When the spec name is missing or the spec directory
does not contain a `PLAN.md`, the system shall display an error message
indicating the problem and listing available specs with plan files.

### Plan Parsing

**FR-003** (Ubiquitous) The system shall parse the `PLAN.md` task table,
extracting each row's ID, Title, Requirement, Effort, Priority, and
Dependencies columns into a structured `PlanTask` representation.

**FR-004** (State-driven) While a task row does not conform to the expected
markdown table format (missing required columns, unparseable dependency
list), the system shall skip the row with a warning and continue parsing
remaining rows.

**FR-005** (Ubiquitous) The system shall resolve task dependencies into a
directed acyclic graph (DAG) and produce a topological execution order,
ensuring no task is scheduled before its dependencies.

**FR-006** (State-driven) While the dependency graph contains a cycle, the
system shall report the cycle (listing the involved task IDs) and abort the
command with an actionable error message.

### Task Execution

**FR-007** (Event-driven) When the execution begins, the system shall set the
spec status to `in_progress` (updating the `SPEC.md` YAML frontmatter).

**FR-008** (Ubiquitous) The system shall execute tasks in topological order.
For each task, the system shall construct a prompt from the task's Title and
Requirement fields, inject it into the current agent session, and wait for
the agent to complete the task.

**FR-009** (State-driven) While a task is being executed, the system shall
update the corresponding `PLAN.md` task row to indicate `in_progress` status
(by appending a status marker or updating a Status column).

**FR-010** (Event-driven) When a task completes successfully, the system shall
mark the task as `completed` in the `PLAN.md` and proceed to the next task
in topological order.

**FR-011** (Event-driven) When a task fails (agent returns an error or user
cancels), the system shall mark the task as `blocked` in the `PLAN.md`, skip
all dependent tasks (marking them `blocked` as well), and continue with any
remaining independent tasks.

**FR-012** (Optional) Where the `--task <ID>` flag is provided
(`/spec impl [specname] --task T-003`), the system shall execute only the
specified task and its dependencies, not the entire plan.

**FR-013** (Optional) Where the `--dry-run` flag is provided
(`/spec impl [specname] --dry-run`), the system shall display the execution
order and task details without actually executing any tasks.

### Progress Tracking

**FR-014** (Ubiquitous) The system shall display a progress summary in the
chat output before execution begins, showing the total number of tasks,
dependency order, and effort estimates.

**FR-015** (Event-driven) When each task completes, the system shall display
a progress update showing completed/total tasks and the ID of the next task.

**FR-016** (Event-driven) When all tasks are complete, the system shall set
the spec status to `implemented` (updating `SPEC.md` frontmatter) and display
a completion summary.

### Completion & Cancellation

**FR-017** (Event-driven) When the user cancels the implementation
(Ctrl+C or `/cancel`), the system shall stop after the current task finishes,
mark remaining tasks as `pending`, and preserve the spec status as
`in_progress`.

**FR-018** (Unwanted) The system shall not modify any files outside the scope
defined by the task requirements. If a task's prompt would cause the agent to
modify files unrelated to the requirement, the permission system shall catch
the violation per existing rules.

### Plan Status Persistence

**FR-019** (Ubiquitous) The system shall persist task statuses in the
`PLAN.md` by adding a `Status` column to the task table. Valid values are:
`pending`, `in_progress`, `completed`, `blocked`.

**FR-020** (Event-driven) When `/spec impl` is invoked on a spec that has
previously completed tasks (Status column already present), the system shall
skip `completed` tasks and resume from the first `pending` or `blocked` task
whose dependencies are all `completed`.

### Prompt Construction

**FR-021** (Ubiquitous) The system shall construct the agent prompt for each
task using the following template:

```
Implement task {ID}: {Title}

Requirement: {Requirement text}

Follow the implementation plan. After completing this task, use the
`spec_task_update` tool to mark task {ID} as completed in spec {specname}.
```

**FR-022** (Optional) Where a task's requirement references specific spec
requirements (e.g. "FR-014"), the system shall include the full text of those
requirements from `SPEC.md` in the prompt.

### Integration with Spec Tools

**FR-023** (Ubiquitous) The system shall use the existing `spec_task_update`
tool (from `ragent-specs`) to update task statuses, ensuring the spec
management system stays in sync with the plan execution.

**FR-024** (Ubiquitous) The system shall use the existing `spec_read` tool
to load spec details and the `spec_coverage` tool to report requirement
coverage after all tasks complete.

### Error Handling

**FR-025** (Unwanted) The system shall not proceed with task execution if
the `PLAN.md` cannot be parsed (zero valid task rows). It shall display an
error and exit without modifying any files.

**FR-026** (Event-driven) When the spec status is already `implemented` or
`verified`, the system shall prompt the user to confirm re-implementation
before proceeding.

## Non-Functional Requirements

**NFR-001** The `/spec impl` command shall display the execution plan within
2 seconds of invocation for plans with up to 30 tasks.

**NFR-002** The plan parser shall handle task tables with up to 50 rows
without performance degradation.

**NFR-003** Task status updates to `PLAN.md` shall be atomic — a partial
write must not corrupt the file.

## Out of Scope

- Parallel task execution (tasks are sequential, respecting dependencies)
- Automatic roll-back of completed tasks on failure
- Integration with CI/CD pipelines
- Spec status transitions beyond `in_progress` → `implemented`
- Support for non-markdown plan formats