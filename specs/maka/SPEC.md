---
status: draft
audit:
  - { time: 1787700243, from: "none", to: "draft", actor: "system" }
---
# Specification: Activity Logging with Rollback and Resume (Maka-inspired)

## Background

A review of [Apache Maka](https://github.com/apache/maka) — an Apache incubating
local-first AI agent runtime — reveals a backend architecture built around a
**Runtime Event Log**: an append-only record of model messages, tool calls,
tool results, permission decisions, and termination events. All session state,
UI projections, recovery decisions, and resume behaviour are *derived* from
that event log rather than maintained as independent sources of truth.

The key architectural insight is the separation of **durable execution facts**
(the append-only event log) from **ephemeral derived state** (active context,
UI, recovery). This separation enables three capabilities that are the subject
of this specification:

1. **Activity logging** — every action (model call, tool invocation,
   permission grant/deny, turn termination) is persisted before it is
   projected into user-facing state.
2. **Rollback** — because state is derived from the event log, the system can
   rebuild any prior state by replaying events up to a chosen point, discarding
   events after it (a logical rollback) without losing the original audit
   record.
3. **Resume** — an interrupted or long-running turn can be resumed by
   replaying its event log to rebuild context, then continuing execution from
   the last committed event.

This specification adapts that model for an activity logging subsystem that
supports rollback and resume of agent execution runs.

## Scope

### In Scope

- An append-only activity log capturing every agent execution event.
- Event types for model messages, tool calls, tool results, permission
  decisions, checkpoints, and turn termination.
- Rollback to a prior checkpoint or event boundary.
- Resume of an interrupted run from its last durable event.
- Derivation of replayable context from the event log.
- Retention of full evidence even when context is pruned for the next model
  call.

### Out of Scope

- Distributed or multi-node event replication.
- Cryptographic tamper-evidence / chain hashing (may be a future extension).
- UI design of the rollback/resume controls.
- Multi-tenant isolation.

## Definitions

| Term          | Meaning                                                                              |
| ------------- | ------------------------------------------------------------------------------------ |
| Event         | An immutable record of a single execution fact (model msg, tool call, etc.)         |
| Event Log     | The append-only, ordered store of all events for a run.                            |
| Run           | A single execution of an agent on a task; identified by a run ID.                    |
| Checkpoint    | A named, durable marker in the event log used as a rollback/resume target.          |
| Projection    | Derived state (active context, UI state) computed by replaying the event log.      |
| Rollback      | Logical reset of derived state to a prior checkpoint, keeping the full log intact. |
| Resume        | Continuing a run from its last durable event after an interruption.                 |
| Turn          | One model invocation + its tool calls and results, ending in a termination event. |

## Requirements

### Functional Requirements

**FR-001 (Ubiquitous)**
The system **shall** persist every execution event — model message, tool call,
tool result, permission decision, checkpoint, and turn termination — to the
append-only event log before projecting it into any user-facing or
derived state.

**FR-002 (Ubiquitous)**
The system **shall** assign each event a monotonically increasing sequence
number within its run and an immutable event identifier.

**FR-003 (Event-driven)**
**When** an agent run is interrupted by a crash, process exit, or explicit
abort, the system **shall** record a termination event marking the run as
interrupted at the last committed sequence number.

**FR-004 (Event-driven)**
**When** a tool call completes, the system **shall** record both the tool
invocation and its result as ordered events, linked by a shared tool-call
identifier, before the next model invocation reads the result.

**FR-005 (Event-driven)**
**When** a permission decision is made (grant or deny) for a tool that
crosses a sandbox boundary, the system **shall** record the decision, the
principal, the tool, and the boundary-crossing target as an event.

**FR-006 (State-driven)**
**While** a run is in the "interrupted" state, the system **shall** expose the
run as resumable and **shall not** allow new events to be appended to it
until a resume operation is initiated.

**FR-007 (State-driven)**
**While** a run is in the "rolled-back" state (derived state reset to a
checkpoint), the system **shall** preserve all events after the checkpoint in
the log for audit purposes and **shall not** delete them.

**FR-008 (Optional)**
**Where** the operator configures automatic checkpointing, the system **may**
create a checkpoint after each completed turn, recording the checkpoint name,
sequence number, and timestamp as an event.

**FR-009 (Optional)**
**Where** the operator enables context pruning, the system **may** omit old
tool results from the next model prompt without deleting the corresponding
events from the log.

**FR-010 (Unwanted)**
**If** an attempt is made to delete or mutate an already-committed event,
the system **shall** reject the attempt and **shall** record the rejected
mutation as a separate audit event.

**FR-011 (Unwanted)**
**If** a resume operation encounters a gap or inconsistency in the event log
(for example, a tool result event missing its matching tool call event), the
system **shall** abort the resume, mark the run as unrecoverable, and
**shall not** produce a partial projection.

**FR-012 (Event-driven)**
**When** a rollback operation is invoked with a checkpoint or sequence
number, the system **shall** rebuild the derived projection by replaying
events from the start of the run up to (and including) the target, and
**shall** ignore all subsequent events for that projection.

**FR-013 (Event-driven)**
**When** a resume operation is invoked on an interrupted run, the system
**shall** replay the event log to reconstruct the active context, then
continue execution from the event following the last committed sequence
number.

**FR-014 (State-driven)**
**While** a context projection is being rebuilt from the event log (during
rollback or resume), the system **shall** present the run as "rebuilding" and
**shall** block concurrent rollback, resume, or append operations on that
run until rebuilding completes.

**FR-015 (Ubiquitous)**
The system **shall** retain the complete event log for a run even after the
run completes, so that the run can be inspected, replayed, or branched later.

**FR-016 (Optional)**
**Where** the operator sets a retention limit, the system **may** archive or
expire event logs for runs older than the limit, provided it records the
expiry as a lifecycle event.

**FR-017 (Unwanted)**
**If** the event log storage becomes unavailable during an append, the system
**shall** fail the operation that produced the event, **shall not** return a
success to the caller, and **shall not** advance the derived state.

**FR-018 (Event-driven)**
**When** a run is branched (a new run created from a checkpoint of an
existing run), the system **shall** copy the events up to the checkpoint into
the new run's log and **shall** record the branch origin in both runs.

### Non-Functional Requirements

**NFR-001** The event log append path **shall** have a p99 latency below 10 ms
for a single event on local storage.

**NFR-002** The system **shall** support rebuilding a projection for a run of
100,000 events in under 5 seconds on commodity hardware.

**NFR-003** The event log format **shall** be self-describing (each event
carries its type, schema version, and run identifier) so logs remain
replayable across version upgrades.

**NFR-004** The system **shall** provide an export of a run's complete event
log in a machine-readable format (JSON Lines) for external audit.

## Open Questions

1. Should checkpoints be operator-named or auto-generated with sequential
   identifiers? (FR-008 leaves this open.)
2. What is the maximum supported run length before a segmented log is
   required?
3. Should resume require explicit operator confirmation or auto-resume on
   startup (Maka gates this behind an environment variable)?