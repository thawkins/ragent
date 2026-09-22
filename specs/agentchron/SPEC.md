---
status: draft
audit:
  - { time: 1786197476, from: "none", to: "draft", actor: "system" }
---
# Agent Cron System — Specification

## Overview

This specification describes an **agent cron system** for ragent that allows
users to schedule agent runs with a designated agent type and an initial prompt.
The system supports both **one-shot** events (fire once at a specified timestamp)
and **repeated** events (fire on a recurring interval). Every event execution is
logged to the project `log/` folder using the same JSONL convention as the
existing edit-log system (`log/edits-<timestamp>.jsonl`).

A `/cron` slash command provides add, remove, list, log, and help sub-commands.
A `/cron log` sub-command renders the execution log showing the agent type,
prompt, and outcome for each fired event.

### Schedule grammar

Scheduling supports three forms:

| # | Form                                   | Behaviour                                             | Example                                   |
|---|----------------------------------------|-------------------------------------------------------|-------------------------------------------|
| 1 | `at <timestamp>`                       | One-shot. Fires once at the specified time.           | `/cron add nightly general at 2025-01-15T09:00 "Run tests"` |
| 2 | `from <timestamp> every <duration>`    | Repeating. First fire at `<timestamp>`, then every `<duration>` thereafter. | `/cron add nightly general from 2025-01-15T09:00 every 30m "Run tests"` |
| 3 | `every <duration>`                     | Repeating with no explicit start. The start time is assumed to be **now**, so the first fire is `<duration>` from now. | `/cron add nightly general every 2h "Run tests"` |

A **duration** is a positive integer followed by a unit. Supported units are:

| Unit    | Meaning  | Aliases           |
|---------|----------|-------------------|
| `m`     | minutes  | `min`, `mins`     |
| `h`     | hours    | `hr`, `hrs`       |
| `d`     | days     | `day`, `days`     |
| `w`     | weeks    | `wk`, `wks`       |
| `mo`    | months   | `month`, `months` |

Timestamps are ISO-8601 (e.g. `2025-01-15T09:00:00Z` or `2025-01-15T09:00:00+02:00`)
or natural-language shortcuts resolved against the user's local timezone
(e.g. `5pm`, `5:30pm`, `17:00`, `5am tomorrow`). When no day is specified, the
next upcoming occurrence is used (today if not yet passed, else tomorrow).

## Scope

**In scope:**

- Defining cron events (one-shot + repeating) with agent type and prompt.
- The three schedule forms above: single timestamp, timestamp + repeat, repeat-from-now.
- Duration parsing for minutes, hours, days, weeks, and months.
- Persisting events to SQLite via the existing `Storage` layer.
- A background scheduler that evaluates due events and spawns agent runs.
- `/cron` slash-command family: `add`, `remove`, `list`, `log`, `help`.
- JSONL execution logging to `<working_dir>/log/cron-<timestamp>.jsonl`.
- Reading back the event log from the slash command.

**Out of scope:**

- Full POSIX cron expressions (5-field `* * * * *` grammar).
- Calendar-day-of-week / day-of-month rules (use fixed intervals instead).
- Cross-machine distributed scheduling.
- Cron event editing (remove + re-add instead).
- Email or external notifications on event completion (future work).

## Definitions

| Term          | Meaning                                                              |
| ------------- | ------------------------------------------------------------------- |
| **Event**     | A scheduled agent run: agent type + prompt + schedule + enabled flag. |
| **One-shot**  | An event that fires exactly once at a specified timestamp (`at <ts>`). |
| **Repeating** | An event that fires on a fixed interval (`from <ts> every <d>` or `every <d>`). |
| **Start time**| The timestamp of the first execution. For `every <d>` with no start, start = now. |
| **Duration**  | A positive integer + unit (`m`, `h`, `d`, `w`, `mo`) defining the repeat interval. |
| **Fire**      | The act of spawning an agent run for a due event.                    |
| **Outcome**   | `"success"`, `"error"`, or `"skipped"` recorded per execution.       |

## Requirements

Requirements are written in EARS (Easy Approach to Requirements Syntax) notation.
Each requirement is uniquely numbered (`FR-NNN`) and classified by template.

### Ubiquitous

> **Ubiquitous** requirements apply at all times without a precondition.

**FR-001** (ubiquitous)
The system **shall** persist all cron events to the existing SQLite `Storage`
database so they survive process restarts.

**FR-002** (ubiquitous)
The system **shall** store, for each event, the agent type, the initial prompt,
the schedule definition (form, optional start timestamp, optional duration), an
enabled flag, a creation timestamp, a computed next-due timestamp, and a unique
event id.

