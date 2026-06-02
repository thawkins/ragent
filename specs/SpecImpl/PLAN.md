# SpecImpl — Implementation Plan

## Architecture

The `/spec impl` command is implemented as a new subcommand handler in the
TUI's `handle_slash_command()` match arm, delegating to a `SpecImplRunner`
struct in `ragent-specs` that orchestrates the plan parsing and execution.

### Component Overview

1. **TUI handler** — Adds `/spec impl [specname]` to the slash command match
   in `app.rs`, parses flags, and invokes the runner.
2. **PlanParser** — New module in `ragent-specs` that parses `PLAN.md` task
   tables into `PlanTask` structs and resolves dependency order.
3. **SpecImplRunner** — Orchestrates task execution: constructs prompts,
   injects them into the session, tracks progress, and updates `PLAN.md`.
4. **PlanTask** — Struct representing a parsed task row (ID, title,
   requirement, effort, priority, dependencies, status).
5. **Status persistence** — Adds a `Status` column to the `PLAN.md` task
   table, persisted after each task transition.

### File Layout

```
crates/ragent-specs/src/
├── impl_runner.rs   # SpecImplRunner + execution orchestration
├── plan_parser.rs   # PlanParser + PlanTask + DAG resolution
├── manager.rs       # +1 method: run_impl()
└── lib.rs           # +1 pub mod

crates/ragent-tui/src/
└── app.rs           # +1 match arm in handle_slash_command()
```

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define PlanTask struct and parsing types | FR-003, FR-004 | S | Critical | completed | — |
| T-002 | Implement PlanParser for markdown task tables | FR-003, FR-004, FR-025 | M | Critical | completed | T-001 |
| T-003 | Implement dependency DAG and topological sort | FR-005, FR-006 | M | Critical | completed | T-001 |
| T-004 | Implement Status column addition to PLAN.md | FR-019 | M | High | completed | T-002 |
| T-005 | Implement SpecImplRunner orchestration | FR-007, FR-008, FR-010, FR-011 | L | Critical | completed | T-002, T-003 |
| T-006 | Implement task prompt construction | FR-021, FR-022 | S | Critical | completed | T-001 |
| T-007 | Implement task status updates via spec_task_update | FR-009, FR-010, FR-011, FR-023 | M | Critical | completed | T-005 |
| T-008 | Implement progress display and summaries | FR-014, FR-015, FR-016 | S | High | completed | T-005 |
| T-009 | Implement completion: set spec status to implemented | FR-016 | S | High | completed | T-005 |
| T-010 | Implement cancellation handling | FR-017 | M | High | completed | T-005 |
| T-011 | Implement resume from previously completed tasks | FR-020 | M | High | completed | T-004, T-005 |
| T-012 | Implement --task flag for single-task execution | FR-012 | S | Medium | completed | T-005 |
| T-013 | Implement --dry-run flag | FR-013 | S | Medium | completed | T-005 |
| T-014 | Add /spec impl to TUI slash command handler | FR-001, FR-002 | M | Critical | completed | T-005 |
| T-015 | Implement spec-already-implemented confirmation | FR-026 | S | Medium | completed | T-014 |
| T-016 | Implement requirement text injection from SPEC.md | FR-022 | S | Low | completed | T-006 |
| T-017 | Write unit tests for PlanParser | FR-003, FR-004, NFR-002 | M | Critical | completed | T-002 |
| T-018 | Write unit tests for dependency DAG | FR-005, FR-006 | M | Critical | completed | T-003 |
| T-019 | Write integration test for full impl flow | NFR-001, NFR-003 | M | High | completed | T-014 |
| T-020 | Update SPEC.md and documentation | — | S | Low | completed | T-014 |
## Task Details

### T-001 — PlanTask Struct (S, Critical)

Define the core data types in `plan_parser.rs`:

```rust
pub struct PlanTask {
    pub id: String,           // e.g. "T-001"
    pub title: String,
    pub requirement: String,
    pub effort: Effort,       // S, M, L
    pub priority: Priority,   // Critical, High, Medium, Low
    pub dependencies: Vec<String>, // IDs of prerequisite tasks
    pub status: TaskStatus,   // pending, in_progress, completed, blocked
}

pub enum Effort { S, M, L }
pub enum Priority { Critical, High, Medium, Low }
pub enum TaskStatus { Pending, InProgress, Completed, Blocked }
```

### T-002 — PlanParser (M, Critical)

Implement `PlanParser::parse(markdown: &str) -> Result<Vec<PlanTask>>`:

- Locate the `## Tasks` section header
- Find the markdown table (lines starting with `|`)
- Skip the separator row (`|---|`)
- Parse each data row into a `PlanTask`
- Validate that the ID matches pattern `T-\d{3}`
- FR-004: Skip malformed rows with a warning
- FR-025: Return error if zero valid rows

### T-003 — Dependency DAG & Topological Sort (M, Critical)

Implement `resolve_execution_order(tasks: &[PlanTask]) -> Result<Vec<&PlanTask>>`:

- Build adjacency list from `dependencies` fields
- Perform Kahn's algorithm (BFS topological sort)
- FR-005: Return ordered task list
- FR-006: Detect and report cycles

