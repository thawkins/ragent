---
status: draft
---

# Manual Test Plan: Activity Logging with Rollback and Resume

This is a manual, human-executed test plan. It does not contain automated
test code. Each test case lists preconditions, exact steps, data to enter,
and expected results.

## Prerequisites

Before executing the manual tests, set up the following environment:

1. **Runtime available** — the activity logging subsystem is built and the
   runtime host is running.
2. **Local storage configured** — the event log store points to a writable
   local directory (e.g. `<workspace>/runtime.sqlite` or an equivalent log
   directory).
3. **A model connection** — at least one send-ready model connection is
   configured (API key entered, default model selected) so agent runs can
   produce model-message and tool-call events.
4. **Sample task** — a simple prompt that triggers at least one tool call,
   for example: `Read the file README.md and summarise it in one sentence.`
5. **A workspace with a file** — a `README.md` file exists in the active
   workspace so the Read tool has a target.
6. **Operator controls exposed** — commands or UI affordances for: start run,
   abort run, list runs, invoke rollback, invoke resume, export run log, and
   create checkpoint are available.
7. **Optional** — automatic checkpointing disabled by default, so manual
  checkpoint creation can be tested explicitly.

## Test Cases

### TC-001 — Activity log records a full turn

**Title:** Verify that a single agent turn produces a complete, ordered event
log.

**Preconditions:**
- Runtime host is running.
- A send-ready model connection is configured.
- The workspace contains a `README.md` file.

**Steps:**
1. Start a new run with the prompt: `Read the file README.md and summarise it in one sentence.`
2. Wait for the run to complete (termination event recorded).

**Test data to enter:**
- Prompt text: `Read the file README.md and summarise it in one sentence.`

**Expected results:**
- The run's event log contains, in order:
  1. A model-message event (the initial prompt).
  2. A tool-call event for the Read tool targeting `README.md`.
  3. A tool-result event containing the file contents, linked by the same
     tool-call identifier as event 2.
  4. A model-message event (the summary).
  5. A turn-termination event.
- Every event has a monotonically increasing sequence number and an
  immutable event identifier.
- No event is missing a sequence number or identifier.

---

### TC-002 — Interrupted run is marked and resumable

**Title:** Verify that an interrupted run records a termination event and is
exposed as resumable.

**Preconditions:**
- A run is in progress (mid-turn).
- An operator control to abort the run is available.

**Steps:**
1. Start a new run with the prompt: `Read README.md, then read CHANGELOG.md, then summarise both.`
2. While the run is executing the first tool call, invoke the abort/stop
   control.
3. List all runs and inspect the interrupted run's status.

**Test data to enter:**
- Prompt text: `Read README.md, then read CHANGELOG.md, then summarise both.`

**Expected results:**
- The run is marked as "interrupted".
- A termination event is recorded at the last committed sequence number.
- The run appears in the run list as resumable.
- No new events can be appended to the run until a resume operation is
  initiated (FR-006).

---

### TC-003 — Resume continues an interrupted run

**Title:** Verify that resuming an interrupted run replays the event log and
continues execution.

**Preconditions:**
- TC-002 has been executed, leaving an interrupted run.
- The model connection is still send-ready.

**Steps:**
1. From the run list, select the interrupted run.
2. Invoke the resume control.
3. Wait for the run to complete.

**Test data to enter:**
- (No prompt; resume continues the existing task.)

**Expected results:**
- The system replays the event log to rebuild the active context.
- Execution continues from the event following the last committed sequence
  number.
- The completed run's log contains the original events followed by the new
  events produced after resume.
- The run's final status is "completed", not "interrupted".

---

### TC-004 — Rollback to a checkpoint preserves the full log

**Title:** Verify that rolling back derived state to a checkpoint resets the
projection without deleting events after the checkpoint.

**Preconditions:**
- A run has completed at least two turns.
- A checkpoint was created after the first turn (manually or via automatic
  checkpointing if enabled).

**Steps:**
1. Open the completed run.
2. Identify the checkpoint created after turn 1.
3. Invoke rollback targeting that checkpoint.
4. Inspect the run's derived state (active context / UI).
5. Export the run's full event log.

**Test data to enter:**
- Checkpoint target: the name or sequence number of the turn-1 checkpoint.

**Expected results:**
- The derived projection reflects state as of the checkpoint (turn 1 only).
- The exported event log still contains all events, including those after the
  checkpoint (FR-007).
- The events after the checkpoint are ignored for the projection but present
  for audit.

---

### TC-005 — Mutation of a committed event is rejected

**Title:** Verify that attempting to delete or mutate a committed event is
rejected and audited.

**Preconditions:**
- A run has produced at least one event.

