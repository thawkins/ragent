# Tools — Scheduling

Scheduled agent runs (cron events): one-shot or recurring prompts executed by
a named agent. New events are enabled by default.

| Tool | Description |
|------|-------------|
| `cron_add` | Create a scheduled agent run. |
| `cron_remove` | Delete a scheduled event. |
| `cron_list` | List all scheduled events. |
| `cron_enable` | Enable a scheduled event. |
| `cron_disable` | Disable a scheduled event. |

---

## cron_add

Create a cron event.

**Schedule grammar:**

- `at <timestamp>` — one-shot
- `from <timestamp> every <duration>` — repeating from a start time
- `every <duration>` — repeating from now

Timestamps accept ISO-8601 (`2025-01-15T09:00:00Z`) or natural language
(`"5pm"`, `"5am tomorrow"`). Durations use `<int><unit>` where unit is
`m`, `h`, `d`, `w`, or `mo`.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `id` | string | yes | Unique event name (primary key; duplicates fail) | `"daily-audit"` |
| `agent` | string | yes | Agent type to run | `"general"`, `"coder"` |
| `schedule` | string | yes | Schedule expression per grammar above | `"every 1h"` |
| `prompt` | string | yes | Prompt executed when the event fires | `"Run cargo clippy and report new warnings"` |

**Example:**
```text
cron_add id="nightly-test" agent="general" schedule="every 1d" prompt="Run cargo test and summarise failures"
```

---

## cron_remove / cron_enable / cron_disable

Delete an event permanently, or toggle whether the scheduler fires it.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `id` | string | yes | Event identifier |

---

## cron_list

List all cron events with id, agent, schedule, enabled status, next-due
timestamp, and prompt preview. No arguments required.

**Example:**
```text
cron_list
cron_disable id="nightly-test"
```