### T-004 — Status Column Persistence (M, High)

Implement `add_status_column(plan_md: &str, statuses: &HashMap<String, TaskStatus>) -> String`:

- If `Status` column doesn't exist, add it to the header and separator rows
- Update each task row with the current status
- NFR-003: Atomic write (write to temp file, rename)

### T-005 — SpecImplRunner Orchestration (L, Critical)

Implement `SpecImplRunner`:

```rust
pub struct SpecImplRunner {
    spec_name: String,
    spec_dir: PathBuf,
    tasks: Vec<PlanTask>,
    execution_order: Vec<usize>, // indices into tasks
}
```

- `run()` method: iterate through tasks in order, construct prompts,
  inject into session processor, wait for completion
- FR-007: Set spec status to `in_progress` at start
- FR-008: Execute in topological order
- FR-010: Mark completed tasks
- FR-011: On failure, mark task as `blocked`, propagate to dependents

### T-006 — Task Prompt Construction (S, Critical)

Implement `build_task_prompt(task: &PlanTask, spec_name: &str) -> String`:

- FR-021: Use the template format
- FR-022: Optionally inject requirement text from SPEC.md

### T-007 — Task Status Updates (M, Critical)

- After each task completes, call `spec_task_update` to update the spec
  system's task tracking
- Update `PLAN.md` Status column
- FR-009: Mark as `in_progress` before execution
- FR-010: Mark as `completed` on success
- FR-011: Mark as `blocked` on failure

### T-008 — Progress Display (S, High)

- Display plan overview before execution: total tasks, effort estimates,
  execution order
- Display progress after each task: `✅ T-001 (3/12) — Next: T-002`
- FR-014: Initial summary
- FR-015: Per-task updates

### T-009 — Completion (S, High)

- FR-016: When all tasks complete, set spec status to `implemented`
- Display completion summary with task results

### T-010 — Cancellation (M, High)

- FR-017: On cancel (Ctrl+C or `/cancel`), stop after current task
- Mark remaining tasks as `pending`
- Preserve spec status as `in_progress`

### T-011 — Resume (M, High)

- FR-020: When `PLAN.md` already has a Status column with `completed` tasks,
  skip them and resume from the first actionable task
- Re-evaluate `blocked` tasks whose dependencies are now all `completed`

### T-012 — --task Flag (S, Medium)

- FR-012: Execute only the specified task and its transitive dependencies
- Build sub-graph from the target task backwards through dependencies

### T-013 — --dry-run Flag (S, Medium)

- FR-013: Display execution order and task details without executing
- Show: task ID, title, effort, priority, dependencies, execution rank

### T-014 — TUI Integration (M, Critical)

Add `/spec impl` to `handle_slash_command()` in `app.rs`:

- Parse: `/spec impl <specname> [--task <ID>] [--dry-run]`
- FR-001: Validate spec name and file existence
- FR-002: Error message with available specs on invalid name
- Invoke `SpecImplRunner::run()` and stream output to chat

### T-015 — Confirmation for Implemented Specs (S, Medium)

- FR-026: If spec status is `implemented` or `verified`, prompt user:
  "Spec {name} is already marked as {status}. Re-implement? [y/N]"

### T-016 — Requirement Text Injection (S, Low)

- FR-022: Parse requirement references (e.g. "FR-014") from task
  requirement column, look up full text in `SPEC.md`, include in prompt

### T-017 — PlanParser Unit Tests (M, Critical)

- Parse valid task table
- Skip malformed rows (FR-004)
- Error on zero valid rows (FR-025)
- Parse with existing Status column (FR-020)

### T-018 — DAG Unit Tests (M, Critical)

- Topological sort with simple chain
- Topological sort with diamond dependencies
- Cycle detection (FR-006)
- Empty dependency list

### T-019 — Integration Test (M, High)

- Full flow: create spec → `/spec impl` → tasks execute → spec status updated
- Resume flow: partial completion → re-run → remaining tasks execute
- NFR-001: Performance check
- NFR-003: Atomic write verification

### T-020 — Documentation (S, Low)

- Update SPEC.md with `/spec impl` command
- Update QUICKSTART.md with usage example
- Add example to docs/userdocs/

## Estimated Effort

| Category | Effort |
|---|---|
| Core implementation (T-001 to T-016) | 16 tasks, ~5–7 days |
| Testing (T-017 to T-019) | 3 tasks, ~1–2 days |
| Documentation (T-020) | 1 task, ~0.5 days |
| **Total** | **~7–10 days** |

## Risks

| Risk | Mitigation |
|---|---|
| Agent goes off-plan during task execution | Prompt template includes scope guard; permission system provides defense-in-depth |
| PLAN.md corruption on partial write | NFR-003: Write to temp file, then atomic rename |
| Dependency cycle in user-authored plans | FR-006: Cycle detection with actionable error message before execution starts |
| Long plans stall the TUI | Tasks execute one at a time; cancellation supported (FR-017) |
| Resume after partial completion | FR-020: Status column persists in PLAN.md; re-parse on each invocation |