**Steps:**
1. Identify an existing committed event (e.g. the first model-message event).
2. Using a low-level/admin interface (or a crafted request), attempt to
   delete or overwrite that event.
3. Inspect the run's event log for a rejection audit event.

**Test data to enter:**
- Target event: the first event's identifier.

**Expected results:**
- The mutation attempt is rejected (no change to the original event).
- A separate audit event is recorded describing the rejected mutation
  (FR-010).
- The original event remains intact and unchanged.

---

### TC-006 — Resume aborts on an inconsistent log

**Title:** Verify that resume aborts when the event log is internally
inconsistent.

**Preconditions:**
- An interrupted run exists.
- A test harness or admin tool can inject/remove an event to create an
  inconsistency (e.g. remove a tool-result event while leaving its matching
  tool-call event).

**Steps:**
1. Using the admin tool, remove the tool-result event from the interrupted
   run's log, leaving the tool-call event orphaned.
2. Invoke resume on the interrupted run.

**Test data to enter:**
- (No prompt; resume continues the existing task.)

**Expected results:**
- The resume operation detects the missing tool-result event.
- The run is marked as "unrecoverable".
- No partial projection is produced (FR-011).
- The original (inconsistent) log is preserved for diagnosis.

---

### TC-007 — Context pruning preserves evidence

**Title:** Verify that pruning old tool results from the next model prompt
does not delete the corresponding events.

**Preconditions:**
- Context pruning is enabled.
- A run has produced enough tool results that pruning would apply (e.g. more
  than the configured context budget).

**Steps:**
1. Start a long run that performs many tool calls (e.g. read 10 files).
2. Let the run progress past the pruning threshold.
3. Inspect the next model prompt (the active context projection).
4. Export the run's full event log.

**Test data to enter:**
- Prompt: `Read file1.txt, file2.txt, file3.txt, file4.txt, file5.txt, then summarise all of them.`

**Expected results:**
- The next model prompt omits some older tool results (FR-009).
- The exported event log still contains every tool-call and tool-result
  event.
- No event has been deleted from the log.

---

### TC-008 — JSON Lines export is complete and machine-readable

**Title:** Verify that a run's event log can be exported as JSON Lines.

**Preconditions:**
- A completed run with at least one turn exists.

**Steps:**
1. Select the completed run.
2. Invoke the export control, choosing JSON Lines format.
3. Open the exported file in a text editor or pipe it through a JSON Lines
   parser.

**Test data to enter:**
- Export format: JSON Lines (`.jsonl`).

**Expected results:**
- The exported file contains one JSON object per line.
- Each object includes the event type, schema version, run identifier,
  sequence number, and event identifier (NFR-003, NFR-004).
- A standard JSON Lines parser accepts the file without errors.

---

### TC-009 — Run branching from a checkpoint

**Title:** Verify that branching a new run from a checkpoint copies events up
to the checkpoint.

**Preconditions:**
- A run has a checkpoint at turn 1 and has continued to turn 3.

**Steps:**
1. Open the original run.
2. Select the turn-1 checkpoint.
3. Invoke the branch control, providing a name for the new run.
4. Open the new branched run and inspect its event log.

**Test data to enter:**
- New run name: `branched-from-turn1`
- Checkpoint target: the turn-1 checkpoint.

**Expected results:**
- A new run is created.
- The new run's event log contains the events up to and including the
  checkpoint.
- Both runs record the branch origin (FR-018).
- The original run's log is unchanged.

---

### TC-010 — Concurrent rollback/resume is blocked during rebuild

**Title:** Verify that a second rollback or resume is blocked while a
projection rebuild is in progress.

**Preconditions:**
- A run with many events (enough that replay is observable).
- Two operator sessions or two rapid invocations.

**Steps:**
1. Invoke rollback on the run.
2. Before the rebuild completes, invoke resume (or a second rollback) on the
   same run from a second session.

**Test data to enter:**
- (No prompt; operations on the existing run.)

**Expected results:**
- The second operation is rejected or queued.
- The run is presented as "rebuilding" during the first operation (FR-014).
- The first rebuild completes before any second operation is permitted.

## Cleanup

After completing the manual tests:

1. **Stop the runtime host** to flush all pending event-log writes.
2. **Archive test runs** — export each test run's JSON Lines log to an archive
   directory for audit retention (per FR-015) before discarding.
3. **Reset the event log store** — if using a disposable test workspace, the
   log database can be deleted; if using a shared workspace, delete only the
   test runs created during this session.
4. **Disable any temporary admin tools** used to inject inconsistencies
   (TC-006) so they cannot affect production runs.
5. **Restore default settings** — re-disable automatic checkpointing and
   context pruning if they were enabled only for testing.