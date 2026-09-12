# /cron

> Schedule, inspect, and manage recurring agent runs (cron events)

## Overview

`/cron` manages scheduled agent runs. An event pairs an agent type with a
schedule expression and a prompt; the scheduler fires the event and runs the
agent with that prompt. Events persist in the storage database, so schedules
survive restarts. Each run's outcome is recorded in a cron log that `/cron log`
can display.

The prompt argument must be enclosed in double quotes. Schedules are parsed by
`ragent_types::parse_schedule` and stored with `storage.insert_cron_event`.

## Syntax

```
/cron add <cronname> <agent> <schedule> "<prompt>"
/cron remove <cronname>
/cron enable <cronname>
/cron disable <cronname>
/cron list
/cron detail <cronname>
/cron log [event_id]
/cron help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `add <cronname> <agent> <schedule> "<prompt>"` | Create a scheduled event (bare `/cron` shows the same add form) |
| `remove <cronname>` | Delete a scheduled event permanently |
| `enable <cronname>` | Re-enable a disabled event |
| `disable <cronname>` | Stop an event from firing without deleting it |
| `list` | Render a table of all events |
| `detail <cronname>` | Show every stored field of one event, prompt untruncated |
| `log [event_id]` | Show recent cron run history, optionally for one event |
| `help` | Print the schedule grammar and subcommand table |

### Schedule grammar

| Form | Example | Description |
|------|---------|-------------|
| `at <timestamp>` | `at 2026-01-15T09:00:00Z` | One-shot run at an ISO-8601 timestamp |
| `at <natural time>` | `at 5pm tomorrow` | One-shot run at a natural-language time |
| `from <ts> every <dur>` | `from 9am every 30m` | Repeating, starting at the given time |
| `every <duration>` | `every 6h` | Repeating from now |

Timestamps accept ISO-8601 or natural language (`5pm`, `5:30pm`, `17:00`,
`5am tomorrow`). Durations use `<int><unit>` with units `m` (minutes), `h`
(hours), `d` (days), `w` (weeks), and `mo` (months).

## Examples

```
From: /cron add nightly-tidy coder "at 2am" "Review yesterday's changes and update the changelog draft."
Creates event nightly-tidy with agent coder, one-shot at 02:00.
```

```
From: /cron add hourly-scan explore "every 1h" "Check for new failing tests and report."
Every duration form: repeats hourly from now.
```

```
From: /cron add weekday-brief general "from 9am every 1d" "Summarise open tasks."
Repeating daily, first fire at 09:00 today.
```

```
From: /cron list
| ID | Agent | Schedule | Enabled | Next due | Prompt |
Renders one row per event; prints "No scheduled events." when none exist.
```

```
From: /cron detail nightly-tidy
Every stored field, including the full untruncated prompt.
```

```
From: /cron log
| Timestamp | Event | Agent | Outcome | Prompt |
Recent runs of any event; pass an id to narrow: /cron log hourly-scan.
```

An unquoted prompt is rejected with
`[warn] The prompt must be enclosed in double quotes.`

## Output

`add` prints a Field/Value table (ID, Agent, Schedule, Next due, Prompt).
`list` prints the ID/Agent/Schedule/Enabled/Next due/Prompt table.
`enable`/`disable`/`remove` print one confirmation line. `log` prints the
Timestamp/Event/Agent/Outcome/Prompt table read from the cron log.

## Related

- `/inbox`  -  findings produced by stateful cron runs
- `/task`  -  session task tracking
- ragent-tools-core cron logging (`read_cron_log`)