# /agent

> Switch the active agent: /agent [<name>] | /agent help

## Overview

`/agent` controls which agent preset drives the session. With no argument it
opens the interactive agent picker dialog, which lists every cycleable agent
(built-in and custom, custom entries flagged) and lets you choose one with the
keyboard. With a name argument it switches directly to that agent without
opening any dialog, logging the switch and publishing an `AgentSwitched` event
on the session event bus.

Unknown names are rejected with a status-bar message listing the available
agent names. `/agent help` prints a small subcommand table instead of acting.

## Syntax

```
/agent              # open the interactive agent picker
/agent <name>       # switch directly to a named agent
/agent help         # show the subcommand table
```

## Options / Subcommands

| Form | Description |
| --- | --- |
| `/agent` | Open the interactive agent picker |
| `/agent <name>` | Switch directly to a named agent (e.g. `coder`, `general`, `architect`) |
| `/agent help` | Show the `/agent` help table |

Notes:

- The picker lists only `cycleable_agents` (agents reachable via agent
  cycling); custom agents are included when they are cycleable and are marked
  with a `[custom]`-style flag in the picker data.
- A successful direct switch updates the current agent index, agent name, and
  agent info, writes an Info log line (`Switched to: <name> (<description>)`),
  and publishes `Event::AgentSwitched` when a session is active.

## Examples

```
/agent
```

```
/agent coder
```

Switch with confirmation in the log:

```
Switched to: coder (Full-stack coding agent with file and shell access)
```

```
/agent architect
```

```
/agent help
```

Unknown agent feedback (status bar + warn log):

```
Unknown agent 'foo'. Available: general, coder, task, architect, ask, debug, code-review, orchestrator
```

## Output

- `/agent` opens the agent picker dialog (no message emitted).
- `/agent <name>` on success sets the status bar to `agent: <name>` and logs
  the switch; on failure the status bar shows the unknown-agent message and a
  Warn log entry is written.
- `/agent help` appends an assistant bubble `From: /agent help` containing the
  subcommand table; status shows `agent: help`.

## Related

- `/agents`  -  list every built-in and custom agent with descriptions
- `/model`  -  switch the active LLM model
- `/help`  -  list all available slash commands