**FR-003** (ubiquitous)
The system **shall** record the agent type, prompt, outcome, and timestamp for
every event execution as a single JSON line appended to a file under the project
`log/` directory, mirroring the existing edit-log JSONL convention.

### Event-Driven

> **Event-driven** requirements trigger **when** a condition occurs.

**FR-004** (event-driven)
**When** a repeating event's next-due time is reached, the system **shall** spawn
a background agent run of the configured agent type with the configured prompt,
and **shall** advance the event's `next_due` by one duration interval.

**FR-005** (event-driven)
**When** a one-shot event's scheduled timestamp is reached, the system **shall**
spawn a background agent run of the configured agent type with the configured
prompt, and **shall** mark the event as fired so it does not execute again.

**FR-006** (event-driven)
**When** a scheduled agent run completes (success or failure), the system
**shall** append a JSONL log entry containing the event id, agent type, prompt,
outcome (`"success"` or `"error"`), error message if any, and completion
timestamp.

**FR-007** (event-driven)
**When** a due event is disabled, the system **shall** skip execution and record
an outcome of `"skipped"` in the event log.

**FR-008** (event-driven)
**When** a user adds an event with the `every <duration>` form (no explicit start
timestamp), the system **shall** set the start time to the current time and
compute `next_due` as now + duration.

**FR-009** (event-driven)
**When** a user adds an event with the `from <timestamp> every <duration>` form,
the system **shall** set the start time to the given timestamp and compute
`next_due` as that timestamp (or, if the timestamp is in the past, advance by
whole duration intervals until `next_due` is in the future).

### State-Driven

> **State-driven** requirements activate **while** a system state holds.

**FR-010** (state-driven)
**While** the TUI session is running, a background scheduler task **shall**
periodically (at most every 30 seconds) evaluate all enabled events and fire
those whose next-due time has passed.

**FR-011** (state-driven)
**While** an event is marked disabled, the scheduler **shall not** fire it even
if its due time has passed.

**FR-012** (state-driven)
**While** a repeating event's previous execution is still running, the scheduler
**shall** skip the current due cycle and log `"skipped"` rather than fire a
second concurrent run.

### Optional

> **Optional** requirements use **may** for non-mandatory behaviour.

**FR-013** (optional)
The `/cron log` sub-command **may** accept an optional event-id filter
(`/cron log <event_id>`) to show only executions of a single event.

**FR-014** (optional)
The duration parser **may** accept plural and long-form unit aliases
(`mins`, `hrs`, `days`, `wks`, `months`) in addition to the canonical
single-letter forms (`m`, `h`, `d`, `w`, `mo`).

**FR-015** (optional)
The `/cron list` sub-command **may** display the human-readable schedule
description (e.g. "every 30m from 2025-01-15T09:00Z") alongside the raw fields.

### Unwanted

> **Unwanted** requirements use **shall not** to forbid behaviour.

**FR-016** (unwanted)
The system **shall not** execute an event whose agent type is not in the set of
built-in or custom agents known to ragent; instead it **shall** log an outcome
of `"error"` with a message identifying the unknown agent type.

**FR-017** (unwanted)
The system **shall not** block the interactive TUI event loop while waiting for
scheduled events; scheduling evaluation and agent spawning **shall** occur on a
background task.

**FR-018** (unwanted)
The system **shall not** accept a duration of zero or a negative value; a
duration of `0m` or `-5h` **shall** be rejected at parse time with an error
message.

**FR-019** (unwanted)
The system **shall not** accept an unrecognized duration unit; an expression like
`every 5s` or `every 3y` **shall** be rejected at parse time with an error
listing the supported units.

## Slash Command Surface

The `/cron` command family exposes the following sub-commands:

| Sub-command                                  | Description                                                                 |
| -------------------------------------------- | --------------------------------------------------------------------------- |
| `/cron add <cronname> <agent> <schedule> <prompt>` | Create a new scheduled event. `cronname` sets the event ID. `schedule` is one of the three forms below.  |
| `/cron remove <event_id\|last>`              | Remove an event by id, or `last` for the most recently added.              |
| `/cron list`                                 | List all events with id, agent, schedule, enabled, next-due.                |
| `/cron log [event_id]`                       | Show the execution log (optionally filtered by event id) with agent type, prompt, and outcome per entry. |
| `/cron help`                                 | Show usage help for the `/cron` family.                                     |

### Schedule forms for `/cron add`

```
at <timestamp>                       # one-shot (ISO-8601 or natural language)
from <timestamp> every <duration>    # repeating with explicit start
every <duration>                     # repeating, start = now
```

