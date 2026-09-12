# /telemetry_panel

> Toggle the telemetry metrics side panel (alias keybinding: Alt+O)

## Overview

`/telemetry_panel` shows or hides the telemetry side panel. The panel is
display-only: metric values are recorded by the instrumentation and printed to
the chat transcript by `/telemetry counters`; the panel renders the current
values without any new collection. Showing the panel hides the log, profile,
tasks, and memory panels so only one side panel is visible at a time. Alt+O is
a keybinding alias for the same toggle.

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

```
(Alt+O)
Same toggle as the slash command, from the keyboard.
```

## Output

One status line: `telemetry panel visible` or `telemetry panel hidden`. While
visible, the panel occupies the side-panel slot and other side panels close.

## Related

- `/telemetry counters`  -  print the metric values into the transcript
- `/task`, `/memory panel` equivalents share the single side-panel slot