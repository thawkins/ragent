# /telemetry_panel

> Toggle the telemetry metrics side panel

## Overview

`/telemetry_panel` shows or hides the telemetry side panel. The panel is
display-only: metric values are recorded by the instrumentation and printed to
the chat transcript by `/telemetry counters`; the panel renders the current
values without any new collection. Showing the panel hides the log, profile,
tasks, and memory panels so only one side panel is visible at a time.

The panel opens with a **Status** block, matching the order `/telemetry counters`
uses:

- `export on` / `export off` - whether anything is leaving the machine.
- `logs <state> - level <level>` - one of `shipping`, `configured but not
  attached`, `off (logs signal disabled)` or `off (telemetry is disabled)`.
- `export health <n> consecutive, <m> total failure(s)` - shown only once a
  failure has been recorded.

It reads the running subsystem's config rather than calling `Config::load()`.
The old version reloaded the config from disk on every frame, which was both a
disk read on the UI thread and a description of the *next* start rather than
this one, so the panel could disagree with what was actually being exported.

There is no keybinding alias: `Alt+O` toggles autopilot mode, not this panel.
Use the slash command.

## Syntax

```
/telemetry_panel
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| (bare) | Toggle the telemetry side panel visible or hidden |

## Examples

```
From: /telemetry_panel
telemetry panel visible
```

```
From: /telemetry_panel
telemetry panel hidden
```

## Output

One status line: `telemetry panel visible` or `telemetry panel hidden`. While
visible, the panel occupies the side-panel slot and other side panels close.

## Related

- `/telemetry counters`  -  print the metric values into the transcript
- `/task`, `/memory panel` equivalents share the single side-panel slot