**Timestamp formats:** ISO-8601 (`2025-01-15T09:00:00Z`) or natural-language
shortcuts (`5pm`, `5:30pm`, `17:00`, `5am tomorrow`).

### Duration grammar

```
<duration>  ::= <int> <unit>
<unit>      ::= "m" | "min" | "mins"        # minutes
              | "h" | "hr" | "hrs"          # hours
              | "d" | "day" | "days"        # days
              | "w" | "wk" | "wks"          # weeks
              | "mo" | "month" | "months"   # months (calendar months)
```

## Data Model

### `cron_events` table (SQLite)

| Column        | Type      | Notes                                                              |
| ------------- | --------- | ------------------------------------------------------------------ |
| `id`          | TEXT PK   | Unique event id (e.g. `cron-<timestamp>-<rand>`).                   |
| `agent_type`  | TEXT      | Built-in or custom agent name.                                     |
| `prompt`      | TEXT      | Initial prompt passed to the agent.                                |
| `schedule`    | TEXT      | Raw schedule expression (e.g. `at 2025-01-15T09:00Z`).             |
| `form`        | TEXT      | `one_shot`, `repeat_from`, or `repeat_now`.                        |
| `start_at`    | TEXT      | ISO-8601 start timestamp (null for `repeat_now` until first fire).  |
| `duration_secs` | INTEGER | Repeat interval in seconds (null for one-shot). Months use 30-day approximation. |
| `repeating`   | INTEGER   | 0 = one-shot, 1 = repeating.                                        |
| `enabled`     | INTEGER   | 0 = disabled, 1 = enabled.                                          |
| `next_due`    | TEXT      | ISO-8601 timestamp of next execution.                              |
| `created_at`  | TEXT      | ISO-8601 creation timestamp.                                        |
| `last_fired`  | TEXT      | ISO-8601 timestamp of last execution (nullable).                   |

### `cron_log` JSONL entries (`log/cron-<timestamp>.jsonl`)

Each execution appends one JSON line:

```json
{
  "timestamp": "2025-01-01T12:00:00Z",
  "event_id": "cron-20250101-120000-abc123",
  "agent_type": "general",
  "prompt": "Run the test suite and report failures",
  "schedule": "every 30m",
  "outcome": "success",
  "error": null,
  "run_id": "session-id-of-spawned-run"
}
```

## Testability

Each requirement maps to a verifiable test:

- **FR-001/002**: Insert an event, reopen `Storage`, assert the event persists with all fields intact.
- **FR-004**: Set a repeating event's `next_due` in the past, tick the scheduler, assert a run is spawned and `next_due` advanced by one interval.
- **FR-005**: Set a one-shot event's `next_due` in the past, tick, assert a run is spawned and the event is marked fired.
- **FR-006**: Fire an event, assert a JSONL line exists in `log/cron-*.jsonl` with the required fields.
- **FR-007**: Disable an event, tick the scheduler, assert outcome is `"skipped"`.
- **FR-008**: Add an event with `every 30m` (no start), assert `next_due` ≈ now + 30 min.
- **FR-009**: Add an event with `from <past-ts> every 1h`, assert `next_due` is advanced to the next future multiple.
- **FR-010/017**: Assert the scheduler runs on a background task and does not block the TUI input loop.
- **FR-011**: Disable an event, assert it is not fired even when due.
- **FR-012**: Fire a repeating event whose previous run is still active, assert outcome `"skipped"`.
- **FR-013**: `/cron log <event_id>` shows only matching entries.
- **FR-014**: Duration parser accepts `mins`, `hrs`, `days`, `wks`, `months` aliases.
- **FR-016**: Add an event with an unknown agent type, tick, assert log entry has outcome `"error"`.
- **FR-018**: `every 0m` is rejected with a parse error.
- **FR-019**: `every 5s` is rejected with an error listing supported units.

## Non-Functional Requirements

- **Performance**: Scheduler tick must complete in < 50 ms for up to 1000
  events (simple timestamp comparison + SQLite read).
- **Persistence**: Events survive restarts via SQLite; the scheduler reloads
  `next_due` on startup and re-evaluates.
- **Observability**: All executions are logged to JSONL; the `/cron log`
  command reads the most recent `cron-*.jsonl` files, matching the edit-log
  file-selection pattern.
- **Security**: The `/cron` command respects the existing permission system;
  spawning an agent run reuses the same `new_task`/background spawn path so
  bash/file permissions still apply.
- **Month handling**: A "month" duration is approximated as 30 days (2,592,000
  seconds) for `next_due` computation, keeping the scheduler arithmetic simple
  and deterministic without calendar libraries.