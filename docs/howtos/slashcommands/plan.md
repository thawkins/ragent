# /plan
> Delegate planning to the plan agent: /plan <task description> | /plan help

## Overview

`/plan` hands a task description to the plan agent, which performs read-only
codebase analysis and returns a plan for the requested work. The current
session delegates the description verbatim and waits for the plan agent to
report back; the plan agent cannot edit files.

The plan agent's output is analysis, not implementation. Implementation
happens in your normal session after you review the plan - the help text
notes that an approval prompt precedes any implementation step.

## Syntax

```
/plan <task description>
/plan help
```

An empty description or the `help` token prints the usage line instead of
delegating.

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/plan <task description>` | Delegate the description to the plan agent |
| `/plan help` | Print usage help |

## Examples

Plan a feature:

```
/plan add retry with exponential backoff to the HTTP client
```

Plan a refactor:

```
/plan split the provider registry into per-provider modules
```

Plan an investigation:

```
/plan find why the compaction test flakes on CI and propose a fix
```

Check the usage line:

```
/plan help
```

Plan with explicit scope constraints in the description:

```
/plan redesign the permission check pipeline, touch only crates/ragent-agent/src/session
```

## Output

- The delegation is recorded in the transcript; the plan agent runs and its
  report streams into the message window as the plan agent completes.
- Empty input or `help` prints the usage line:
  `Usage: /plan <task description>` (with the note that implementation
  follows an approval prompt).
- The current session remains usable after the plan returns; no mode change
  occurs.

## Related

- `plan_enter` - the underlying sub-agent delegation used by `/plan`
- `/mode architect` - switch role mode without delegating
- `/agent` - switch the session's agent entirely
- `/loop` - goal-driven loop execution for carrying a plan out