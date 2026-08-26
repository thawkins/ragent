# Activity Logging: Operator Controls

This guide documents the operator controls for the activity logging
subsystem — the append-only event log that records every agent execution
event (model messages, tool calls, tool results, permission decisions,
checkpoints, and terminations) and supports rollback, resume, and retention.

All controls are exposed as methods on the [`ActivityLog`] store
(`ragent_storage::activity_log::ActivityLog`).

## Overview

| Control | Method | Requirement |
|---------|--------|-------------|
| Create a checkpoint | `record_checkpoint(run_id, name)` | FR-008 |
| Find a checkpoint | `find_checkpoint(run_id, name)` | FR-008 |
| Roll back to a checkpoint | `rollback_to_checkpoint(run_id, name)` | FR-012 |
| Roll back to a sequence | `rollback_to_seq(run_id, seq)` | FR-012 |
| Resume an interrupted run | `resume_run(run_id)` | FR-006, FR-013 |
| Check run status | `run_status(run_id)` | FR-015 |
| List all runs | `list_runs()` | FR-015 |
| Expire a run | `expire_run(run_id, reason)` | FR-016 |
| Archive a run | `archive_run(run_id, reason)` | FR-016 |
| Expire old runs | `expire_runs_older_than(max_age)` | FR-016 |
| Export a run log | `export_jsonl(run_id)` / `export_jsonl_to(run_id, writer)` | NFR-004 |

## Checkpoints

A checkpoint is a named, durable marker in a run's event log used as a
rollback or resume target.

### Creating a checkpoint

```rust
use ragent_storage::activity_log::ActivityLog;
use ragent_types::id::RunId;

let log = ActivityLog::open_in_memory()?;
let run = RunId::from("run-1");

// ... append events ...

// Create a named checkpoint after a completed turn.
let cp = log.record_checkpoint(&run, "after-turn-1")?;
```

The checkpoint records the last committed sequence number at the time it
was taken. The checkpoint event itself is appended at the next sequence
number.

### Finding a checkpoint

```rust
let cp = log.find_checkpoint(&run, "after-turn-1")?;
```

If multiple checkpoints share a name, the most recent (highest sequence
number) is returned.

### Automatic checkpointing (FR-008)

Where the operator configures automatic checkpointing, the system may create
a checkpoint after each completed turn. The checkpoint name should be
descriptive (e.g. `"after-turn-{n}"`) to avoid collisions with manual
checkpoints.

## Rollback

Rollback rebuilds the derived projection by replaying events from the start
of the run up to (and including) a target, ignoring all subsequent events
(FR-012). Events after the target are **preserved** in the log for audit
(FR-007) — no events are deleted.

### Rolling back to a checkpoint

```rust
let result = log.rollback_to_checkpoint(&run, "after-turn-1")?;
// result.projection: the rebuilt active context
// result.target_seq: the checkpoint's sequence number
// result.ignored_count: events after the target (preserved for audit)
```

### Rolling back to a sequence number

```rust
let result = log.rollback_to_seq(&run, 5)?;
```

The rebuilt projection contains only events up to (and including) the
target. The full log remains readable and exportable after a rollback.

## Resume

An interrupted run (one whose last event is a termination with reason
`Interrupted` or `Aborted`) is **resumable** but **not appendable** (FR-006).
New events cannot be appended until a resume operation is initiated.

### Resuming an interrupted run

```rust
let result = log.resume_run(&run)?;
// result.projection: the reconstructed active context
// result.resume_from_seq: the sequence number to continue from
```

Resume:
1. Verifies the run is in the `Interrupted` state.
2. Replays the event log to reconstruct the active context (FR-013).
3. Appends a `Lifecycle` "resumed" event to transition the run back to
   `Active`.
4. Returns the projection and the sequence number from which to continue.

After resume, new events can be appended normally.

```rust
// After resume, continue execution.
log.record_tool_result(&run, "c1", "read", true, "content")?;
log.record_termination(&run, TerminationReason::Completed)?;
```

### Checking if a run is resumable

```rust
use ragent_types::activity::RunStatus;

let status = log.run_status(&run)?;
if status == RunStatus::Interrupted {
    // The run can be resumed.
}
```

### Pending tool calls on resume

The projection returned by `resume_run` includes `pending_tool_calls()` —
tool calls that were in-flight when the run was interrupted (calls without
matching results). These need to be completed before the next model
invocation.

```rust
let result = log.resume_run(&run)?;
for call in result.projection.pending_tool_calls() {
    // Re-execute the tool call and record its result.
}
```

## Retention

### Run status

```rust
use ragent_types::activity::RunStatus;

let status = log.run_status(&run)?;
```

| Status | Meaning |
|--------|---------|
| `Active` | The run is accepting new events. |
| `Interrupted` | The run was interrupted and is resumable (FR-006). |
| `Completed` | The run finished normally. |
| `Rebuilding` | The projection is being rebuilt (FR-014). |
| `RolledBack` | The run was rolled back to a checkpoint. |

### Listing all runs

```rust
let runs = log.list_runs()?;
for run in &runs {
    let status = log.run_status(run)?;
    println!("{run}: {status:?}");
}
```

Completed runs are retained in the log and can be inspected, replayed, or
branched later (FR-015).

### Expiring a run (FR-016)

Where the operator sets a retention limit, runs older than the limit may be
expired. Expiration records the expiry as a lifecycle event (for audit)
before removing the run's events.

```rust
log.expire_run(&run, "retention limit")?;
```

### Archiving a run

`archive_run` exports the run as JSON Lines (for external storage) and then
expires it:

```rust
let jsonl = log.archive_run(&run, "age > 30 days")?;
// Save jsonl to external storage (e.g. S3, cold storage).
std::fs::write("archive/run-1.jsonl", jsonl)?;
```

### Expiring runs older than a limit

```rust
use chrono::Duration;

// Expire all runs whose last activity is older than 30 days.
let expired = log.expire_runs_older_than(Duration::days(30))?;
println!("Expired {} runs", expired.len());
```

Each expired run has a lifecycle event recorded before deletion, satisfying
FR-016's "provided it records the expiry as a lifecycle event."

## Export (NFR-004)

A run's complete event log can be exported as JSON Lines for external audit:

```rust
// Export to a string.
let jsonl = log.export_jsonl(&run)?;

// Export to a file.
let mut file = std::fs::File::create("run-1.jsonl")?;
log.export_jsonl_to(&run, &mut file)?;
```

Each line is a self-describing JSON object carrying the event type, schema
version, and run identifier (NFR-003).

## Run Branching (FR-018)

A new run can be created from a checkpoint of an existing run:

```rust
let source = RunId::from("run-source");
let new_run = RunId::from("run-branch");

let branch_event = log.branch_from_checkpoint(&source, "cp1", &new_run)?;
```

The new run receives a copy of the source's events up to (and including) the
checkpoint, a `BranchOrigin` event recording where it came from, and can then
accept new events independently. The source run records a lifecycle event
noting the branch.

## Performance (NFR-001, NFR-002)

- **Append latency** (NFR-001): a single event append is one `INSERT` inside a
  short transaction. Benchmarked at ~18 µs in-memory (target: p99 below 10 ms
  on local storage).
- **Replay speed** (NFR-002): rebuilding a projection for a run of 100,000
  events is benchmarked at ~166 ms (target: under 5 seconds).

Run benchmarks with:

```bash
cargo bench -p ragent-storage --bench activity_log_bench
```