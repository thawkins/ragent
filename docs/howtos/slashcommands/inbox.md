# /inbox

> Triage inbox findings left by stateful cron jobs

## Overview

The inbox collects findings from stateful cron jobs that use the `<inbox>` tag
protocol. Findings are stored in a global JSONL file shared across all
sessions, so a scheduled nightly job can leave a note that any later session
can read, claim, and act on.

Entries move through statuses: pending entries can be claimed (picked up for
work) or dismissed (dropped without action). `clear` empties the inbox.

## Syntax

```
/inbox list
/inbox claim <entry_id>
/inbox dismiss <entry_id>
/inbox clear
/inbox help
```

## Options / Subcommands

| Form | Description |
|------|-------------|
| `list` | Show all inbox entries with IDs, sources, and statuses |
| `claim <entry_id>` | Mark an entry as claimed |
| `dismiss <entry_id>` | Mark an entry as dismissed |
| `clear` | Remove every finding from the inbox |
| `help` | Show the subcommand table and the `<inbox>` tag protocol note |

## Examples

```
From: /inbox list
# | ID | Source | Status | Content (60-char preview)
Empty inbox prints:
Inbox is empty. Findings are added by stateful cron jobs that use the
`<inbox>` tag protocol.
```

```
From: /inbox list
| 1 | a1b2c3d4 | cron/nightly | pending | Flaky test test_render_cost_scaling failed 3x...
1 finding(s) in inbox.
```

```
From: /inbox claim a1b2c3d4
[ok] Entry `a1b2c3d4` marked as claimed.
```

```
From: /inbox dismiss a1b2c3d4
[clr] Entry `a1b2c3d4` marked as dismissed.
```

```
From: /inbox dismiss nosuchid
[warn] Entry `nosuchid` not found in inbox.
```

```
From: /inbox clear
[clr] Cleared 4 finding(s) from the inbox.
```

## Output

`list` renders a table with the 8-character entry ID, source, status, and a
60-character content preview, ending with `N finding(s) in inbox.` Claim and
dismiss print a one-line `[ok]`/`[clr]` confirmation. Clearing prints the
removed count. Unknown IDs print `[warn] Entry ... not found in inbox.`
Statuses land in the status bar (`inbox: list`, `inbox: claimed`, and so on).

## Related

- `/cron`  -  scheduled runs that write inbox findings
- `/task`  -  move a claimed finding into the session task list