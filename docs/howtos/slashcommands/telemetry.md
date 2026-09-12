# /telemetry

> OpenTelemetry instrumentation: /telemetry help|on|off|setup|counters

## Overview

`/telemetry` controls ragent's OpenTelemetry export and displays metric
counter help. Turning it on starts OTLP export; `setup` opens a dialog for the
endpoint, protocol, and export timing; `counters` prints the catalogue of
metric names the instrumentation records, grouped into three tables.

## Syntax

```
/telemetry
/telemetry help
/telemetry on
/telemetry off
/telemetry setup
/telemetry counters
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `help` | Show the subcommand table |
| `on` | Enable OpenTelemetry export |
| `off` | Disable OpenTelemetry export |
| `setup` | Open the telemetry setup dialog |
| `counters` | Print the metric counter catalogue |

The setup dialog fields are: endpoint, protocol, export interval, export
timeout, and internal port.

## Examples

```
From: /telemetry on
OpenTelemetry export enabled; spans export to the configured endpoint.
```

```
From: /telemetry off
OpenTelemetry export disabled.
```

```
From: /telemetry setup
Opens the ProviderSetupStep::TelemetrySetup dialog with endpoint, protocol,
export interval, export timeout, and internal port fields.
```

```
From: /telemetry counters
## Usage
| Metric | Description |
| ragent.llm.requests | ...
| ragent.sessions.active | ...
| ragent.sessions.total | ...
| ragent.messages.user | ...
| ragent.tool.invocations | ...
| ragent.agents.active | ...
| ragent.agents.completed | ...
| ragent.subagent.spawns | ...
| ragent.team.members | ...
```

```
From: /telemetry counters (performance section)
## Performance
| ragent.llm.duration | ...
| ragent.llm.time_to_first_token | ...
| ragent.tool.duration | ...
| ragent.agent_loop.duration | ...
| ragent.agent_loop.iterations | ...
| ragent.session.duration | ...
| ragent.tool.permission_wait | ...
```

```
From: /telemetry counters (cost section)
## Cost
| ragent.tokens.input | ...
| ragent.tokens.output | ...
| ragent.tokens.cache_read | ...
| ragent.tokens.cache_write | ...
| ragent.cost.estimated | ...
| ragent.cost.session | ...
(8 cost metrics in total)
```

## Output

`on`/`off` print a one-line confirmation. `setup` opens a dialog whose values
persist to the config. `counters` prints three grouped metric tables:
**Usage** (9 metrics: requests, active/total sessions, user messages, tool
invocations, active/completed agents, subagent spawns, team members),
**Performance** (7 metrics: LLM duration, time-to-first-token, tool duration,
agent-loop duration and iterations, session duration, tool permission wait),
and **Cost** (8 metrics: input/output/cache tokens, estimated and session
cost).

## Related

- `/telemetry_panel`  -  display-only side panel fed by these counters
- `/telemetry setup` values persist in `ragent.json`