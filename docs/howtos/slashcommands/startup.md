# /startup

> "Show startup timing report" (SlashCommandDef, crates/ragent-tui/src/app/state.rs).

## Overview

`/startup` renders the startup timing report for the current session when the
application recorded one, and warns when no timings were captured. Startup
timings cover the phases between process launch and the TUI becoming
interactive; the report is produced by the `StartupTimings::format_report`
renderer. Timings are session-scoped: they are only available in the session
that performed the startup work.

The handler is the `startup` arm in `crates/ragent-tui/src/app/slash.rs`
(lines 3045-3056).

## Syntax

```
/startup
```

No arguments. Additional text after the command is ignored.

## Options / Subcommands

None. `/startup` takes no forms.

## Examples

### Show the timing report

```
/startup
```

Output (representative, condensed):

```
From: /startup
Startup timings:
  ...
```

The exact columns come from `StartupTimings::format_report()` in
`crates/ragent-tui`; this doc does not enumerate them because the renderer was
not traced for this page.

### No timings recorded

```
/startup
```

Output (status line):

```
[warn] No startup timings recorded for this session.
```

This is the expected result when the session was resumed rather than freshly
launched, or when timing instrumentation did not run.

## Output

- With `Some(timings)` recorded for the session, the command delegates to
  `timings.format_report()` and prints the resulting report in the message
  window.
- With `None`, the status line shows
  `[warn] No startup timings recorded for this session.` and no report is
  produced.

## Related

- `/inputdiag` - dump input and pane state for diagnostics
- `/doctor` - run environment health checks
- `/bug-report` - collect diagnostics into a report file